use std::rc::Rc;

use rune::{Color, Cursor, El, Phase, Tx, el};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropPlacement {
    Before,
    Inside,
    After,
}

/// A proposed drop on a node of the same tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropTarget {
    pub id: String,
    pub placement: DropPlacement,
}

impl DropTarget {
    fn from_key(key: &str) -> Option<Self> {
        let (zone, id) = key.split_once(':')?;
        let placement = match zone {
            "drop-before" => DropPlacement::Before,
            "drop-inside" => DropPlacement::Inside,
            "drop-after" => DropPlacement::After,
            _ => return None,
        };
        Some(Self { id: id.into(), placement })
    }
}

type Accept<S> = dyn Fn(&S, &str, &DropTarget) -> bool;
type Drop<S> = dyn Fn(&mut S, &str, DropTarget, &mut Tx);

/// Adds pointer drag and drop to a virtual tree. The application owns the
/// destination indicator and decides how to mutate its hierarchy on release.
pub struct TreeDrag<S> {
    target: fn(&S) -> &Option<DropTarget>,
    target_mut: fn(&mut S) -> &mut Option<DropTarget>,
    accept: Option<Rc<Accept<S>>>,
    drop: Rc<Drop<S>>,
}

impl<S: 'static> TreeDrag<S> {
    pub fn new(
        target: fn(&S) -> &Option<DropTarget>,
        target_mut: fn(&mut S) -> &mut Option<DropTarget>,
        drop: impl Fn(&mut S, &str, DropTarget, &mut Tx) + 'static,
    ) -> Self {
        Self { target, target_mut, accept: None, drop: Rc::new(drop) }
    }

    /// Rejects a destination during hover and again when the pointer is released.
    pub fn accepts(mut self, accept: impl Fn(&S, &str, &DropTarget) -> bool + 'static) -> Self {
        self.accept = Some(Rc::new(accept));
        self
    }

    pub(super) fn row(&self, s: &S, row: El, id: &str, tree_key: &str, height: f32, accent: Color) -> El {
        let active = (self.target)(s).as_ref();
        let edge = height * 0.25;
        let id = id.to_owned();
        let region = |zone, placement, top, height| {
            let region =
                el().key(format!("{zone}:{id}")).pin(Some(0.0), Some(top), Some(0.0), None).h(height).a11y_hidden();
            if active.is_some_and(|target| target.id == id && target.placement == placement) {
                match placement {
                    DropPlacement::Inside => region.fill(accent.alpha(0.14)),
                    DropPlacement::Before => region.border_sides([2.0, 0.0, 0.0, 0.0], accent),
                    DropPlacement::After => region.border_sides([0.0, 0.0, 2.0, 0.0], accent),
                }
            } else {
                region
            }
        };
        let row = row
            .cursor(Cursor::Grab)
            .child(region("drop-before", DropPlacement::Before, 0.0, edge))
            .child(region("drop-inside", DropPlacement::Inside, edge, height - 2.0 * edge))
            .child(region("drop-after", DropPlacement::After, height - edge, edge));
        let tree_key = tree_key.to_owned();
        let track = self.target_mut;
        let accept = self.accept.clone();
        let drop = self.drop.clone();
        row.drag_threshold(4.0).on_drag(move |s: &mut S, drag, tx| {
            let release = match drag.phase {
                Phase::Down => return,
                Phase::Up if drag.cancelled() => {
                    *track(s) = None;
                    return;
                }
                Phase::Move => false,
                Phase::Up => true,
            };
            let [x, y] = drag.window_point();
            let source = id.clone();
            let accept = accept.clone();
            let drop = drop.clone();
            tx.element_at_in(&tree_key, x, y, move |s: &mut S, picked, tx| {
                let target = picked.as_ref().and_then(|node| DropTarget::from_key(node.key()?)).filter(|target| {
                    target.id != source && accept.as_ref().is_none_or(|accept| accept(s, &source, target))
                });
                if release {
                    *track(s) = None;
                    if let Some(target) = target {
                        drop(s, &source, target, tx);
                    }
                } else if *track(s) != target {
                    *track(s) = target;
                } else {
                    tx.live_only();
                }
            });
        })
    }
}
