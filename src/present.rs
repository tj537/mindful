//! Präsentationsmodus: eine Titelfolie und pro Hauptzweig eine Folie.

use crate::document::Document;
use crate::model::MindMap;
use crate::theme::Theme;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Key, Layout, Modifiers, Rect, RichText, Sense, UiBuilder,
    Vec2,
};
use egui_phosphor::regular as icon;

pub enum Event {
    Exit,
    ToggleFullscreen,
}

struct Slide {
    title: String,
    note: String,
    /// (Ebene, Text, Farbe)
    bullets: Vec<(usize, String, Color32)>,
    color: Color32,
    is_title: bool,
}

fn color_of(map: &MindMap, theme: &Theme, id: u64) -> Color32 {
    map.color_index(id)
        .map(|c| theme.branches[c % theme.branches.len()])
        .unwrap_or(theme.root)
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

pub fn show(ui: &mut egui::Ui, doc: &mut Document, theme: &Theme) -> Option<Event> {
    let slides = slides(&doc.map, theme);
    if slides.is_empty() {
        return Some(Event::Exit);
    }
    let last = slides.len() - 1;
    let mut event = None;

    // Tastatur
    let (next, prev, home, end, exit, full) = ui.input_mut(|i| {
        let mut any = |keys: &[Key]| keys.iter().any(|k| i.consume_key(Modifiers::NONE, *k));
        (
            any(&[
                Key::ArrowRight,
                Key::ArrowDown,
                Key::Space,
                Key::PageDown,
                Key::Enter,
            ]),
            any(&[Key::ArrowLeft, Key::ArrowUp, Key::PageUp, Key::Backspace]),
            any(&[Key::Home]),
            any(&[Key::End]),
            any(&[Key::Escape]),
            any(&[Key::F]),
        )
    });

    let area = ui.max_rect();
    let background = ui.allocate_rect(area, Sense::click());
    if next || background.clicked() {
        doc.slide = (doc.slide + 1).min(last);
    }
    if prev || background.secondary_clicked() {
        doc.slide = doc.slide.saturating_sub(1);
    }
    if home {
        doc.slide = 0;
    }
    if end {
        doc.slide = last;
    }
    if exit {
        event = Some(Event::Exit);
    }
    if full {
        event = Some(Event::ToggleFullscreen);
    }
    doc.slide = doc.slide.min(last);
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

    // Buttons oben rechts
    let mut corner = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                area.right_top() + Vec2::new(-110.0, 10.0),
                Vec2::new(100.0, 36.0),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    let big = |ic: &str| RichText::new(ic).size(18.0);
    if corner
        .button(big(icon::X))
        .on_hover_text("End presentation (Esc)")
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
        // Bei vielen Punkten die Schrift etwas verkleinern
        let shrink = (8.0 / slide.bullets.len().max(8) as f32).max(0.6);
        for (depth, text, color) in &slide.bullets {
            let size = (30.0 - *depth as f32 * 4.0) * s * shrink;
            ui.horizontal_top(|ui| {
                ui.add_space(*depth as f32 * 40.0 * s);
                let (dot, _) =
                    ui.allocate_exact_size(Vec2::new(size * 0.8, size * 1.2), Sense::hover());
                let radius = if *depth == 0 { size * 0.2 } else { size * 0.14 };
                ui.painter().circle_filled(dot.center(), radius, *color);
                ui.add(egui::Label::new(RichText::new(text).size(size).color(theme.text)).wrap());
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

    painter.text(
        egui::pos2(area.center().x, card.bottom() + 24.0),
        Align2::CENTER_CENTER,
        "←  →  navigate   ·   F  full screen   ·   Esc  exit",
        FontId::proportional(13.0),
        theme.text.gamma_multiply(0.45),
    );

    event
}
