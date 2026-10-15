//! Präsentationsmodus: eine Titelfolie und pro Hauptzweig eine Folie.
//! Stichpunkte erscheinen nacheinander, Taste G zeigt eine Übersicht.
//!
//! Die gemeinsamen Teile für Folien und Kamerafahrt (Tastatur, Buttons,
//! Übersicht, Animationskurven) liegen ebenfalls hier.

use crate::document::Document;
use crate::model::MindMap;
use crate::theme::Theme;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Key, Layout, Modifiers, Pos2, Rect, RichText, Sense,
    UiBuilder, Vec2,
};
use egui_phosphor::regular as icon;

pub enum Event {
    Exit,
    ToggleFullscreen,
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

pub fn color_of(map: &MindMap, theme: &Theme, id: u64) -> Color32 {
    map.color_index(id)
        .map(|c| theme.branches[c % theme.branches.len()])
        .unwrap_or(theme.root)
}

/// Tasten beim Präsentieren
#[derive(Default)]
pub struct Nav {
    pub next: bool,
    pub prev: bool,
    pub home: bool,
    pub end: bool,
    pub escape: bool,
    pub fullscreen: bool,
    pub grid: bool,
}

pub fn read_nav(ui: &mut egui::Ui) -> Nav {
    ui.input_mut(|i| {
        let mut any = |keys: &[Key]| keys.iter().any(|k| i.consume_key(Modifiers::NONE, *k));
        Nav {
            next: any(&[
                Key::ArrowRight,
                Key::ArrowDown,
                Key::Space,
                Key::PageDown,
                Key::Enter,
            ]),
            prev: any(&[Key::ArrowLeft, Key::ArrowUp, Key::PageUp, Key::Backspace]),
            home: any(&[Key::Home]),
            end: any(&[Key::End]),
            escape: any(&[Key::Escape]),
            fullscreen: any(&[Key::F]),
            grid: any(&[Key::G]),
        }
    })
}

/// Buttons oben rechts: Beenden, Vollbild, Übersicht.
/// Gibt das Ereignis und „Übersicht umschalten“ zurück.
pub fn corner_buttons(ui: &mut egui::Ui, area: Rect, exit_tip: &str) -> (Option<Event>, bool) {
    let mut corner = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                area.right_top() + Vec2::new(-150.0, 10.0),
                Vec2::new(140.0, 36.0),
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
    let grid = corner
        .button(big(icon::SQUARES_FOUR))
        .on_hover_text("Overview (G)")
        .clicked();
    (event, grid)
}

pub fn hint(painter: &egui::Painter, pos: Pos2, theme: &Theme, text: &str) {
    painter.text(
        pos,
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(13.0),
        theme.text.gamma_multiply(0.45),
    );
}

/// Eine Kachel in der Übersicht
pub struct GridItem {
    pub label: String,
    pub title: String,
    pub color: Color32,
}

/// Übersicht als Kachel-Raster. Gibt die angeklickte Kachel zurück.
pub fn overview_grid(
    ui: &mut egui::Ui,
    area: Rect,
    theme: &Theme,
    items: &[GridItem],
    current: usize,
) -> Option<usize> {
    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);

    let n = items.len().max(1);
    let cols = ((n as f32).sqrt().ceil() as usize).clamp(1, 4);
    let rows = n.div_ceil(cols);
    let gap = 24.0;
    let inner = area.shrink2(Vec2::new(70.0, 90.0));
    let cell_w = ((inner.width() - gap * (cols - 1) as f32) / cols as f32)
        .min((inner.height() - gap * (rows - 1) as f32) / rows as f32 * 16.0 / 9.0)
        .max(60.0);
    let cell = Vec2::new(cell_w, cell_w * 9.0 / 16.0);
    let total = Vec2::new(
        cols as f32 * cell.x + (cols - 1) as f32 * gap,
        rows as f32 * cell.y + (rows - 1) as f32 * gap,
    );
    let origin = inner.center() - total / 2.0;

    painter.text(
        egui::pos2(area.center().x, origin.y - 34.0),
        Align2::CENTER_CENTER,
        "Overview",
        FontId::proportional(22.0),
        theme.text,
    );

    let mut clicked = None;
    for (i, item) in items.iter().enumerate() {
        let (row, col) = (i / cols, i % cols);
        let rect = Rect::from_min_size(
            origin + Vec2::new(col as f32 * (cell.x + gap), row as f32 * (cell.y + gap)),
            cell,
        );
        let resp = ui.interact(rect, ui.id().with(("overview", i)), Sense::click());
        if i == current {
            painter.rect_filled(rect.expand(4.0), 16.0, theme.selection);
        }
        let fill = if resp.hovered() {
            theme.card.lerp_to_gamma(item.color, 0.12)
        } else {
            theme.card
        };
        painter.rect_filled(rect, 12.0, fill);
        painter.rect_filled(
            Rect::from_min_size(
                rect.min + Vec2::new(14.0, 14.0),
                Vec2::new(5.0, cell.y - 28.0),
            ),
            2.5,
            item.color,
        );
        let clip = painter.with_clip_rect(rect.shrink(6.0));
        clip.text(
            rect.min + Vec2::new(30.0, 16.0),
            Align2::LEFT_TOP,
            &item.label,
            FontId::proportional(13.0),
            item.color,
        );
        let size = (cell.y * 0.12).clamp(13.0, 26.0);
        let galley = clip.layout(
            item.title.clone(),
            FontId::proportional(size),
            theme.text,
            cell.x - 50.0,
        );
        clip.galley(rect.min + Vec2::new(30.0, 38.0), galley, theme.text);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    hint(
        &painter,
        egui::pos2(area.center().x, area.bottom() - 30.0),
        theme,
        "Click to jump   ·   G or Esc to close",
    );
    clicked
}

// ---------------------------------------------------------------------------
// Folien
// ---------------------------------------------------------------------------

struct Slide {
    title: String,
    note: String,
    /// (Ebene, Text, Farbe)
    bullets: Vec<(usize, String, Color32)>,
    color: Color32,
    is_title: bool,
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

fn collect(
    map: &MindMap,
    id: u64,
    depth: usize,
    color: Color32,
    out: &mut Vec<(usize, String, Color32)>,
) {
    for child in map.children(id) {
        out.push((depth, child.text.clone(), color));
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
            .map(|c| (0, c.text.clone(), color_of(map, theme, c.id)))
            .collect(),
        color: theme.root,
        is_title: true,
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
) -> Option<Event> {
    let slides = slides(&doc.map, theme);
    if slides.is_empty() {
        return Some(Event::Exit);
    }
    let last = slides.len() - 1;
    doc.slide = doc.slide.min(last);
    let now = ui.input(|i| i.time);
    let groups_of = |i: usize| {
        if build_bullets {
            slides[i].groups()
        } else {
            0
        }
    };
    let mut event = None;
    let nav = read_nav(ui);
    let area = ui.max_rect();

    // Übersicht
    if doc.grid {
        let items: Vec<GridItem> = slides
            .iter()
            .enumerate()
            .map(|(i, s)| GridItem {
                label: if s.is_title {
                    "Title".into()
                } else {
                    format!("{i} / {last}")
                },
                title: s.title.clone(),
                color: s.color,
            })
            .collect();
        if let Some(i) = overview_grid(ui, area, theme, &items, doc.slide) {
            doc.slide = i;
            doc.build = 0;
            doc.grid = false;
        }
        if nav.grid || nav.escape {
            doc.grid = false;
        }
        if nav.fullscreen {
            event = Some(Event::ToggleFullscreen);
        }
        return event;
    }

    let background = ui.allocate_rect(area, Sense::click());
    if nav.next || background.clicked() {
        if doc.build < groups_of(doc.slide) {
            doc.build += 1;
            doc.build_time = now;
        } else if doc.slide < last {
            doc.slide += 1;
            doc.build = 0;
        }
    }
    if nav.prev || background.secondary_clicked() {
        if doc.build > 0 {
            doc.build -= 1;
        } else if doc.slide > 0 {
            doc.slide -= 1;
            doc.build = groups_of(doc.slide);
        }
    }
    if nav.home {
        doc.slide = 0;
        doc.build = 0;
    }
    if nav.end {
        doc.slide = last;
        doc.build = groups_of(last);
    }
    if nav.escape {
        event = Some(Event::Exit);
    }
    if nav.fullscreen {
        event = Some(Event::ToggleFullscreen);
    }
    if nav.grid {
        doc.grid = true;
    }
    let slide = &slides[doc.slide];

    // Folie im Format 16:9, alle Maße relativ zu 1280 px Breite
    let avail = area.shrink2(Vec2::new(40.0, 50.0));
    let w = avail.width().min(avail.height() * 16.0 / 9.0);
    let card = Rect::from_center_size(avail.center(), Vec2::new(w, w * 9.0 / 16.0));
    let s = w / 1280.0;

    // Weicher Übergang: alte Folie gleitet raus, neue blendet ein
    let anim = ui
        .ctx()
        .animate_value_with_time(ui.id().with("slide_anim"), doc.slide as f32, 0.35);
    let offset = doc.slide as f32 - anim;
    let opacity = (1.0 - offset.abs()).clamp(0.0, 1.0);

    let painter = ui.painter().clone();
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
    painter.rect_filled(accent_bar, 4.0 * s, slide.color.gamma_multiply(opacity));

    let (corner_event, grid_clicked) = corner_buttons(ui, area, "End presentation (Esc)");
    if corner_event.is_some() {
        event = corner_event;
    }
    if grid_clicked {
        doc.grid = true;
    }

    let content = card
        .shrink2(Vec2::new(100.0, 70.0) * s)
        .translate(Vec2::new(offset * 90.0 * s, 0.0));
    let mut ui = ui.new_child(
        UiBuilder::new()
            .max_rect(content)
            .layout(Layout::top_down(Align::Min)),
    );
    ui.multiply_opacity(opacity);
    ui.style_mut().interaction.selectable_labels = false;
    ui.spacing_mut().item_spacing.y = 10.0 * s;

    if slide.is_title {
        ui.add_space(content.height() * 0.22);
        ui.add(
            egui::Label::new(
                RichText::new(&slide.title)
                    .size(76.0 * s)
                    .strong()
                    .color(theme.text),
            )
            .wrap(),
        );
        if !slide.note.trim().is_empty() {
            ui.add(
                egui::Label::new(
                    RichText::new(slide.note.trim())
                        .size(28.0 * s)
                        .color(theme.text.gamma_multiply(0.7)),
                )
                .wrap(),
            );
        }
        ui.add_space(36.0 * s);
        // Die Hauptzweige als farbige Kapitel-Marken
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(12.0, 12.0) * s;
            for (_, text, color) in &slide.bullets {
                // Feste Größe vorab, damit eine Marke als Ganzes in die nächste Zeile rutscht
                let galley = ui.painter().layout_no_wrap(
                    text.clone(),
                    FontId::proportional(20.0 * s),
                    theme.on_accent,
                );
                let size = galley.size() + Vec2::new(36.0, 16.0) * s;
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                ui.painter().rect_filled(rect, rect.height() / 2.0, *color);
                ui.painter()
                    .galley(rect.center() - galley.size() / 2.0, galley, theme.on_accent);
            }
        });
    } else {
        ui.label(
            RichText::new(format!("{} / {}", doc.slide, last))
                .size(18.0 * s)
                .color(slide.color),
        );
        ui.add(
            egui::Label::new(
                RichText::new(&slide.title)
                    .size(54.0 * s)
                    .strong()
                    .color(theme.text),
            )
            .wrap(),
        );
        let (line, _) = ui.allocate_exact_size(Vec2::new(90.0, 6.0) * s, Sense::hover());
        painter.rect_filled(line, 3.0 * s, slide.color.gamma_multiply(opacity));
        ui.add_space(14.0 * s);
        if !slide.note.trim().is_empty() {
            ui.add(
                egui::Label::new(
                    RichText::new(slide.note.trim())
                        .size(24.0 * s)
                        .color(theme.text.gamma_multiply(0.75)),
                )
                .wrap(),
            );
            ui.add_space(10.0 * s);
        }

        // Stichpunkte: noch verdeckte nehmen schon ihren Platz ein, damit
        // nichts verrutscht; der neueste gleitet herein, ältere werden blasser.
        let building = build_bullets && slide.groups() > 0;
        let appear = ease_out_cubic(((now - doc.build_time) / 0.35).clamp(0.0, 1.0) as f32);
        if building && appear < 1.0 {
            ui.ctx().request_repaint();
        }
        let shrink = (8.0 / slide.bullets.len().max(8) as f32).max(0.6);
        let mut group = 0;
        for (i, (depth, text, color)) in slide.bullets.iter().enumerate() {
            if *depth == 0 && i > 0 {
                group += 1;
            }
            let (alpha, shift) = if !building {
                (1.0, 0.0)
            } else if group >= doc.build {
                (0.0, 0.0)
            } else if group + 1 == doc.build {
                (appear, (1.0 - appear) * 36.0 * s)
            } else {
                (0.4, 0.0)
            };
            let size = (30.0 - *depth as f32 * 4.0) * s * shrink;
            ui.scope(|ui| {
                ui.multiply_opacity(alpha);
                ui.horizontal_top(|ui| {
                    ui.add_space(*depth as f32 * 40.0 * s + shift);
                    let (dot, _) =
                        ui.allocate_exact_size(Vec2::new(size * 0.8, size * 1.2), Sense::hover());
                    let radius = if *depth == 0 { size * 0.2 } else { size * 0.14 };
                    ui.painter().circle_filled(dot.center(), radius, *color);
                    ui.add(
                        egui::Label::new(RichText::new(text).size(size).color(theme.text)).wrap(),
                    );
                });
            });
        }
    }

    // Fortschritt unten auf der Folie
    let track = Rect::from_min_size(
        card.left_bottom() + Vec2::new(44.0, -34.0) * s,
        Vec2::new(card.width() - 88.0 * s, 4.0 * s),
    );
    painter.rect_filled(track, 2.0 * s, theme.grid);
    let progress = ((anim + 1.0) / slides.len() as f32).clamp(0.0, 1.0);
    painter.rect_filled(
        Rect::from_min_size(
            track.min,
            Vec2::new(track.width() * progress, track.height()),
        ),
        2.0 * s,
        slide.color,
    );

    hint(
        &painter,
        egui::pos2(area.center().x, card.bottom() + 24.0),
        theme,
        "←  →  navigate   ·   G  overview   ·   F  full screen   ·   Esc  exit",
    );

    event
}
