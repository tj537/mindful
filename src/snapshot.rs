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

        // Titelfolie ohne Themen-Marken
        fx.title_topics = false;
        doc.slide = 0;
        doc.slide_time = 3000.0;
        let mut shapes = Vec::new();
        for t in [3000.0, 3020.0] {
            shapes = snap.frame(t, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme.background))
                    .show(ctx, |ui| present::show(ui, &mut doc, theme, false, &mut fx));
            });
        }
        snap.save(
            &shapes,
            Path::new(&format!("target/snapshots/{name}_title_plain.png")),
        );
        fx.title_topics = true;

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
            // QR-Code für die Handy-Fernbedienung
            doc.presenter = false;
            doc.remote_open = true;
            for t in [2020.0, 2021.0, 2022.0] {
                shapes = wide.frame(t, |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::default().fill(theme.background))
                        .show(ctx, |ui| present::show(ui, &mut doc, theme, true, &mut fx));
                });
            }
            wide.save(&shapes, Path::new("target/snapshots/remote.png"));
            doc.remote_open = false;
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
        // Einstellungen
        snap.ctx.memory_mut(|m| m.close_popup());
        app.open_settings_popup = true;
        for t in [27.0, 27.5, 27.8] {
            shapes = snap.frame(t, |ctx| app.show(ctx));
        }
        snap.save(&shapes, Path::new("target/snapshots/app_settings.png"));
        snap.ctx.memory_mut(|m| m.close_popup());
        app.open_color_editor_for_snapshot();
        for t in [27.9, 27.95, 27.99] {
            shapes = snap.frame(t, |ctx| app.show(ctx));
        }
        snap.save(&shapes, Path::new("target/snapshots/app_colours.png"));
        app.set_theme_for_snapshot(1);
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

    /// Vorschaubild für GitHub (Profil, Social Preview): 1280 × 640
    #[test]
    #[ignore]
    fn render_social_preview() {
        use crate::present::Backdrop;
        use eframe::egui::{Align2, Color32, FontId, Pos2, Rect, Stroke};
        let theme = &THEMES[1];
        let fx = Effects::default();
        let mut map =
            MindMap::from_outline(include_str!("../assets/examples/universe.md")).unwrap();
        crate::images::resolve(&mut map, None);
        let find = |map: &MindMap, title: &str| {
            1 + map
                .children(map.root().unwrap())
                .position(|n| n.text == title)
                .unwrap()
        };
        let (gallery, chart) = (find(&map, "Planet Portraits"), find(&map, "Planet Sizes"));
        let mut snap = Snapshot::retina(1280.0, 640.0, 1.0);
        install_fonts(&snap.ctx);
        snap.ctx.set_visuals(visuals_for(theme));
        snap.ctx.style_mut(apply_spacing);
        let mut shapes = Vec::new();
        for t in [10.0, 11.0, 30.0] {
            shapes = snap.frame(t, |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::default().fill(theme.background))
                    .show(ctx, |ui| {
                        let area = ui.max_rect();
                        let p = ui.painter().clone();
                        present::backdrop(
                            &p,
                            area,
                            theme,
                            Backdrop::Stars,
                            30.0,
                            Vec2::ZERO,
                            Vec2::ZERO,
                        );
                        // Links: Name, Satz, Stichworte
                        let accent = theme.branches[0];
                        let x = 70.0;
                        crate::logo::paint(&p, Pos2::new(x + 34.0, 150.0), 72.0, theme.text);
                        p.text(
                            Pos2::new(x + 88.0, 150.0),
                            Align2::LEFT_CENTER,
                            "Mindful",
                            FontId::proportional(64.0),
                            theme.text,
                        );
                        let lines = ["Mind maps, Markdown and", "presentations in one small app"];
                        for (i, line) in lines.iter().enumerate() {
                            p.text(
                                Pos2::new(x, 240.0 + i as f32 * 40.0),
                                Align2::LEFT_CENTER,
                                *line,
                                FontId::proportional(31.0),
                                theme.text.gamma_multiply(0.8),
                            );
                        }
                        let mut cx = x;
                        for (i, chip) in ["Mind map", "Slides", "Charts", "Presenter view"]
                            .iter()
                            .enumerate()
                        {
                            let g = p.layout_no_wrap(
                                chip.to_string(),
                                FontId::proportional(19.0),
                                theme.on_accent,
                            );
                            let r = Rect::from_min_size(
                                Pos2::new(cx, 345.0),
                                g.size() + Vec2::new(28.0, 14.0),
                            );
                            p.rect_filled(
                                r,
                                r.height() / 2.0,
                                theme.branches[i % theme.branches.len()],
                            );
                            p.galley(r.center() - g.size() / 2.0, g, theme.on_accent);
                            cx = r.right() + 10.0;
                        }
                        p.text(
                            Pos2::new(x, 430.0),
                            Align2::LEFT_CENTER,
                            "Free & open source  ·  macOS & Windows  ·  written in Rust",
                            FontId::proportional(19.0),
                            theme.text.gamma_multiply(0.55),
                        );
                        p.line_segment(
                            [Pos2::new(x, 470.0), Pos2::new(x + 60.0, 470.0)],
                            Stroke::new(4.0_f32, accent),
                        );
                        // Rechts: zwei Folien übereinander
                        let back =
                            Rect::from_min_size(Pos2::new(700.0, 90.0), Vec2::new(500.0, 281.0));
                        let front =
                            Rect::from_min_size(Pos2::new(640.0, 280.0), Vec2::new(520.0, 292.0));
                        let mut ui_back =
                            ui.new_child(egui::UiBuilder::new().max_rect(back).id_salt("back"));
                        present::draw_card(&mut ui_back, back, &map, theme, &fx, gallery);
                        let mut ui_front =
                            ui.new_child(egui::UiBuilder::new().max_rect(front).id_salt("front"));
                        present::draw_card(&mut ui_front, front, &map, theme, &fx, chart);
                        let _ = Color32::WHITE;
                    });
            });
        }
        snap.save(&shapes, Path::new("target/snapshots/social-preview.png"));
    }

    /// Klick auf den Platzhalter und Tippen ersetzt ihn; Lostippen auf einem
    /// ausgewählten Knoten ersetzt dessen Text
    #[test]
    fn placeholders_can_be_typed_over() {
        use crate::app::MindMapApp;
        use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2};
        let mut snap = Snapshot::new(1200.0, 800.0);
        let mut app = MindMapApp::for_snapshot(&snap.ctx);
        app.new_tab_for_snapshot();
        let mut t = 0.0;
        let mut step = |snap: &mut Snapshot, app: &mut MindMapApp, events: Vec<Event>| {
            t += 0.1;
            snap.frame_with_events(t, events, |ctx| app.show(ctx));
        };
        for _ in 0..4 {
            step(&mut snap, &mut app, vec![]);
        }
        // Der Mittelpunkt steht nach dem Einpassen in der Mitte der Zeichenfläche
        let center = Pos2::new(600.0, 430.0);
        let click = |pressed| Event::PointerButton {
            pos: center,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        step(
            &mut snap,
            &mut app,
            vec![Event::PointerMoved(center), click(true)],
        );
        step(&mut snap, &mut app, vec![click(false)]);
        step(&mut snap, &mut app, vec![]);
        assert!(
            app.root_for_test().1,
            "clicking the placeholder starts editing"
        );
        step(&mut snap, &mut app, vec![Event::Text("Holiday".into())]);
        let enter = |pressed| Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        step(&mut snap, &mut app, vec![enter(true)]);
        step(&mut snap, &mut app, vec![enter(false)]);
        step(&mut snap, &mut app, vec![]);
        assert_eq!(app.root_for_test(), ("Holiday".to_string(), false));

        // Ausgewählt lassen und lostippen: ersetzt den Text, weitere Zeichen hängen sich an
        step(&mut snap, &mut app, vec![Event::Text("M".into())]);
        step(&mut snap, &mut app, vec![]);
        step(&mut snap, &mut app, vec![Event::Text("ars".into())]);
        step(&mut snap, &mut app, vec![]);
        assert_eq!(app.root_for_test(), ("Mars".to_string(), true));
    }
}
