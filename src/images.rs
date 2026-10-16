//! Pictures: reading them in (file, clipboard), scaling them down, drawing them as textures.
//!
//! Inserted pictures are scaled down to at most `MAX_SIDE` pixels and stored in the
//! mind map as JPEG (or PNG if they have transparent areas).
//! That way everything lives in one file, and the undo steps
//! share the data via `Arc`.

use crate::model::{MindMap, StoredImage};
use eframe::egui::{
    self, epaint::RectShape, Color32, ColorImage, Context, CornerRadius, Pos2, Rect, Shape,
    TextureHandle, TextureOptions, Vec2,
};
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Longest side of a stored picture in pixels
const MAX_SIDE: u32 = 1920;
/// Smaller originals stay unchanged if they are no bigger than this
const KEEP_ORIGINAL_BYTES: usize = 1_500_000;

/// File extensions that can be inserted as a picture
pub const EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

/// Pictures of the examples – built into the app
const BUILTIN: [(&str, &[u8]); 5] = [
    (
        "black-hole.jpg",
        include_bytes!("../assets/examples/images/black-hole.jpg"),
    ),
    (
        "nebula.jpg",
        include_bytes!("../assets/examples/images/nebula.jpg"),
    ),
    (
        "rocky-planet.jpg",
        include_bytes!("../assets/examples/images/rocky-planet.jpg"),
    ),
    (
        "gas-giant.jpg",
        include_bytes!("../assets/examples/images/gas-giant.jpg"),
    ),
    (
        "ringed-planet.jpg",
        include_bytes!("../assets/examples/images/ringed-planet.jpg"),
    ),
];

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Short, readable key: file name plus checksum, e.g. `nebel-3fa2c19d.jpg`
fn key_for(name: &str, data: &[u8], ext: &str) -> String {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    let stem: String = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let stem = stem.trim_matches('-');
    let stem: String = if stem.is_empty() { "image" } else { stem }
        .chars()
        .take(24)
        .collect();
    format!("{stem}-{:08x}.{ext}", hasher.finish() as u32)
}

/// Turn an SVG into a pixel image
fn rasterize_svg(bytes: &[u8]) -> Result<DynamicImage, String> {
    let tree = resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default())
        .map_err(|e| e.to_string())?;
    let size = tree.size();
    let scale = (1600.0 / size.width().max(size.height())).min(4.0);
    let (w, h) = (
        (size.width() * scale).ceil() as u32,
        (size.height() * scale).ceil() as u32,
    );
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("SVG is empty")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let mut rgba = image::RgbaImage::new(pixmap.width(), pixmap.height());
    for (out, px) in rgba.pixels_mut().zip(pixmap.pixels()) {
        let c = px.demultiply();
        *out = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(DynamicImage::ImageRgba8(rgba))
}

/// Read a picture from file data and prepare it for saving.
/// `name` is the file name (for the key and for detecting SVG).
pub fn import(bytes: &[u8], name: &str) -> Result<(String, StoredImage), String> {
    let is_svg = name.to_lowercase().ends_with(".svg")
        || bytes.trim_ascii_start().starts_with(b"<svg")
        || bytes.trim_ascii_start().starts_with(b"<?xml");
    let format = image::guess_format(bytes).ok();
    let img = if is_svg {
        rasterize_svg(bytes)?
    } else {
        image::load_from_memory(bytes).map_err(|e| format!("Not a supported image ({e})"))?
    };
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("The image is empty".into());
    }

    // Take small JPEGs and PNGs over unchanged
    let small = w.max(h) <= MAX_SIDE && bytes.len() <= KEEP_ORIGINAL_BYTES;
    match format {
        Some(ImageFormat::Jpeg) if small && !is_svg => {
            return Ok((key_for(name, bytes, "jpg"), stored(w, h, bytes.to_vec())));
        }
        Some(ImageFormat::Png) if small && !is_svg => {
            return Ok((key_for(name, bytes, "png"), stored(w, h, bytes.to_vec())));
        }
        _ => {}
    }

    let img = if w.max(h) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let (w, h) = img.dimensions();
    let transparent = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 250);
    let mut data = Vec::new();
    let ext = if transparent {
        img.write_to(&mut Cursor::new(&mut data), ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        "png"
    } else {
        let rgb = img.to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 85)
            .encode_image(&rgb)
            .map_err(|e| e.to_string())?;
        "jpg"
    };
    Ok((key_for(name, &data, ext), stored(w, h, data)))
}

fn stored(width: u32, height: u32, data: Vec<u8>) -> StoredImage {
    StoredImage {
        width,
        height,
        data,
    }
}

pub fn import_file(path: &Path) -> Result<(String, StoredImage), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
    import(&bytes, name)
}

/// Picture from the clipboard (e.g. a screenshot), if there is one
pub fn from_clipboard() -> Option<(String, StoredImage)> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let data = clipboard.get_image().ok()?;
    let rgba = image::RgbaImage::from_raw(
        data.width as u32,
        data.height as u32,
        data.bytes.into_owned(),
    )?;
    let mut png = Vec::new();
    DynamicImage::ImageRgba8(rgba)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    import(&png, "pasted.png").ok()
}

/// Put a picture into the mind map and assign it to the node
pub fn attach(map: &mut MindMap, node: u64, (key, img): (String, StoredImage)) {
    map.images
        .entry(key.clone())
        .or_insert_with(|| Arc::new(img));
    if let Some(n) = map.get_mut(node) {
        n.image = Some(key);
    }
}

/// Resolve references to pictures that aren't in the mind map yet:
/// first the built-in example pictures, then files (relative to `base`).
pub fn resolve(map: &mut MindMap, base: Option<&Path>) {
    let missing: Vec<(u64, String)> = map
        .nodes
        .iter()
        .filter_map(|n| Some((n.id, n.image.clone()?)))
        .filter(|(_, key)| !map.images.contains_key(key))
        .collect();
    for (id, key) in missing {
        if let Some((_, bytes)) = BUILTIN.iter().find(|(name, _)| *name == key) {
            if let Ok((_, img)) = import(bytes, &key) {
                map.images.insert(key.clone(), Arc::new(img));
            }
            continue;
        }
        if let Some(path) = file_path(&key, base) {
            if let Ok(found) = import_file(&path) {
                attach(map, id, found);
            }
        }
    }
}

/// Path from a Markdown image reference (`~/…`, `file://…`, relative)
fn file_path(target: &str, base: Option<&Path>) -> Option<PathBuf> {
    if target.starts_with("http://") || target.starts_with("https://") {
        return None;
    }
    let target = target.strip_prefix("file://").unwrap_or(target);
    let target = target.replace("%20", " ");
    let path = match target.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(PathBuf::from)?.join(rest),
        None => PathBuf::from(&target),
    };
    let path = if path.is_relative() {
        base?.join(path)
    } else {
        path
    };
    path.is_file().then_some(path)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Texture of a picture; decoded the first time and reused afterwards
pub fn texture(ctx: &Context, key: &str, img: &StoredImage) -> Option<TextureHandle> {
    let id = egui::Id::new(("mindful-image", key));
    if let Some(cached) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) {
        return cached;
    }
    let handle = image::load_from_memory(&img.data).ok().map(|decoded| {
        let rgba = decoded.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let options = TextureOptions {
            mipmap_mode: Some(egui::TextureFilter::Linear),
            ..TextureOptions::LINEAR
        };
        ctx.load_texture(key, color, options)
    });
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    handle
}

/// How a picture fits into its frame
#[derive(Clone, Copy, PartialEq)]
pub enum Fit {
    /// Fully visible, the frame shrinks to the aspect ratio
    Contain,
    /// Fills the frame, the edges are cut off
    Cover,
}

/// Rectangle a picture with `aspect` takes up in `frame` with `Contain`
pub fn contain(frame: Rect, aspect: f32) -> Rect {
    let size = if frame.aspect_ratio() > aspect {
        Vec2::new(frame.height() * aspect, frame.height())
    } else {
        Vec2::new(frame.width(), frame.width() / aspect)
    };
    Rect::from_center_size(frame.center(), size)
}

/// Draw a picture. `zoom` > 1 enlarges the visible part (for the slow zoom).
/// Returns the rectangle actually painted.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    p: &egui::Painter,
    ctx: &Context,
    key: &str,
    img: &StoredImage,
    frame: Rect,
    fit: Fit,
    radius: f32,
    tint: Color32,
    zoom: f32,
) -> Rect {
    let aspect = img.aspect();
    let rect = match fit {
        Fit::Contain => contain(frame, aspect),
        Fit::Cover => frame,
    };
    let Some(tex) = texture(ctx, key, img) else {
        placeholder(p, rect, radius);
        return rect;
    };
    // Visible part in texture coordinates
    let mut uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    if fit == Fit::Cover {
        let frame_aspect = rect.aspect_ratio();
        if frame_aspect > aspect {
            let h = aspect / frame_aspect;
            uv = Rect::from_center_size(Pos2::new(0.5, 0.5), Vec2::new(1.0, h));
        } else {
            let w = frame_aspect / aspect;
            uv = Rect::from_center_size(Pos2::new(0.5, 0.5), Vec2::new(w, 1.0));
        }
    }
    if zoom > 1.0 {
        uv = Rect::from_center_size(uv.center(), uv.size() / zoom);
    }
    p.add(Shape::Rect(
        RectShape::filled(rect, CornerRadius::same(radius.round() as u8), tint)
            .with_texture(tex.id(), uv),
    ));
    rect
}

/// Grey box with an icon when a picture is missing or unreadable
pub fn placeholder(p: &egui::Painter, rect: Rect, radius: f32) {
    let color = Color32::from_gray(128);
    p.rect_filled(
        rect,
        CornerRadius::same(radius.round() as u8),
        color.gamma_multiply(0.2),
    );
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        egui_phosphor::regular::IMAGE_BROKEN,
        egui::FontId::proportional((rect.height() * 0.3).clamp(12.0, 64.0)),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_images_are_scaled_and_stored_as_jpeg() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4000,
            2000,
            image::Rgb([20, 40, 200]),
        ));
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let (key, stored) = import(&png, "My Photo.png").unwrap();
        assert_eq!((stored.width, stored.height), (1920, 960));
        assert!(
            key.starts_with("my-photo-") && key.ends_with(".jpg"),
            "{key}"
        );
        assert_eq!(
            image::guess_format(&stored.data).unwrap(),
            ImageFormat::Jpeg
        );
    }

    #[test]
    fn builtin_example_images_resolve() {
        let mut map = MindMap::from_outline("# A\n\n## B\n\n![](nebula.jpg)\n").unwrap();
        resolve(&mut map, None);
        let b = map.children(map.root().unwrap()).next().unwrap().id;
        let (key, img) = map.image_of(b).unwrap();
        assert_eq!(key, "nebula.jpg");
        assert!(img.width > 0);
    }
}
