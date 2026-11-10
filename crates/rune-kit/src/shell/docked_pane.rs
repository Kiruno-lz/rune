use rune::machine::Effects;
use rune::{Color, Cursor, El, Phase, Spring, el};

use crate::{Kit, PaneState};

/// Input state for an embedded pane's resize handle. Keep one per divider.
#[derive(Default)]
pub struct ResizeInteraction(ResizeState);

#[derive(Default)]
enum ResizeState {
    #[default]
    Rest,
    Hover,
    Drag {
        hovered: bool,
        start: f32,
        excess: f32,
    },
}

enum Event {
    Hover(bool),
    Begin(f32),
    Move { delta: f32, min: f32, max: f32, reduced: bool },
    End { cancelled: bool },
}

enum Effect {
    Width(f32),
    Snap,
    Rebound,
}

impl ResizeInteraction {
    fn step(&mut self, event: Event) -> Effects<Effect> {
        match event {
            Event::Hover(on) => {
                match &mut self.0 {
                    ResizeState::Drag { hovered, .. } => *hovered = on,
                    _ => self.0 = if on { ResizeState::Hover } else { ResizeState::Rest },
                }
                Effects::none()
            }
            Event::Begin(start) => {
                self.0 = ResizeState::Drag { hovered: matches!(self.0, ResizeState::Hover), start, excess: 0.0 };
                Effects::one(Effect::Snap)
            }
            Event::Move { delta, min, max, reduced } => {
                let ResizeState::Drag { start, excess, .. } = &mut self.0 else {
                    return Effects::none();
                };
                let requested = *start + delta;
                let clamped = requested.clamp(min, max);
                let beyond = requested - clamped;
                *excess = if reduced { 0.0 } else { beyond.signum() * 16.0 * (1.0 - (-beyond.abs() / 100.0).exp()) };
                Effects::two(Effect::Width(clamped), Effect::Snap)
            }
            Event::End { cancelled } => {
                let ResizeState::Drag { hovered, start, excess } = self.0 else {
                    return Effects::none();
                };
                self.0 = if hovered { ResizeState::Hover } else { ResizeState::Rest };
                if cancelled {
                    Effects::two(Effect::Width(start), Effect::Snap)
                } else if excess != 0.0 {
                    Effects::one(Effect::Rebound)
                } else {
                    Effects::none()
                }
            }
        }
    }

    fn excess(&self) -> f32 {
        match self.0 {
            ResizeState::Drag { excess, .. } => excess,
            _ => 0.0,
        }
    }

    fn engaged(&self) -> bool {
        !matches!(self.0, ResizeState::Rest)
    }
}

/// A leading pane inside an existing row. The app places the pane, divider
/// and remaining content next to each other and owns the open/close control.
pub fn leading(pane: &PaneState, interaction: &ResizeInteraction, contents: El, motion: Spring) -> El {
    let width = pane.width();
    let mut contents = contents.pin(Some(0.0), Some(0.0), None, Some(0.0)).w(width);
    if !pane.is_open() {
        contents = contents.inert().a11y_hidden();
    }
    let excess = if pane.is_open() && motion != Spring::NONE { interaction.excess() } else { 0.0 };
    el().w(pane.shown_width() + excess).h_full().clip_soft(0.0).motion(motion).child(contents)
}

/// The visual style and accessible name of a standalone resize divider.
pub struct PaneDivider {
    label: String,
    line: Color,
    glow: Option<(f32, Color, f32)>,
}

impl PaneDivider {
    pub fn new(label: impl Into<String>, kit: &Kit) -> Self {
        Self { label: label.into(), line: kit.c.border_muted, glow: None }
    }

    /// Adds a centered hover glow with vertical fade at the ends.
    pub fn glow(mut self, width: f32, color: Color, end_fade: f32) -> Self {
        self.glow = Some((width.max(0.0), color, end_fade.max(0.0)));
        self
    }

    pub fn build<S: 'static>(
        self,
        pane: &PaneState,
        interaction: &ResizeInteraction,
        access: fn(&mut S) -> (&mut PaneState, &mut ResizeInteraction),
        motion: Spring,
    ) -> El {
        let reduced = motion == Spring::NONE;
        let fade = if reduced { Spring::NONE } else { Spring::new(0.21, 0.0) };
        let glow_opacity = if self.glow.is_some() && interaction.engaged() { 1.0 } else { 0.0 };

        el().key("pane-divider")
            .w(0.0)
            .h_full()
            .motion(motion)
            .paint_order(1)
            .child(
                el().pin(Some(-0.5), Some(0.0), None, Some(0.0))
                    .w(1.0)
                    .fill(self.line)
                    .inert()
                    .a11y_hidden()
                    .opacity(1.0 - glow_opacity)
                    .opacity_motion(fade),
            )
            .children(self.glow.map(|(width, peak, end_fade)| {
                el().key("pane-resize-glow")
                    .pin(Some(-width / 2.0), Some(0.0), None, Some(0.0))
                    .w(width)
                    .inert()
                    .a11y_hidden()
                    .opacity(glow_opacity)
                    .opacity_motion(fade)
                    .clip_fade(0.0, end_fade, 0.0, end_fade)
                    .child(el().w_full().h_full().linear_gradient(
                        std::f32::consts::FRAC_PI_2,
                        [
                            (0.0, peak.alpha(0.0)),
                            (0.19, peak.alpha(peak.a * 0.08)),
                            (0.34, peak.alpha(peak.a * 0.28)),
                            (0.44, peak.alpha(peak.a * 0.78)),
                            (0.5, peak),
                            (0.56, peak.alpha(peak.a * 0.78)),
                            (0.66, peak.alpha(peak.a * 0.28)),
                            (0.81, peak.alpha(peak.a * 0.08)),
                            (1.0, peak.alpha(0.0)),
                        ],
                    ))
            }))
            .children(pane.is_open().then(|| grip(self.label, pane, access, reduced)))
    }
}

fn grip<S: 'static>(
    label: String,
    pane: &PaneState,
    access: fn(&mut S) -> (&mut PaneState, &mut ResizeInteraction),
    reduced: bool,
) -> El {
    const GRIP_WIDTH: f32 = 7.0;
    el().key("pane-resize")
        .pin(Some(-GRIP_WIDTH / 2.0), Some(0.0), None, Some(0.0))
        .w(GRIP_WIDTH)
        .role("separator")
        .label(label)
        .description("Drag or use the arrow keys. Double-click to reset.")
        .focusable(true)
        .cursor(Cursor::ResizeColumn)
        .range_value(f64::from(pane.width()), f64::from(pane.min_width()), f64::from(pane.max_width()))
        .range_step(8.0)
        .on_hover_change(move |s: &mut S, hovered, _| {
            access(s).1.step(Event::Hover(hovered));
        })
        .on_drag(move |s: &mut S, drag, tx| {
            let (pane, interaction) = access(s);
            let event = match drag.phase {
                Phase::Down => Event::Begin(pane.width()),
                Phase::Move => Event::Move { delta: drag.dx, min: pane.min_width(), max: pane.max_width(), reduced },
                Phase::Up => Event::End { cancelled: drag.cancelled() },
            };
            for effect in interaction.step(event) {
                match effect {
                    Effect::Width(width) => pane.set_width(width),
                    Effect::Snap => tx.instant(),
                    Effect::Rebound => {
                        tx.animated();
                        tx.motion(Spring::new(0.34, 0.55));
                    }
                }
            }
        })
        .on_double_press(move |s: &mut S| access(s).0.reset_width())
        .on_step(move |s: &mut S, delta, _| {
            let (pane, _) = access(s);
            pane.set_width(pane.width() + 8.0 * delta as f32);
        })
        .on_step_limit(move |s: &mut S, end, _| {
            let (pane, _) = access(s);
            pane.set_width(if end { pane.max_width() } else { pane.min_width() });
        })
}
