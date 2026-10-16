//! Nur für Tests: zeichnet egui-Oberflächen ohne Fenster in PNG-Dateien.
//!
//! egui liefert pro Frame Dreiecke mit Farben und Texturkoordinaten. Ein
//! kleiner Software-Rasterizer malt sie in ein Bild – so lassen sich Folien
//! und Diagramme ansehen, ohne die App zu starten.
//!
//!     cargo test render_slides -- --ignored    → target/snapshots/*.png

use eframe::egui::{
    self,
    epaint::{ClippedPrimitive, ImageData, Primitive, TextureId},
    Color32, Pos2, Rect, Vec2,
};
use std::collections::HashMap;
use std::path::Path;

struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
}

pub struct Snapshot {
    pub ctx: egui::Context,
    textures: HashMap<TextureId, Texture>,
    size: Vec2,
    /// Pixel pro Punkt (2 = Retina)
    scale: f32,
}

impl Snapshot {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            ctx: egui::Context::default(),
            textures: HashMap::new(),
            size: Vec2::new(width, height),
            scale: 1.0,
        }
    }

    /// Wie `new`, aber mit `scale` Pixeln pro Punkt gerastert (schärfer)
    pub fn retina(width: f32, height: f32, scale: f32) -> Self {
        Self {
            scale,
            ..Self::new(width, height)
        }
    }

    /// Einen Frame laufen lassen und die fertigen Dreiecke zurückgeben
    pub fn frame(
        &mut self,
        time: f64,
        run_ui: impl FnMut(&egui::Context),
    ) -> Vec<ClippedPrimitive> {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(time),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale);
        let output = self.ctx.run(input, run_ui);
        for (id, delta) in output.textures_delta.set {
            let (width, height, pixels) = match &delta.image {
                ImageData::Color(image) => (image.size[0], image.size[1], image.pixels.clone()),
                ImageData::Font(font) => (
                    font.size[0],
                    font.size[1],
                    font.srgba_pixels(None).collect(),
                ),
            };
            match delta.pos {
                None => {
                    self.textures.insert(
                        id,
                        Texture {
                            width,
                            height,
                            pixels,
                        },
                    );
                }
                Some([x0, y0]) => {
                    if let Some(texture) = self.textures.get_mut(&id) {
                        for y in 0..height {
                            for x in 0..width {
                                texture.pixels[(y0 + y) * texture.width + x0 + x] =
                                    pixels[y * width + x];
                            }
                        }
                    }
                }
            }
        }
        self.ctx.tessellate(output.shapes, output.pixels_per_point)
    }

    /// Dreiecke rastern und als PNG speichern
    pub fn save(&self, primitives: &[ClippedPrimitive], path: &Path) {
        let scale = self.scale;
        let (w, h) = (
            (self.size.x * scale) as usize,
            (self.size.y * scale) as usize,
        );
        // Vormultiplizierte Farben 0…1, wie egui sie liefert
        let mut buffer = vec![[0.0_f32; 4]; w * h];
        let rgba = |c: Color32| c.to_array().map(|v| v as f32 / 255.0);
        let edge =
            |a: Pos2, b: Pos2, p: Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);

        for primitive in primitives {
            let Primitive::Mesh(mesh) = &primitive.primitive else {
                continue;
            };
            let clip = Rect::from_min_max(
                primitive.clip_rect.min * scale,
                primitive.clip_rect.max * scale,
            )
            .intersect(Rect::from_min_size(Pos2::ZERO, self.size * scale));
            let texture = self.textures.get(&mesh.texture_id);
            for tri in mesh.indices.as_chunks::<3>().0 {
                let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| {
                    let mut v = mesh.vertices[i as usize];
                    v.pos = (v.pos.to_vec2() * scale).to_pos2();
                    v
                });
                let area = edge(a.pos, b.pos, c.pos);
                if area.abs() < 1e-6 {
                    continue;
                }
                let min = a.pos.min(b.pos).min(c.pos).max(clip.min);
                let max = a.pos.max(b.pos).max(c.pos).min(clip.max);
                if min.x >= max.x || min.y >= max.y {
                    continue;
                }
                let (ca, cb, cc) = (rgba(a.color), rgba(b.color), rgba(c.color));
                for py in min.y.floor() as usize..(max.y.ceil() as usize).min(h) {
                    for px in min.x.floor() as usize..(max.x.ceil() as usize).min(w) {
                        let p = Pos2::new(px as f32 + 0.5, py as f32 + 0.5);
                        let w0 = edge(b.pos, c.pos, p) / area;
                        let w1 = edge(c.pos, a.pos, p) / area;
                        let w2 = edge(a.pos, b.pos, p) / area;
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let uv = Pos2::new(
                            a.uv.x * w0 + b.uv.x * w1 + c.uv.x * w2,
                            a.uv.y * w0 + b.uv.y * w1 + c.uv.y * w2,
                        );
                        let texel = texture
                            .map(|t| {
                                let x = ((uv.x * t.width as f32) as usize).min(t.width - 1);
                                let y = ((uv.y * t.height as f32) as usize).min(t.height - 1);
                                rgba(t.pixels[y * t.width + x])
                            })
                            .unwrap_or([1.0; 4]);
                        let mut src = [0.0; 4];
                        for k in 0..4 {
                            src[k] = (ca[k] * w0 + cb[k] * w1 + cc[k] * w2) * texel[k];
                        }
                        let dst = &mut buffer[py * w + px];
                        for k in 0..4 {
                            dst[k] = src[k] + dst[k] * (1.0 - src[3]);
                        }
                    }
                }
            }
        }

        let mut pixmap = resvg::tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
        for (out, px) in pixmap
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&buffer)
        {
            for k in 0..4 {
                out[k] = (px[k].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap();
        }
        pixmap.save_png(path).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Document, Mode};
    use crate::model::MindMap;
    use crate::present::{self, Effects};
    use crate::theme::{apply_spacing, install_fonts, visuals_for, THEMES};

    /// Alle Folien der beiden Beispiele rendern (darin kommt jedes Layout vor)
    #[test]
    #[ignore]
    fn render_slides() {
        for (name, md) in [
            ("universe", include_str!("../assets/examples/universe.md")),
            ("weltall", include_str!("../assets/examples/weltall.md")),
        ] {
            let mut map = MindMap::from_outline(md).unwrap();
            crate::images::resolve(&mut map, None);
            render(name, map);
        }
    }

    fn render(name: &str, map: MindMap) {
        let root = map.root().unwrap();
        let theme = &THEMES[1];
        let mut fx = Effects::default();
        let mut doc = Document::new(map, None);
        doc.set_mode(Mode::Present);
        let mut snap = Snapshot::new(1280.0, 760.0);
        install_fonts(&snap.ctx);
        snap.ctx.set_visuals(visuals_for(theme));
        snap.ctx.style_mut(apply_spacing);

        let slides = 1 + doc.map.children(root).count();
        for slide in 0..slides {
            let start = slide as f64 * 100.0;
            doc.slide = slide;
            doc.slide_time = start;
            doc.build = 0;
            let mut shapes = Vec::new();
            for t in [start, start + 20.0] {
                shapes = snap.frame(t, |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::default().fill(theme.background))
                        .show(ctx, |ui| present::show(ui, &mut doc, theme, false, &mut fx));
                });
            }
            snap.save(
                &shapes,
                Path::new(&format!("target/snapshots/{name}_{slide:02}.png")),
            );
        }

        // Kamerafahrt, Übersicht: zeigt die Knoten samt Vorschaubildern
        let sizes = doc
            .map
            .nodes
            .iter()
            .map(|n| (n.id, crate::nodes::node_size(&snap.ctx, &doc.map, theme, n)))
            .collect();
        doc.map.auto_layout(&sizes);
        doc.set_mode(Mode::Tour);
        let mut shapes = Vec::new();
        for t in [1000.0, 1001.0, 1030.0] {
            shapes = snap.frame(t, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme.background))
                    .show(ctx, |ui| crate::tour::show(ui, &mut doc, theme, &mut fx));
            });
        }
        snap.save(
            &shapes,
            Path::new(&format!("target/snapshots/{name}_tour.png")),
        );
    }

    /// Das ganze Fenster in allen Ansichten und einigen Farbschemas rendern
    #[test]
    #[ignore]
    fn render_app() {
        use crate::app::MindMapApp;
        let mut snap = Snapshot::retina(1400.0, 860.0, 2.0);
        let mut app = MindMapApp::for_snapshot(&snap.ctx);
        for (name, mode) in [("mindmap", Mode::Mindmap), ("markdown", Mode::Markdown)] {
            app.set_mode_for_snapshot(mode);
            let mut shapes = Vec::new();
            for t in [0.0, 0.5, 5.0, 10.0] {
                shapes = snap.frame(t, |ctx| app.show(ctx));
            }
            snap.save(
                &shapes,
                Path::new(&format!("target/snapshots/app_{name}.png")),
            );
        }
        // Helles Farbschema und eine leere Mindmap mit Tipp-Karte
        app.set_mode_for_snapshot(Mode::Mindmap);
        app.set_theme_for_snapshot(0);
        app.new_tab_for_snapshot();
        let mut shapes = Vec::new();
        for t in [20.0, 20.5, 25.0] {
            shapes = snap.frame(t, |ctx| app.show(ctx));
        }
        snap.save(&shapes, Path::new("target/snapshots/app_light_empty.png"));

        // „AI prompt“-Fenster
        app.set_theme_for_snapshot(1);
        snap.size = Vec2::new(1400.0, 860.0);
        app.open_ai_popup = true;
        let mut shapes = Vec::new();
        for t in [26.0, 26.5, 27.0] {
            shapes = snap.frame(t, |ctx| app.show(ctx));
        }
        snap.save(&shapes, Path::new("target/snapshots/app_ai_prompt.png"));
        // schmal: kompakter Knopf
        snap.ctx.memory_mut(|m| m.close_popup());
        snap.size = Vec2::new(1000.0, 200.0);
        for t in [28.0, 28.5] {
            shapes = snap.frame(t, |ctx| app.show(ctx));
        }
        snap.save(&shapes, Path::new("target/snapshots/app_ai_compact.png"));

        // Schmale Fenster: nichts darf sich überlappen
        app.set_theme_for_snapshot(1);
        for width in [1180.0, 1050.0, 940.0, 880.0, 800.0, 700.0, 600.0] {
            snap.size = Vec2::new(width, 240.0);
            let mut shapes = Vec::new();
            for t in [30.0, 30.5, 31.0] {
                shapes = snap.frame(t, |ctx| app.show(ctx));
            }
            snap.save(
                &shapes,
                Path::new(&format!("target/snapshots/app_width_{width}.png")),
            );
        }
    }
}
