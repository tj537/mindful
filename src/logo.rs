//! The Mindful mark (circle with an M, branches, brackets) for the user interface.
//!
//! Its source is the group `id="logo"` in `assets/icon.svg` – the same file the app icon
//! is made from. So the Dock/taskbar and the toolbar always look the same.

use eframe::egui::{self, Color32, ColorImage, Context, Pos2, Rect, TextureHandle, Vec2};
use resvg::{tiny_skia, usvg};

const SVG: &str = include_str!("../assets/icon.svg");

/// The mark as a white mask of `px` × `px` pixels (square, centred)
fn render(px: u32) -> Option<ColorImage> {
    let tree = usvg::Tree::from_str(SVG, &usvg::Options::default()).ok()?;
    let node = tree.node_by_id("logo")?;
    let bbox = node.abs_layer_bounding_box()?;
    let s = px as f32 / bbox.width().max(bbox.height());
    let offset = (
        (px as f32 - bbox.width() * s) / 2.0,
        (px as f32 - bbox.height() * s) / 2.0,
    );
    let mut pixmap = tiny_skia::Pixmap::new(px, px)?;
    let transform = tiny_skia::Transform::from_translate(offset.0, offset.1).pre_scale(s, s);
    resvg::render_node(node, transform, &mut pixmap.as_mut())?;
    // Only the opacity counts: make it white, the colour is added when drawing
    let mut rgba = pixmap.take();
    for p in rgba.chunks_exact_mut(4) {
        p[0] = p[3];
        p[1] = p[3];
        p[2] = p[3];
    }
    Some(ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        &rgba,
    ))
}

/// Texture in the matching pixel size; rendered once and then reused
fn texture(ctx: &Context, px: u32) -> Option<TextureHandle> {
    let id = egui::Id::new(("mindful-logo", px));
    if let Some(cached) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) {
        return cached;
    }
    let handle = render(px).map(|img| ctx.load_texture("mindful-logo", img, Default::default()));
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    handle
}

/// Draws the mark as a square around `center` in the colour `color`
pub fn paint(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    let px = (size * painter.ctx().pixels_per_point()).round().max(1.0) as u32;
    if let Some(tex) = texture(painter.ctx(), px) {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(tex.id(), rect, uv, color);
    }
}

/// The mark as an item in a row (e.g. in the menu bar)
pub fn show(ui: &mut egui::Ui, size: f32, color: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint(ui.painter(), rect.center(), size, color);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_is_found_in_the_app_icon() {
        let img = render(64).expect("icon.svg needs a group with id=\"logo\"");
        assert_eq!(img.size, [64, 64]);
        // Something is drawn, but not the whole area (the background is not part of it)
        let opaque = img.pixels.iter().filter(|p| p.a() > 128).count();
        assert!(opaque > 64 * 64 / 10, "logo is almost empty: {opaque}");
        assert!(
            opaque < 64 * 64 / 2,
            "logo includes the background: {opaque}"
        );
    }
}
