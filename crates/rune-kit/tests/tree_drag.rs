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
    let mut tree = TreeModel::new().key("files");
    for (id, label) in [("one", "One"), ("blocked", "Blocked"), ("two", "Two")] {
        assert!(tree.insert(None, id, label));
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
