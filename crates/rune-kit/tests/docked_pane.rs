use rune::{Cx, El, Env, Key, KeyEvent, Mods, Runtime, Spring, el, row};
use rune_kit::shell::docked_pane::{PaneDivider, ResizeInteraction, leading};
use rune_kit::{Kit, PaneState};

struct State {
    pane: PaneState,
    resize: ResizeInteraction,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    row()
        .w_full()
        .h_full()
        .child(leading(&s.pane, &s.resize, el(), Spring::NONE).key("pane"))
        .child(PaneDivider::new("Resize outline", &Kit::neutral(false)).build(
            &s.pane,
            &s.resize,
            |s: &mut State| (&mut s.pane, &mut s.resize),
            Spring::NONE,
        ))
        .child(el().grow(1.0).key("content"))
}

#[test]
fn standalone_divider_clamps_and_can_be_resized_by_keyboard() {
    let state = State { pane: PaneState::new(160.0).range(120.0, 220.0), resize: ResizeInteraction::default() };
    let mut rt = Runtime::new(state, view, (440.0, 260.0), Env { system_dark: false, scale: 1.0 });
    rt.frame(0.0);
    let (x, y) = rt.find("pane-resize").unwrap();
    rt.pointer_down(x, y);
    rt.frame(0.01);
    rt.pointer_move(x + 150.0, y);
    rt.frame(0.01);
    rt.pointer_up(x + 150.0, y);
    rt.frame(0.01);
    assert_eq!(rt.state.pane.width(), 220.0);
    assert_eq!(rt.presented("content").unwrap().0[0], 220.0);

    rt.key(KeyEvent { key: Key::Left, mods: Mods::default() });
    rt.frame(0.01);
    assert_eq!(rt.state.pane.width(), 212.0);
}
