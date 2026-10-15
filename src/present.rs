//! Präsentationsmodus: eine Titelfolie und pro Hauptzweig eine Folie.
//!
//! Hier liegen auch die Teile, die Folien und Kamerafahrt gemeinsam nutzen:
//! Effekt-Einstellungen, Tastatur, Buttons, Folienstreifen, animierte
//! Hintergründe, Laserpointer und Animationskurven.

use crate::document::Document;
use crate::model::MindMap;
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
    #[default]
    Aurora,
    Stars,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    pub transition: Transition,
    pub bullets: BulletFx,
    pub background: Backdrop,
    /// Langsames Schweben der Kamera in der Tour
    pub drift: bool,
    /// Stand der Einstellungen; fehlt bei älteren gespeicherten Daten (= 0)
    #[serde(default)]
    pub version: u32,
}

const EFFECTS_VERSION: u32 = 1;

impl Default for Effects {
    fn default() -> Self {
        Self {
            transition: Transition::default(),
            bullets: BulletFx::default(),
            background: Backdrop::default(),
            drift: true,
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
        self.version = EFFECTS_VERSION;
    }
}

/// Auswahl der Effekte – in der Präsentation (Button oben rechts) und im Ansicht-Menü
pub fn effects_menu(ui: &mut egui::Ui, fx: &mut Effects, slides: bool, tour: bool) {
    ui.set_min_width(210.0);
    if slides {
        ui.strong("Slide transition");
        for (value, label) in [
            (Transition::Slide, "Slide"),
            (Transition::Fade, "Fade"),
            (Transition::Zoom, "Zoom"),
            (Transition::Rise, "Rise"),
        ] {
            ui.radio_value(&mut fx.transition, value, label);
        }
        ui.separator();
        ui.strong("Bullet points");
        for (value, label) in [
            (BulletFx::SlideIn, "Slide in"),
            (BulletFx::Fade, "Fade in"),
            (BulletFx::Pop, "Pop"),
            (BulletFx::Typewriter, "Typewriter"),
        ] {
            ui.radio_value(&mut fx.bullets, value, label);
        }
        ui.separator();
    }
    ui.strong("Background");
    for (value, label) in [
        (Backdrop::Aurora, "Aurora"),
        (Backdrop::Stars, "Starfield"),
        (Backdrop::Plain, "Plain"),
    ] {
        ui.radio_value(&mut fx.background, value, label);
    }
    if tour {
        ui.separator();
        ui.checkbox(&mut fx.drift, "Slow camera drift");
    }
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
        }
    })
}

impl Nav {
    pub fn next(&self) -> bool {
        self.right || self.space || self.enter
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
) -> (Option<Event>, bool) {
    let mut corner = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                area.right_top() + Vec2::new(-200.0, 10.0),
                Vec2::new(190.0, 36.0),
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
    corner
        .menu_button(big(icon::GEAR), |ui| {
            effects_menu(ui, fx, !tour, tour);
            ui.separator();
            shortcuts_list(ui, tour);
        })
        .response
        .on_hover_text("Settings and shortcuts");
    (event, strip)
}

/// Animierter Hintergrund hinter Folie bzw. Mindmap.
/// Gibt `true` zurück, wenn weiter neu gezeichnet werden muss.
pub fn backdrop(
    painter: &egui::Painter,
    area: Rect,
    theme: &Theme,
    kind: Backdrop,
    now: f64,
) -> bool {
    let t = now as f32;
    match kind {
        Backdrop::Plain => false,
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
            // Feste Pseudo-Zufallspositionen, funkeln und ziehen langsam vorbei
            for i in 0..180 {
                let h = |salt: f32| {
                    ((i as f32 * 12.9898 + salt * 78.233).sin() * 43758.547)
                        .fract()
                        .abs()
                };
                let depth = 0.3 + 0.7 * h(3.0);
                let x = area.left()
                    + (h(1.0) * area.width() + t * 5.0 * depth).rem_euclid(area.width());
                let y = area.top() + h(2.0) * area.height();
                let twinkle = 0.35
                    + 0.65
                        * (0.5
                            + 0.5
                                * (t * (0.6 + 1.4 * h(4.0)) + h(5.0) * std::f32::consts::TAU)
                                    .sin());
                painter.circle_filled(
                    Pos2::new(x, y),
                    0.6 + 1.3 * depth,
                    theme.text.gamma_multiply(0.55 * twinkle * depth),
                );
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
    /// Hauptzweig der Folie (für den Fortschritt der Checkboxen)
    branch: Option<u64>,
}

impl Slide {
    /// Stichpunkt-Gruppen: ein Punkt der obersten Ebene mit seinen Unterpunkten.
    /// Die Titelfolie wird nicht schrittweise aufgebaut.
    fn groups(&self) -> usize {
        if self.is_title {
            0
        } else {
            self.bullets.iter().filter(|b| b.0 == 0).count()
        }
    }
}

fn collect(map: &MindMap, id: u64, depth: usize, color: Color32, out: &mut Vec<Bullet>) {
    for child in map.children(id) {
        out.push((depth, child.text.clone(), color, child.task, child.id));
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
        title: root.text.clone(),
        note: root.note.clone(),
        bullets: map
            .children(root.id)
            .map(|c| (0, c.text.clone(), color_of(map, theme, c.id), c.task, c.id))
            .collect(),
        color: theme.root,
        is_title: true,
        branch: None,
    }];
    for branch in map.children(root.id) {
        let color = color_of(map, theme, branch.id);
        let mut bullets = Vec::new();
        collect(map, branch.id, 0, color, &mut bullets);
        out.push(Slide {
            title: branch.text.clone(),
            note: branch.note.clone(),
            bullets,
            color,
            is_title: false,
            branch: Some(branch.id),
        });
    }
    out
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
    let nav = read_nav(ui);
    let area = ui.max_rect();
    let ctx = ui.ctx().clone();

    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);
    if backdrop(&painter, area, theme, fx.background, now) {
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
    let card = Rect::from_center_size(base.center() + shift * (w / 1280.0), base.size() * scale);
    // Alle Maße relativ zu einer 1280 px breiten Folie
    let s = card.width() / 1280.0;

    painter.rect_filled(
        card.translate(Vec2::new(0.0, 10.0 * s)).expand(2.0 * s),
        24.0 * s,
        Color32::from_black_alpha(if theme.dark { 110 } else { 35 }),
    );
    painter.rect_filled(card, 24.0 * s, theme.card);
    let accent_bar = Rect::from_min_max(
        card.left_top() + Vec2::new(44.0, 80.0) * s,
        card.left_bottom() + Vec2::new(52.0, -80.0) * s,
    );
    painter.rect_filled(accent_bar, 4.0 * s, slide.color.gamma_multiply(appear));

    let (corner_event, strip_clicked) =
        corner_buttons(ui, area, "End presentation (Esc)", fx, false);
    if corner_event.is_some() {
        event = corner_event;
    }
    if strip_clicked {
        doc.strip = !doc.strip;
        doc.strip_cursor = doc.slide;
    }

    // Wie viele Checkboxen dieses Zweigs schon erledigt sind
    let tasks = slide.branch.and_then(|b| doc.map.task_progress(b));
    // Angeklickte Checkbox, wird nach dem Zeichnen umgeschaltet
    let mut ticked: Option<u64> = None;

    // Titel steigt auf und blendet ein, die Linie darunter zieht sich auf
    let title_in = ease_out_cubic(progress(now, doc.slide_time, 0.55));
    let line_in = ease_out_cubic(progress(now, doc.slide_time + 0.2, 0.5));
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

        let title = |ui: &mut egui::Ui, text: &str, size: f32| {
            ui.add_space((1.0 - title_in) * 26.0 * s);
            ui.scope(|ui| {
                ui.multiply_opacity(title_in);
                ui.add(
                    egui::Label::new(RichText::new(text).size(size).strong().color(theme.text))
                        .wrap(),
                );
            });
        };

        if slide.is_title {
            ui.add_space(content.height() * 0.22);
            title(&mut ui, &slide.title, 76.0 * s);
            if !slide.note.trim().is_empty() {
                ui.scope(|ui| {
                    ui.multiply_opacity(line_in);
                    ui.add(
                        egui::Label::new(
                            RichText::new(slide.note.trim())
                                .size(28.0 * s)
                                .color(theme.text.gamma_multiply(0.7)),
                        )
                        .wrap(),
                    );
                });
            }
            ui.add_space(36.0 * s);
            // Kapitel-Marken ploppen nacheinander auf
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0) * s;
                for (i, (_, text, color, _, _)) in slide.bullets.iter().enumerate() {
                    let pop = progress(now, doc.slide_time + 0.35 + i as f64 * 0.09, 0.4);
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
                let mut counter = format!("{} / {}", doc.slide, last);
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
                                .color(theme.text.gamma_multiply(0.75)),
                        )
                        .wrap(),
                    );
                });
                ui.add_space(10.0 * s);
            }

            // Stichpunkte: noch verdeckte nehmen schon ihren Platz ein, damit nichts
            // verrutscht. Jede Gruppe erscheint mit dem gewählten Effekt; beim
            // schrittweisen Aufbau werden ältere Gruppen blasser.
            let building = build_bullets && slide.groups() > 0;
            let shrink = (8.0 / slide.bullets.len().max(8) as f32).max(0.6);
            let mut group = 0;
            for (i, (depth, text, color, task, node)) in slide.bullets.iter().enumerate() {
                if *depth == 0 && i > 0 {
                    group += 1;
                }
                let start = if building {
                    if group >= doc.build {
                        f64::INFINITY
                    } else if group + 1 == doc.build {
                        doc.build_time
                    } else {
                        f64::NEG_INFINITY
                    }
                } else {
                    doc.slide_time + 0.35 + group as f64 * 0.12
                };
                let duration = match fx.bullets {
                    BulletFx::Typewriter => (text.chars().count() as f64 * 0.025).max(0.35),
                    _ => 0.35,
                };
                let raw = progress(now, start, duration);
                animating |= raw < 1.0 && start.is_finite();
                let e = ease_out_cubic(raw);
                let dim = if building && group + 1 < doc.build {
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
                            let r = Rect::from_center_size(dot.center(), Vec2::splat(size * 0.62));
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
                                    ticked = Some(*node);
                                }
                            }
                            if done {
                                label = label.strikethrough().color(theme.text.gamma_multiply(0.5));
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
    if animating {
        ctx.request_repaint();
    }
    if let Some(id) = ticked {
        doc.checkpoint();
        if let Some(n) = doc.map.get_mut(id) {
            n.task = n.task.map(|done| !done);
        }
    }

    // Fortschritt unten auf der Folie
    let track = Rect::from_min_size(
        card.left_bottom() + Vec2::new(44.0, -34.0) * s,
        Vec2::new(card.width() - 88.0 * s, 4.0 * s),
    );
    painter.rect_filled(track, 2.0 * s, theme.grid);
    let done = ((anim + 1.0) / slides.len() as f32).clamp(0.0, 1.0);
    painter.rect_filled(
        Rect::from_min_size(track.min, Vec2::new(track.width() * done, track.height())),
        2.0 * s,
        slide.color,
    );

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
    laser(ui, nav.laser, now);

    event
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
