use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rune::{Color, Cursor, El, Phase, ScrollPos, Tx, el};

use super::TreeRow;

#[derive(Default)]
pub(super) struct DragViewport(Mutex<ViewportGeometry>);

#[derive(Default)]
struct ViewportGeometry {
    rect: [f32; 4],
    scroll: ScrollPos,
}

impl DragViewport {
    pub(super) fn set_rect(&self, rect: [f32; 4]) {
        self.0.lock().expect("tree viewport geometry lock").rect = rect;
    }

    pub(super) fn set_scroll(&self, scroll: ScrollPos) {
        self.0.lock().expect("tree viewport geometry lock").scroll = scroll;
    }

    fn next_offset(&self, x: f32, y: f32) -> Option<f32> {
        let geometry = self.0.lock().expect("tree viewport geometry lock");
        let [left, top, width, height] = geometry.rect;
        if !(left..left + width).contains(&x) || !(top..top + height).contains(&y) {
            return None;
        }
        let edge = 26.0_f32.min(height / 4.0);
        let local_y = y - top;
        let pressure = if local_y < edge {
            -(edge - local_y) / edge
        } else if local_y > height - edge {
            (local_y - height + edge) / edge
        } else {
            return None;
        };
        let scroll = geometry.scroll;
        let offset = scroll.offset[1];
        let next = (offset + 22.0 * pressure).clamp(0.0, scroll.max[1]);
        (next != offset).then_some(next)
    }
}

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

    pub(super) fn row(
        &self,
        s: &S,
        row: El,
        r: &TreeRow,
        (tree_key, viewport): (&str, &Arc<DragViewport>),
        height: f32,
        accent: Color,
    ) -> El {
        let active = (self.target)(s).as_ref();
        let edge = height * 0.25;
        let id = r.id.to_owned();
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
        let viewport_key = format!("{tree_key}/viewport");
        let viewport = Arc::clone(viewport);
        let track = self.target_mut;
        let accept = self.accept.clone();
        let drop = self.drop.clone();
        row.on_drag(move |s: &mut S, drag, tx| {
            let release = match drag.phase {
                Phase::Down => {
                    tx.live_only();
                    return;
                }
                Phase::Up if drag.cancelled() => {
                    *track(s) = None;
                    return;
                }
                Phase::Move => false,
                Phase::Up => true,
            };
            // Rune keeps the drag's scratch across events, even when the row unmounts.
            if drag.data == 0.0 {
                if drag.dx.abs().max(drag.dy.abs()) <= 4.0 {
                    tx.live_only();
                    return;
                }
                drag.data = 1.0;
            }
            let [x, y] = drag.window_point();
            if !release && let Some(next) = viewport.next_offset(x, y) {
                tx.scroll_to_y(&viewport_key, next);
                tx.drag_repeat_after(Duration::from_millis(16));
            }
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
