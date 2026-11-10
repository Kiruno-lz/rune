use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, Tx, col};
use rune_kit::{DropPlacement, DropTarget, Kit, Tree, TreeDrag, TreeModel};

struct State {
    tree: TreeModel<&'static str>,
    target: Option<DropTarget>,
    selected: Option<String>,
    moves: Vec<(String, DropTarget)>,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().h_full().child(
        Tree::new(|s: &State| &s.tree, |s| &mut s.tree, |label| label)
            .on_select(|s, id, _| s.selected = Some(id.to_owned()))
            .drag(
                TreeDrag::new(
                    |s: &State| &s.target,
                    |s| &mut s.target,
                    |s, source, target, _: &mut Tx| s.moves.push((source.to_owned(), target)),
                )
                .accepts(|_, _, target| target.id != "blocked"),
            )
            .build(s, &Kit::neutral(false))
            .grow(1.0),
    )
}

fn setup() -> Runtime<State> {
    setup_with_extra_rows(0)
}

fn setup_with_extra_rows(rows: usize) -> Runtime<State> {
    let mut tree = TreeModel::new().key("files");
    for (id, label) in [("one", "One"), ("blocked", "Blocked"), ("two", "Two")] {
        assert!(tree.insert(None, id, label));
    }
    for index in 0..rows {
        assert!(tree.insert(None, format!("row-{index}"), "Extra row"));
    }
    let state = State { tree, target: None, selected: None, moves: Vec::new() };
    let mut rt = Runtime::new(state, view, (360.0, 200.0), Env { system_dark: false, scale: 1.0 });
    rt.frame(0.0);
    rt
}

#[test]
fn drag_previews_a_destination_and_commits_only_on_release() {
    let mut rt = setup();
    let (x, y) = rt.find("one").unwrap();
    let (tx, ty) = rt.find("drop-after:two").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.01);
    rt.pointer_move(tx, ty);
    rt.frame(0.01);
    let target = DropTarget { id: "two".to_owned(), placement: DropPlacement::After };
    assert_eq!(rt.state.target.as_ref(), Some(&target));
    assert!(rt.state.moves.is_empty());
    rt.pointer_up(tx, ty);
    rt.frame(0.01);
    assert_eq!(rt.state.moves, [("one".to_owned(), target)]);
    assert!(rt.state.target.is_none());
}

#[test]
fn a_rejected_or_cancelled_drop_does_not_commit_or_break_click_selection() {
    let mut rt = setup();
    let (x, y) = rt.find("one").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.01);
    assert_eq!(rt.state.selected.as_deref(), Some("one"));

    let (blocked_x, blocked_y) = rt.find("drop-inside:blocked").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.01);
    rt.pointer_move(blocked_x, blocked_y);
    rt.frame(0.01);
    assert!(rt.state.target.is_none());
    rt.pointer_up(blocked_x, blocked_y);
    rt.frame(0.01);
    assert!(rt.state.moves.is_empty());

    let (tx, ty) = rt.find("drop-inside:two").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.01);
    rt.pointer_move(tx, ty);
    rt.frame(0.01);
    assert!(rt.state.target.is_some());
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(0.01);
    assert!(rt.state.target.is_none());
    assert!(rt.state.moves.is_empty());
}

#[test]
fn an_unmounted_source_can_still_drop_in_the_tree() {
    let mut rt = setup_with_extra_rows(100);
    let (x, y) = rt.find("one").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.01);
    rt.update(|_, tx| tx.scroll_center_in_instant("files", "row-60"));
    for _ in 0..4 {
        rt.frame(0.016);
    }
    assert!(rt.find("one").is_none(), "the virtual list must actually cull the source");
    let (x, y) = rt.find("drop-inside:row-60").unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.016);
    assert_eq!(rt.state.target.as_ref().map(|target| target.id.as_str()), Some("row-60"));
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert_eq!(
        rt.state.moves,
        [("one".to_owned(), DropTarget { id: "row-60".to_owned(), placement: DropPlacement::Inside })]
    );
}

#[test]
fn holding_near_an_edge_scrolls_until_release_even_after_the_source_is_culled() {
    let mut rt = setup_with_extra_rows(100);
    let (x, y) = rt.find("one").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.016);
    rt.pointer_move(x, 195.0);
    for _ in 0..70 {
        rt.frame(0.016);
    }
    assert!(rt.find("one").is_none(), "the drag should scroll beyond the source row");
    let target = rt.state.target.clone().expect("the stationary pointer follows the rows underneath it");
    assert!(target.id.starts_with("row-"));
    assert!(rt.state.moves.is_empty());
    rt.pointer_up(x, 195.0);
    rt.frame(0.016);
    let offset = rt.elements().into_iter().find(|node| node.key.as_deref() == Some("rows")).unwrap().scrolled;
    for _ in 0..10 {
        rt.frame(0.016);
    }
    let settled = rt.elements().into_iter().find(|node| node.key.as_deref() == Some("rows")).unwrap().scrolled;
    assert_eq!(offset, settled);
    assert_eq!(rt.state.moves.len(), 1);
    assert!(rt.state.target.is_none());
}

#[test]
fn escape_or_removal_cancels_a_drag_even_with_the_source_unmounted() {
    for remove_source in [false, true] {
        let mut rt = setup_with_extra_rows(100);
        let (x, y) = rt.find("one").unwrap();
        rt.pointer_down(x, y);
        rt.frame(0.016);
        rt.update(|_, tx| tx.scroll_center_in_instant("files", "row-60"));
        for _ in 0..4 {
            rt.frame(0.016);
        }
        assert!(rt.find("one").is_none());
        let (x, y) = rt.find("drop-inside:row-60").unwrap();
        rt.pointer_move(x, y);
        rt.frame(0.016);
        assert!(rt.state.target.is_some());
        if remove_source {
            rt.update(|s, _| {
                s.tree.clear();
                s.tree.insert(None, "row-60", "Kept row");
            });
        } else {
            rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        }
        rt.frame(0.016);
        rt.frame(0.016);
        assert!(rt.state.target.is_none(), "remove source: {remove_source}");
        assert!(!rt.pressing());
        rt.pointer_up(x, y);
        rt.frame(0.016);
        assert!(rt.state.moves.is_empty());
    }
}
