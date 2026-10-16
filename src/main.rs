// On Windows, don't open a console window next to the app (release builds only)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_prompt;
mod app;
mod diagrams;
mod document;
mod export;
mod icons;
mod images;
mod logo;
mod markdown_view;
mod model;
mod nodes;
mod offscreen;
mod pdf;
mod present;
mod remote;
mod search;
#[cfg(test)]
mod snapshot;
mod theme;
mod tour;
mod update;

use eframe::egui;

const APP_NAME: &str = "Mindful";

/// The app used to be called "Mindmap": take over saved tabs and settings once.
fn migrate_storage() {
    let (Some(old), Some(new)) = (
        eframe::storage_dir("Mindmap"),
        eframe::storage_dir(APP_NAME),
    ) else {
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
        .with_min_inner_size([600.0, 420.0])
        .with_title("Mindful");
    // Icon for the window, taskbar and Dock (when started without the .app bundle)
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
