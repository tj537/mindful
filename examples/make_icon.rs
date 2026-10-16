//! Rendert assets/icon.svg in alle Größen, die ein macOS-Iconset braucht.
//!
//!     cargo run --release --example make_icon -- <Zielordner.iconset>
//!
//! Mit `--assets` statt eines Ordners werden stattdessen `assets/icon.png` (Fenster-Symbol)
//! und `assets/icon.ico` (Windows, mehrere Größen) neu geschrieben.

use resvg::{tiny_skia, usvg};
use std::path::{Path, PathBuf};

fn render(tree: &usvg::Tree, px: u32) -> tiny_skia::Pixmap {
    let mut pixmap = tiny_skia::Pixmap::new(px, px).unwrap();
    let s = px as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(s, s),
        &mut pixmap.as_mut(),
    );
    pixmap
}

/// ICO-Datei mit eingebetteten PNGs (versteht Windows seit Vista)
fn write_ico(tree: &usvg::Tree, path: &Path) {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let pngs: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&px| render(tree, px).encode_png().expect("could not encode PNG"))
        .collect();
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&px, png) in sizes.iter().zip(&pngs) {
        // 256 wird als 0 eingetragen
        let side = if px >= 256 { 0 } else { px as u8 };
        ico.extend_from_slice(&[side, side, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in &pngs {
        ico.extend_from_slice(png);
    }
    std::fs::write(path, ico).expect("could not write ICO");
}

fn main() {
    let target = std::env::args()
        .nth(1)
        .expect("usage: make_icon <output.iconset> | --assets");

    let svg = include_str!("../assets/icon.svg");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("could not read SVG");

    if target == "--assets" {
        let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        render(&tree, 256)
            .save_png(assets.join("icon.png"))
            .expect("could not write PNG");
        write_ico(&tree, &assets.join("icon.ico"));
        return;
    }

    let out = PathBuf::from(target);
    std::fs::create_dir_all(&out).expect("could not create output folder");
    for base in [16u32, 32, 128, 256, 512] {
        for scale in [1u32, 2] {
            let px = base * scale;
            let suffix = if scale == 2 { "@2x" } else { "" };
            let name = format!("icon_{base}x{base}{suffix}.png");
            render(&tree, px)
                .save_png(out.join(name))
                .expect("could not write PNG");
        }
    }
}
