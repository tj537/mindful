//! Rendert assets/icon.svg in alle Größen, die ein macOS-Iconset braucht.
//!
//!     cargo run --release --example make_icon -- <Zielordner.iconset>

use resvg::{tiny_skia, usvg};
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("Zielordner angeben"));
    std::fs::create_dir_all(&out).expect("Zielordner anlegen");

    let svg = include_str!("../assets/icon.svg");
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("SVG lesen");

    for base in [16u32, 32, 128, 256, 512] {
        for scale in [1u32, 2] {
            let px = base * scale;
            let mut pixmap = tiny_skia::Pixmap::new(px, px).unwrap();
            let s = px as f32 / tree.size().width();
            resvg::render(&tree, tiny_skia::Transform::from_scale(s, s), &mut pixmap.as_mut());
            let suffix = if scale == 2 { "@2x" } else { "" };
            let name = format!("icon_{base}x{base}{suffix}.png");
            pixmap.save_png(out.join(name)).expect("PNG schreiben");
        }
    }
}
