//! Markdown mode: editor on the left (with highlighting), preview on the right.
//! Every change to the text is applied to the mind map right away.

use crate::document::Document;
use crate::icons;
use crate::images;
use crate::model::{callout_start, notes_start, MindMap, Node};
use crate::nodes::paint_checkbox;
use crate::theme::Theme;
use eframe::egui::{
    self, text::LayoutJob, Color32, FontId, Margin, Rect, RichText, Sense, TextFormat, Vec2,
};
use egui_phosphor::regular as icon;

const BODY: f32 = 15.0;

/// Length of the bullet at the start of the line ("- ", "* ", "1. " …)
fn bullet_len(trimmed: &str) -> Option<usize> {
    for bullet in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ "] {
        if trimmed.starts_with(bullet) {
            return Some(bullet.len());
        }
    }
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    (digits > 0 && trimmed[digits..].starts_with(". ")).then_some(digits + 2)
}

/// Colours headings and bullets – in the colour of the
/// main branch they belong to, as in the mind map. Speaker notes
/// (`<!-- notes: … -->`) are set apart in italics, search matches get a background.
fn highlight(text: &str, theme: &Theme, query: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let fmt = |size: f32, color: Color32| TextFormat {
        font_id: FontId::proportional(size),
        color,
        ..Default::default()
    };
    let note_color = theme.text.gamma_multiply(0.72);
    let notes_fmt = TextFormat {
        italics: true,
        ..fmt(BODY, theme.text.gamma_multiply(0.5))
    };
    let mut branch_color = theme.root;
    let mut branch = 0;
    let mut in_notes = false;
    // Inside a callout box: its colour
    let mut callout: Option<Color32> = None;

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let hashes = trimmed.chars().take_while(|&c| c == '#').count();

        if let Some((kind, _)) = callout_start(line) {
            let color = crate::callout::look(kind).0;
            callout = Some(color);
            job.append(line, 0.0, fmt(BODY, color));
            continue;
        }
        match callout {
            Some(color) if trimmed.starts_with('>') => {
                job.append(&line[..indent + 1], 0.0, fmt(BODY, color));
                job.append(&line[indent + 1..], 0.0, fmt(BODY, theme.text));
                continue;
            }
            _ => callout = None,
        }
        if in_notes || notes_start(line).is_some() {
            // Comment ends on this line? (On the first line only after `<!--`)
            let body = if in_notes { line } else { &trimmed[4..] };
            in_notes = !body.contains("-->");
            job.append(line, 0.0, notes_fmt.clone());
        } else if hashes > 0 && trimmed[hashes..].starts_with(' ') {
            let size = match hashes {
                1 => 26.0,
                2 => 20.0,
                _ => 17.0,
            };
            let marker = match hashes {
                1 => theme.root,
                2 => {
                    branch_color = theme.branches[branch % theme.branches.len()];
                    branch += 1;
                    branch_color
                }
                _ => branch_color,
            };
            let split = indent + hashes + 1;
            job.append(&line[..split], 0.0, fmt(size, marker.gamma_multiply(0.75)));
            // Layout comment (`<!-- timeline -->`) dimmed
            let rest = &line[split..];
            match rest.find("<!--") {
                Some(c) => {
                    job.append(&rest[..c], 0.0, fmt(size, theme.text));
                    job.append(&rest[c..], 0.0, fmt(BODY, theme.grid.gamma_multiply(1.6)));
                }
                None => job.append(rest, 0.0, fmt(size, theme.text)),
            }
        } else if trimmed.starts_with("![") {
            // Image line dimmed, like a comment
            job.append(line, 0.0, fmt(BODY, theme.grid.gamma_multiply(1.6)));
        } else if let Some(len) = bullet_len(trimmed) {
            let split = indent + len;
            // Finished tasks paler
            let done = trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ");
            let text_color = if done {
                note_color.gamma_multiply(0.8)
            } else {
                theme.text
            };
            job.append(&line[..split], 0.0, fmt(BODY, branch_color));
            job.append(&line[split..], 0.0, fmt(BODY, text_color));
        } else {
            job.append(line, 0.0, fmt(BODY, note_color));
        }
    }
    crate::search::mark(&mut job, query, crate::nodes::SEARCH.gamma_multiply(0.45));
    job
}

pub fn show(ui: &mut egui::Ui, doc: &mut Document, theme: &Theme, preview: bool, query: &str) {
    if preview {
        egui::SidePanel::right("md_preview")
            .resizable(true)
            .default_width(ui.available_width() * 0.42)
            .min_width(240.0)
            .frame(
                egui::Frame::default()
                    .fill(theme.card)
                    .inner_margin(Margin::symmetric(28, 20)),
            )
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("md_preview_scroll")
                    .auto_shrink([false; 2])
                    .show(ui, |ui| preview_outline(ui, &doc.map, theme));
            });
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(theme.background).inner_margin(Margin::symmetric(36, 22)))
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("md_editor_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
                        let mut job = highlight(text, theme, query);
                        job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(job))
                    };
                    let editor = egui::TextEdit::multiline(&mut doc.md_text)
                        .id_salt("md_editor")
                        .frame(false)
                        .desired_width(f32::INFINITY)
                        .desired_rows(30)
                        .lock_focus(true)
                        .hint_text("# Topic\n\n## Branch\n\nA note about the branch\n\n- Child\n  - deeper")
                        .layouter(&mut layouter);
                    let output = editor.show(ui);
                    let resp = output.response;
                    // Search: scroll to the current match once
                    if std::mem::take(&mut doc.md_find_scroll) {
                        if let Some(pos) = doc.md_find {
                            let index = doc.md_text.get(..pos).map_or(0, |t| t.chars().count());
                            let cursor = egui::text::CCursor::new(index);
                            let at = output.galley.pos_from_ccursor(cursor);
                            ui.scroll_to_rect(
                                at.translate(output.galley_pos.to_vec2()).expand(40.0),
                                Some(egui::Align::Center),
                            );
                        }
                    }
                    if std::mem::take(&mut doc.md_focus) {
                        resp.request_focus();
                    }
                    if resp.changed() {
                        doc.markdown_changed();
                    }
                });
        });
}

fn branch_color(map: &MindMap, theme: &Theme, id: u64) -> Color32 {
    map.color_index(id)
        .map(|c| theme.branches[c % theme.branches.len()])
        .unwrap_or(theme.root)
}

/// Small preview picture under an entry
fn thumb(ui: &mut egui::Ui, map: &MindMap, node: &Node, indent: f32) {
    let Some(key) = &node.image else { return };
    ui.horizontal_top(|ui| {
        ui.add_space(indent);
        let aspect = map.images.get(key).map_or(1.3, |img| img.aspect());
        let height = 72.0;
        let size = Vec2::new((height * aspect).min(ui.available_width()), height);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        match map.images.get(key) {
            Some(img) => {
                images::paint(
                    ui.painter(),
                    ui.ctx(),
                    key,
                    img,
                    rect,
                    images::Fit::Cover,
                    8.0,
                    Color32::WHITE,
                    1.0,
                );
            }
            None => images::placeholder(ui.painter(), rect, 8.0),
        }
    });
}

fn note(ui: &mut egui::Ui, note: &str, indent: f32) {
    if note.trim().is_empty() {
        return;
    }
    ui.horizontal_top(|ui| {
        ui.add_space(indent);
        ui.add(egui::Label::new(RichText::new(note.trim()).weak()).wrap());
    });
}

/// Speaker notes in the preview: small, italic, with a speech bubble
fn speaker(ui: &mut egui::Ui, notes: &str, indent: f32) {
    if notes.trim().is_empty() {
        return;
    }
    ui.horizontal_top(|ui| {
        ui.add_space(indent);
        ui.add(
            egui::Label::new(
                RichText::new(format!("{}  {}", icon::CHAT_CIRCLE_TEXT, notes.trim()))
                    .small()
                    .italics()
                    .weak(),
            )
            .wrap(),
        )
        .on_hover_text("Speaker notes – only in the presenter view");
    });
}

/// Callout boxes in the preview
fn callouts(ui: &mut egui::Ui, node: &Node, theme: &Theme, indent: f32) {
    for c in &node.callouts {
        ui.horizontal_top(|ui| {
            ui.add_space(indent);
            let width = ui.available_width();
            crate::callout::show(ui, c, 13.5, width, theme);
        });
    }
}

fn preview_outline(ui: &mut egui::Ui, map: &MindMap, theme: &Theme) {
    let Some(root) = map.root().and_then(|r| map.get(r)) else {
        return;
    };
    ui.label(
        RichText::new(format!("{}  Preview", icon::EYE))
            .small()
            .weak(),
    );
    ui.add_space(6.0);
    ui.add(egui::Label::new(RichText::new(icons::label(root)).size(26.0).strong()).wrap());
    thumb(ui, map, root, 0.0);
    note(ui, &root.note, 0.0);
    callouts(ui, root, theme, 0.0);
    speaker(ui, &root.speaker_notes, 0.0);

    for branch in map.children(root.id) {
        let color = branch_color(map, theme, branch.id);
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let (bar, _) = ui.allocate_exact_size(Vec2::new(4.0, 22.0), Sense::hover());
            ui.painter().rect_filled(bar, 2.0, color);
            ui.add(
                egui::Label::new(RichText::new(icons::label(branch)).size(19.0).strong()).wrap(),
            );
            if let Some(layout) = branch.layout {
                ui.label(
                    RichText::new(format!("{} {}", layout.icon(), layout.label()))
                        .small()
                        .color(color),
                );
            }
        });
        thumb(ui, map, branch, 12.0);
        note(ui, &branch.note, 12.0);
        callouts(ui, branch, theme, 12.0);
        speaker(ui, &branch.speaker_notes, 12.0);
        bullets(ui, map, theme, branch.id, 0, color);
    }
}

fn bullets(ui: &mut egui::Ui, map: &MindMap, theme: &Theme, id: u64, depth: usize, color: Color32) {
    for child in map.children(id) {
        let indent = 12.0 + depth as f32 * 20.0;
        ui.horizontal_top(|ui| {
            ui.add_space(indent);
            match child.task {
                Some(done) => {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                    let r =
                        Rect::from_center_size(r.center() + Vec2::new(0.0, 2.0), Vec2::splat(13.0));
                    paint_checkbox(ui.painter(), r, done, color, ui.visuals().text_color());
                    let mut text = RichText::new(icons::label(child));
                    if done {
                        text = text.strikethrough().weak();
                    }
                    ui.add(egui::Label::new(text).wrap());
                }
                None => {
                    ui.label(RichText::new("•").color(color).strong());
                    ui.add(egui::Label::new(icons::label(child)).wrap());
                }
            }
        });
        thumb(ui, map, child, indent + 16.0);
        note(ui, &child.note, indent + 16.0);
        callouts(ui, child, theme, indent + 16.0);
        speaker(ui, &child.speaker_notes, indent + 16.0);
        bullets(ui, map, theme, child.id, depth + 1, color);
    }
}
