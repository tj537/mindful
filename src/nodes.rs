//! Aussehen und Zeichnen der Mindmap-Knoten – gemeinsam genutzt von der
//! Mindmap-Fläche (app.rs), der Kamerafahrt (tour.rs) und dem Export.

use crate::model::{MindMap, Node};
use crate::theme::Theme;
use eframe::egui::{self, Color32, FontId, Rect, Vec2};

/// Mindestgröße eines Knotens bei Zoom 1
pub const MIN_NODE: Vec2 = Vec2::new(90.0, 34.0);

pub struct NodeStyle {
    pub fill: Color32,
    pub text: Color32,
    /// Farbiger Rand (ab der 2. Ebene) und Farbe der Verbindungslinie
    pub accent: Color32,
    pub outlined: bool,
    pub font_size: f32,
    pub padding: Vec2,
    pub edge_width: f32,
}

pub fn node_style(map: &MindMap, theme: &Theme, id: u64) -> NodeStyle {
    let accent = match map.color_index(id) {
        Some(i) => theme.branches[i % theme.branches.len()],
        None => theme.root,
    };
    match map.depth(id) {
        0 => NodeStyle {
            fill: accent,
            text: theme.on_accent,
            accent,
            outlined: false,
            font_size: 20.0,
            padding: Vec2::new(44.0, 26.0),
            edge_width: 3.5,
        },
        1 => NodeStyle {
            fill: accent,
            text: theme.on_accent,
            accent,
            outlined: false,
            font_size: 16.0,
            padding: Vec2::new(30.0, 16.0),
            edge_width: 3.5,
        },
        _ => NodeStyle {
            fill: theme.card,
            text: theme.text,
            accent,
            outlined: true,
            font_size: 14.5,
            padding: Vec2::new(30.0, 16.0),
            edge_width: 2.0,
        },
    }
}

/// Größe eines Knotens bei Zoom 1
pub fn node_size(ctx: &egui::Context, map: &MindMap, theme: &Theme, node: &Node) -> Vec2 {
    let style = node_style(map, theme, node.id);
    let galley = ctx.fonts(|f| {
        f.layout_no_wrap(
            node.text.clone(),
            FontId::proportional(style.font_size),
            style.text,
        )
    });
    (galley.size() + style.padding).max(MIN_NODE)
}

pub fn paint_node(
    painter: &egui::Painter,
    theme: &Theme,
    rect: Rect,
    style: &NodeStyle,
    zoom: f32,
    selected: bool,
    hit: bool,
) {
    let radius = rect.height() / 2.0;
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 3.0 * zoom)).expand(zoom),
        radius,
        Color32::from_black_alpha(if theme.dark { 90 } else { 30 }),
    );
    if hit {
        let grow = 9.0 * zoom;
        painter.rect_filled(
            rect.expand(grow),
            radius + grow,
            theme.selection.gamma_multiply(0.35),
        );
    }
    if selected {
        let grow = 5.0 * zoom;
        painter.rect_filled(rect.expand(grow), radius + grow, theme.selection);
    }
    if style.outlined {
        let grow = 2.0 * zoom;
        painter.rect_filled(rect.expand(grow), radius + grow, style.accent);
    }
    painter.rect_filled(rect, radius, style.fill);
}
