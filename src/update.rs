//! Update notice: on startup Mindful asks GitHub once in the background
//! for the newest version. It uses the `curl` that is already there
//! (built into macOS, Windows 10+ and Linux) – no extra library.

use std::process::Command;
use std::sync::{Arc, Mutex};

const LATEST: &str = "https://api.github.com/repos/tj537/mindful/releases/latest";
pub const RELEASES: &str = "https://github.com/tj537/mindful/releases/latest";
/// Project page (Help menu)
pub const REPOSITORY: &str = "https://github.com/tj537/mindful";

/// A newer version than the one running
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub version: String,
    pub url: String,
}

/// Result of the check: `None` while it is running
pub type Check = Arc<Mutex<Option<Result<Option<Update>, String>>>>;

/// "v0.2.0" → (0, 2, 0)
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut next = || parts.next()?.split(['-', '+']).next()?.parse().ok();
    Some((next()?, next()?, next().unwrap_or(0)))
}

/// Is `latest` newer than `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Start the check in the background; `repaint` wakes up the UI afterwards
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
    let mut command = Command::new("curl");
    hide_console(&mut command);
    let output = command
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

/// On Windows, don't let a black console window flash up
fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// Open a web page in the default browser
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        hide_console(&mut command);
        let _ = command.args(["/C", "start", "", url]).spawn();
    }
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
