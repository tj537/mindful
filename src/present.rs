//! Presentation mode: a title slide and one slide per main branch.
//!
//! This is also where the parts live that slides and the camera tour share:
//! effect settings, keyboard, buttons, slide strip, animated
//! backgrounds, laser pointer and easing curves.

use crate::diagrams::{draw as draw_diagram, layout_menu, Item};
use crate::document::Document;
use crate::icons;
use crate::images::{self, Fit};
use crate::model::{MindMap, SlideLayout, StoredImage};
use crate::nodes::paint_checkbox;
use crate::theme::Theme;
use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Id, Key, LayerId, Layout, Modifiers, Order,
    Pos2, Rect, RichText, Sense, Stroke, UiBuilder, Vec2,
};
use egui_phosphor::regular as icon;
use serde::{Deserialize, Serialize};

pub enum Event {
    Exit,
    ToggleFullscreen,
}

// ---------------------------------------------------------------------------
// Effect settings
// ---------------------------------------------------------------------------

/// Transition between two slides
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Transition {
    Slide,
    #[default]
    Fade,
    Zoom,
    Rise,
}

/// How a bullet appears
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum BulletFx {
    #[default]
    SlideIn,
    Fade,
    Pop,
    Typewriter,
}

/// Animated background
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Backdrop {
    Plain,
    Aurora,
    #[default]
    Stars,
    /// Soft, floating circles of light
    Bokeh,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    pub transition: Transition,
    pub bullets: BulletFx,
    pub background: Backdrop,
    /// Slow camera drift in the tour
    pub drift: bool,
    /// Tour: the camera springs slightly on arrival
    pub bounce: bool,
    /// Tour: glow around the current branch
    pub glow: bool,
    /// Tour: a wave that spreads on arrival
    pub ripple: bool,
    /// Tour: light pulses running along the connections
    pub pulses: bool,
    /// Tour: reveal sub-points level by level on a key press
    pub step_levels: bool,
    /// Confetti when ticking off a checkbox
    pub confetti: bool,
    /// Darker image edges
    pub vignette: bool,
    /// Slides: coloured bar at the left edge of the slide
    pub accent_bar: bool,
    /// Title slide: show the main branches as coloured topic chips
    pub title_topics: bool,
    /// Slides: a shimmer of light that sweeps across the title
    pub shine: bool,
    /// Slides: slow zoom while a slide is visible
    pub ken_burns: bool,
    /// Tour: stars stretch into streaks during the flight
    pub warp: bool,
    /// Tour: nodes outside the spotlight move back
    pub depth_of_field: bool,
    /// Tour: sparks circle the current branch
    pub orbit: bool,
    /// Version of the settings; missing in older saved data (= 0)
    #[serde(default)]
    pub version: u32,
}

const EFFECTS_VERSION: u32 = 3;

impl Default for Effects {
    fn default() -> Self {
        Self {
            transition: Transition::default(),
            bullets: BulletFx::default(),
            background: Backdrop::default(),
            drift: true,
            bounce: false,
            glow: false,
            ripple: true,
            pulses: false,
            step_levels: false,
            confetti: true,
            vignette: false,
            accent_bar: true,
            title_topics: true,
            shine: false,
            ken_burns: false,
            warp: false,
            depth_of_field: false,
            orbit: false,
            version: EFFECTS_VERSION,
        }
    }
}

impl Effects {
    /// Bring older saved settings up to the new defaults
    pub fn migrate(&mut self) {
        if self.version < 1 {
            // Since version 1, fade is the default transition
            self.transition = Transition::Fade;
        }
        if self.version < 2 {
            // Since version 2, the starfield is the default background
            self.background = Backdrop::Stars;
        }
        if self.version < 3 {
            // Since version 3, glow and light pulses are only on when asked for
            self.glow = false;
            self.pulses = false;
        }
        self.version = EFFECTS_VERSION;
    }
}

/// Choice of effects – in the presentation (button at the top right) and in the View menu
pub fn effects_menu(ui: &mut egui::Ui, fx: &mut Effects, slides: bool, tour: bool) {
    ui.set_min_width(260.0);
    egui::ScrollArea::vertical()
        .id_salt("effects_menu")
        .max_height(560.0)
        .show(ui, |ui| {
            if slides {
                ui.strong("Slide transition");
                ui.horizontal_wrapped(|ui| {
                    for (value, label) in [
                        (Transition::Fade, "Fade"),
                        (Transition::Slide, "Slide"),
                        (Transition::Zoom, "Zoom"),
                        (Transition::Rise, "Rise"),
                    ] {
                        ui.selectable_value(&mut fx.transition, value, label);
                    }
                });
                ui.strong("Bullet points");
                ui.horizontal_wrapped(|ui| {
                    for (value, label) in [
                        (BulletFx::SlideIn, "Slide in"),
                        (BulletFx::Fade, "Fade in"),
                        (BulletFx::Pop, "Pop"),
                        (BulletFx::Typewriter, "Typewriter"),
                    ] {
                        ui.selectable_value(&mut fx.bullets, value, label);
                    }
                });
                ui.checkbox(&mut fx.accent_bar, "Colored bar on the left");
                ui.checkbox(&mut fx.title_topics, "Topics on the title slide");
                ui.checkbox(&mut fx.shine, "Light sweep over titles");
                ui.checkbox(&mut fx.ken_burns, "Slow zoom on each slide");
                ui.separator();
            }
            ui.strong("Background");
            ui.horizontal_wrapped(|ui| {
                for (value, label) in [
                    (Backdrop::Stars, "Starfield"),
                    (Backdrop::Bokeh, "Bokeh"),
                    (Backdrop::Aurora, "Aurora"),
                    (Backdrop::Plain, "Plain"),
                ] {
                    ui.selectable_value(&mut fx.background, value, label);
                }
            });
            ui.checkbox(&mut fx.vignette, "Vignette (darker edges)");
            if tour {
                ui.separator();
                ui.strong("Tour");
                ui.checkbox(&mut fx.step_levels, "Reveal levels one by one (Space / →)");
                ui.checkbox(&mut fx.ripple, "Ripple on arrival");
                ui.checkbox(&mut fx.glow, "Glow around the current branch");
                ui.checkbox(&mut fx.pulses, "Light pulses along connections");
                ui.checkbox(&mut fx.orbit, "Sparks orbiting the current branch");
                ui.checkbox(&mut fx.depth_of_field, "Depth of field");
                ui.checkbox(&mut fx.warp, "Warp streaks while flying");
                ui.checkbox(&mut fx.bounce, "Camera bounce on arrival");
                ui.checkbox(&mut fx.drift, "Slow camera drift");
            }
            ui.separator();
            ui.checkbox(&mut fx.confetti, "Confetti when ticking a checkbox");
        });
}

/// Keyboard shortcuts while presenting (in the settings menu)
fn shortcuts_list(ui: &mut egui::Ui, tour: bool) {
    ui.strong("Keyboard");
    let next = if tour {
        "Next stop"
    } else {
        "Next bullet point / slide"
    };
    // Mac keyboards mostly have no Home/End keys: fn + arrow does the same
    let home_end = if ui.ctx().os() == egui::os::OperatingSystem::Mac {
        "fn←  fn→"
    } else {
        "Home  End"
    };
    egui::Grid::new("present_keys")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (keys, what) in [
                ("Space  →", next),
                ("←", "Back"),
                (home_end, "First / last"),
                ("G", "Quick jump (← → Enter)"),
                ("L", "Laser pointer"),
                ("P", "Presenter view (slides only)"),
                ("R", "Phone as remote control (slides only)"),
                ("F", "Full screen"),
                ("Esc", "End"),
            ] {
                ui.label(RichText::new(keys).monospace());
                ui.label(what);
                ui.end_row();
            }
        });
    ui.weak("Click a checkbox to tick it off.");
}

// ---------------------------------------------------------------------------
// Shared building blocks
// ---------------------------------------------------------------------------

pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Overshoots the target slightly and springs back ("pop")
pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Progress 0…1 of an animation that begins at `start` and lasts `duration`
fn progress(now: f64, start: f64, duration: f64) -> f32 {
    ((now - start) / duration).clamp(0.0, 1.0) as f32
}

pub fn color_of(map: &MindMap, theme: &Theme, id: u64) -> Color32 {
    map.color_index(id)
        .map(|c| theme.branches[c % theme.branches.len()])
        .unwrap_or(theme.root)
}

/// Keys while presenting
#[derive(Default)]
pub struct Nav {
    /// Arrow right/down, Page Down
    pub right: bool,
    /// Arrow left/up, Page Up, Backspace
    pub left: bool,
    pub space: bool,
    pub enter: bool,
    pub home: bool,
    pub end: bool,
    pub escape: bool,
    pub fullscreen: bool,
    pub strip: bool,
    pub laser: bool,
    /// P: presenter view on/off
    pub presenter: bool,
    /// R: QR code for the phone remote
    pub remote: bool,
}

pub fn read_nav(ui: &mut egui::Ui) -> Nav {
    ui.input_mut(|i| {
        let mut any = |keys: &[Key]| keys.iter().any(|k| i.consume_key(Modifiers::NONE, *k));
        Nav {
            right: any(&[Key::ArrowRight, Key::ArrowDown, Key::PageDown]),
            left: any(&[Key::ArrowLeft, Key::ArrowUp, Key::PageUp, Key::Backspace]),
            space: any(&[Key::Space]),
            enter: any(&[Key::Enter]),
            home: any(&[Key::Home]),
            end: any(&[Key::End]),
            escape: any(&[Key::Escape]),
            fullscreen: any(&[Key::F]),
            strip: any(&[Key::G]),
            laser: any(&[Key::L]),
            presenter: any(&[Key::P]),
            remote: any(&[Key::R]),
        }
    })
}

impl Nav {
    pub fn next(&self) -> bool {
        self.right || self.space || self.enter
    }

    /// Merge the keys from two windows
    fn merge(self, o: Nav) -> Nav {
        Nav {
            right: self.right || o.right,
            left: self.left || o.left,
            space: self.space || o.space,
            enter: self.enter || o.enter,
            home: self.home || o.home,
            end: self.end || o.end,
            escape: self.escape || o.escape,
            fullscreen: self.fullscreen || o.fullscreen,
            strip: self.strip || o.strip,
            laser: self.laser || o.laser,
            presenter: self.presenter || o.presenter,
            remote: self.remote || o.remote,
        }
    }
}

/// What should happen in the open quick-jump strip
pub enum StripKey {
    Stay,
    Jump(usize),
    Close,
}

/// Keys with the quick jump open: ← → select, Enter/Space jumps, Esc/G closes
pub fn strip_keys(nav: &Nav, cursor: &mut usize, last: usize) -> StripKey {
    if nav.right {
        *cursor = (*cursor + 1).min(last);
    }
    if nav.left {
        *cursor = cursor.saturating_sub(1);
    }
    if nav.home {
        *cursor = 0;
    }
    if nav.end {
        *cursor = last;
    }
    if nav.enter || nav.space {
        StripKey::Jump(*cursor)
    } else if nav.escape || nav.strip {
        StripKey::Close
    } else {
        StripKey::Stay
    }
}

/// Buttons at the top right: quit, full screen, slide strip, effects.
/// Returns the event and "toggle slide strip".
pub fn corner_buttons(
    ui: &mut egui::Ui,
    area: Rect,
    exit_tip: &str,
    fx: &mut Effects,
    tour: bool,
) -> Corner {
    let mut corner = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                area.right_top() + Vec2::new(-300.0, 10.0),
                Vec2::new(290.0, 36.0),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    let big = |ic: &str| RichText::new(ic).size(18.0);
    let mut event = None;
    if corner
        .button(big(icon::X))
        .on_hover_text(exit_tip)
        .clicked()
    {
        event = Some(Event::Exit);
    }
    if corner
        .button(big(icon::CORNERS_OUT))
        .on_hover_text("Full screen (F)")
        .clicked()
    {
        event = Some(Event::ToggleFullscreen);
    }
    let strip = corner
        .button(big(icon::FILM_STRIP))
        .on_hover_text("Quick jump (G)")
        .clicked();
    let presenter = !tour
        && corner
            .button(big(icon::PROJECTOR_SCREEN))
            .on_hover_text("Presenter view: notes, next slide and timer in a second window (P)")
            .clicked();
    let remote = !tour
        && corner
            .button(big(icon::DEVICE_MOBILE))
            .on_hover_text("Use your phone as a remote control (R)")
            .clicked();
    corner
        .menu_button(big(icon::GEAR), |ui| {
            effects_menu(ui, fx, !tour, tour);
            ui.separator();
            shortcuts_list(ui, tour);
        })
        .response
        .on_hover_text("Settings and shortcuts");
    Corner {
        event,
        strip,
        presenter,
        remote,
    }
}

/// What was clicked in the button bar at the top right
pub struct Corner {
    pub event: Option<Event>,
    pub strip: bool,
    pub presenter: bool,
    pub remote: bool,
}

/// Animated background behind the slide or mind map.
/// Returns `true` if it needs to keep repainting.
/// Pseudo-random number 0…1, always the same for the same inputs
fn hash(i: f32, salt: f32) -> f32 {
    ((i * 12.9898 + salt * 78.233).sin() * 43758.547)
        .fract()
        .abs()
}

/// `parallax`: offset (e.g. camera), near stars move more than far ones.
/// `warp`: camera movement; if it is large enough, stars turn into streaks.
pub fn backdrop(
    painter: &egui::Painter,
    area: Rect,
    theme: &Theme,
    kind: Backdrop,
    now: f64,
    parallax: Vec2,
    warp: Vec2,
) -> bool {
    let t = now as f32;
    match kind {
        Backdrop::Plain => false,
        Backdrop::Bokeh => {
            // Soft circles of light in the branch colours, slowly drifting upwards
            let (w, h) = (area.width(), area.height());
            for i in 0..22 {
                let n = i as f32 * 3.7;
                let depth = 0.3 + 0.7 * hash(n, 3.0);
                let radius = (25.0 + 70.0 * hash(n, 4.0)) * depth;
                let x = area.left()
                    + (hash(n, 1.0) * w + parallax.x * depth * 0.2 + (t * 0.3 + n).sin() * 20.0)
                        .rem_euclid(w);
                let y = area.top()
                    + (hash(n, 2.0) * h - t * 8.0 * depth + parallax.y * depth * 0.2)
                        .rem_euclid(h + 2.0 * radius)
                    - radius;
                let flicker = 0.7 + 0.3 * (t * (0.4 + hash(n, 5.0)) + n).sin();
                let color = theme.branches[i % 6]
                    .gamma_multiply((if theme.dark { 0.035 } else { 0.05 }) * flicker);
                for ring in 0..6 {
                    painter.circle_filled(
                        Pos2::new(x, y),
                        radius * (1.0 - ring as f32 * 0.12),
                        color,
                    );
                }
            }
            true
        }
        Backdrop::Aurora => {
            // Soft colour clouds in the branch colours, wandering slowly
            let radius = area.width().max(area.height()) * 0.38;
            let strength = if theme.dark { 0.02 } else { 0.03 };
            for k in 0..4 {
                let phase = k as f32 * 1.7;
                let center = Pos2::new(
                    area.center().x
                        + area.width() * 0.32 * (t * (0.05 + 0.012 * k as f32) + phase).sin(),
                    area.center().y
                        + area.height() * 0.28 * (t * (0.04 + 0.01 * k as f32) + phase * 1.3).cos(),
                );
                let color = theme.branches[k].gamma_multiply(strength);
                for ring in 0..14 {
                    let f = 1.0 - ring as f32 / 14.0;
                    painter.circle_filled(center, radius * f, color);
                }
            }
            true
        }
        Backdrop::Stars => {
            let (w, h) = (area.width(), area.height());
            let star = if theme.dark {
                theme.text.lerp_to_gamma(Color32::WHITE, 0.6)
            } else {
                theme.text
            };

            // Delicate nebula in two branch colours, barely visible
            let radius = w.max(h) * 0.45;
            for (k, color) in [theme.branches[0], theme.branches[4]]
                .into_iter()
                .enumerate()
            {
                let phase = k as f32 * 2.4;
                let center = area.center()
                    + Vec2::new(
                        w * 0.3 * (t * 0.02 + phase).sin(),
                        h * 0.25 * (t * 0.015 + phase).cos(),
                    )
                    + parallax * 0.02;
                let color = color.gamma_multiply(if theme.dark { 0.012 } else { 0.02 });
                for ring in 0..12 {
                    painter.circle_filled(center, radius * (1.0 - ring as f32 / 12.0), color);
                }
            }

            // Three layers: far (many, small), middle, near (few, bright, with a glow)
            let layers = [
                (220, 0.04, 0.45, 0.9, 0.35),
                (90, 0.12, 0.8, 1.5, 0.6),
                (26, 0.3, 1.3, 2.3, 0.95),
            ];
            for (layer, (count, depth, small, big, bright)) in layers.into_iter().enumerate() {
                for i in 0..count {
                    let n = (i * 7 + layer * 1013) as f32;
                    let x = area.left()
                        + (hash(n, 1.0) * w + parallax.x * depth + t * 3.0 * depth).rem_euclid(w);
                    let y = area.top() + (hash(n, 2.0) * h + parallax.y * depth).rem_euclid(h);
                    let pos = Pos2::new(x, y);
                    let twinkle = 0.45
                        + 0.55
                            * (0.5
                                + 0.5
                                    * (t * (0.5 + 1.8 * hash(n, 4.0))
                                        + hash(n, 5.0) * std::f32::consts::TAU)
                                        .sin());
                    // Some stars slightly tinted in the branch colours
                    let color = if hash(n, 6.0) < 0.15 {
                        star.lerp_to_gamma(theme.branches[(hash(n, 7.0) * 6.0) as usize % 6], 0.55)
                    } else {
                        star
                    };
                    let r = small + (big - small) * hash(n, 3.0);
                    let a = bright * twinkle;
                    // During a fast flight: streaks of light, near stars longer than far ones
                    let streak = warp * (depth * 4.0);
                    if streak.length() > 2.0 {
                        painter.line_segment(
                            [pos, pos - streak],
                            Stroke::new(r * 1.4, color.gamma_multiply(a * 0.8)),
                        );
                    }
                    if layer == 2 {
                        painter.circle_filled(pos, r * 4.5, color.gamma_multiply(0.05 * a));
                        painter.circle_filled(pos, r * 2.2, color.gamma_multiply(0.12 * a));
                        // The brightest ones twinkle with a small cross
                        if hash(n, 8.0) > 0.6 {
                            let len = r * 6.0 * twinkle;
                            let stroke = Stroke::new(0.8_f32, color.gamma_multiply(0.35 * a));
                            painter
                                .line_segment([pos - Vec2::X * len, pos + Vec2::X * len], stroke);
                            painter
                                .line_segment([pos - Vec2::Y * len, pos + Vec2::Y * len], stroke);
                        }
                    }
                    painter.circle_filled(pos, r, color.gamma_multiply(a));
                }
            }

            // Now and then a shooting star
            let period = 7.0;
            let cycle = (now / period).floor();
            let phase = (now - cycle * period) as f32;
            if phase < 1.1 {
                let seed = cycle as f32;
                let start = Pos2::new(
                    area.left() + w * (0.35 + 0.6 * hash(seed, 11.0)),
                    area.top() + h * 0.3 * hash(seed, 12.0),
                );
                let dir = Vec2::new(-0.86, 0.5);
                let travel = w * 0.35;
                let head = start + dir * travel * ease_out_cubic(phase / 1.1);
                let fade = (1.0 - phase / 1.1) * (phase * 6.0).min(1.0);
                for k in 0..18 {
                    let a = head - dir * (k as f32 * 7.0);
                    let b = head - dir * ((k + 1) as f32 * 7.0);
                    let falloff = 1.0 - k as f32 / 18.0;
                    painter.line_segment(
                        [a, b],
                        Stroke::new(2.2 * falloff, star.gamma_multiply(0.8 * fade * falloff)),
                    );
                }
                painter.circle_filled(head, 5.0, star.gamma_multiply(0.15 * fade));
                painter.circle_filled(head, 1.8, star.gamma_multiply(fade));
            }
            true
        }
    }
}

#[derive(Clone, Default)]
struct LaserState {
    on: bool,
    trail: Vec<(Pos2, f64)>,
}

/// Laser pointer (key L): glowing dot with a short tail instead of the mouse pointer
pub fn laser(ui: &mut egui::Ui, toggle: bool, now: f64) {
    let id = Id::new("laser_pointer");
    let ctx = ui.ctx().clone();
    let mut state: LaserState = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    if toggle {
        state.on = !state.on;
        state.trail.clear();
    }
    if state.on {
        if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
            state.trail.push((p, now));
        }
        state.trail.retain(|(_, t)| now - t < 0.25);
        ctx.set_cursor_icon(CursorIcon::None);
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, id));
        let red = Color32::from_rgb(255, 45, 70);
        for pair in state.trail.windows(2) {
            let age = ((now - pair[0].1) / 0.25) as f32;
            painter.line_segment(
                [pair[0].0, pair[1].0],
                Stroke::new(6.0 * (1.0 - age), red.gamma_multiply(0.5 * (1.0 - age))),
            );
        }
        if let Some((p, _)) = state.trail.last() {
            for (r, a) in [(18.0, 0.08), (12.0, 0.15), (8.0, 0.3)] {
                painter.circle_filled(*p, r, red.gamma_multiply(a));
            }
            painter.circle_filled(*p, 4.5, red);
            painter.circle_filled(*p, 2.0, Color32::WHITE);
        }
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(id, state));
}

/// Darker image edges ("vignette")
pub fn vignette(painter: &egui::Painter, area: Rect) {
    let steps = 24;
    let band = area.width().min(area.height()) * 0.22 / steps as f32;
    for i in 0..steps {
        let f = 1.0 - i as f32 / steps as f32;
        painter.rect_stroke(
            area.shrink(i as f32 * band),
            0.0,
            Stroke::new(band + 0.5, Color32::from_black_alpha((f * f * 22.0) as u8)),
            egui::StrokeKind::Inside,
        );
    }
}

#[derive(Clone)]
struct Particle {
    pos: Pos2,
    vel: Vec2,
    color: Color32,
    born: f64,
    spin: f32,
}

const CONFETTI_LIFE: f64 = 1.4;

/// Confetti burst at `at` (e.g. when ticking off a checkbox)
pub fn confetti_burst(ctx: &egui::Context, at: Pos2, colors: &[Color32], now: f64) {
    let id = Id::new("confetti");
    let mut parts: Vec<Particle> = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    let seed = (now * 1000.0) as f32 % 1000.0;
    for i in 0..40 {
        let n = seed + i as f32;
        let angle = -std::f32::consts::FRAC_PI_2 + (hash(n, 1.0) - 0.5) * 2.6;
        let speed = 220.0 + 320.0 * hash(n, 2.0);
        parts.push(Particle {
            pos: at,
            vel: Vec2::new(angle.cos(), angle.sin()) * speed,
            color: colors[i % colors.len().max(1)],
            born: now,
            spin: hash(n, 3.0) * std::f32::consts::TAU,
        });
    }
    ctx.data_mut(|d| d.insert_temp(id, parts));
}

/// Draw the flying confetti (call every frame)
pub fn confetti_draw(ctx: &egui::Context, now: f64) {
    let id = Id::new("confetti");
    let mut parts: Vec<Particle> = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    parts.retain(|p| now - p.born < CONFETTI_LIFE);
    if parts.is_empty() {
        return;
    }
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, id));
    for p in &parts {
        let t = (now - p.born) as f32;
        let pos = p.pos + p.vel * t + Vec2::new(0.0, 520.0) * t * t * 0.5;
        let fade = 1.0 - t / CONFETTI_LIFE as f32;
        let angle = p.spin + t * 9.0;
        let half = Vec2::new(angle.cos(), angle.sin()) * 4.0;
        painter.line_segment(
            [pos - half, pos + half],
            Stroke::new(3.5_f32, p.color.gamma_multiply(fade)),
        );
    }
    ctx.data_mut(|d| d.insert_temp(id, parts));
    ctx.request_repaint();
}

/// An entry in the slide strip
pub struct StripItem {
    pub label: String,
    pub title: String,
    pub color: Color32,
}

/// Quick-jump strip at the bottom (key G). `cursor` is the slide highlighted
/// with the keyboard, `current` the one shown. Returns the clicked slide.
pub fn filmstrip(
    ui: &mut egui::Ui,
    area: Rect,
    theme: &Theme,
    items: &[StripItem],
    current: usize,
    cursor: usize,
    open: bool,
) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let id = ui.id().with("filmstrip");
    let height = 136.0;
    let t = ctx.animate_bool_with_time(id.with("anim"), open, 0.18);
    if t <= 0.0 {
        return None;
    }

    let top = area.bottom() - height * ease_out_cubic(t);
    let bar = Rect::from_min_max(
        Pos2::new(area.left(), top),
        Pos2::new(area.right(), top + height),
    );
    // Clicks on the strip should not reach checkboxes underneath
    ui.interact(bar, id.with("bar"), Sense::click());
    let mut painter = ui.painter().clone();
    painter.set_opacity(t);
    painter.rect_filled(bar, 0.0, theme.card);
    painter.line_segment(
        [bar.left_top(), bar.right_top()],
        Stroke::new(1.0_f32, theme.grid),
    );

    let size = Vec2::new(168.0, 94.5);
    let gap = 14.0;
    let total = items.len() as f32 * (size.x + gap) - gap;
    let start = if total < area.width() - 48.0 {
        area.center().x - total / 2.0
    } else {
        // Highlighted slide to the centre, without scrolling past the edges
        (area.center().x - cursor as f32 * (size.x + gap) - size.x / 2.0)
            .clamp(area.right() - 24.0 - total, area.left() + 24.0)
    };

    let mut clicked = None;
    for (i, item) in items.iter().enumerate() {
        let rect = Rect::from_min_size(
            Pos2::new(start + i as f32 * (size.x + gap), top + 20.0),
            size,
        );
        if rect.right() < area.left() || rect.left() > area.right() {
            continue;
        }
        let resp = ui.interact(rect, id.with(i), Sense::click());
        if i == cursor {
            painter.rect_filled(rect.expand(3.0), 10.0, theme.selection);
        }
        if i == current {
            // Small dot: this slide is being shown right now
            painter.circle_filled(rect.center_bottom() + Vec2::new(0.0, 9.0), 3.0, theme.text);
        }
        let fill = if resp.hovered() {
            theme.background.lerp_to_gamma(item.color, 0.15)
        } else {
            theme.background
        };
        painter.rect_filled(rect, 8.0, fill);
        painter.rect_filled(
            Rect::from_min_size(
                rect.min + Vec2::new(10.0, 10.0),
                Vec2::new(4.0, size.y - 20.0),
            ),
            2.0,
            item.color,
        );
        let clip = painter.with_clip_rect(rect.shrink(4.0));
        clip.text(
            rect.min + Vec2::new(22.0, 10.0),
            Align2::LEFT_TOP,
            &item.label,
            FontId::proportional(11.0),
            item.color,
        );
        let galley = clip.layout(
            item.title.clone(),
            FontId::proportional(14.0),
            theme.text,
            size.x - 32.0,
        );
        clip.galley(rect.min + Vec2::new(22.0, 28.0), galley, theme.text);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

// ---------------------------------------------------------------------------
// Slides
// ---------------------------------------------------------------------------

/// A bullet: level, text, colour, checkbox, node ID
type Bullet = (usize, String, Color32, Option<bool>, u64);

struct Slide {
    title: String,
    note: String,
    bullets: Vec<Bullet>,
    color: Color32,
    is_title: bool,
    /// Main branch of the slide (for checkbox progress and the layout choice)
    branch: Option<u64>,
    /// Diagram instead of a bullet list
    layout: Option<SlideLayout>,
    /// Pictures: the branch's own (without a caption) and those of bullets
    images: Vec<SlideImage>,
}

struct SlideImage {
    key: String,
    caption: Option<String>,
}

impl Slide {
    /// A single picture without bullets (or on the title slide) fills the whole slide
    fn full_bleed(&self) -> bool {
        !self.images.is_empty()
            && (self.is_title
                || (self.layout.is_none() && self.bullets.is_empty() && self.images.len() == 1))
    }
}

impl Slide {
    /// Bullet groups: a top-level point with its sub-points.
    /// The title slide is not built up step by step.
    fn groups(&self) -> usize {
        if self.is_title {
            return 0;
        }
        let top = self.bullets.iter().filter(|b| b.0 == 0).count();
        match self.layout {
            Some(layout) => layout.steps(top),
            None => top,
        }
    }

    /// Top-level points with their direct sub-points (for diagrams)
    fn items(&self) -> Vec<Item> {
        let mut out: Vec<Item> = Vec::new();
        for (depth, text, ..) in &self.bullets {
            match depth {
                0 => out.push(Item {
                    text: text.clone(),
                    children: Vec::new(),
                }),
                1 => {
                    if let Some(last) = out.last_mut() {
                        last.children.push(text.clone());
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Blurred shadow under `rect`: offset downwards by `drop`,
/// with a soft edge of width `blur`
fn soft_shadow(rect: Rect, radius: f32, drop: f32, blur: f32, color: Color32) -> egui::Shape {
    let shadow = egui::epaint::Shadow {
        offset: [0, drop.round().clamp(0.0, 127.0) as i8],
        blur: blur.round().clamp(0.0, 255.0) as u8,
        spread: 0,
        color,
    };
    egui::Shape::Rect(shadow.as_shape(rect, radius.round().clamp(0.0, 255.0) as u8))
}

/// Darken the picture at the top (and at the bottom for the progress bar) so light text stays readable.
/// A mesh with shared edges that follows the slide's rounding at the corners –
/// so there are neither stripes nor seams.
fn darken_for_text(p: &egui::Painter, card: Rect, s: f32, title_slide: bool, alpha: f32) {
    let radius = 24.0 * s;
    if title_slide {
        // Title slide: text in the middle, so darken evenly
        p.rect_filled(
            card,
            radius.round() as u8,
            Color32::from_black_alpha((120.0 * alpha) as u8),
        );
    }
    // Inset at the rounded corners, `d` = distance from the top or bottom edge
    let inset = |d: f32| {
        if d >= radius {
            0.0
        } else {
            radius - (radius * radius - (radius - d).powi(2)).max(0.0).sqrt()
        }
    };
    // Gradient from the edge (`edge`) over `reach` pixels inwards; `dir` = +1 top, -1 bottom
    let gradient = |edge: f32, dir: f32, reach: f32, strength: f32, curve: f32| {
        let mut ys: Vec<f32> = (0..8).map(|k| radius * k as f32 / 8.0).collect();
        ys.extend((0..=24).map(|k| radius + (reach - radius) * k as f32 / 24.0));
        let mut mesh = egui::Mesh::default();
        for (k, d) in ys.iter().enumerate() {
            let t = d / reach;
            let c = Color32::from_black_alpha((strength * (1.0 - t).powf(curve) * alpha) as u8);
            let y = edge + dir * d;
            let x = inset(*d);
            mesh.colored_vertex(Pos2::new(card.left() + x, y), c);
            mesh.colored_vertex(Pos2::new(card.right() - x, y), c);
            if k > 0 {
                let i = 2 * k as u32;
                mesh.add_triangle(i - 2, i - 1, i);
                mesh.add_triangle(i - 1, i + 1, i);
            }
        }
        p.add(egui::Shape::mesh(mesh));
    };
    let reach = card.height() * if title_slide { 0.35 } else { 0.6 };
    gradient(card.top(), 1.0, reach, 190.0, 1.6);
    // A narrow shadow at the bottom for the progress bar
    gradient(card.bottom(), -1.0, 100.0 * s, 120.0, 1.0);
}

/// Pictures in a grid: a single one fully visible, several filling their cells with captions
#[allow(clippy::too_many_arguments)]
fn picture_grid(
    p: &egui::Painter,
    ctx: &egui::Context,
    stored: &std::collections::BTreeMap<String, std::sync::Arc<StoredImage>>,
    pictures: &[SlideImage],
    rect: Rect,
    s: f32,
    appear: f32,
    theme: &Theme,
) {
    let pictures = &pictures[..pictures.len().min(6)];
    let n = pictures.len();
    if n == 0 || !rect.is_positive() {
        return;
    }
    let gap = 18.0 * s;
    let caption_h = if pictures.iter().any(|pic| pic.caption.is_some()) {
        34.0 * s
    } else {
        0.0
    };
    // Choose the number of columns so the pictures get as large as possible
    let cell_for = |cols: usize| {
        let rows = n.div_ceil(cols);
        let w = (rect.width() - gap * (cols - 1) as f32) / cols as f32;
        let h = (rect.height() - gap * (rows - 1) as f32) / rows as f32 - caption_h;
        // No wider than 16:9 and no taller than 3:4
        Vec2::new(w.min(h * 16.0 / 9.0), h.min(w * 4.0 / 3.0))
    };
    let cols = (1..=n)
        .max_by(|a, b| {
            let (ca, cb) = (cell_for(*a), cell_for(*b));
            (ca.x * ca.y).total_cmp(&(cb.x * cb.y))
        })
        .unwrap_or(1);
    let rows = n.div_ceil(cols);
    let cell = cell_for(cols);
    if cell.x <= 0.0 || cell.y <= 0.0 {
        return;
    }
    let total = Vec2::new(
        cell.x * cols as f32 + gap * (cols - 1) as f32,
        (cell.y + caption_h) * rows as f32 + gap * (rows - 1) as f32,
    );
    let origin = rect.center() - total / 2.0;
    for (i, pic) in pictures.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        // Incomplete last row centred
        let in_row = if row + 1 == rows {
            n - row * cols
        } else {
            cols
        };
        let indent = (cols - in_row) as f32 * (cell.x + gap) / 2.0;
        // Fade in one after another, rising slightly
        let e = ((appear * (1.0 + 0.25 * n as f32)) - 0.25 * i as f32).clamp(0.0, 1.0);
        let frame = Rect::from_min_size(
            origin
                + Vec2::new(
                    indent + col as f32 * (cell.x + gap),
                    row as f32 * (cell.y + caption_h + gap) + (1.0 - e) * 16.0 * s,
                ),
            cell,
        );
        let tint = Color32::WHITE.gamma_multiply(e);
        let fit = if n == 1 { Fit::Contain } else { Fit::Cover };
        let shown = match stored.get(&pic.key) {
            Some(img) => {
                let shown = images::contain(frame, img.aspect());
                let target = if fit == Fit::Contain { shown } else { frame };
                // Soft shadow under the picture
                p.add(soft_shadow(
                    target,
                    14.0 * s,
                    8.0 * s,
                    24.0 * s,
                    Color32::from_black_alpha((90.0 * e) as u8),
                ));
                images::paint(p, ctx, &pic.key, img, frame, fit, 14.0 * s, tint, 1.0)
            }
            None => {
                images::placeholder(p, frame, 14.0 * s);
                frame
            }
        };
        if let Some(caption) = &pic.caption {
            let galley = p.layout(
                caption.clone(),
                FontId::proportional(18.0 * s),
                theme.text.gamma_multiply(0.8 * e),
                frame.width(),
            );
            let pos = Pos2::new(
                shown.center().x - galley.size().x / 2.0,
                shown.bottom() + 8.0 * s,
            );
            p.with_clip_rect(Rect::from_min_size(
                Pos2::new(frame.left(), shown.bottom()),
                Vec2::new(frame.width(), caption_h),
            ))
            .galley(pos, galley, theme.text);
        }
    }
}

fn collect(map: &MindMap, id: u64, depth: usize, color: Color32, out: &mut Vec<Bullet>) {
    for child in map.children(id) {
        out.push((depth, icons::label(child), color, child.task, child.id));
        if depth < 2 {
            collect(map, child.id, depth + 1, color, out);
        }
    }
}

fn slides(map: &MindMap, theme: &Theme) -> Vec<Slide> {
    let Some(root) = map.root().and_then(|r| map.get(r)) else {
        return Vec::new();
    };
    let mut out = vec![Slide {
        title: icons::label(root),
        note: root.note.clone(),
        bullets: map
            .children(root.id)
            .map(|c| (0, icons::label(c), color_of(map, theme, c.id), c.task, c.id))
            .collect(),
        color: theme.root,
        is_title: true,
        branch: None,
        layout: None,
        images: root
            .image
            .iter()
            .map(|key| SlideImage {
                key: key.clone(),
                caption: None,
            })
            .collect(),
    }];
    for branch in map.children(root.id) {
        let color = color_of(map, theme, branch.id);
        let mut bullets = Vec::new();
        collect(map, branch.id, 0, color, &mut bullets);
        let mut images: Vec<SlideImage> = branch
            .image
            .iter()
            .map(|key| SlideImage {
                key: key.clone(),
                caption: None,
            })
            .collect();
        // Bullets with a picture become captioned pictures. Without sub-points
        // of their own they disappear from the bullet list.
        if branch.layout.is_none() {
            bullets.retain(|(_, text, _, _, id)| {
                let Some(key) = map.get(*id).and_then(|n| n.image.clone()) else {
                    return true;
                };
                images.push(SlideImage {
                    key,
                    caption: Some(text.clone()),
                });
                map.has_children(*id)
            });
        }
        out.push(Slide {
            images,
            title: icons::label(branch),
            note: branch.note.clone(),
            bullets,
            color,
            is_title: false,
            branch: Some(branch.id),
            layout: branch.layout,
        });
    }
    out
}

/// What is needed to draw a slide – in the talk, in the
/// presenter view and for the PDF export
pub struct SlideView<'a> {
    slides: &'a [Slide],
    pub index: usize,
    /// Visible bullet groups
    pub build: usize,
    pub build_time: f64,
    pub slide_time: f64,
    pub now: f64,
    /// Opacity during the transition
    pub appear: f32,
    /// Progress bar at the bottom (0…1)
    pub progress: f32,
    /// Checkboxes and the right-click menu can be used
    pub interactive: bool,
    /// As a whole page (PDF): without shadow and rounded corners
    pub page: bool,
}

/// What was clicked while drawing
pub struct SlideOutput {
    pub ticked: Option<(u64, Pos2)>,
    pub new_layout: Option<Option<SlideLayout>>,
    pub animating: bool,
}

/// Draw a slide in `card`
pub fn render_slide(
    ui: &mut egui::Ui,
    card: Rect,
    view: &SlideView,
    map: &MindMap,
    theme: &Theme,
    fx: &Effects,
    build_bullets: bool,
) -> SlideOutput {
    let painter = ui.painter().clone();
    let ctx = ui.ctx().clone();
    let slide = &view.slides[view.index];
    let last = view.slides.len() - 1;
    let now = view.now;
    let appear = view.appear;
    // All measurements relative to a slide 1280 px wide
    let s = card.width() / 1280.0;

    let radius = if view.page { 0.0 } else { 24.0 * s };
    if !view.page {
        // Soft, blurred shadow under the slide
        let alpha = if theme.dark { 150 } else { 55 };
        painter.add(soft_shadow(
            card,
            radius,
            18.0 * s,
            56.0 * s,
            Color32::from_black_alpha(alpha),
        ));
    }
    painter.rect_filled(card, radius, theme.card);
    let full_bleed = slide.full_bleed();
    // Pictures fade in with the slide
    let pictures_in = ease_out_cubic(progress(now, view.slide_time + 0.1, 0.8));
    if full_bleed {
        let picture = &slide.images[0];
        let zoom = if fx.ken_burns {
            1.0 + 0.07 * ease_in_out_cubic(progress(now, view.slide_time, 16.0))
        } else {
            1.0
        };
        match map.images.get(&picture.key) {
            Some(img) => {
                images::paint(
                    &painter,
                    &ctx,
                    &picture.key,
                    img,
                    card,
                    Fit::Cover,
                    radius,
                    Color32::WHITE.gamma_multiply(appear * pictures_in),
                    zoom,
                );
            }
            None => images::placeholder(&painter, card, 24.0 * s),
        }
        darken_for_text(&painter, card, s, slide.is_title, appear);
    }
    if fx.accent_bar {
        let accent_bar = Rect::from_min_max(
            card.left_top() + Vec2::new(44.0, 80.0) * s,
            card.left_bottom() + Vec2::new(52.0, -80.0) * s,
        );
        painter.rect_filled(accent_bar, 4.0 * s, slide.color.gamma_multiply(appear));
    }

    // Right-click on the slide: choose a layout (before the content, so checkboxes take priority)
    let mut new_layout: Option<Option<SlideLayout>> = None;
    if view.interactive && slide.branch.is_some() {
        ui.interact(card, ui.id().with("slide_card"), Sense::click())
            .context_menu(|ui| {
                ui.label(RichText::new("Slide layout").strong());
                new_layout = layout_menu(ui, slide.layout);
            });
    }

    // How many checkboxes of this branch are already done
    let tasks = slide.branch.and_then(|b| map.task_progress(b));
    // Clicked checkbox, toggled after drawing
    let mut ticked: Option<(u64, Pos2)> = None;

    // The title rises and fades in, the line below it draws itself
    let title_in = ease_out_cubic(progress(now, view.slide_time, 0.55));
    let line_in = ease_out_cubic(progress(now, view.slide_time + 0.2, 0.5));
    let mut animating = title_in < 1.0 || line_in < 1.0 || appear < 1.0;

    // Slide contents in their own area that fades in and out with the transition
    {
        let content = card.shrink2(Vec2::new(100.0, 70.0) * s);
        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(content)
                .layout(Layout::top_down(Align::Min)),
        );
        ui.multiply_opacity(appear);
        ui.style_mut().interaction.selectable_labels = false;
        ui.spacing_mut().item_spacing.y = 10.0 * s;

        // On a full-screen picture the text is light on the darkened picture
        let ink = if full_bleed {
            Color32::WHITE
        } else {
            theme.text
        };
        let title = |ui: &mut egui::Ui, text: &str, size: f32| {
            ui.add_space((1.0 - title_in) * 26.0 * s);
            let rect = ui
                .scope(|ui| {
                    ui.multiply_opacity(title_in);
                    ui.add(egui::Label::new(title_job(text, size, ink, slide.color)).wrap())
                        .rect
                })
                .inner;
            // Shimmer: a bright, slanted streak sweeps across the title once
            let sweep = progress(now, view.slide_time + 0.45, 0.9);
            if fx.shine && sweep > 0.0 && sweep < 1.0 {
                let clip = ui.painter().with_clip_rect(rect.expand(4.0));
                let band = rect.height() * 0.9;
                let x = rect.left() - band + (rect.width() + 2.0 * band) * sweep;
                let tilt = rect.height() * 0.4;
                let glow =
                    Color32::WHITE.gamma_multiply(0.35 * (std::f32::consts::PI * sweep).sin());
                clip.add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(x + tilt, rect.top()),
                        Pos2::new(x + tilt + band * 0.5, rect.top()),
                        Pos2::new(x + band * 0.5, rect.bottom()),
                        Pos2::new(x, rect.bottom()),
                    ],
                    glow,
                    Stroke::NONE,
                ));
            }
        };

        if slide.is_title {
            // Without topic chips the title sits a little lower, closer to the centre
            let top = if fx.title_topics { 0.22 } else { 0.3 };
            ui.add_space(content.height() * top);
            title(&mut ui, &slide.title, 76.0 * s);
            if !slide.note.trim().is_empty() {
                ui.scope(|ui| {
                    ui.multiply_opacity(line_in);
                    ui.add(
                        egui::Label::new(
                            RichText::new(slide.note.trim())
                                .size(28.0 * s)
                                .color(ink.gamma_multiply(0.8)),
                        )
                        .wrap(),
                    );
                });
            }
            ui.add_space(36.0 * s);
            // Chapter chips pop up one after another (can be switched off)
            let topics = if fx.title_topics {
                slide.bullets.as_slice()
            } else {
                &[]
            };
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0) * s;
                for (i, (_, text, color, _, _)) in topics.iter().enumerate() {
                    let pop = progress(now, view.slide_time + 0.35 + i as f64 * 0.09, 0.4);
                    animating |= pop < 1.0;
                    // Fixed size up front so a chip moves to the next line as a whole
                    let galley = ui.painter().layout_no_wrap(
                        text.clone(),
                        FontId::proportional(20.0 * s),
                        theme.on_accent,
                    );
                    let size = galley.size() + Vec2::new(36.0, 16.0) * s;
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    if pop > 0.0 {
                        let mut p = ui.painter().clone();
                        p.multiply_opacity(pop.min(1.0));
                        let r =
                            Rect::from_center_size(rect.center(), rect.size() * ease_out_back(pop));
                        p.rect_filled(r, r.height() / 2.0, *color);
                        if pop > 0.5 {
                            p.galley(rect.center() - galley.size() / 2.0, galley, theme.on_accent);
                        }
                    }
                }
            });
        } else {
            ui.scope(|ui| {
                ui.multiply_opacity(title_in);
                let mut counter = format!("{} / {}", view.index, last);
                if let Some((done, total)) = tasks {
                    counter.push_str(&format!("   ·   {done} of {total} done"));
                }
                ui.label(RichText::new(counter).size(18.0 * s).color(slide.color));
            });
            title(&mut ui, &slide.title, 54.0 * s);
            let (line, _) = ui.allocate_exact_size(Vec2::new(90.0, 6.0) * s, Sense::hover());
            let drawn =
                Rect::from_min_size(line.min, Vec2::new(line.width() * line_in, line.height()));
            ui.painter().rect_filled(drawn, 3.0 * s, slide.color);
            ui.add_space(14.0 * s);
            if !slide.note.trim().is_empty() {
                ui.scope(|ui| {
                    ui.multiply_opacity(line_in);
                    ui.add(
                        egui::Label::new(
                            RichText::new(slide.note.trim())
                                .size(24.0 * s)
                                .color(ink.gamma_multiply(0.85)),
                        )
                        .wrap(),
                    );
                });
                ui.add_space(10.0 * s);
            }

            if let Some(layout) = slide.layout {
                // Diagram instead of a bullet list, builds up step by step in the same way
                let building = build_bullets && slide.groups() > 0;
                let reveal = |g: usize| {
                    let start = if building {
                        if g >= view.build {
                            f64::INFINITY
                        } else if g + 1 == view.build {
                            view.build_time
                        } else {
                            f64::NEG_INFINITY
                        }
                    } else {
                        view.slide_time + 0.4 + g as f64 * 0.18
                    };
                    progress(now, start, 0.55)
                };
                let count_up = progress(now, view.slide_time + 0.3, 1.4);
                animating |= now - view.slide_time.max(view.build_time) < 3.0;
                let rect = ui.available_rect_before_wrap();
                ui.allocate_rect(rect, Sense::hover());
                draw_diagram(
                    ui.painter(),
                    rect.shrink2(Vec2::new(0.0, 6.0 * s)),
                    layout,
                    &slide.items(),
                    &slide.note,
                    theme,
                    slide.color,
                    s,
                    &reveal,
                    count_up,
                );
            } else {
                // Bullets: hidden ones already take up their space so nothing
                // shifts. Every group appears with the chosen effect; when building
                // up step by step, older groups get paler.
                // Pictures next to the bullets, or as a gallery if there are none
                let mut side_ui;
                let ui = if full_bleed || slide.images.is_empty() {
                    &mut ui
                } else {
                    let rect = ui.available_rect_before_wrap();
                    let (text_rect, picture_rect) = if slide.bullets.is_empty() {
                        (None, rect)
                    } else {
                        let split = rect.left() + rect.width() * 0.55;
                        (
                            Some(Rect::from_min_max(
                                rect.min,
                                Pos2::new(split, rect.bottom()),
                            )),
                            Rect::from_min_max(Pos2::new(split + 30.0 * s, rect.top()), rect.max),
                        )
                    };
                    picture_grid(
                        ui.painter(),
                        &ctx,
                        &map.images,
                        &slide.images,
                        picture_rect.shrink2(Vec2::new(0.0, 4.0 * s)),
                        s,
                        pictures_in,
                        theme,
                    );
                    animating |= pictures_in < 1.0;
                    match text_rect {
                        Some(text_rect) => {
                            side_ui = ui.new_child(
                                UiBuilder::new()
                                    .max_rect(text_rect)
                                    .layout(Layout::top_down(Align::Min)),
                            );
                            &mut side_ui
                        }
                        None => &mut ui,
                    }
                };
                let building = build_bullets && slide.groups() > 0;
                let shrink = (8.0 / slide.bullets.len().max(8) as f32).max(0.6);
                let mut group = 0;
                for (i, (depth, text, color, task, node)) in slide.bullets.iter().enumerate() {
                    if *depth == 0 && i > 0 {
                        group += 1;
                    }
                    let start = if building {
                        if group >= view.build {
                            f64::INFINITY
                        } else if group + 1 == view.build {
                            view.build_time
                        } else {
                            f64::NEG_INFINITY
                        }
                    } else {
                        view.slide_time + 0.35 + group as f64 * 0.12
                    };
                    let duration = match fx.bullets {
                        BulletFx::Typewriter => (text.chars().count() as f64 * 0.025).max(0.35),
                        _ => 0.35,
                    };
                    let raw = progress(now, start, duration);
                    animating |= raw < 1.0 && start.is_finite();
                    let e = ease_out_cubic(raw);
                    let dim = if building && group + 1 < view.build {
                        0.4
                    } else {
                        1.0
                    };
                    let base_size = (30.0 - *depth as f32 * 4.0) * s * shrink;
                    let (alpha, shift, size, shown) = match fx.bullets {
                        BulletFx::SlideIn => (e, (1.0 - e) * 36.0 * s, base_size, text.clone()),
                        BulletFx::Fade => (e, 0.0, base_size, text.clone()),
                        BulletFx::Pop => (
                            e,
                            0.0,
                            base_size * (0.55 + 0.45 * ease_out_back(raw)),
                            text.clone(),
                        ),
                        BulletFx::Typewriter => {
                            let n = (text.chars().count() as f32 * raw).ceil() as usize;
                            let alpha = if raw > 0.0 { 1.0 } else { 0.0 };
                            (alpha, 0.0, base_size, text.chars().take(n).collect())
                        }
                    };
                    ui.scope(|ui| {
                        ui.multiply_opacity(alpha * dim);
                        ui.horizontal_top(|ui| {
                            ui.add_space(*depth as f32 * 40.0 * s + shift);
                            let (dot, _) = ui.allocate_exact_size(
                                Vec2::new(base_size * 0.8, base_size * 1.2),
                                Sense::hover(),
                            );
                            let mut label = RichText::new(shown).size(size).color(theme.text);
                            if let Some(done) = *task {
                                // Checkbox instead of a bullet point; done items struck through.
                                // Visible checkboxes can be clicked.
                                let r =
                                    Rect::from_center_size(dot.center(), Vec2::splat(size * 0.62));
                                paint_checkbox(
                                    ui.painter(),
                                    r,
                                    done,
                                    *color,
                                    theme.text.gamma_multiply(0.7),
                                );
                                if alpha > 0.0 {
                                    let click = ui
                                        .interact(
                                            r.expand(6.0 * s),
                                            ui.id().with(("slide_task", node)),
                                            Sense::click(),
                                        )
                                        .on_hover_cursor(CursorIcon::PointingHand);
                                    if click.clicked() {
                                        ticked = Some((*node, r.center()));
                                    }
                                }
                                if done {
                                    label =
                                        label.strikethrough().color(theme.text.gamma_multiply(0.5));
                                }
                            } else {
                                let radius = if *depth == 0 { size * 0.2 } else { size * 0.14 };
                                ui.painter().circle_filled(dot.center(), radius, *color);
                            }
                            ui.add(egui::Label::new(label).wrap());
                        });
                    });
                }
            }
        }
    }
    animating |= full_bleed && (pictures_in < 1.0 || fx.ken_burns);

    // Progress at the bottom of the slide
    let track = Rect::from_min_size(
        card.left_bottom() + Vec2::new(44.0, -34.0) * s,
        Vec2::new(card.width() - 88.0 * s, 4.0 * s),
    );
    painter.rect_filled(track, 2.0 * s, theme.grid);
    painter.rect_filled(
        Rect::from_min_size(
            track.min,
            Vec2::new(
                track.width() * view.progress.clamp(0.0, 1.0),
                track.height(),
            ),
        ),
        2.0 * s,
        slide.color,
    );
    SlideOutput {
        ticked,
        new_layout,
        animating,
    }
}

/// Slide title; an icon at the start (Phosphor glyph) in the slide's colour
fn title_job(text: &str, size: f32, ink: Color32, accent: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat::simple(FontId::proportional(size), color);
    let mut chars = text.chars();
    match chars.next() {
        // Phosphor lives in the private use area
        Some(c) if ('\u{E000}'..='\u{F8FF}').contains(&c) => {
            job.append(&c.to_string(), 0.0, format(accent));
            job.append(chars.as_str(), 0.0, format(ink));
        }
        _ => job.append(text, 0.0, format(ink)),
    }
    job
}

/// Notes for the slide: those of the branch and of its bullets
fn speaker_notes(map: &MindMap, slide: &Slide) -> Vec<(String, String)> {
    let mut notes = Vec::new();
    let node = slide.branch.or_else(|| map.root());
    let Some(node) = node.and_then(|id| map.get(id)) else {
        return notes;
    };
    if !node.note.trim().is_empty() {
        notes.push((String::new(), node.note.trim().to_string()));
    }
    if slide.branch.is_some() {
        for id in map.subtree(node.id).into_iter().skip(1) {
            if let Some(n) = map.get(id).filter(|n| !n.note.trim().is_empty()) {
                notes.push((n.text.clone(), n.note.trim().to_string()));
            }
        }
    }
    notes
}

/// Contents of the presenter view; returns the keys pressed
#[allow(clippy::too_many_arguments)]
fn presenter_body(
    ui: &mut egui::Ui,
    doc: &mut Document,
    slides: &[Slide],
    theme: &Theme,
    fx: &Effects,
    build_bullets: bool,
    now: f64,
    start: f64,
) -> Nav {
    let weak = theme.text.gamma_multiply(0.55);
    let nav = read_nav(ui);
    let last = slides.len() - 1;
    let slide = &slides[doc.slide.min(last)];

    // Header: stopwatch, clock, position
    ui.horizontal(|ui| {
        let secs = (now - start).max(0.0) as u64;
        ui.label(
            RichText::new(format!("{:02}:{:02}", secs / 60, secs % 60))
                .size(34.0)
                .strong()
                .color(theme.text),
        );
        if ui
            .button(format!("{}  Reset", icon::ARROW_COUNTER_CLOCKWISE))
            .on_hover_text("Start the timer again")
            .clicked()
        {
            doc.presenter_start = Some(now);
        }
        ui.separator();
        ui.label(
            RichText::new(format!(
                "{}  {}",
                icon::CLOCK,
                chrono::Local::now().format("%H:%M")
            ))
            .size(20.0)
            .color(weak),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut position = if slide.is_title {
                format!("Title slide · {} slides", last)
            } else {
                format!("Slide {} of {}", doc.slide, last)
            };
            let groups = slide.groups();
            if build_bullets && groups > 0 {
                position.push_str(&format!("  ·  point {} of {groups}", doc.build));
            }
            ui.label(RichText::new(position).size(18.0).color(weak));
        });
    });
    ui.add_space(12.0);

    // Leave room at the bottom for the key hints
    let full = ui.available_rect_before_wrap();
    let rest = Rect::from_min_max(full.min, full.max - Vec2::new(0.0, 28.0));
    let gap = 20.0;
    let left_w = (rest.width() - gap) * 0.62;
    let right_w = rest.width() - gap - left_w;
    let label = |ui: &egui::Ui, pos: Pos2, text: &str| {
        ui.painter().text(
            pos,
            Align2::LEFT_BOTTOM,
            text,
            FontId::proportional(13.0),
            weak,
        );
    };

    // Current slide, as the audience sees it
    let current = Rect::from_min_size(
        rest.min + Vec2::new(0.0, 20.0),
        Vec2::new(left_w, (left_w * 9.0 / 16.0).min(rest.height() - 20.0)),
    );
    label(ui, current.left_top() - Vec2::new(0.0, 4.0), "NOW");
    let view = SlideView {
        slides,
        index: doc.slide.min(last),
        build: doc.build,
        build_time: doc.build_time,
        slide_time: doc.slide_time,
        now,
        appear: 1.0,
        progress: (doc.slide + 1) as f32 / slides.len() as f32,
        interactive: false,
        page: false,
    };
    let mut child = ui.new_child(UiBuilder::new().max_rect(current).id_salt("presenter_now"));
    let out = render_slide(
        &mut child,
        current,
        &view,
        &doc.map,
        theme,
        fx,
        build_bullets,
    );
    if out.animating {
        ui.ctx().request_repaint();
    }

    // Next slide, fully built up
    let right = Rect::from_min_max(
        Pos2::new(current.right() + gap, rest.top() + 20.0),
        rest.max,
    );
    let next = Rect::from_min_size(right.min, Vec2::new(right_w, right_w * 9.0 / 16.0));
    label(ui, next.left_top() - Vec2::new(0.0, 4.0), "NEXT");
    if doc.slide < last {
        let index = doc.slide + 1;
        let view = SlideView {
            slides,
            index,
            build: slides[index].groups(),
            build_time: f64::NEG_INFINITY,
            slide_time: now - 1000.0,
            now,
            appear: 1.0,
            progress: (index + 1) as f32 / slides.len() as f32,
            interactive: false,
            page: false,
        };
        let mut child = ui.new_child(UiBuilder::new().max_rect(next).id_salt("presenter_next"));
        render_slide(&mut child, next, &view, &doc.map, theme, fx, build_bullets);
    } else {
        ui.painter().rect_filled(next, 14.0, theme.card);
        ui.painter().text(
            next.center(),
            Align2::CENTER_CENTER,
            "End of the presentation",
            FontId::proportional(18.0),
            weak,
        );
    }

    // Notes below the next slide
    let notes_rect = Rect::from_min_max(Pos2::new(right.left(), next.bottom() + 28.0), right.max);
    label(ui, notes_rect.left_top() - Vec2::new(0.0, 6.0), "NOTES");
    let mut notes_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(notes_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    egui::Frame::default()
                .fill(theme.card)
                .corner_radius(12.0)
                .inner_margin(egui::Margin::same(14))
                .show(&mut notes_ui, |ui| {
                    let inner = (notes_rect.size() - Vec2::splat(28.0)).max(Vec2::ZERO);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    egui::ScrollArea::vertical()
                        .id_salt("presenter_notes")
                        .max_height(inner.y)
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            let notes = speaker_notes(&doc.map, slide);
                            if notes.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "No notes on this slide. Add them in the mind map: right-click a node → Note …",
                                    )
                                    .size(15.0)
                                    .color(weak),
                                );
                            }
                            for (title, note) in notes {
                                if !title.is_empty() {
                                    ui.label(
                                        RichText::new(title)
                                            .size(15.0)
                                            .strong()
                                            .color(slide.color),
                                    );
                                }
                                ui.label(RichText::new(note).size(17.0).color(theme.text));
                                ui.add_space(8.0);
                            }
                        });
                });
    // Hint at the bottom left
    ui.painter().text(
        Pos2::new(full.left(), full.bottom()),
        Align2::LEFT_BOTTOM,
        "Space · → next   ← back   G quick jump   P close this window   Esc end",
        FontId::proportional(13.0),
        weak,
    );
    nav
}

/// For tests: draw the presenter view straight into `ui`
#[cfg(test)]
pub fn presenter_snapshot(ui: &mut egui::Ui, doc: &mut Document, theme: &Theme, fx: &Effects) {
    let slides = slides(&doc.map, theme);
    let now = ui.input(|i| i.time);
    let start = doc.presenter_start.unwrap_or(now);
    presenter_body(ui, doc, &slides, theme, fx, true, now, start);
}

/// Presenter view in a window of its own: current and next slide,
/// notes, stopwatch and clock. Returns the keys pressed there.
fn presenter_window(
    ctx: &egui::Context,
    doc: &mut Document,
    slides: &[Slide],
    theme: &Theme,
    fx: &Effects,
    build_bullets: bool,
    now: f64,
) -> Nav {
    let start = *doc.presenter_start.get_or_insert(now);
    let builder = egui::ViewportBuilder::default()
        .with_title("Mindful – Presenter view")
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([640.0, 420.0]);
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("mindful_presenter"),
        builder,
        |ctx, class| {
            if ctx.input(|i| i.viewport().close_requested()) {
                doc.presenter = false;
            }
            // Update the stopwatch and clock every second
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            let mut nav = Nav::default();
            let mut body = |ui: &mut egui::Ui| {
                nav = presenter_body(ui, doc, slides, theme, fx, build_bullets, now, start);
            };
            if class == egui::ViewportClass::Embedded {
                // No second window possible: as a window inside the app
                let mut open = true;
                egui::Window::new("Presenter view")
                    .open(&mut open)
                    .resizable(false)
                    .fixed_size([960.0, 600.0])
                    .frame(egui::Frame::window(&ctx.style()).fill(theme.background))
                    .show(ctx, |ui| {
                        body(ui);
                    });
                if !open {
                    doc.presenter = false;
                }
            } else {
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::default()
                            .fill(theme.background)
                            .inner_margin(egui::Margin::same(18)),
                    )
                    .show(ctx, |ui| body(ui));
            }
            nav
        },
    )
}

/// For tests: draw slide `index` fully built up as a card (with shadow) in `card`
#[cfg(test)]
pub fn draw_card(
    ui: &mut egui::Ui,
    card: Rect,
    map: &MindMap,
    theme: &Theme,
    fx: &Effects,
    index: usize,
) {
    let slides = slides(map, theme);
    let now = ui.input(|i| i.time);
    let view = SlideView {
        slides: &slides,
        index,
        build: slides[index].groups(),
        build_time: f64::NEG_INFINITY,
        slide_time: now - 1000.0,
        now,
        appear: 1.0,
        progress: (index + 1) as f32 / slides.len() as f32,
        interactive: false,
        page: false,
    };
    render_slide(ui, card, &view, map, theme, fx, true);
}

/// Number of slides (title slide plus one per main branch)
pub fn slide_count(map: &MindMap, theme: &Theme) -> usize {
    slides(map, theme).len()
}

/// Draw slide `index` fully built up and without animation into the whole area
/// (PDF export, preview of the next slide)
pub fn draw_static(ui: &mut egui::Ui, map: &MindMap, theme: &Theme, fx: &Effects, index: usize) {
    let slides = slides(map, theme);
    let Some(slide) = slides.get(index) else {
        return;
    };
    let now = ui.input(|i| i.time);
    let view = SlideView {
        slides: &slides,
        index,
        build: slide.groups(),
        build_time: f64::NEG_INFINITY,
        slide_time: now - 1000.0,
        now,
        appear: 1.0,
        progress: (index + 1) as f32 / slides.len() as f32,
        interactive: false,
        page: true,
    };
    render_slide(ui, ui.max_rect(), &view, map, theme, fx, true);
}

/// `build_bullets`: reveal bullets one after another
pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    theme: &Theme,
    build_bullets: bool,
    fx: &mut Effects,
) -> Option<Event> {
    let slides = slides(&doc.map, theme);
    if slides.is_empty() {
        return Some(Event::Exit);
    }
    let last = slides.len() - 1;
    let now = ui.input(|i| i.time);
    if doc.slide_time.is_nan() {
        doc.slide_time = now;
    }
    doc.slide = doc.slide.min(last);
    let groups_of = |i: usize| {
        if build_bullets {
            slides[i].groups()
        } else {
            0
        }
    };
    let go_to = |doc: &mut Document, slide: usize, build: usize| {
        if slide != doc.slide {
            doc.slide_time = now;
        }
        doc.slide = slide;
        doc.build = build;
    };
    let mut event = None;
    let mut nav = read_nav(ui);
    // Presenter view in the second window; keys pressed there count too
    if doc.presenter {
        let keys = presenter_window(ui.ctx(), doc, &slides, theme, fx, build_bullets, now);
        nav = nav.merge(keys);
    }
    if nav.presenter {
        doc.presenter = !doc.presenter;
    }
    // Phone remote: treat buttons from the phone like arrow keys
    if let Some(remote) = crate::remote::running() {
        for command in remote.take_commands() {
            match command {
                crate::remote::Command::Next => nav.right = true,
                crate::remote::Command::Prev => nav.left = true,
            }
        }
    }
    if nav.remote {
        doc.remote_open = !doc.remote_open;
    }
    // Esc first closes the QR window, only then the talk
    if doc.remote_open && nav.escape {
        doc.remote_open = false;
        nav.escape = false;
    }
    let area = ui.max_rect();
    let ctx = ui.ctx().clone();

    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);
    // When paging forward, the stars move along a little
    let drift = ctx.animate_value_with_time(ui.id().with("star_drift"), doc.slide as f32, 0.9);
    let parallax = Vec2::new(-drift * 120.0, 0.0);
    if backdrop(
        &painter,
        area,
        theme,
        fx.background,
        now,
        parallax,
        Vec2::ZERO,
    ) {
        ctx.request_repaint();
    }

    // Paging is keyboard only – clicks are for ticking off
    if doc.strip {
        match strip_keys(&nav, &mut doc.strip_cursor, last) {
            StripKey::Jump(i) => {
                go_to(doc, i, 0);
                doc.strip = false;
            }
            StripKey::Close => doc.strip = false,
            StripKey::Stay => {}
        }
    } else {
        if nav.next() {
            if doc.build < groups_of(doc.slide) {
                doc.build += 1;
                doc.build_time = now;
            } else if doc.slide < last {
                go_to(doc, doc.slide + 1, 0);
            }
        }
        if nav.left {
            if doc.build > 0 {
                doc.build -= 1;
            } else if doc.slide > 0 {
                let prev = doc.slide - 1;
                go_to(doc, prev, groups_of(prev));
            }
        }
        if nav.home {
            go_to(doc, 0, 0);
        }
        if nav.end {
            go_to(doc, last, groups_of(last));
        }
        if nav.escape {
            event = Some(Event::Exit);
        }
        if nav.strip {
            doc.strip = true;
            doc.strip_cursor = doc.slide;
        }
    }
    if nav.fullscreen {
        event = Some(Event::ToggleFullscreen);
    }
    let slide = &slides[doc.slide];

    // Transition: the new slide comes in differently depending on the effect
    let anim = ctx.animate_value_with_time(ui.id().with("slide_anim"), doc.slide as f32, 0.4);
    let offset = doc.slide as f32 - anim;
    let appear = (1.0 - offset.abs()).clamp(0.0, 1.0);
    let avail = area.shrink2(Vec2::new(40.0, 50.0));
    let w = avail.width().min(avail.height() * 16.0 / 9.0);
    let base = Rect::from_center_size(avail.center(), Vec2::new(w, w * 9.0 / 16.0));
    let (shift, scale) = match fx.transition {
        Transition::Slide => (Vec2::new(offset * 90.0, 0.0), 1.0),
        Transition::Fade => (Vec2::ZERO, 1.0),
        Transition::Zoom => (Vec2::ZERO, 0.88 + 0.12 * ease_out_cubic(appear)),
        Transition::Rise => (Vec2::new(0.0, offset.abs() * 70.0), 1.0),
    };
    // Slow zoom while the slide is visible
    let slow_zoom = if fx.ken_burns {
        1.0 + 0.03 * ease_in_out_cubic(progress(now, doc.slide_time, 14.0))
    } else {
        1.0
    };
    let card = Rect::from_center_size(
        base.center() + shift * (w / 1280.0),
        base.size() * scale * slow_zoom,
    );
    let view = SlideView {
        slides: &slides,
        index: doc.slide,
        build: doc.build,
        build_time: doc.build_time,
        slide_time: doc.slide_time,
        now,
        appear,
        progress: (anim + 1.0) / slides.len() as f32,
        interactive: true,
        page: false,
    };
    let SlideOutput {
        ticked,
        new_layout,
        animating,
    } = render_slide(ui, card, &view, &doc.map, theme, fx, build_bullets);

    let Corner {
        event: corner_event,
        strip: strip_clicked,
        presenter: presenter_clicked,
        remote: remote_clicked,
    } = corner_buttons(ui, area, "End presentation (Esc)", fx, false);
    if presenter_clicked {
        doc.presenter = !doc.presenter;
    }
    if remote_clicked {
        doc.remote_open = !doc.remote_open;
    }
    if corner_event.is_some() {
        event = corner_event;
    }
    if strip_clicked {
        doc.strip = !doc.strip;
        doc.strip_cursor = doc.slide;
    }

    if animating {
        ctx.request_repaint();
    }
    if let (Some(layout), Some(branch)) = (new_layout, slide.branch) {
        doc.checkpoint();
        if let Some(n) = doc.map.get_mut(branch) {
            n.layout = layout;
        }
        // Show a new layout animated right away
        doc.build = 0;
        doc.slide_time = now;
    }
    if let Some((id, pos)) = ticked {
        doc.checkpoint();
        if let Some(n) = doc.map.get_mut(id) {
            n.task = n.task.map(|done| !done);
            if n.task == Some(true) && fx.confetti {
                confetti_burst(&ctx, pos, &theme.branches, now);
            }
        }
    }

    if fx.vignette {
        vignette(&painter, area);
    }

    // Quick jump and laser pointer are on top of everything
    let items: Vec<StripItem> = slides
        .iter()
        .enumerate()
        .map(|(i, s)| StripItem {
            label: if s.is_title {
                "Title".into()
            } else {
                format!("{i} / {last}")
            },
            title: s.title.clone(),
            color: s.color,
        })
        .collect();
    if let Some(i) = filmstrip(
        ui,
        area,
        theme,
        &items,
        doc.slide,
        doc.strip_cursor,
        doc.strip,
    ) {
        go_to(doc, i, 0);
        doc.strip = false;
    }
    confetti_draw(&ctx, now);
    laser(ui, nav.laser, now);

    // State for the phone, and the QR window
    if let Some(remote) = crate::remote::running() {
        let slide = &slides[doc.slide];
        let start = *doc.presenter_start.get_or_insert(now);
        // The phone doesn't know the glyphs of the Phosphor font
        let plain = |text: &str| {
            text.chars()
                .filter(|c| !('\u{E000}'..='\u{F8FF}').contains(c))
                .collect::<String>()
                .trim()
                .to_string()
        };
        remote.publish(crate::remote::RemoteState {
            title: plain(&slide.title),
            next: slides
                .get(doc.slide + 1)
                .map(|s| plain(&s.title))
                .unwrap_or_default(),
            slide: doc.slide,
            total: slides.len(),
            point: doc.build,
            points: if build_bullets { slide.groups() } else { 0 },
            notes: speaker_notes(&doc.map, slide),
            elapsed: (now - start).max(0.0) as u64,
        });
    }
    if doc.remote_open && !remote_overlay(ui, area, theme) {
        doc.remote_open = false;
    }

    event
}

/// Window with the QR code for the phone remote. Returns `false`
/// when it was closed.
fn remote_overlay(ui: &mut egui::Ui, area: Rect, theme: &Theme) -> bool {
    let ctx = ui.ctx().clone();
    // While it is open, check regularly whether a phone checks in
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
    // Darken above the slide, but below the window
    let painter = ctx.layer_painter(LayerId::new(Order::Middle, Id::new("remote_dim")));
    painter.rect_filled(area, 0.0, Color32::from_black_alpha(150));
    let card = Rect::from_center_size(area.center(), Vec2::new(420.0, 540.0));
    let mut open = true;
    egui::Area::new(Id::new("remote_overlay"))
        .order(Order::Foreground)
        .fixed_pos(card.min)
        .show(&ctx, |ui| {
            egui::Frame::default()
                .fill(theme.card)
                .corner_radius(18.0)
                .inner_margin(egui::Margin::same(24))
                .show(ui, |ui| {
                    ui.set_width(card.width() - 48.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}  Phone remote", icon::DEVICE_MOBILE))
                                .size(20.0)
                                .strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(RichText::new(icon::X).size(16.0)).clicked() {
                                open = false;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    match crate::remote::start(&ctx) {
                        Ok(remote) => {
                            ui.label(
                                RichText::new(
                                    "Scan the code with your phone's camera. Your phone must be in the same Wi-Fi.",
                                )
                                .color(theme.text.gamma_multiply(0.7)),
                            );
                            ui.add_space(12.0);
                            if let Some((width, dark)) = remote.qr() {
                                let side = 300.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), side),
                                    Sense::hover(),
                                );
                                let qr = Rect::from_center_size(rect.center(), Vec2::splat(side));
                                let p = ui.painter();
                                p.rect_filled(qr, 14.0, Color32::WHITE);
                                // Quiet zone of 4 modules all around
                                let cell = side / (width + 8) as f32;
                                for (i, is_dark) in dark.iter().enumerate() {
                                    if *is_dark {
                                        let (x, y) = ((i % width + 4) as f32, (i / width + 4) as f32);
                                        p.rect_filled(
                                            Rect::from_min_size(
                                                qr.min + Vec2::new(x, y) * cell,
                                                Vec2::splat(cell + 0.3),
                                            ),
                                            0.0,
                                            Color32::BLACK,
                                        );
                                    }
                                }
                            }
                            ui.add_space(12.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new(&remote.url).monospace().size(12.0).weak());
                                ui.add_space(8.0);
                                if remote.connected() {
                                    ui.label(
                                        RichText::new(format!("{}  Phone connected", icon::CHECK_CIRCLE))
                                            .color(theme.branches[2 % theme.branches.len()])
                                            .size(16.0),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("Waiting for your phone …")
                                            .color(theme.text.gamma_multiply(0.6)),
                                    );
                                }
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("R or Esc closes this window – the remote keeps working.")
                                        .size(12.0)
                                        .color(theme.text.gamma_multiply(0.5)),
                                );
                            });
                        }
                        Err(e) => {
                            ui.label(format!("The remote could not start: {e}."));
                            ui.label(
                                RichText::new("Connect to a Wi-Fi network and try again.")
                                    .color(theme.text.gamma_multiply(0.6)),
                            );
                        }
                    }
                });
        });
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_switch_to_fade_once() {
        // Saved before version 1: field `version` is missing → 0
        let mut old: Effects = serde_json::from_str(r#"{"transition":"Slide"}"#).unwrap();
        assert_eq!(old.version, 0);
        old.migrate();
        assert!(old.transition == Transition::Fade);
        // After that a deliberate choice is kept
        old.transition = Transition::Zoom;
        old.migrate();
        assert!(old.transition == Transition::Zoom);
        assert!(Effects::default().transition == Transition::Fade);
        // Versions 0 and 1 get the starfield once
        assert!(old.background == Backdrop::Stars);
        let mut v1: Effects =
            serde_json::from_str(r#"{"background":"Aurora","version":1}"#).unwrap();
        v1.migrate();
        assert!(v1.background == Backdrop::Stars);
        v1.background = Backdrop::Plain;
        v1.migrate();
        assert!(v1.background == Backdrop::Plain);
        // Version 3: glow and light pulses only when asked for
        let mut v2: Effects =
            serde_json::from_str(r#"{"glow":true,"pulses":true,"version":2}"#).unwrap();
        v2.migrate();
        assert!(!v2.glow && !v2.pulses);
        v2.glow = true;
        v2.migrate();
        assert!(v2.glow);
        let fresh = Effects::default();
        assert!(!fresh.glow && !fresh.pulses && !fresh.warp && !fresh.vignette);
    }

    #[test]
    fn quick_jump_keys() {
        let mut cursor = 2;
        let right = Nav {
            right: true,
            ..Nav::default()
        };
        assert!(matches!(strip_keys(&right, &mut cursor, 3), StripKey::Stay));
        assert_eq!(cursor, 3);
        // Not past the end
        strip_keys(&right, &mut cursor, 3);
        assert_eq!(cursor, 3);
        let enter = Nav {
            enter: true,
            ..Nav::default()
        };
        assert!(matches!(
            strip_keys(&enter, &mut cursor, 3),
            StripKey::Jump(3)
        ));
        let escape = Nav {
            escape: true,
            ..Nav::default()
        };
        assert!(matches!(
            strip_keys(&escape, &mut cursor, 3),
            StripKey::Close
        ));
    }
}
