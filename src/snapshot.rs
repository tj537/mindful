//! Nur für Tests: Bildschirmfotos von Folien und vom ganzen Fenster.
//!
//!     cargo test --release render_ -- --ignored    → target/snapshots/*.png

#[cfg(test)]
mod tests {
    use crate::document::{Document, Mode};
    use crate::model::MindMap;
    use crate::offscreen::Offscreen as Snapshot;
    use crate::present::{self, Effects};
    use crate::theme::{apply_spacing, install_fonts, visuals_for, THEMES};
    use eframe::egui::{self, Vec2};
    use std::path::Path;

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

        // Referentenansicht (ohne echtes zweites Fenster als eingebettetes Fenster)
        if name == "universe" {
            let mut wide = Snapshot::retina(1500.0, 900.0, 1.0);
            install_fonts(&wide.ctx);
            wide.ctx.set_visuals(visuals_for(theme));
            wide.ctx.style_mut(apply_spacing);
            doc.presenter = true;
            doc.slide = 9;
            doc.build = 1;
            doc.slide_time = 2000.0;
            doc.presenter_start = Some(2000.0 - 754.0);
            let mut shapes = Vec::new();
            for t in [2000.0, 2001.0, 2010.0] {
                shapes = wide.frame(t, |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::default().fill(theme.background))
                        .show(ctx, |ui| present::show(ui, &mut doc, theme, true, &mut fx));
                });
            }
            wide.save(&shapes, Path::new("target/snapshots/presenter.png"));
            // Nur die Referentenansicht, wie im zweiten Fenster
            let mut window = Snapshot::retina(1100.0, 680.0, 1.5);
            install_fonts(&window.ctx);
            window.ctx.set_visuals(visuals_for(theme));
            window.ctx.style_mut(apply_spacing);
            for t in [2000.0, 2001.0, 2010.0] {
                shapes = window.frame(t, |ctx| {
                    egui::CentralPanel::default()
                        .frame(
                            egui::Frame::default()
                                .fill(theme.background)
                                .inner_margin(egui::Margin::same(18)),
                        )
                        .show(ctx, |ui| {
                            present::presenter_snapshot(ui, &mut doc, theme, &fx)
                        });
                });
            }
            window.save(&shapes, Path::new("target/snapshots/presenter_window.png"));
            doc.presenter = false;
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
