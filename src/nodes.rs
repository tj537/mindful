//! Aussehen und Zeichnen der Mindmap-Knoten – gemeinsam genutzt von der
//! Mindmap-Fläche (app.rs), der Kamerafahrt (tour.rs) und dem Export.

use crate::model::{MindMap, Node};
use crate::theme::Theme;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

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
    let task = if node.task.is_some() {
        Vec2::new(TASK_SPACE, 0.0)
    } else {
        Vec2::ZERO
    };
    (galley.size() + style.padding + task).max(MIN_NODE)
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

/// Platz für die Checkbox links im Knoten (bei Zoom 1)
pub const TASK_SPACE: f32 = 24.0;

/// Wo die Checkbox in einem Knoten sitzt
pub fn task_box_rect(node: Rect, zoom: f32) -> Rect {
    let size = 14.0 * zoom;
    Rect::from_center_size(
        Pos2::new(
            node.left() + node.height() * 0.5 + 4.0 * zoom,
            node.center().y,
        ),
        Vec2::splat(size),
    )
}

/// Freistehende Checkbox (Folien, Vorschau): gefüllt mit Haken oder nur Rahmen
pub fn paint_checkbox(
    painter: &egui::Painter,
    r: Rect,
    done: bool,
    color: Color32,
    border: Color32,
) {
    let radius = r.width() * 0.2;
    if done {
        painter.rect_filled(r, radius, color);
        check_mark(painter, r, Color32::WHITE, (r.width() * 0.14).max(1.2));
    } else {
        painter.rect_stroke(
            r,
            radius,
            Stroke::new((r.width() * 0.11).max(1.0), border),
            egui::StrokeKind::Inside,
        );
    }
}

/// Haken in einem Quadrat zeichnen
fn check_mark(painter: &egui::Painter, r: Rect, color: Color32, width: f32) {
    let at = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);
    painter.line_segment([at(0.22, 0.52), at(0.42, 0.72)], Stroke::new(width, color));
    painter.line_segment([at(0.42, 0.72), at(0.78, 0.3)], Stroke::new(width, color));
}

/// Checkbox im Knoten zeichnen; gibt ihr Rechteck zurück (zum Anklicken)
pub fn paint_task(
    painter: &egui::Painter,
    node: Rect,
    style: &NodeStyle,
    done: bool,
    zoom: f32,
) -> Rect {
    let r = task_box_rect(node, zoom);
    let radius = 3.0 * zoom;
    // Auf farbigen Knoten in Textfarbe, auf hellen Karten in der Zweigfarbe
    let (fill, mark) = if style.outlined {
        (style.accent, Color32::WHITE)
    } else {
        (style.text, style.fill)
    };
    if done {
        painter.rect_filled(r, radius, fill);
        check_mark(painter, r, mark, 2.0 * zoom);
    } else {
        painter.rect_stroke(
            r,
            radius,
            Stroke::new(1.6 * zoom, style.text.gamma_multiply(0.85)),
            egui::StrokeKind::Inside,
        );
    }
    r
}

/// Text eines Knotens zeichnen – mit Platz für die Checkbox und durchgestrichen, wenn erledigt
pub fn paint_label(
    painter: &egui::Painter,
    node: Rect,
    galley: std::sync::Arc<egui::Galley>,
    style: &NodeStyle,
    task: Option<bool>,
    zoom: f32,
) {
    let shift = if task.is_some() {
        TASK_SPACE * zoom / 2.0
    } else {
        0.0
    };
    let done = task == Some(true);
    let color = if done {
        style.text.gamma_multiply(0.55)
    } else {
        style.text
    };
    let pos = node.center() + Vec2::new(shift, 0.0) - galley.size() / 2.0;
    let size = galley.size();
    painter.galley_with_override_text_color(pos, galley, color);
    if done {
        let y = pos.y + size.y * 0.55;
        painter.line_segment(
            [
                Pos2::new(pos.x - 1.0, y),
                Pos2::new(pos.x + size.x + 1.0, y),
            ],
            Stroke::new(1.3 * zoom.max(0.6), color),
        );
    }
}
