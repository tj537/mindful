//! Präsentationsmodus: eine Titelfolie und pro Hauptzweig eine Folie.
//!
//! Hier liegen auch die Teile, die Folien und Kamerafahrt gemeinsam nutzen:
//! Effekt-Einstellungen, Tastatur, Buttons, Folienstreifen, animierte
//! Hintergründe, Laserpointer und Animationskurven.

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
// Effekt-Einstellungen
// ---------------------------------------------------------------------------

/// Übergang zwischen zwei Folien
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Transition {
    Slide,
    #[default]
    Fade,
    Zoom,
    Rise,
}

/// Wie ein Stichpunkt erscheint
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum BulletFx {
    #[default]
    SlideIn,
    Fade,
    Pop,
    Typewriter,
}

/// Animierter Hintergrund
#[derive(Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Backdrop {
    Plain,
    Aurora,
    #[default]
    Stars,
    /// Weiche, schwebende Lichtkreise
    Bokeh,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    pub transition: Transition,
    pub bullets: BulletFx,
    pub background: Backdrop,
    /// Langsames Schweben der Kamera in der Tour
    pub drift: bool,
    /// Tour: Kamera federt beim Ankommen leicht nach
    pub bounce: bool,
    /// Tour: Glühen um den aktuellen Zweig
    pub glow: bool,
    /// Tour: Welle, die sich beim Ankommen ausbreitet
    pub ripple: bool,
    /// Tour: Lichtpulse, die an den Verbindungen entlanglaufen
    pub pulses: bool,
    /// Tour: Unterpunkte Ebene für Ebene per Tastendruck aufdecken
    pub step_levels: bool,
    /// Konfetti beim Abhaken einer Checkbox
    pub confetti: bool,
    /// Dunklere Bildränder
    pub vignette: bool,
    /// Folien: farbiger Balken am linken Rand der Folie
    pub accent_bar: bool,
    /// Titelfolie: die Hauptzweige als farbige Themen-Marken zeigen
    pub title_topics: bool,
    /// Folien: Lichtschimmer, der über den Titel zieht
    pub shine: bool,
    /// Folien: langsamer Zoom, solange eine Folie zu sehen ist
    pub ken_burns: bool,
    /// Tour: Sterne ziehen beim Flug zu Streifen
    pub warp: bool,
    /// Tour: Knoten außerhalb des Scheinwerfers rücken zurück
    pub depth_of_field: bool,
    /// Tour: Funken kreisen um den aktuellen Zweig
    pub orbit: bool,
    /// Stand der Einstellungen; fehlt bei älteren gespeicherten Daten (= 0)
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
    /// Ältere gespeicherte Einstellungen auf neue Standards bringen
    pub fn migrate(&mut self) {
        if self.version < 1 {
            // Seit Version 1 ist Überblenden der Standard-Übergang
            self.transition = Transition::Fade;
        }
        if self.version < 2 {
            // Seit Version 2 ist der Sternenhimmel der Standard-Hintergrund
            self.background = Backdrop::Stars;
        }
        if self.version < 3 {
            // Seit Version 3 sind Glühen und Lichtpulse nur noch auf Wunsch an
            self.glow = false;
            self.pulses = false;
        }
        self.version = EFFECTS_VERSION;
    }
}

/// Auswahl der Effekte – in der Präsentation (Button oben rechts) und im Ansicht-Menü
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

/// Tastenkürzel beim Präsentieren (im Einstellungsmenü)
fn shortcuts_list(ui: &mut egui::Ui, tour: bool) {
    ui.strong("Keyboard");
    let next = if tour {
        "Next stop"
    } else {
        "Next bullet point / slide"
    };
    egui::Grid::new("present_keys")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .show(ui, |ui| {
            for (keys, what) in [
                ("Space  →", next),
                ("←", "Back"),
                ("Home  End", "First / last"),
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
// Gemeinsame Bausteine
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

/// Schießt leicht über das Ziel hinaus und federt zurück („Aufploppen“)
pub fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Fortschritt 0…1 einer Animation, die zu `start` beginnt und `duration` dauert
fn progress(now: f64, start: f64, duration: f64) -> f32 {
    ((now - start) / duration).clamp(0.0, 1.0) as f32
}

pub fn color_of(map: &MindMap, theme: &Theme, id: u64) -> Color32 {
    map.color_index(id)
        .map(|c| theme.branches[c % theme.branches.len()])
        .unwrap_or(theme.root)
}

/// Tasten beim Präsentieren
#[derive(Default)]
pub struct Nav {
    /// Pfeil rechts/unten, Bild ab
    pub right: bool,
    /// Pfeil links/oben, Bild auf, Rücktaste
    pub left: bool,
    pub space: bool,
    pub enter: bool,
    pub home: bool,
    pub end: bool,
    pub escape: bool,
    pub fullscreen: bool,
    pub strip: bool,
    pub laser: bool,
    /// P: Referentenansicht ein/aus
    pub presenter: bool,
    /// R: QR-Code für die Handy-Fernbedienung
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

    /// Tasten aus zwei Fenstern zusammenführen
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

/// Was im geöffneten Schnellwahl-Streifen passieren soll
pub enum StripKey {
    Stay,
    Jump(usize),
    Close,
}

/// Tasten bei geöffneter Schnellwahl: ← → wählen, Enter/Leertaste springt, Esc/G schließt
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

/// Buttons oben rechts: Beenden, Vollbild, Folienstreifen, Effekte.
/// Gibt das Ereignis und „Folienstreifen umschalten“ zurück.
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

/// Was in der Knopfleiste oben rechts angeklickt wurde
pub struct Corner {
    pub event: Option<Event>,
    pub strip: bool,
    pub presenter: bool,
    pub remote: bool,
}

/// Animierter Hintergrund hinter Folie bzw. Mindmap.
/// Gibt `true` zurück, wenn weiter neu gezeichnet werden muss.
/// Pseudo-Zufallszahl 0…1, immer gleich für dieselben Eingaben
fn hash(i: f32, salt: f32) -> f32 {
    ((i * 12.9898 + salt * 78.233).sin() * 43758.547)
        .fract()
        .abs()
}

/// `parallax`: Verschiebung (z. B. Kamera), nahe Sterne bewegen sich stärker als ferne.
/// `warp`: Bewegung der Kamera; ist sie groß genug, werden Sterne zu Streifen.
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
            // Weiche Lichtkreise in den Zweigfarben, die langsam nach oben treiben
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
            // Weiche Farbwolken in den Zweigfarben, die langsam wandern
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

            // Zarter Nebel in zwei Zweigfarben, kaum sichtbar
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

            // Drei Ebenen: ferne (viele, klein), mittlere, nahe (wenige, hell, mit Schein)
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
                    // Einzelne Sterne leicht in den Zweigfarben
                    let color = if hash(n, 6.0) < 0.15 {
                        star.lerp_to_gamma(theme.branches[(hash(n, 7.0) * 6.0) as usize % 6], 0.55)
                    } else {
                        star
                    };
                    let r = small + (big - small) * hash(n, 3.0);
                    let a = bright * twinkle;
                    // Beim schnellen Flug: Lichtstreifen, nahe Sterne länger als ferne
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
                        // Die hellsten funkeln mit einem kleinen Kreuz
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

            // Ab und zu eine Sternschnuppe
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

/// Laserpointer (Taste L): leuchtender Punkt mit kurzem Schweif statt Mauszeiger
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

/// Dunklere Bildränder („Vignette“)
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

/// Konfetti-Explosion an `at` (z. B. beim Abhaken einer Checkbox)
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

/// Fliegendes Konfetti zeichnen (jeden Frame aufrufen)
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

/// Ein Eintrag im Folienstreifen
pub struct StripItem {
    pub label: String,
    pub title: String,
    pub color: Color32,
}

/// Schnellwahl-Streifen am unteren Rand (Taste G). `cursor` ist die per
/// Tastatur markierte Folie, `current` die gezeigte. Gibt die angeklickte Folie zurück.
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
    // Klicks auf den Streifen sollen nicht auf darunterliegende Checkboxen gehen
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
        // Markierte Folie in die Mitte, ohne über die Ränder hinauszuschieben
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
            // Kleiner Punkt: diese Folie wird gerade gezeigt
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
// Folien
// ---------------------------------------------------------------------------

/// Ein Stichpunkt: Ebene, Text, Farbe, Checkbox, Knoten-ID
type Bullet = (usize, String, Color32, Option<bool>, u64);

struct Slide {
    title: String,
    note: String,
    bullets: Vec<Bullet>,
    color: Color32,
    is_title: bool,
    /// Hauptzweig der Folie (für Checkbox-Fortschritt und Layout-Wahl)
    branch: Option<u64>,
    /// Diagramm statt Aufzählung
    layout: Option<SlideLayout>,
    /// Bilder: das des Zweigs selbst (ohne Beschriftung) und die von Stichpunkten
    images: Vec<SlideImage>,
}

struct SlideImage {
    key: String,
    caption: Option<String>,
}

impl Slide {
    /// Ein einzelnes Bild ohne Stichpunkte (oder auf der Titelfolie) füllt die ganze Folie
    fn full_bleed(&self) -> bool {
        !self.images.is_empty()
            && (self.is_title
                || (self.layout.is_none() && self.bullets.is_empty() && self.images.len() == 1))
    }
}

impl Slide {
    /// Stichpunkt-Gruppen: ein Punkt der obersten Ebene mit seinen Unterpunkten.
    /// Die Titelfolie wird nicht schrittweise aufgebaut.
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

    /// Punkte der obersten Ebene mit ihren direkten Unterpunkten (für Diagramme)
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

/// Weichgezeichneter Schatten unter `rect`: um `drop` nach unten versetzt,
/// mit einem weichen Rand der Breite `blur`
fn soft_shadow(rect: Rect, radius: f32, drop: f32, blur: f32, color: Color32) -> egui::Shape {
    let shadow = egui::epaint::Shadow {
        offset: [0, drop.round().clamp(0.0, 127.0) as i8],
        blur: blur.round().clamp(0.0, 255.0) as u8,
        spread: 0,
        color,
    };
    egui::Shape::Rect(shadow.as_shape(rect, radius.round().clamp(0.0, 255.0) as u8))
}

/// Bild oben (und für die Fortschrittsleiste unten) abdunkeln, damit heller Text lesbar bleibt.
/// Ein Netz mit gemeinsamen Kanten, das an den Ecken der Rundung der Folie folgt –
/// so gibt es weder Streifen noch Nähte.
fn darken_for_text(p: &egui::Painter, card: Rect, s: f32, title_slide: bool, alpha: f32) {
    let radius = 24.0 * s;
    if title_slide {
        // Titelfolie: Text in der Mitte, also gleichmäßig abdunkeln
        p.rect_filled(
            card,
            radius.round() as u8,
            Color32::from_black_alpha((120.0 * alpha) as u8),
        );
    }
    // Einrückung an den runden Ecken, `d` = Abstand zur Ober- bzw. Unterkante
    let inset = |d: f32| {
        if d >= radius {
            0.0
        } else {
            radius - (radius * radius - (radius - d).powi(2)).max(0.0).sqrt()
        }
    };
    // Verlauf von der Kante (`edge`) über `reach` Pixel nach innen; `dir` = +1 oben, -1 unten
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
    // Unten ein schmaler Schatten für die Fortschrittsleiste
    gradient(card.bottom(), -1.0, 100.0 * s, 120.0, 1.0);
}

/// Bilder im Raster: ein einzelnes ganz sichtbar, mehrere bildfüllend mit Beschriftung
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
    // Spaltenzahl so wählen, dass die Bilder möglichst groß werden
    let cell_for = |cols: usize| {
        let rows = n.div_ceil(cols);
        let w = (rect.width() - gap * (cols - 1) as f32) / cols as f32;
        let h = (rect.height() - gap * (rows - 1) as f32) / rows as f32 - caption_h;
        // Nicht breiter als 16:9 und nicht höher als 3:4
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
        // Unvollständige letzte Reihe mittig
        let in_row = if row + 1 == rows {
            n - row * cols
        } else {
            cols
        };
        let indent = (cols - in_row) as f32 * (cell.x + gap) / 2.0;
        // Nacheinander einblenden, mit leichtem Aufsteigen
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
                // Weicher Schatten unter dem Bild
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
        // Stichpunkte mit Bild werden zu beschrifteten Bildern. Ohne eigene
        // Unterpunkte verschwinden sie dabei aus der Aufzählung.
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

/// Was zum Zeichnen einer Folie gebraucht wird – im Vortrag, in der
/// Referentenansicht und beim PDF-Export
pub struct SlideView<'a> {
    slides: &'a [Slide],
    pub index: usize,
    /// Sichtbare Stichpunkt-Gruppen
    pub build: usize,
    pub build_time: f64,
    pub slide_time: f64,
    pub now: f64,
    /// Deckkraft während des Übergangs
    pub appear: f32,
    /// Fortschrittsleiste unten (0…1)
    pub progress: f32,
    /// Checkboxen und Rechtsklick-Menü bedienbar
    pub interactive: bool,
    /// Als ganze Seite (PDF): ohne Schatten und runde Ecken
    pub page: bool,
}

/// Was beim Zeichnen angeklickt wurde
pub struct SlideOutput {
    pub ticked: Option<(u64, Pos2)>,
    pub new_layout: Option<Option<SlideLayout>>,
    pub animating: bool,
}

/// Eine Folie in `card` zeichnen
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
    // Alle Maße relativ zu einer 1280 px breiten Folie
    let s = card.width() / 1280.0;

    let radius = if view.page { 0.0 } else { 24.0 * s };
    if !view.page {
        // Weicher, weichgezeichneter Schatten unter der Folie
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
    // Bilder blenden mit der Folie ein
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

    // Rechtsklick auf die Folie: Layout wählen (vor dem Inhalt, damit Checkboxen Vorrang haben)
    let mut new_layout: Option<Option<SlideLayout>> = None;
    if view.interactive && slide.branch.is_some() {
        ui.interact(card, ui.id().with("slide_card"), Sense::click())
            .context_menu(|ui| {
                ui.label(RichText::new("Slide layout").strong());
                new_layout = layout_menu(ui, slide.layout);
            });
    }

    // Wie viele Checkboxen dieses Zweigs schon erledigt sind
    let tasks = slide.branch.and_then(|b| map.task_progress(b));
    // Angeklickte Checkbox, wird nach dem Zeichnen umgeschaltet
    let mut ticked: Option<(u64, Pos2)> = None;

    // Titel steigt auf und blendet ein, die Linie darunter zieht sich auf
    let title_in = ease_out_cubic(progress(now, view.slide_time, 0.55));
    let line_in = ease_out_cubic(progress(now, view.slide_time + 0.2, 0.5));
    let mut animating = title_in < 1.0 || line_in < 1.0 || appear < 1.0;

    // Inhalt der Folie in eigenem Bereich, der mit dem Übergang ein- und ausblendet
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

        // Auf einem Vollbild steht der Text hell auf abgedunkeltem Bild
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
            // Lichtschimmer: ein heller, schräger Streifen zieht einmal über den Titel
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
            // Ohne Themen-Marken steht der Titel etwas tiefer, näher an der Mitte
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
            // Kapitel-Marken ploppen nacheinander auf (abschaltbar)
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
                    // Feste Größe vorab, damit eine Marke als Ganzes in die nächste Zeile rutscht
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
                // Diagramm statt Aufzählung, baut sich genauso schrittweise auf
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
                // Stichpunkte: noch verdeckte nehmen schon ihren Platz ein, damit nichts
                // verrutscht. Jede Gruppe erscheint mit dem gewählten Effekt; beim
                // schrittweisen Aufbau werden ältere Gruppen blasser.
                // Bilder neben den Stichpunkten, oder als Galerie, wenn es keine gibt
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
                                // Checkbox statt Aufzählungspunkt; Erledigtes durchgestrichen.
                                // Sichtbare Checkboxen lassen sich anklicken.
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

    // Fortschritt unten auf der Folie
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

/// Folientitel; ein Symbol am Anfang (Phosphor-Zeichen) in der Farbe der Folie
fn title_job(text: &str, size: f32, ink: Color32, accent: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let format = |color| egui::TextFormat::simple(FontId::proportional(size), color);
    let mut chars = text.chars();
    match chars.next() {
        // Phosphor liegt im Bereich für private Zeichen
        Some(c) if ('\u{E000}'..='\u{F8FF}').contains(&c) => {
            job.append(&c.to_string(), 0.0, format(accent));
            job.append(chars.as_str(), 0.0, format(ink));
        }
        _ => job.append(text, 0.0, format(ink)),
    }
    job
}

/// Notizen zur Folie: die des Zweigs und die seiner Stichpunkte
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

/// Inhalt der Referentenansicht; gibt die gedrückten Tasten zurück
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

    // Kopfzeile: Stoppuhr, Uhrzeit, Position
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

    // Unten Platz für die Tastenhinweise lassen
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

    // Aktuelle Folie, so wie das Publikum sie sieht
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

    // Nächste Folie, fertig aufgebaut
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

    // Notizen unter der nächsten Folie
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
    // Hinweis unten links
    ui.painter().text(
        Pos2::new(full.left(), full.bottom()),
        Align2::LEFT_BOTTOM,
        "Space · → next   ← back   G quick jump   P close this window   Esc end",
        FontId::proportional(13.0),
        weak,
    );
    nav
}

/// Für Tests: die Referentenansicht direkt in `ui` zeichnen
#[cfg(test)]
pub fn presenter_snapshot(ui: &mut egui::Ui, doc: &mut Document, theme: &Theme, fx: &Effects) {
    let slides = slides(&doc.map, theme);
    let now = ui.input(|i| i.time);
    let start = doc.presenter_start.unwrap_or(now);
    presenter_body(ui, doc, &slides, theme, fx, true, now, start);
}

/// Referentenansicht in einem eigenen Fenster: aktuelle und nächste Folie,
/// Notizen, Stoppuhr und Uhrzeit. Gibt die dort gedrückten Tasten zurück.
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
            // Stoppuhr und Uhr jede Sekunde aktualisieren
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
            let mut nav = Nav::default();
            let mut body = |ui: &mut egui::Ui| {
                nav = presenter_body(ui, doc, slides, theme, fx, build_bullets, now, start);
            };
            if class == egui::ViewportClass::Embedded {
                // Kein zweites Fenster möglich: als Fenster in der App
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

/// Für Tests: Folie `index` fertig aufgebaut als Karte (mit Schatten) in `card` zeichnen
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

/// Anzahl der Folien (Titelfolie plus eine je Hauptzweig)
pub fn slide_count(map: &MindMap, theme: &Theme) -> usize {
    slides(map, theme).len()
}

/// Folie `index` fertig aufgebaut und ohne Animation in die ganze Fläche zeichnen
/// (PDF-Export, Vorschau der nächsten Folie)
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

/// `build_bullets`: Stichpunkte nacheinander aufdecken
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
    // Referentenansicht im zweiten Fenster; dort gedrückte Tasten zählen mit
    if doc.presenter {
        let keys = presenter_window(ui.ctx(), doc, &slides, theme, fx, build_bullets, now);
        nav = nav.merge(keys);
    }
    if nav.presenter {
        doc.presenter = !doc.presenter;
    }
    // Handy-Fernbedienung: Knöpfe vom Handy wie Pfeiltasten behandeln
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
    // Esc schließt zuerst das QR-Fenster, erst dann den Vortrag
    if doc.remote_open && nav.escape {
        doc.remote_open = false;
        nav.escape = false;
    }
    let area = ui.max_rect();
    let ctx = ui.ctx().clone();

    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);
    // Beim Weiterblättern ziehen die Sterne ein Stück mit
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

    // Geblättert wird nur per Tastatur – Klicks sind fürs Abhaken da
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

    // Übergang: die neue Folie kommt je nach Effekt anders herein
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
    // Langsamer Zoom, solange die Folie zu sehen ist
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
        // Neues Layout gleich animiert zeigen
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

    // Schnellwahl und Laserpointer liegen über allem
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

    // Stand fürs Handy, und das QR-Fenster
    if let Some(remote) = crate::remote::running() {
        let slide = &slides[doc.slide];
        let start = *doc.presenter_start.get_or_insert(now);
        // Symbole aus der Phosphor-Schrift kennt das Handy nicht
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

/// Fenster mit QR-Code für die Handy-Fernbedienung. Gibt `false` zurück,
/// wenn es geschlossen wurde.
fn remote_overlay(ui: &mut egui::Ui, area: Rect, theme: &Theme) -> bool {
    let ctx = ui.ctx().clone();
    // Solange es offen ist, regelmäßig prüfen, ob sich ein Handy meldet
    ctx.request_repaint_after(std::time::Duration::from_millis(500));
    // Abdunkeln über der Folie, aber unter dem Fenster
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
                                // Ruhezone von 4 Feldern rundherum
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
        // Gespeichert vor Version 1: Feld `version` fehlt → 0
        let mut old: Effects = serde_json::from_str(r#"{"transition":"Slide"}"#).unwrap();
        assert_eq!(old.version, 0);
        old.migrate();
        assert!(old.transition == Transition::Fade);
        // Danach bleibt eine bewusste Wahl erhalten
        old.transition = Transition::Zoom;
        old.migrate();
        assert!(old.transition == Transition::Zoom);
        assert!(Effects::default().transition == Transition::Fade);
        // Version 0 und 1 bekommen einmalig den Sternenhimmel
        assert!(old.background == Backdrop::Stars);
        let mut v1: Effects =
            serde_json::from_str(r#"{"background":"Aurora","version":1}"#).unwrap();
        v1.migrate();
        assert!(v1.background == Backdrop::Stars);
        v1.background = Backdrop::Plain;
        v1.migrate();
        assert!(v1.background == Backdrop::Plain);
        // Version 3: Glühen und Lichtpulse nur noch auf Wunsch
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
        // Nicht über das Ende hinaus
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
