//! Bilder: einlesen (Datei, Zwischenablage), verkleinern, als Textur zeichnen.
//!
//! Eingefügte Bilder werden auf höchstens `MAX_SIDE` Pixel verkleinert und als
//! JPEG (oder PNG, wenn sie durchsichtige Stellen haben) in der Mindmap
//! gespeichert. So steckt alles in einer Datei, und die Rückgängig-Schritte
//! teilen sich die Daten über `Arc`.

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

/// Längste Seite eines gespeicherten Bildes in Pixeln
const MAX_SIDE: u32 = 1920;
/// Kleinere Originale bleiben unverändert, wenn sie nicht größer als das sind
const KEEP_ORIGINAL_BYTES: usize = 1_500_000;

/// Dateiendungen, die als Bild eingefügt werden können
pub const EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

/// Bilder der Beispiele – fest in die App eingebaut
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

/// Kurzer, lesbarer Schlüssel: Dateiname plus Prüfsumme, z. B. `nebel-3fa2c19d.jpg`
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

/// SVG in ein Pixelbild umwandeln
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

/// Ein Bild aus Dateidaten einlesen und für das Speichern vorbereiten.
/// `name` ist der Dateiname (für Schlüssel und SVG-Erkennung).
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

    // Kleine JPEGs und PNGs unverändert übernehmen
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

/// Bild aus der Zwischenablage (z. B. ein Bildschirmfoto), falls dort eines liegt
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

/// Bild in die Mindmap legen und dem Knoten zuweisen
pub fn attach(map: &mut MindMap, node: u64, (key, img): (String, StoredImage)) {
    map.images
        .entry(key.clone())
        .or_insert_with(|| Arc::new(img));
    if let Some(n) = map.get_mut(node) {
        n.image = Some(key);
    }
}

/// Verweise auf Bilder, die noch nicht in der Mindmap liegen, auflösen:
/// erst die eingebauten Beispielbilder, dann Dateien (relativ zu `base`).
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

/// Pfad aus einem Markdown-Bildverweis (`~/…`, `file://…`, relativ)
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
// Zeichnen
// ---------------------------------------------------------------------------

/// Textur eines Bildes; wird beim ersten Mal dekodiert und dann wiederverwendet
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

/// Wie ein Bild in seinen Rahmen passt
#[derive(Clone, Copy, PartialEq)]
pub enum Fit {
    /// Ganz sichtbar, Rahmen schrumpft auf das Seitenverhältnis
    Contain,
    /// Füllt den Rahmen, Ränder werden abgeschnitten
    Cover,
}

/// Rechteck, das ein Bild mit `aspect` in `frame` bei `Contain` einnimmt
pub fn contain(frame: Rect, aspect: f32) -> Rect {
    let size = if frame.aspect_ratio() > aspect {
        Vec2::new(frame.height() * aspect, frame.height())
    } else {
        Vec2::new(frame.width(), frame.width() / aspect)
    };
    Rect::from_center_size(frame.center(), size)
}

/// Bild zeichnen. `zoom` > 1 vergrößert den Ausschnitt (für den langsamen Zoom).
/// Gibt das tatsächlich bemalte Rechteck zurück.
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
    // Sichtbarer Ausschnitt in Texturkoordinaten
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

/// Grauer Kasten mit Symbol, wenn ein Bild fehlt oder nicht lesbar ist
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
