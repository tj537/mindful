use crate::theme::Theme;
use eframe::egui::{Color32, Pos2, Rect, Vec2};
use std::fmt::Write;
use std::path::Path;

/// Ein Knoten so, wie er exportiert wird (Weltkoordinaten, Zoom 1).
pub struct ExportNode {
    pub center: Pos2,
    pub size: Vec2,
    pub parent: Option<Pos2>,
    pub fill: Color32,
    pub text_color: Color32,
    pub accent: Color32,
    pub outlined: bool,
    pub font_size: f32,
    pub edge_width: f32,
    pub text: String,
}

const FONT: &str = "Helvetica Neue, Helvetica, Arial, sans-serif";

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn rect(s: &mut String, r: Rect, radius: f32, fill: &str, opacity: f32) {
    let _ = writeln!(
        s,
        r#"  <rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}" fill="{fill}" fill-opacity="{opacity}"/>"#,
        r.min.x,
        r.min.y,
        r.width(),
        r.height(),
        radius
    );
}

pub fn to_svg(nodes: &[ExportNode], theme: &Theme) -> String {
    let mut bounds = Rect::NOTHING;
    for n in nodes {
        bounds = bounds.union(Rect::from_center_size(n.center, n.size));
    }
    if !bounds.is_positive() {
        bounds = Rect::from_center_size(Pos2::ZERO, Vec2::splat(100.0));
    }
    let b = bounds.expand(40.0);
    let (w, h) = (b.width().ceil(), b.height().ceil());

    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{:.1} {:.1} {w} {h}">"#,
        b.min.x, b.min.y
    );
    rect(
        &mut s,
        Rect::from_min_size(b.min, Vec2::new(w, h)),
        0.0,
        &hex(theme.background),
        1.0,
    );

    // Verbindungslinien
    for n in nodes {
        let Some(a) = n.parent else { continue };
        let e = n.center;
        let dx = (e.x - a.x) * 0.5;
        let _ = writeln!(
            s,
            r#"  <path d="M{:.1} {:.1} C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1}" fill="none" stroke="{}" stroke-opacity="0.8" stroke-width="{:.1}" stroke-linecap="round"/>"#,
            a.x,
            a.y,
            a.x + dx,
            a.y,
            e.x - dx,
            e.y,
            e.x,
            e.y,
            hex(n.accent),
            n.edge_width
        );
    }

    // Knoten
    let shadow = if theme.dark { 0.35 } else { 0.12 };
    for n in nodes {
        let r = Rect::from_center_size(n.center, n.size);
        let radius = r.height() / 2.0;
        rect(
            &mut s,
            r.translate(Vec2::new(0.0, 3.0)).expand(1.0),
            radius,
            "#000000",
            shadow,
        );
        if n.outlined {
            rect(&mut s, r.expand(2.0), radius + 2.0, &hex(n.accent), 1.0);
        }
        rect(&mut s, r, radius, &hex(n.fill), 1.0);
        let _ = writeln!(
            s,
            r#"  <text x="{:.1}" y="{:.1}" font-family="{FONT}" font-size="{:.1}" text-anchor="middle" fill="{}">{}</text>"#,
            n.center.x,
            n.center.y + n.font_size * 0.35,
            n.font_size,
            hex(n.text_color),
            escape(&n.text)
        );
    }

    s.push_str("</svg>\n");
    s
}

/// Rendert das SVG in doppelter Auflösung als PNG.
pub fn svg_to_png(svg: &str, path: &Path) -> Result<(), String> {
    use resvg::{tiny_skia, usvg};
    let scale = 2.0;

    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree
        .size()
        .to_int_size()
        .scale_by(scale)
        .ok_or_else(|| "Bild ist zu groß".to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| "Bild ist zu groß".to_string())?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::THEMES;

    #[test]
    fn svg_and_png_export() {
        let nodes = vec![
            ExportNode {
                center: Pos2::ZERO,
                size: Vec2::new(160.0, 50.0),
                parent: None,
                fill: Color32::BLUE,
                text_color: Color32::WHITE,
                accent: Color32::BLUE,
                outlined: false,
                font_size: 20.0,
                edge_width: 3.0,
                text: "Mitte & <Test>".into(),
            },
            ExportNode {
                center: Pos2::new(250.0, 40.0),
                size: Vec2::new(120.0, 34.0),
                parent: Some(Pos2::ZERO),
                fill: Color32::WHITE,
                text_color: Color32::BLACK,
                accent: Color32::RED,
                outlined: true,
                font_size: 14.0,
                edge_width: 2.0,
                text: "Zweig".into(),
            },
        ];
        let svg = to_svg(&nodes, &THEMES[0]);
        assert!(svg.contains("Mitte &amp; &lt;Test&gt;"));
        let path = std::env::temp_dir().join("mindmap_export_test.png");
        svg_to_png(&svg, &path).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 1000);
    }
}
