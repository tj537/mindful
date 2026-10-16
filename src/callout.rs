//! Callout boxes (`> [!NOTE]`, `> [!TIP]`, `> [!IMPORTANT]`, `> [!WARNING]`,
//! `> [!CAUTION]`, as on GitHub): colour, icon and the box itself – shared by the
//! slides, the Markdown preview, the mind map and the free-flight tour.

use crate::model::{Callout, CalloutKind};
use crate::theme::Theme;
use eframe::egui::{
    self, text::LayoutJob, Color32, FontId, Margin, Rect, RichText, Stroke, TextFormat, Vec2,
};
use egui_phosphor::regular as icon;

/// Colour and icon of a kind – the same in every theme, so a warning always looks like one
pub fn look(kind: CalloutKind) -> (Color32, &'static str) {
    match kind {
        CalloutKind::Note => (Color32::from_rgb(68, 147, 248), icon::INFO),
        CalloutKind::Tip => (Color32::from_rgb(63, 185, 80), icon::LIGHTBULB),
        CalloutKind::Important => (Color32::from_rgb(171, 125, 248), icon::SEAL_WARNING),
        CalloutKind::Warning => (Color32::from_rgb(214, 158, 46), icon::WARNING),
        CalloutKind::Caution => (Color32::from_rgb(248, 81, 73), icon::WARNING_OCTAGON),
    }
}

/// Background tint of the box
pub fn fill(kind: CalloutKind, theme: &Theme) -> Color32 {
    look(kind)
        .0
        .gamma_multiply(if theme.dark { 0.16 } else { 0.12 })
}

/// The box as a widget: icon and label in colour, then the text in the same paragraph,
/// a bar on the left.
/// `size` is the font size of the text; `width` the width of the whole box.
pub fn show(ui: &mut egui::Ui, callout: &Callout, size: f32, width: f32, theme: &Theme) {
    let (color, glyph) = look(callout.kind);
    let pad = Vec2::new(size * 0.9, size * 0.55);
    let inner = (width - 2.0 * pad.x).max(40.0);
    let response = egui::Frame::new()
        .fill(fill(callout.kind, theme))
        .corner_radius(size * 0.5)
        .inner_margin(Margin::symmetric(
            pad.x.round().clamp(0.0, 127.0) as i8,
            pad.y.round().clamp(0.0, 127.0) as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(inner);
            let mut job = LayoutJob::default();
            let format = |size: f32, color: Color32| TextFormat {
                font_id: FontId::proportional(size),
                color,
                valign: egui::Align::Center,
                ..Default::default()
            };
            job.append(
                &format!("{glyph}  {}", callout.kind.label()),
                0.0,
                format(size * 0.85, color),
            );
            job.append(callout.text.trim(), size * 0.6, format(size, theme.text));
            ui.add(egui::Label::new(job).wrap());
        })
        .response;
    let r = response.rect;
    ui.painter().rect_filled(
        Rect::from_min_size(
            r.min + Vec2::new(0.0, size * 0.35),
            Vec2::new(size * 0.16, (r.height() - size * 0.7).max(0.0)),
        ),
        size * 0.08,
        color,
    );
    ui.painter().rect_stroke(
        r,
        size * 0.5,
        Stroke::new(1.0_f32, color.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
}

/// Picker for the kind (in the callout window of the mind map)
pub fn kind_picker(ui: &mut egui::Ui, kind: &mut CalloutKind) {
    ui.horizontal_wrapped(|ui| {
        for k in CalloutKind::ALL {
            let (color, glyph) = look(k);
            let text = RichText::new(format!("{glyph}  {}", k.label())).color(color);
            ui.selectable_value(kind, k, text);
        }
    });
}
