# Mindful – notes for working on the code

Mind maps, Markdown and presentations in one desktop app. Rust + eframe/egui 0.31. Architecture in detail: `docs/ARCHITECTURE.md`; user-facing docs: `README.md`. Keep both up to date with every feature.

## Commands

| Command | What it does |
|---|---|
| `cargo test --release` | all tests (fast) |
| `cargo clippy --release --all-targets` | must stay free of warnings |
| `cargo fmt` | CI checks `cargo fmt --check` |
| `cargo test --release render_ -- --ignored` | renders every slide, the presenter view, the phone-remote card, the app window (several widths/themes) and the social preview to `target/snapshots/*.png`, and the sounds to `target/snapshots/sounds/*.wav` – look at (and listen to) them after UI changes |
| `./scripts/build-mac.sh --install` | builds `dist/Mindful.app` and replaces `/Applications/Mindful.app` |
| `./scripts/build-mac.sh --zip` | builds `dist/Mindful-macos-arm64.zip` (release asset) |
| `cargo run --release --example make_space_images` | regenerates the example pictures in `assets/examples/images/` |
| `cargo run --release --example make_icon -- --assets` | regenerates `assets/icon.png` and `assets/icon.ico` from `assets/icon.svg` (the source of the app icon and the in-app logo) |

Commit only when `cargo test` exits 0 (check the exit code; don't hide it behind a pipe).

## Conventions

- **Language:** UI, README, docs, release notes, commit messages in English. Code comments are English too.
- **egui pins:** egui/eframe 0.31 with `egui-phosphor` 0.9 – they must be upgraded together.
- **Visual checks:** there is no way to drive the real app from the terminal. Verify UI with the offscreen renderer (`src/offscreen.rs`, tests in `src/snapshot.rs`, `MindMapApp::for_snapshot`). Don't take full-screen screenshots of the user's desktop.
- **Markdown round trip:** `MindMap::to_markdown()` must reproduce the bundled examples and templates exactly (tests check this). New node properties need both directions: written in `to_markdown`, read in `parse_outline`, carried through `apply_outline`/`add_outline`.
- **Line endings:** `.gitattributes` forces LF everywhere (Windows checkouts broke the round-trip tests otherwise).
- **New `Node` / settings fields:** add `#[serde(default)]` so older `.mindmap` files and saved state still load.
- **Examples:** `assets/examples/universe.md` (English) and `weltall.md` (German) show every slide layout, pictures, icons and a checklist; keep them in sync and update the counts in `model.rs` tests.
- **Slides** are drawn by `present::render_slide` – shared by the talk, the presenter view and the PDF export. Change drawing there, not in three places.

## Releases

Asset names never change (`Mindful-macos-arm64.zip`, `Mindful-windows-x64.zip`) so `releases/latest/download/…` links keep working.

1. Raise `version` in `Cargo.toml`, commit.
2. Write `.github/release-notes/vX.Y.Z.md` (English).
3. `git tag -a vX.Y.Z -m "Mindful X.Y.Z" && git push origin vX.Y.Z` – `.github/workflows/release.yml` builds both platforms, adds `SHA256SUMS.txt` and publishes the release. Running apps then offer the update (`src/update.rs`).

`ci.yml` runs fmt, clippy and tests on macOS, Windows and Linux for every push.

## Platform notes

- Windows: `build.rs` embeds `assets/icon.ico` (only when building on Windows); child processes use `CREATE_NO_WINDOW`; `main.rs` sets the GUI subsystem.
- Sound (`src/sound.rs`) uses `rodio` with `playback` only; Linux builds need `libasound2-dev` (CI installs it).
- The phone remote (`src/remote.rs`) is a tiny `std::net` HTTP server with a random token; the firewall may ask once.
- The Mac bundle is ad-hoc signed only (no notarization yet), so first launch needs *Open Anyway*.
