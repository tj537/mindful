//! Folien als PDF: jede Folie wird ohne Fenster gezeichnet (`offscreen.rs`),
//! als JPEG kodiert und als eigene Seite in eine PDF-Datei geschrieben.
//! Ein kleiner eigener PDF-Schreiber reicht dafür – Seiten aus Bildern
//! brauchen nur wenige PDF-Objekte.

use crate::model::MindMap;
use crate::offscreen::Offscreen;
use crate::present::{self, Effects};
use crate::theme::{apply_spacing, install_fonts, visuals_for, Theme};
use eframe::egui;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Folie in Punkten (1280 × 720) und Pixel pro Punkt → 1920 × 1080 Bilder
const SLIDE: egui::Vec2 = egui::Vec2::new(1280.0, 720.0);
const SCALE: f32 = 1.5;
/// Seitengröße im PDF in pt (13,33 × 7,5 Zoll, wie PowerPoint im Breitformat)
const PAGE: (f32, f32) = (960.0, 540.0);

/// Alle Folien rendern und als PDF speichern. `done` zählt die fertigen Folien
/// (für die Fortschrittsanzeige); läuft in einem eigenen Thread.
pub fn export(
    map: &MindMap,
    theme: &Theme,
    fx: &Effects,
    path: &Path,
    done: &AtomicUsize,
) -> Result<(), String> {
    let mut fx = *fx;
    fx.ken_burns = false;
    let mut screen = Offscreen::retina(SLIDE.x, SLIDE.y, SCALE);
    install_fonts(&screen.ctx);
    screen.ctx.set_visuals(visuals_for(theme));
    screen.ctx.style_mut(apply_spacing);

    let count = present::slide_count(map, theme);
    let mut pages = Vec::with_capacity(count);
    for index in 0..count {
        let mut shapes = Vec::new();
        // Zwei Frames: im ersten werden Schrift und Bilder geladen
        for t in [index as f64 * 10.0, index as f64 * 10.0 + 5.0] {
            shapes = screen.frame(t, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme.card))
                    .show(ctx, |ui| present::draw_static(ui, map, theme, &fx, index));
            });
        }
        let (w, h, rgba) = screen.rgba(&shapes);
        let rgb: Vec<u8> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 88)
            .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
            .map_err(|e| e.to_string())?;
        pages.push((w, h, jpeg));
        done.store(index + 1, Ordering::Relaxed);
    }
    let title = map
        .root()
        .and_then(|r| map.get(r))
        .map(|n| n.text.clone())
        .unwrap_or_default();
    let bytes = write_pdf(&title, &pages);
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Text für eine PDF-Zeichenkette maskieren (nur ASCII, Rest wird zu „?“)
fn pdf_string(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '(' | ')' | '\\' => format!("\\{c}"),
            c if c.is_ascii() && !c.is_ascii_control() => c.to_string(),
            _ => "?".into(),
        })
        .collect()
}

/// Minimales PDF: Katalog, Seitenbaum, je Seite ein Bild und ein Zeichenbefehl
fn write_pdf(title: &str, pages: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    // Objektnummern: 1 Katalog, 2 Seitenbaum, 3 Info, dann je Seite 3 Objekte
    let page_id = |i: usize| 4 + i * 3;
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        let id = offsets.len();
        writeln!(out, "{id} 0 obj").unwrap();
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", page_id(i)))
        .collect();
    object(
        &mut out,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    object(
        &mut out,
        format!("<< /Title ({}) /Producer (Mindful) >>", pdf_string(title)).as_bytes(),
    );
    let (pw, ph) = PAGE;
    for (i, (w, h, jpeg)) in pages.iter().enumerate() {
        let id = page_id(i);
        object(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw} {ph}] \
                 /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>",
                id + 2,
                id + 1
            )
            .as_bytes(),
        );
        let draw = format!("q {pw} 0 0 {ph} 0 0 cm /Im0 Do Q");
        object(
            &mut out,
            format!("<< /Length {} >>\nstream\n{draw}\nendstream", draw.len()).as_bytes(),
        );
        let mut image = format!(
            "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB \
             /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpeg.len()
        )
        .into_bytes();
        image.extend_from_slice(jpeg);
        image.extend_from_slice(b"\nendstream");
        object(&mut out, &image);
    }
    let xref = out.len();
    write!(out, "xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).unwrap();
    for offset in &offsets {
        writeln!(out, "{offset:010} 00000 n ").unwrap();
    }
    write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        offsets.len() + 1
    )
    .unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::THEMES;

    #[test]
    fn slides_become_pdf_pages() {
        let map = MindMap::from_outline(
            "# Test\n\n## One\n\n- A\n- B\n\n## Two <!-- bars -->\n\n- X: 3\n- Y: 5\n",
        )
        .unwrap();
        let path = std::env::temp_dir().join("mindful-test.pdf");
        let done = AtomicUsize::new(0);
        export(&map, &THEMES[1], &Effects::default(), &path, &done).unwrap();
        assert_eq!(done.load(Ordering::Relaxed), 3);
        let pdf = std::fs::read(&path).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Count 3"));
        assert_eq!(text.matches("/Subtype /Image").count(), 3);
        // Verweise in der xref-Tabelle zeigen auf die Objekte
        let xref = pdf.windows(6).position(|w| w == b"\nxref\n").unwrap() + 1;
        let table = String::from_utf8_lossy(&pdf[xref..]);
        let first = table.lines().nth(3).unwrap();
        let offset: usize = first[..10].parse().unwrap();
        assert!(pdf[offset..].starts_with(b"1 0 obj"));
    }
}
