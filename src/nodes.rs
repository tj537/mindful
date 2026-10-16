//! Look and drawing of the mind map nodes – shared by the
//! mind map canvas (app.rs), the camera tour (tour.rs) and the export.

use crate::icons;
use crate::images::{self, Fit};
use crate::model::{MindMap, Node};
use crate::theme::Theme;
use eframe::egui::{self, text::LayoutJob, Color32, FontId, Pos2, Rect, Stroke, TextFormat, Vec2};

/// Colour of search matches – the same in every theme, so matches stand out
pub const SEARCH: Color32 = Color32::from_rgb(255, 190, 40);

/// Minimum size of a node at zoom 1
pub const MIN_NODE: Vec2 = Vec2::new(90.0, 34.0);

pub struct NodeStyle {
    pub fill: Color32,
    pub text: Color32,
    /// Coloured outline (from level 2 on) and colour of the connecting line
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

/// Label of a node: icon (in the branch colour) and text
pub fn label_job(node: &Node, style: &NodeStyle, size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    if let Some(glyph) = node.icon.as_deref().and_then(icons::glyph) {
        // On coloured nodes in the text colour, on light cards in the branch colour
        let color = if style.outlined {
            style.accent
        } else {
            style.text
        };
        job.append(
            &format!("{glyph}  "),
            0.0,
            TextFormat::simple(FontId::proportional(size * 1.05), color),
        );
    }
    job.append(
        &node.text,
        0.0,
        TextFormat::simple(FontId::proportional(size), style.text),
    );
    job
}

/// Size of a node at zoom 1
pub fn node_size(ctx: &egui::Context, map: &MindMap, theme: &Theme, node: &Node) -> Vec2 {
    let style = node_style(map, theme, node.id);
    let galley = ctx.fonts(|f| f.layout_job(label_job(node, &style, style.font_size)));
    let task = if node.task.is_some() {
        Vec2::new(TASK_SPACE, 0.0)
    } else {
        Vec2::ZERO
    };
    let size = (galley.size() + style.padding + task).max(MIN_NODE);
    match thumb_size(map, node) {
        Some(thumb) => with_thumb(size, thumb),
        None => size,
    }
}

/// Distance of the preview picture from the card edge (at zoom 1)
const THUMB_PAD: f32 = 8.0;

/// Size of a node's preview picture at zoom 1 (`None` without a picture)
pub fn thumb_size(map: &MindMap, node: &Node) -> Option<Vec2> {
    let key = node.image.as_ref()?;
    let height = if node.parent.is_none() { 130.0 } else { 100.0 };
    let aspect = map.images.get(key).map(|img| img.aspect()).unwrap_or(1.3);
    Some(Vec2::new((height * aspect).clamp(80.0, 240.0), height))
}

/// Card size: picture on top, text below
pub fn with_thumb(label: Vec2, thumb: Vec2) -> Vec2 {
    Vec2::new(
        label.x.max(thumb.x + 2.0 * THUMB_PAD),
        label.y + thumb.y + THUMB_PAD,
    )
}

/// Paint the preview picture at the top of the card. Returns the area for text and checkbox
/// (the whole rectangle without a picture).
pub fn paint_thumb(
    painter: &egui::Painter,
    map: &MindMap,
    node: &Node,
    rect: Rect,
    zoom: f32,
) -> Rect {
    let Some(thumb) = thumb_size(map, node) else {
        return rect;
    };
    let pad = THUMB_PAD * zoom;
    let frame = Rect::from_min_max(
        rect.min + Vec2::splat(pad),
        Pos2::new(rect.right() - pad, rect.top() + pad + thumb.y * zoom),
    );
    let radius = 9.0 * zoom;
    match node
        .image
        .as_ref()
        .and_then(|k| map.images.get(k).map(|img| (k, img)))
    {
        Some((key, img)) => {
            images::paint(
                painter,
                painter.ctx(),
                key,
                img,
                frame,
                Fit::Cover,
                radius,
                Color32::WHITE.gamma_multiply(painter.opacity()),
                1.0,
            );
        }
        None => images::placeholder(painter, frame, radius),
    }
    Rect::from_min_max(Pos2::new(rect.left(), frame.bottom()), rect.max)
}

#[allow(clippy::too_many_arguments)]
pub fn paint_node(
    painter: &egui::Painter,
    theme: &Theme,
    rect: Rect,
    style: &NodeStyle,
    zoom: f32,
    selected: bool,
    hit: bool,
    card: bool,
) {
    // With a picture a card with rounded corners, otherwise a pill
    let radius = if card {
        (16.0 * zoom).min(rect.height() / 2.0)
    } else {
        rect.height() / 2.0
    };
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 3.0 * zoom)).expand(zoom),
        radius,
        Color32::from_black_alpha(if theme.dark { 90 } else { 30 }),
    );
    if hit {
        // Soft glow plus a clear ring, still visible when zoomed far out
        let grow = 12.0 * zoom;
        painter.rect_filled(
            rect.expand(grow),
            radius + grow,
            SEARCH.gamma_multiply(0.22),
        );
        let ring = 7.0 * zoom;
        painter.rect_stroke(
            rect.expand(ring),
            radius + ring,
            Stroke::new((3.0 * zoom).max(1.5), SEARCH),
            egui::StrokeKind::Outside,
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

/// Room for the checkbox on the left of the node (at zoom 1)
pub const TASK_SPACE: f32 = 24.0;

/// Where the checkbox sits in a node
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

/// Free-standing checkbox (slides, preview): filled with a tick or just an outline
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

/// Draw a tick in a square
fn check_mark(painter: &egui::Painter, r: Rect, color: Color32, width: f32) {
    let at = |x: f32, y: f32| Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y);
    painter.line_segment([at(0.22, 0.52), at(0.42, 0.72)], Stroke::new(width, color));
    painter.line_segment([at(0.42, 0.72), at(0.78, 0.3)], Stroke::new(width, color));
}

/// Draw the checkbox in the node; returns its rectangle (for clicking)
pub fn paint_task(
    painter: &egui::Painter,
    node: Rect,
    style: &NodeStyle,
    done: bool,
    zoom: f32,
) -> Rect {
    let r = task_box_rect(node, zoom);
    let radius = 3.0 * zoom;
    // On coloured nodes in the text colour, on light cards in the branch colour
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

/// Draw a node's text – with room for the checkbox and struck through when done
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
    if done {
        painter.galley_with_override_text_color(pos, galley, color);
    } else {
        // Keep the colours from the galley (icon in the branch colour)
        painter.galley(pos, galley, color);
    }
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
