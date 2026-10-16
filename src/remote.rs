//! Handy als Fernbedienung: Mindful startet einen kleinen Webserver im
//! eigenen WLAN. Das Handy öffnet über den QR-Code eine Seite mit großen
//! Knöpfen, Folientitel, Notizen und Stoppuhr – ohne App und ohne Cloud.
//! Ein zufälliger Schlüssel in der Adresse sorgt dafür, dass nur wer den
//! QR-Code sieht, mitsteuern kann.

use eframe::egui;
use serde::Serialize;
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Was das Handy anzeigt
#[derive(Clone, Default, Serialize)]
pub struct RemoteState {
    pub title: String,
    pub next: String,
    /// 0 = Titelfolie
    pub slide: usize,
    pub total: usize,
    pub point: usize,
    pub points: usize,
    /// (Stichpunkt, Notiz)
    pub notes: Vec<(String, String)>,
    pub elapsed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Next,
    Prev,
}

#[derive(Default)]
struct Shared {
    state: RemoteState,
    commands: Vec<Command>,
    last_seen: Option<Instant>,
}

pub struct Remote {
    /// Adresse für den QR-Code
    pub url: String,
    /// Schlüssel in der Adresse (nur Tests lesen ihn direkt)
    #[cfg_attr(not(test), allow(dead_code))]
    token: String,
    shared: Arc<Mutex<Shared>>,
}

static REMOTE: OnceLock<Result<Remote, String>> = OnceLock::new();

/// Server beim ersten Aufruf starten; danach immer derselbe
pub fn start(ctx: &egui::Context) -> Result<&'static Remote, &'static String> {
    REMOTE.get_or_init(|| Remote::start(ctx.clone())).as_ref()
}

/// Läuft der Server schon?
pub fn running() -> Option<&'static Remote> {
    REMOTE.get().and_then(|r| r.as_ref().ok())
}

/// IP-Adresse im eigenen Netz (ohne etwas zu senden: UDP „verbindet“ nur)
fn local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.168.0.1:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

fn random_token() -> String {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    format!("{:012x}", h.finish() & 0xffff_ffff_ffff)
}

impl Remote {
    fn start(ctx: egui::Context) -> Result<Remote, String> {
        let listener = (8765..8775)
            .find_map(|port| TcpListener::bind(("0.0.0.0", port)).ok())
            .or_else(|| TcpListener::bind(("0.0.0.0", 0)).ok())
            .ok_or("could not open a network port")?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let ip = local_ip().ok_or("not connected to a network")?;
        let token = random_token();
        let shared: Arc<Mutex<Shared>> = Arc::default();
        let (server_token, server_shared) = (token.clone(), shared.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (token, shared, ctx) =
                    (server_token.clone(), server_shared.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let _ = handle(stream, &token, &shared, &ctx);
                });
            }
        });
        Ok(Remote {
            url: format!("http://{ip}:{port}/?t={token}"),
            token,
            shared,
        })
    }

    /// Aktuellen Stand fürs Handy hinterlegen
    pub fn publish(&self, state: RemoteState) {
        if let Ok(mut s) = self.shared.lock() {
            s.state = state;
        }
    }

    /// Vom Handy gedrückte Knöpfe abholen
    pub fn take_commands(&self) -> Vec<Command> {
        self.shared
            .lock()
            .map(|mut s| std::mem::take(&mut s.commands))
            .unwrap_or_default()
    }

    /// Hat sich in den letzten Sekunden ein Handy gemeldet?
    pub fn connected(&self) -> bool {
        self.shared
            .lock()
            .ok()
            .and_then(|s| s.last_seen)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(4))
    }

    /// QR-Code der Adresse: Breite und Felder (true = dunkel)
    pub fn qr(&self) -> Option<(usize, Vec<bool>)> {
        let code = qrcode::QrCode::new(self.url.as_bytes()).ok()?;
        let dark = code
            .to_colors()
            .into_iter()
            .map(|c| c == qrcode::Color::Dark)
            .collect();
        Some((code.width(), dark))
    }

    #[cfg(test)]
    fn token(&self) -> &str {
        &self.token
    }
}

/// Eine Anfrage beantworten
fn handle(
    mut stream: TcpStream,
    token: &str,
    shared: &Mutex<Shared>,
    ctx: &egui::Context,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    // Bis zum Ende der Kopfzeilen lesen – eine Anfrage kann in mehreren Teilen ankommen
    let mut data = Vec::new();
    let mut buf = [0u8; 2048];
    while !data.windows(4).any(|w| w == b"\r\n\r\n") && data.len() < 16 * 1024 {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    let request = String::from_utf8_lossy(&data);
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let authorized = query
        .split('&')
        .any(|pair| pair.strip_prefix("t=") == Some(token));
    if !authorized {
        return respond(
            &mut stream,
            "403 Forbidden",
            "text/html; charset=utf-8",
            FORBIDDEN.as_bytes(),
        );
    }
    let command = match path {
        "/next" => Some(Command::Next),
        "/prev" => Some(Command::Prev),
        _ => None,
    };
    let state = {
        let mut s = shared.lock().map_err(|_| std::io::ErrorKind::Other)?;
        s.last_seen = Some(Instant::now());
        if let Some(c) = command {
            s.commands.push(c);
        }
        s.state.clone()
    };
    if command.is_some() {
        ctx.request_repaint();
    }
    match path {
        "/" => respond(
            &mut stream,
            "200 OK",
            "text/html; charset=utf-8",
            PAGE.as_bytes(),
        ),
        "/state" | "/next" | "/prev" => {
            let json = serde_json::to_vec(&state).unwrap_or_default();
            respond(&mut stream, "200 OK", "application/json", &json)
        }
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

const FORBIDDEN: &str = r#"<!doctype html><meta name="viewport" content="width=device-width">
<body style="font-family:-apple-system,sans-serif;background:#1c1f26;color:#e6e8ee;padding:32px">
<h2>Mindful remote</h2><p>This link is no longer valid. Scan the QR code in Mindful again.</p>"#;

/// Die Seite fürs Handy
const PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<meta name="theme-color" content="#1c1f26">
<meta name="apple-mobile-web-app-capable" content="yes">
<title>Mindful remote</title>
<style>
  :root { --bg:#1c1f26; --card:#262a33; --text:#e6e8ee; --weak:#9aa1ad; --accent:#5aaef0; }
  * { box-sizing:border-box; -webkit-tap-highlight-color:transparent; }
  html, body { margin:0; height:100%; background:var(--bg); color:var(--text);
    font-family:-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
  body { display:flex; flex-direction:column; padding:max(16px, env(safe-area-inset-top)) 16px max(16px, env(safe-area-inset-bottom)); gap:12px; }
  header { display:flex; justify-content:space-between; align-items:baseline; }
  #timer { font-size:34px; font-weight:600; font-variant-numeric:tabular-nums; }
  #pos { color:var(--weak); font-size:15px; }
  #title { font-size:24px; font-weight:600; line-height:1.25; }
  #next { color:var(--weak); font-size:15px; }
  #notes { flex:1; overflow:auto; background:var(--card); border-radius:14px; padding:14px 16px; font-size:17px; line-height:1.45; }
  #notes h4 { margin:10px 0 2px; font-size:14px; color:var(--accent); }
  #notes p { margin:0 0 8px; }
  .empty { color:var(--weak); }
  .buttons { display:grid; grid-template-columns:1fr 2fr; gap:12px; height:30vh; min-height:150px; }
  button { border:0; border-radius:18px; font-size:22px; font-weight:600; color:#fff; }
  #prev { background:#3a3f4b; }
  #nextBtn { background:var(--accent); }
  button:active { transform:scale(0.97); filter:brightness(1.15); }
  #status { text-align:center; color:var(--weak); font-size:13px; }
  #status.off { color:#e06c75; }
</style>
</head>
<body>
  <header><div id="timer">00:00</div><div id="pos"></div></header>
  <div id="title">Connecting …</div>
  <div id="next"></div>
  <div id="notes"><p class="empty">Notes appear here.</p></div>
  <div class="buttons">
    <button id="prev">‹ Back</button>
    <button id="nextBtn">Next ›</button>
  </div>
  <div id="status">Mindful remote</div>
<script>
  const token = new URLSearchParams(location.search).get("t");
  const $ = id => document.getElementById(id);
  const esc = s => s.replace(/[&<>"]/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;"}[c]));
  let offline = 0;
  function show(s) {
    const m = Math.floor(s.elapsed / 60), sec = s.elapsed % 60;
    $("timer").textContent = String(m).padStart(2, "0") + ":" + String(sec).padStart(2, "0");
    let pos = s.slide === 0 ? "Title slide" : "Slide " + s.slide + " of " + (s.total - 1);
    if (s.points > 0) pos += " · point " + s.point + " of " + s.points;
    $("pos").textContent = pos;
    $("title").textContent = s.title || "Mindful";
    $("next").textContent = s.next ? "Next: " + s.next : "Last slide";
    $("notes").innerHTML = s.notes.length
      ? s.notes.map(([t, n]) => (t ? "<h4>" + esc(t) + "</h4>" : "") + "<p>" + esc(n) + "</p>").join("")
      : '<p class="empty">No notes on this slide.</p>';
    $("status").textContent = "Connected to Mindful";
    $("status").className = "";
    offline = 0;
  }
  async function call(path, method) {
    try {
      const r = await fetch(path + "?t=" + token, { method: method || "GET", cache: "no-store" });
      if (!r.ok) throw new Error(r.status);
      show(await r.json());
    } catch (e) {
      if (++offline > 2) { $("status").textContent = "No connection – is Mindful still presenting?"; $("status").className = "off"; }
    }
  }
  function press(path) { if (navigator.vibrate) navigator.vibrate(15); call(path, "POST"); }
  $("nextBtn").onclick = () => press("/next");
  $("prev").onclick = () => press("/prev");
  // Wischen: nach links = weiter, nach rechts = zurück
  let x0 = null;
  document.addEventListener("touchstart", e => { x0 = e.touches[0].clientX; }, { passive: true });
  document.addEventListener("touchend", e => {
    if (x0 === null) return;
    const dx = e.changedTouches[0].clientX - x0; x0 = null;
    if (Math.abs(dx) > 60) press(dx < 0 ? "/next" : "/prev");
  });
  call("/state");
  setInterval(() => call("/state"), 1000);
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    fn get(url: &str) -> String {
        let rest = url.trim_start_matches("http://");
        let (host, path) = rest.split_once('/').unwrap();
        let mut stream = TcpStream::connect(host).unwrap();
        let request = format!("POST /{path} HTTP/1.1\r\nHost: {host}\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn phone_requests_need_the_token_and_queue_commands() {
        let Ok(remote) = Remote::start(egui::Context::default()) else {
            // Ohne Netzwerk (z. B. in einer Sandbox) gibt es nichts zu testen
            eprintln!("remote test skipped: no network");
            return;
        };
        remote.publish(RemoteState {
            title: "Stars".into(),
            total: 3,
            slide: 1,
            ..Default::default()
        });
        let base = remote
            .url
            .split('?')
            .next()
            .unwrap()
            .trim_end_matches('/')
            .to_string();
        let local = base.replacen(
            base.trim_start_matches("http://")
                .split(':')
                .next()
                .unwrap(),
            "127.0.0.1",
            1,
        );
        assert!(get(&format!("{local}/next?t=wrong")).starts_with("HTTP/1.1 403"));
        let reply = get(&format!("{local}/next?t={}", remote.token()));
        assert!(reply.starts_with("HTTP/1.1 200"));
        assert!(reply.contains("\"title\":\"Stars\""));
        assert_eq!(remote.take_commands(), vec![Command::Next]);
        assert!(remote.connected());
        let page = get(&format!("{local}/?t={}", remote.token()));
        assert!(page.contains("Mindful remote"));
        assert!(remote.qr().is_some());
    }
}
