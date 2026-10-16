//! Update-Hinweis: Beim Start fragt Mindful einmal im Hintergrund bei GitHub
//! nach der neuesten Version. Dafür wird das vorhandene `curl` benutzt
//! (auf macOS, Windows 10+ und Linux eingebaut) – keine zusätzliche Bibliothek.

use std::process::Command;
use std::sync::{Arc, Mutex};

const LATEST: &str = "https://api.github.com/repos/tj537/mindful/releases/latest";
pub const RELEASES: &str = "https://github.com/tj537/mindful/releases/latest";

/// Eine neuere Version, als gerade läuft
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub version: String,
    pub url: String,
}

/// Ergebnis der Prüfung: `None` solange sie läuft
pub type Check = Arc<Mutex<Option<Result<Option<Update>, String>>>>;

/// „v0.2.0“ → (0, 2, 0)
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut next = || parts.next()?.split(['-', '+']).next()?.parse().ok();
    Some((next()?, next()?, next().unwrap_or(0)))
}

/// Ist `latest` neuer als `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Prüfung im Hintergrund starten; `repaint` weckt die Oberfläche danach
pub fn start(repaint: impl Fn() + Send + 'static) -> Check {
    let result: Check = Arc::default();
    let slot = result.clone();
    std::thread::spawn(move || {
        let outcome = fetch();
        if let Ok(mut s) = slot.lock() {
            *s = Some(outcome);
        }
        repaint();
    });
    result
}

fn fetch() -> Result<Option<Update>, String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            "-A",
            concat!("Mindful/", env!("CARGO_PKG_VERSION")),
            LATEST,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("no connection".into());
    }
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let tag = json["tag_name"].as_str().unwrap_or_default();
    let url = json["html_url"].as_str().unwrap_or(RELEASES).to_string();
    Ok(is_newer(tag, env!("CARGO_PKG_VERSION")).then(|| Update {
        version: tag.trim_start_matches('v').to_string(),
        url,
    }))
}

/// Eine Webseite im Standardbrowser öffnen
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = Command::new("xdg-open").arg(url).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared() {
        assert!(is_newer("v0.3.0", "0.2.0"));
        assert!(is_newer("v0.2.1", "0.2.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("v0.1.9", "0.2.0"));
        assert!(!is_newer("nonsense", "0.2.0"));
    }
}
