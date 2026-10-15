// Unter Windows kein Konsolenfenster neben der App öffnen (nur im Release-Build)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod document;
mod export;
mod markdown_view;
mod model;
mod present;
mod theme;

use eframe::egui;

const APP_NAME: &str = "Mindful";

/// Die App hieß früher "Mindmap": gespeicherte Tabs und Einstellungen einmalig übernehmen.
fn migrate_storage() {
    let (Some(old), Some(new)) = (eframe::storage_dir("Mindmap"), eframe::storage_dir(APP_NAME)) else {
        return;
    };
    if !old.exists() || new.exists() || std::fs::create_dir_all(&new).is_err() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(&old) {
        for entry in entries.flatten() {
            let _ = std::fs::copy(entry.path(), new.join(entry.file_name()));
        }
    }
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([900.0, 500.0])
        .with_title("Mindful");
    // Icon für Fenster, Taskleiste und Dock (wenn ohne .app-Paket gestartet)
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    migrate_storage();
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::MindMapApp::new(cc)))),
    )
}
