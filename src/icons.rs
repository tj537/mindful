//! Symbole an Knoten. Im Markdown stehen sie als Kurzname vor dem Text
//! (`## :rocket: Spaceflight`, wie GitHub-Emojis), gezeichnet werden sie mit
//! der Phosphor-Schrift – scharf und in der Farbe des Zweigs.

use crate::model::Node;
use eframe::egui::{self, RichText, Vec2};
use egui_phosphor::regular as icon;

/// Kurzname, Zeichen, Beschriftung im Auswahlmenü
pub const ICONS: &[(&str, &str, &str)] = &[
    ("bulb", icon::LIGHTBULB, "Idea"),
    ("star", icon::STAR, "Star"),
    ("heart", icon::HEART, "Heart"),
    ("fire", icon::FIRE, "Hot"),
    ("zap", icon::LIGHTNING, "Energy"),
    ("sparkles", icon::SPARKLE, "Sparkle"),
    ("rocket", icon::ROCKET_LAUNCH, "Launch"),
    ("dart", icon::TARGET, "Goal"),
    ("trophy", icon::TROPHY, "Success"),
    ("tada", icon::CONFETTI, "Celebrate"),
    ("check", icon::CHECK_CIRCLE, "Done"),
    ("x", icon::X_CIRCLE, "No"),
    ("warning", icon::WARNING, "Warning"),
    ("question", icon::QUESTION, "Question"),
    ("thumbsup", icon::THUMBS_UP, "Pro"),
    ("thumbsdown", icon::THUMBS_DOWN, "Con"),
    ("flag", icon::FLAG, "Milestone"),
    ("pushpin", icon::PUSH_PIN, "Pinned"),
    ("calendar", icon::CALENDAR, "Date"),
    ("clock", icon::CLOCK, "Time"),
    ("hourglass", icon::HOURGLASS, "Waiting"),
    ("chart", icon::TREND_UP, "Growth"),
    ("bar_chart", icon::CHART_BAR, "Data"),
    ("moneybag", icon::COINS, "Money"),
    ("briefcase", icon::BRIEFCASE, "Work"),
    ("handshake", icon::HANDSHAKE, "Deal"),
    ("people", icon::USERS, "Team"),
    ("person", icon::USER, "Person"),
    ("speech", icon::CHAT_CIRCLE, "Talk"),
    ("mega", icon::MEGAPHONE, "Marketing"),
    ("email", icon::ENVELOPE, "Mail"),
    ("phone", icon::PHONE, "Call"),
    ("books", icon::BOOK_OPEN, "Learn"),
    ("mortar_board", icon::GRADUATION_CAP, "Study"),
    ("brain", icon::BRAIN, "Think"),
    ("puzzle", icon::PUZZLE_PIECE, "Piece"),
    ("pencil", icon::PENCIL_SIMPLE, "Write"),
    ("gear", icon::GEAR, "Settings"),
    ("code", icon::CODE, "Code"),
    ("art", icon::PALETTE, "Design"),
    ("camera", icon::CAMERA, "Photo"),
    ("music", icon::MUSIC_NOTES, "Music"),
    ("lock", icon::LOCK, "Private"),
    ("key", icon::KEY, "Key"),
    ("shield", icon::SHIELD_CHECK, "Safe"),
    ("link", icon::LINK, "Link"),
    ("mag", icon::MAGNIFYING_GLASS, "Search"),
    ("binoculars", icon::BINOCULARS, "Explore"),
    ("eye", icon::EYE, "See"),
    ("bell", icon::BELL, "Reminder"),
    ("house", icon::HOUSE, "Home"),
    ("map", icon::MAP_TRIFOLD, "Map"),
    ("globe", icon::GLOBE_HEMISPHERE_WEST, "World"),
    ("seedling", icon::PLANT, "Nature"),
    ("tree", icon::TREE, "Tree"),
    ("sunny", icon::SUN, "Sun"),
    ("moon", icon::MOON, "Moon"),
    ("planet", icon::PLANET, "Planet"),
    ("atom", icon::ATOM, "Science"),
    ("spiral", icon::SPIRAL, "Spiral"),
    ("comet", icon::SHOOTING_STAR, "Shooting star"),
    ("star4", icon::STAR_FOUR, "Shine"),
    ("half", icon::CIRCLE_HALF, "Contrast"),
    ("quote", icon::QUOTES, "Quote"),
];

/// Zeichen zu einem Kurznamen
pub fn glyph(name: &str) -> Option<&'static str> {
    ICONS.iter().find(|(n, ..)| *n == name).map(|(_, g, _)| *g)
}

/// Trennt `:name:` am Anfang vom Text, wenn es ein bekanntes Symbol ist
pub fn split_icon(text: &str) -> (Option<String>, String) {
    if let Some(rest) = text.strip_prefix(':') {
        if let Some((name, after)) = rest.split_once(':') {
            if glyph(name).is_some() && (after.is_empty() || after.starts_with(' ')) {
                return (Some(name.to_string()), after.trim_start().to_string());
            }
        }
    }
    (None, text.to_string())
}

/// Markdown-Schreibweise: `:name: `
pub fn prefix(icon: Option<&str>) -> String {
    icon.map(|n| format!(":{n}: ")).unwrap_or_default()
}

/// Text eines Knotens mit Symbol davor (für Folien und Vorschau)
pub fn label(node: &Node) -> String {
    match node.icon.as_deref().and_then(glyph) {
        Some(g) => format!("{g}  {}", node.text),
        None => node.text.clone(),
    }
}

/// Auswahl im Kontextmenü. `Some(None)` = Symbol entfernen.
pub fn picker(ui: &mut egui::Ui, current: Option<&str>) -> Option<Option<String>> {
    let mut picked = None;
    ui.set_min_width(300.0);
    egui::Grid::new("icon_picker")
        .spacing(Vec2::splat(2.0))
        .show(ui, |ui| {
            for (i, (name, g, label)) in ICONS.iter().enumerate() {
                let selected = current == Some(*name);
                let button = egui::Button::new(RichText::new(*g).size(18.0))
                    .selected(selected)
                    .min_size(Vec2::splat(34.0));
                if ui
                    .add(button)
                    .on_hover_text(format!("{label}   :{name}:"))
                    .clicked()
                {
                    picked = Some(Some(name.to_string()));
                }
                if i % 8 == 7 {
                    ui.end_row();
                }
            }
        });
    ui.separator();
    if ui
        .add_enabled(current.is_some(), egui::Button::new("No icon"))
        .clicked()
    {
        picked = Some(None);
    }
    ui.weak("Emojis work too: type them into the text (⌃⌘Space).");
    if picked.is_some() {
        ui.close_menu();
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_are_read_from_shortcodes() {
        assert_eq!(
            split_icon(":rocket: Spaceflight"),
            (Some("rocket".into()), "Spaceflight".into())
        );
        // Unbekannte Namen und Uhrzeiten bleiben Text
        assert_eq!(split_icon(":nope: Text"), (None, ":nope: Text".into()));
        assert_eq!(
            split_icon("10:30: Meeting"),
            (None, "10:30: Meeting".into())
        );
        // Jeder Kurzname kommt nur einmal vor
        for (i, (a, ..)) in ICONS.iter().enumerate() {
            assert!(ICONS[i + 1..].iter().all(|(b, ..)| a != b), "{a}");
        }
    }
}
