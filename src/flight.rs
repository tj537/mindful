//! Tour style "Free flight": the mind map turned into cards floating in space.
//!
//! Every stop is a station on a spiral that leads into the depth: the title card
//! first, then one card per main branch. Its sub-points float around it as cards
//! of their own (deeper levels are listed inside them), a little in front of or
//! behind it, and bob gently. There are no connecting lines – only a faint trail
//! of the route. The camera flies through space with a real perspective; when it
//! skips stations they rush past. At the end all branch cards fly past the camera
//! into a grid: the whole picture.
//!
//! Stops, keys, quick jump, controls and ticking off are shared with `tour.rs`.

use crate::document::{Camera, Document, TourState};
use crate::icons;
use crate::images::{self, Fit};
use crate::model::{CalloutKind, MindMap};
use crate::nodes::paint_checkbox;
use crate::present::{
    backdrop, color_of, ease_in_out_cubic, ease_out_back, ease_out_cubic, read_nav, soft_shadow,
    title_job, Backdrop, Effects, Event,
};
use crate::theme::Theme;
use crate::tour::{controls, growth, navigate, stop_label, stops, tick, Stop};
use eframe::egui::{
    self, text::LayoutJob, Align2, Color32, FontId, Galley, Pos2, Rect, Sense, Stroke, Vec2,
};
use std::f32::consts::PI;
use std::sync::Arc;

/// Duration of a flight in seconds – longer than in the mind map, the way is longer
pub(crate) const FLIGHT: f64 = 1.8;
/// Distance between two stations in depth
const STEP_Z: f32 = 2600.0;
/// Closest distance at which a card is still drawn
const NEAR: f32 = 80.0;

/// A point in space: x to the right, y down, z into the screen
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub(crate) struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }
    fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        Vec3::new(
            self.x + (o.x - self.x) * t,
            self.y + (o.y - self.y) * t,
            self.z + (o.z - self.z) * t,
        )
    }
    fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
    fn from_array(a: [f32; 3]) -> Self {
        Vec3::new(a[0], a[1], a[2])
    }
}

// ---------------------------------------------------------------------------
// Cards
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// The central topic at the start
    Title,
    /// A main branch: the card of a station
    Main,
    /// A sub-point floating around its branch
    Child,
}

impl Kind {
    /// Width in world units (at distance = focal length 1 pixel per unit)
    fn width(self) -> f32 {
        match self {
            Kind::Title => 980.0,
            Kind::Main => 660.0,
            Kind::Child => 330.0,
        }
    }
    fn padding(self) -> f32 {
        match self {
            Kind::Title => 52.0,
            Kind::Main => 38.0,
            Kind::Child => 22.0,
        }
    }
    fn radius(self) -> f32 {
        match self {
            Kind::Title | Kind::Main => 26.0,
            Kind::Child => 18.0,
        }
    }
}

enum Piece {
    Text(Arc<Galley>),
    /// Background of a callout box
    Callout(CalloutKind),
    Image(String),
    Check(bool),
}

/// A card laid out at scale `s`: its size and what goes where (relative to the top left)
/// Largest scale at which card text is laid out (see `nodes::MAX_TEXT_SIZE`)
const MAX_CARD_TEXT_SCALE: f32 = 1.0;

struct CardLayout {
    size: Vec2,
    pieces: Vec<(Rect, Piece)>,
}

impl CardLayout {
    /// The same layout `k` times as large (positions, sizes and text)
    fn scaled(self, k: f32) -> CardLayout {
        if (k - 1.0).abs() < 1e-4 {
            return self;
        }
        let pieces = self
            .pieces
            .into_iter()
            .map(|(r, piece)| {
                let r = Rect::from_min_size((r.min.to_vec2() * k).to_pos2(), r.size() * k);
                let piece = match piece {
                    Piece::Text(g) => Piece::Text(crate::nodes::scale_galley(g, k)),
                    other => other,
                };
                (r, piece)
            })
            .collect();
        CardLayout {
            size: self.size * k,
            pieces,
        }
    }
}

/// Lines below a sub-point card: its own sub-points, depth first
fn sub_lines(map: &MindMap, id: u64) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<(u64, usize)> = map.children(id).map(|n| (n.id, 0)).collect();
    stack.reverse();
    while let Some((id, depth)) = stack.pop() {
        let Some(n) = map.get(id) else { continue };
        let mark = match n.task {
            Some(true) => egui_phosphor::regular::CHECK_SQUARE,
            Some(false) => egui_phosphor::regular::SQUARE,
            None => "•",
        };
        out.push((depth, format!("{mark}  {}", icons::label(n))));
        let mut children: Vec<(u64, usize)> = map.children(id).map(|c| (c.id, depth + 1)).collect();
        children.reverse();
        stack.extend(children);
    }
    out
}

fn layout_card(
    ctx: &egui::Context,
    map: &MindMap,
    theme: &Theme,
    id: u64,
    kind: Kind,
    s: f32,
) -> CardLayout {
    let mut pieces = Vec::new();
    let Some(node) = map.get(id) else {
        return CardLayout {
            size: Vec2::ZERO,
            pieces,
        };
    };
    let accent = color_of(map, theme, id);
    let w = kind.width() * s;
    let pad = kind.padding() * s;
    let inner = w - 2.0 * pad;
    let mut y = pad;
    let galley = |job: LayoutJob| ctx.fonts(|f| f.layout_job(job));
    let text = |text: &str, size: f32, color: Color32, width: f32| {
        let mut job = LayoutJob::simple(text.to_string(), FontId::proportional(size), color, width);
        job.wrap.max_width = width;
        job
    };

    // Picture on top
    if let Some(key) = &node.image {
        let aspect = map.images.get(key).map_or(1.5, |img| img.aspect());
        let max_h = match kind {
            Kind::Child => 170.0 * s,
            _ => inner * 0.6,
        };
        let h = (inner / aspect).min(max_h);
        pieces.push((
            Rect::from_min_size(Pos2::new(pad, y), Vec2::new(inner, h)),
            Piece::Image(key.clone()),
        ));
        y += h + 20.0 * s;
    }

    // Title, with the checkbox in front on sub-points
    let (title_size, check) = match kind {
        Kind::Title => (66.0, false),
        Kind::Main => (46.0, false),
        Kind::Child => (26.0, node.task.is_some()),
    };
    let shift = if check { 36.0 * s } else { 0.0 };
    let mut job = title_job(&icons::label(node), title_size * s, theme.text, accent);
    job.wrap.max_width = inner - shift;
    if kind == Kind::Title {
        job.halign = egui::Align::Center;
    }
    let title = galley(job);
    let title_h = title.size().y;
    if check {
        let box_size = 24.0 * s;
        pieces.push((
            Rect::from_min_size(
                Pos2::new(pad, y + (title_size * s * 1.2 - box_size) / 2.0),
                Vec2::splat(box_size),
            ),
            Piece::Check(node.task == Some(true)),
        ));
    }
    // Centred text: the box starts where the widest line starts
    let x = if kind == Kind::Title {
        (w - title.size().x) / 2.0
    } else {
        pad + shift
    };
    pieces.push((
        Rect::from_min_size(Pos2::new(x, y), title.size()),
        Piece::Text(title),
    ));
    y += title_h;

    // Description
    let note = node.note.trim();
    if !note.is_empty() {
        let size = match kind {
            Kind::Title => 28.0,
            Kind::Main => 23.0,
            Kind::Child => 17.0,
        };
        y += 14.0 * s;
        let mut job = text(note, size * s, theme.text.gamma_multiply(0.78), inner);
        if kind == Kind::Title {
            job.halign = egui::Align::Center;
        }
        let g = galley(job);
        let h = g.size().y;
        let x = if kind == Kind::Title {
            (w - g.size().x) / 2.0
        } else {
            pad
        };
        pieces.push((
            Rect::from_min_size(Pos2::new(x, y), g.size()),
            Piece::Text(g),
        ));
        y += h;
    }

    // Callout boxes
    for c in &node.callouts {
        let (color, glyph) = crate::callout::look(c.kind);
        let size = match kind {
            Kind::Title => 22.0,
            Kind::Main => 20.0,
            Kind::Child => 15.0,
        } * s;
        let (bx, by) = (size * 0.9, size * 0.6);
        y += 14.0 * s;
        let label = galley(text(
            &format!("{glyph}  {}", c.kind.label()),
            size * 0.8,
            color,
            inner - 2.0 * bx,
        ));
        let body = galley(text(c.text.trim(), size, theme.text, inner - 2.0 * bx));
        let (lh, bh) = (label.size().y, body.size().y);
        let h = 2.0 * by + lh + size * 0.25 + bh;
        pieces.push((
            Rect::from_min_size(Pos2::new(pad, y), Vec2::new(inner, h)),
            Piece::Callout(c.kind),
        ));
        pieces.push((
            Rect::from_min_size(Pos2::new(pad + bx, y + by), label.size()),
            Piece::Text(label),
        ));
        pieces.push((
            Rect::from_min_size(Pos2::new(pad + bx, y + by + lh + size * 0.25), body.size()),
            Piece::Text(body),
        ));
        y += h;
    }

    // Sub-points of a sub-point, as a short list
    if kind == Kind::Child {
        let lines = sub_lines(map, id);
        let shown = lines.len().min(5);
        if shown > 0 {
            y += 10.0 * s;
        }
        for (depth, line) in &lines[..shown] {
            let indent = *depth as f32 * 16.0 * s;
            let g = galley(text(
                line,
                18.0 * s,
                theme.text.gamma_multiply(0.85),
                inner - indent,
            ));
            let h = g.size().y;
            pieces.push((
                Rect::from_min_size(Pos2::new(pad + indent, y + 3.0 * s), g.size()),
                Piece::Text(g),
            ));
            y += h + 3.0 * s;
        }
        if lines.len() > shown {
            let g = galley(text(
                &format!("+ {} more", lines.len() - shown),
                16.0 * s,
                accent,
                inner,
            ));
            let h = g.size().y;
            pieces.push((
                Rect::from_min_size(Pos2::new(pad, y + 4.0 * s), g.size()),
                Piece::Text(g),
            ));
            y += h + 4.0 * s;
        }
    }

    CardLayout {
        size: Vec2::new(w, y + pad),
        pieces,
    }
}

/// Draws a laid-out card into `rect` (screen). Returns the checkbox rectangle, if any.
#[allow(clippy::too_many_arguments)]
fn paint_card(
    p: &egui::Painter,
    ctx: &egui::Context,
    map: &MindMap,
    theme: &Theme,
    id: u64,
    kind: Kind,
    layout: CardLayout,
    rect: Rect,
    s: f32,
) -> Option<Rect> {
    let accent = color_of(map, theme, id);
    let radius = kind.radius() * s;
    p.add(soft_shadow(
        rect,
        radius,
        16.0 * s,
        48.0 * s,
        Color32::from_black_alpha(if theme.dark { 150 } else { 60 }),
    ));
    p.rect_filled(rect, radius, theme.card);
    p.rect_stroke(
        rect,
        radius,
        Stroke::new((1.5 * s).max(0.5), accent.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    match kind {
        // A coloured bar along the top edge
        Kind::Title | Kind::Main => {
            let bar = Rect::from_min_size(
                rect.min + Vec2::new(radius, -3.0 * s),
                Vec2::new(rect.width() - 2.0 * radius, 6.0 * s),
            );
            p.rect_filled(bar, 3.0 * s, accent);
        }
        // … or on the left
        Kind::Child => {
            let bar = Rect::from_min_size(
                rect.min + Vec2::new(-2.0 * s, 16.0 * s),
                Vec2::new(4.0 * s, (rect.height() - 32.0 * s).max(0.0)),
            );
            p.rect_filled(bar, 2.0 * s, accent);
        }
    }
    let mut check = None;
    for (r, piece) in layout.pieces {
        let r = r.translate(rect.min.to_vec2());
        match piece {
            Piece::Text(g) => {
                // Centred text is laid out around x = 0
                let pos = r.min - g.rect.min.to_vec2();
                p.galley(pos, g, theme.text);
            }
            Piece::Image(key) => match map.images.get(&key) {
                Some(img) => {
                    images::paint(
                        p,
                        ctx,
                        &key,
                        img,
                        r,
                        Fit::Cover,
                        12.0 * s,
                        Color32::WHITE,
                        1.0,
                    );
                }
                None => images::placeholder(p, r, 12.0 * s),
            },
            Piece::Callout(kind) => {
                let color = crate::callout::look(kind).0;
                p.rect_filled(r, 10.0 * s, crate::callout::fill(kind, theme));
                p.rect_filled(
                    Rect::from_min_size(
                        r.min + Vec2::new(0.0, 8.0 * s),
                        Vec2::new(3.5 * s, (r.height() - 16.0 * s).max(0.0)),
                    ),
                    1.75 * s,
                    color,
                );
            }
            Piece::Check(done) => {
                paint_checkbox(p, r, done, accent, theme.text.gamma_multiply(0.7));
                check = Some(r);
            }
        }
    }
    check
}

// ---------------------------------------------------------------------------
// The world
// ---------------------------------------------------------------------------

/// A sub-point floating around its station
struct Satellite {
    id: u64,
    /// Offset from the station centre
    offset: Vec3,
    /// Phase of the gentle bobbing
    phase: f32,
}

struct Station {
    stop: Stop,
    center: Vec3,
    /// Node of the big card (the root at the start, otherwise the branch)
    card: u64,
    kind: Kind,
    satellites: Vec<Satellite>,
    /// Area of the whole group around `center` (world units)
    bounds: Rect,
}

/// Where a station lies: on a spiral that leads into the depth
fn station_center(k: usize) -> Vec3 {
    let k = k as f32;
    Vec3::new(
        1500.0 * (k * 0.85).sin(),
        650.0 * (k * 1.3).sin(),
        k * STEP_Z,
    )
}

/// Pseudo-random 0…1, the same for the same input
fn hash(n: u64) -> f32 {
    let x = (n as f32 * 12.9898).sin() * 43758.547;
    x - x.floor()
}

fn build_world(ctx: &egui::Context, map: &MindMap, theme: &Theme, all: &[Stop]) -> Vec<Station> {
    let mut stations = Vec::new();
    for (k, stop) in all.iter().enumerate() {
        let (card, kind) = match stop {
            Stop::Overview => match map.root() {
                Some(r) => (r, Kind::Title),
                None => continue,
            },
            Stop::Branch(b) => (*b, Kind::Main),
            Stop::Finale => continue,
        };
        let main = layout_card(ctx, map, theme, card, kind, 1.0).size;
        let mut bounds = Rect::from_center_size(Pos2::ZERO, main);
        let mut satellites = Vec::new();
        if kind == Kind::Main {
            // Sub-points in two loose columns, left and right of the card
            let children: Vec<u64> = map
                .children(card)
                .filter(|n| !map.is_hidden(n.id))
                .map(|n| n.id)
                .collect();
            let sizes: Vec<Vec2> = children
                .iter()
                .map(|id| layout_card(ctx, map, theme, *id, Kind::Child, 1.0).size)
                .collect();
            let gap = 46.0;
            for side in [1.0f32, -1.0] {
                let column: Vec<usize> = (0..children.len())
                    .filter(|i| (i % 2 == 0) == (side > 0.0))
                    .collect();
                let height: f32 = column.iter().map(|i| sizes[*i].y).sum::<f32>()
                    + gap * column.len().saturating_sub(1) as f32;
                let mut y = -height / 2.0;
                for (row, i) in column.iter().enumerate() {
                    let id = children[*i];
                    let size = sizes[*i];
                    let jitter = (hash(id) - 0.5) * 90.0;
                    let x = side * (main.x / 2.0 + 110.0 + size.x / 2.0 + jitter.abs() * 0.6)
                        + if row % 2 == 1 { side * 40.0 } else { 0.0 };
                    let center = Vec2::new(x, y + size.y / 2.0 + jitter * 0.3);
                    // Some a bit closer, some further away: parallax while flying
                    let z = (hash(id + 7) - 0.4) * 420.0;
                    satellites.push(Satellite {
                        id,
                        offset: Vec3::new(center.x, center.y, z),
                        phase: hash(id + 3) * std::f32::consts::TAU,
                    });
                    bounds = bounds.union(Rect::from_center_size(center.to_pos2(), size));
                    y += size.y + gap;
                }
            }
        }
        stations.push(Station {
            stop: *stop,
            center: station_center(k),
            card,
            kind,
            satellites,
            bounds,
        });
    }
    stations
}

/// The grid at the end: where each branch card goes, how much it shrinks,
/// and the camera that sees all of it
struct Finale {
    places: Vec<(u64, Vec3, f32)>,
    camera: Vec3,
    distance: f32,
}

fn finale(
    ctx: &egui::Context,
    map: &MindMap,
    theme: &Theme,
    stations: &[Station],
    area: Rect,
    focal: f32,
) -> Finale {
    let branches: Vec<&Station> = stations.iter().filter(|s| s.kind == Kind::Main).collect();
    let n = branches.len().max(1);
    let gap = Vec2::new(90.0, 70.0);
    let cell_w = Kind::Main.width();
    // Every card as wide as possible, pictures not too tall
    let cards: Vec<(u64, Vec2, f32)> = branches
        .iter()
        .map(|st| {
            let size = layout_card(ctx, map, theme, st.card, Kind::Main, 1.0).size;
            let factor = (420.0 / size.y).min(1.0);
            (st.card, size * factor, factor)
        })
        .collect();
    let usable = usable(area);
    let typical_h = cards.iter().map(|c| c.1.y).sum::<f32>() / n as f32 + gap.y;
    let aspect = usable.width() / usable.height();
    let cols = ((n as f32 * aspect * typical_h / (cell_w + gap.x))
        .sqrt()
        .ceil() as usize)
        .clamp(1, n);
    let rows: Vec<&[(u64, Vec2, f32)]> = cards.chunks(cols).collect();
    let row_heights: Vec<f32> = rows
        .iter()
        .map(|r| r.iter().map(|c| c.1.y).fold(0.0, f32::max))
        .collect();
    let title_size = stations.iter().find(|s| s.kind == Kind::Title).map(|t| {
        (
            t.card,
            layout_card(ctx, map, theme, t.card, Kind::Title, 1.0).size,
        )
    });
    let title_h = title_size.map_or(0.0, |(_, size)| size.y * 0.8 + gap.y);
    let grid = Vec2::new(
        cols as f32 * cell_w + (cols - 1) as f32 * gap.x,
        title_h + row_heights.iter().sum::<f32>() + rows.len().saturating_sub(1) as f32 * gap.y,
    );
    let last_z = stations.last().map_or(0.0, |s| s.center.z);
    let center = Vec3::new(0.0, 0.0, last_z + STEP_Z * 1.4);
    let mut places = Vec::new();
    let top = center.y - grid.y / 2.0;
    // The title above the grid, a little smaller
    if let Some((card, size)) = title_size {
        places.push((card, Vec3::new(center.x, top + size.y * 0.4, center.z), 0.8));
    }
    let mut y = top + title_h;
    for (row, h) in rows.iter().zip(&row_heights) {
        for (col, (card, size, factor)) in row.iter().enumerate() {
            // An incomplete last row is centred; cards of a row share their top edge
            let x = (col as f32 - (row.len() as f32 - 1.0) / 2.0) * (cell_w + gap.x);
            places.push((
                *card,
                Vec3::new(center.x + x, y + size.y / 2.0, center.z),
                *factor,
            ));
        }
        y += h + gap.y;
    }
    let distance = focal * (grid.x / usable.width()).max(grid.y / usable.height()) * 1.04;
    let shift = (usable.center() - area.center()) * distance / focal;
    Finale {
        places,
        camera: Vec3::new(center.x - shift.x, center.y - shift.y, center.z - distance),
        distance,
    }
}

/// Where cards may go: everything apart from a margin for the labels and buttons
fn usable(area: Rect) -> Rect {
    Rect::from_min_max(
        area.min + Vec2::new(70.0, 80.0),
        area.max - Vec2::new(70.0, 70.0),
    )
}

/// Camera position looking at a station so the whole group fills the screen
fn station_camera(st: &Station, area: Rect, focal: f32) -> (Vec3, f32) {
    let usable = usable(area);
    let b = if st.kind == Kind::Title {
        // The title card alone, but not too big
        st.bounds
            .expand2(Vec2::new(st.bounds.width() * 0.35, 120.0))
    } else {
        st.bounds
    };
    let distance = focal * (b.width() / usable.width()).max(b.height() / usable.height()) * 1.05;
    let shift = (usable.center() - area.center()) * distance / focal;
    let c = st.center.xy() + b.center().to_vec2() - shift;
    (Vec3::new(c.x, c.y, st.center.z - distance), distance)
}

fn target(
    all: &[Stop],
    stop: usize,
    stations: &[Station],
    fin: &Finale,
    area: Rect,
    focal: f32,
) -> (Vec3, f32) {
    match all.get(stop) {
        Some(Stop::Finale) => (fin.camera, fin.distance),
        Some(s) => stations
            .iter()
            .find(|st| st.stop == *s)
            .map(|st| station_camera(st, area, focal))
            .unwrap_or((Vec3::new(0.0, 0.0, -2000.0), 2000.0)),
        None => (Vec3::new(0.0, 0.0, -2000.0), 2000.0),
    }
}

/// Camera on the way: glides in an arc – it pulls back and rises a little on long
/// distances, so stations in between rush past at the edge of the picture
fn fly3(from: Vec3, to: Vec3, t: f32) -> Vec3 {
    let e = ease_in_out_cubic(t);
    let mut p = from.lerp(to, e);
    let dist = to.sub(from).length();
    let arc = (PI * e).sin();
    p.z -= arc * (dist * 0.22).min(5200.0);
    p.y -= arc * (dist * 0.05).min(700.0);
    p
}

/// Soft step from 0 (at `a`) to 1 (at `b`)
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Current camera (flight, then an optional slow drift)
fn camera(tour: &TourState, to: Vec3, distance: f32, now: f64, fx: &Effects) -> Vec3 {
    let t = ((now - tour.started) / FLIGHT).clamp(0.0, 1.0) as f32;
    let mut cam = fly3(Vec3::from_array(tour.from3), to, t);
    if fx.drift {
        let d = ease_in_out_cubic(((now - tour.started - FLIGHT) / 30.0).clamp(0.0, 1.0) as f32);
        cam.z += distance * 0.07 * d;
        cam.x += (now * 0.11).sin() as f32 * distance * 0.006 * d;
        cam.y += (now * 0.07).cos() as f32 * distance * 0.006 * d;
    }
    cam
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

struct Drawn {
    id: u64,
    kind: Kind,
    pos: Vec3,
    /// Extra scale (sub-points popping up, cards shrinking into the grid)
    factor: f32,
    alpha: f32,
    /// The card of the current stop
    focus: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    theme: &Theme,
    fx: &mut Effects,
) -> Option<Event> {
    let ctx = ui.ctx().clone();
    let area = ui.max_rect();
    let now = ui.input(|i| i.time);
    let focal = area.height().max(200.0);
    let all = stops(&doc.map);
    let last = all.len() - 1;
    doc.tour.stop = doc.tour.stop.min(last);
    doc.tour.prev_stop = doc.tour.prev_stop.min(last);
    // Free flight shows every level at once
    doc.tour.level = 0;

    let stations = build_world(&ctx, &doc.map, theme, &all);
    let fin = finale(&ctx, &doc.map, theme, &stations, area, focal);

    // Switched over from the mind map style (or back after a while): start at the stop
    let stale = doc
        .tour
        .last_cam3
        .is_none_or(|(_, then)| (now - then).abs() > 1.0);
    if doc.tour.needs_init || stale {
        let (cam, _) = target(&all, doc.tour.stop, &stations, &fin, area, focal);
        doc.tour.from3 = cam.to_array();
        doc.tour.prev_stop = doc.tour.stop;
        doc.tour.started = now - FLIGHT;
        doc.tour.needs_init = false;
    }
    let (to, distance) = target(&all, doc.tour.stop, &stations, &fin, area, focal);
    let cam_before = camera(&doc.tour, to, distance, now, fx);

    // Keys (shared with the mind map tour); a new flight starts where the camera is now
    let nav = read_nav(ui);
    let mut event = navigate(doc, &all, &nav, Camera::default(), now, false);
    if doc.tour.started == now {
        doc.tour.from3 = cam_before.to_array();
    }
    let (to, distance) = target(&all, doc.tour.stop, &stations, &fin, area, focal);
    let cam = camera(&doc.tour, to, distance, now, fx);
    let t = ((now - doc.tour.started) / FLIGHT).clamp(0.0, 1.0) as f32;
    let arrived = (now - doc.tour.started - FLIGHT) as f32;
    ctx.request_repaint();

    let project = |p: Vec3| -> Option<(Pos2, f32)> {
        let dz = p.z - cam.z;
        (dz > NEAR).then(|| {
            let k = focal / dz;
            (area.center() + (p.xy() - cam.xy()) * k, k)
        })
    };
    // Haze: very close cards fade out (we fly through them), far ones fade into the dark
    let fog = |dz: f32| {
        smooth(0.12, 0.42, dz / distance) * (1.0 - 0.7 * smooth(3.0, 12.0, dz / distance))
    };

    // Background: stars with streaks while flying
    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);
    let mut warp = Vec2::ZERO;
    if let Some((prev, then)) = doc.tour.last_cam3 {
        let dt = (now - then) as f32;
        if dt > 0.0 && dt < 0.2 {
            let v = cam.sub(Vec3::from_array(prev));
            let lateral = v.xy() * (focal / distance) / dt;
            let forward = v.z / distance / dt;
            let strength = if fx.warp { 1.0 } else { 0.55 };
            warp = ((-lateral * 0.03 + Vec2::splat(forward * 30.0)) * strength)
                .clamp(Vec2::splat(-110.0), Vec2::splat(110.0));
        }
    }
    doc.tour.last_cam3 = Some((cam.to_array(), now));
    if fx.background != Backdrop::Plain {
        backdrop(
            &painter,
            area,
            theme,
            fx.background,
            now,
            -cam.xy() * (focal / distance) * 0.12,
            warp,
        );
    }

    let map = &doc.map;
    let spot = ease_out_cubic(t);
    let current = all[doc.tour.stop];
    let previous = all[doc.tour.prev_stop];
    // How far the cards have moved into the final grid (0 = at their stations)
    let finale_mix = match (current, previous) {
        (Stop::Finale, _) => ease_in_out_cubic(t),
        (_, Stop::Finale) => 1.0 - ease_in_out_cubic(t),
        _ => 0.0,
    };

    // Faint trail along the route
    let route: Vec<Vec3> = stations.iter().map(|s| s.center).collect();
    for pair in route.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let mid = a.lerp(b, 0.5).add(Vec3::new(0.0, -260.0, 0.0));
        for i in 0..28 {
            let u = i as f32 / 28.0;
            // Quadratic curve through a raised midpoint
            let p = a.lerp(mid, u).lerp(mid.lerp(b, u), u);
            if let Some((pos, k)) = project(p) {
                let a = fog(p.z - cam.z) * 0.35 * (1.0 - finale_mix);
                if a > 0.01 && area.expand(20.0).contains(pos) {
                    painter.circle_filled(
                        pos,
                        (5.0 * k).clamp(0.8, 4.0),
                        theme.text.gamma_multiply(a),
                    );
                }
            }
        }
    }

    // Everything that floats, then drawn from far to near
    let mut drawn: Vec<Drawn> = Vec::new();
    let lit = |stop: Stop, own: Stop| -> f32 {
        match stop {
            Stop::Finale => 1.0,
            s if s == own => 1.0,
            _ => 0.42,
        }
    };
    for st in &stations {
        let before = lit(previous, st.stop);
        let after = lit(current, st.stop);
        let light = before + (after - before) * spot;
        let bob = |phase: f32, amount: f32| {
            Vec3::new(
                (now as f32 * 0.6 + phase).cos() * amount * 0.6,
                (now as f32 * 0.8 + phase).sin() * amount,
                0.0,
            )
        };
        // The station card, on its way into the grid at the end
        let place = fin.places.iter().find(|(id, ..)| *id == st.card);
        let order = fin
            .places
            .iter()
            .position(|(id, ..)| *id == st.card)
            .unwrap_or(0) as f32;
        let mix = if finale_mix > 0.0 {
            let stagger = order / fin.places.len().max(1) as f32 * 0.35;
            ease_in_out_cubic(((finale_mix - stagger) / (1.0 - 0.35)).clamp(0.0, 1.0))
        } else {
            0.0
        };
        let home = st.center.add(bob(st.card as f32, 8.0));
        let (pos, factor) = match place {
            Some((_, grid, f)) if mix > 0.0 => (home.lerp(*grid, mix), 1.0 + (f - 1.0) * mix),
            _ => (home, 1.0),
        };
        drawn.push(Drawn {
            id: st.card,
            kind: st.kind,
            pos,
            factor,
            alpha: light,
            focus: st.stop == current,
        });
        // Sub-points pop out of their card when the station is reached
        for sat in &st.satellites {
            let grow = growth(map, &all, &doc.tour, sat.id, now, false);
            if grow <= 0.0 {
                continue;
            }
            let g = ease_out_cubic(grow);
            let spot_pos = st.center.add(sat.offset).add(bob(sat.phase, 14.0));
            drawn.push(Drawn {
                id: sat.id,
                kind: Kind::Child,
                pos: st.center.lerp(spot_pos, g),
                factor: if grow < 1.0 {
                    ease_out_back(grow).max(0.05)
                } else {
                    1.0
                },
                alpha: light * grow.min(1.0) * (1.0 - finale_mix),
                focus: false,
            });
        }
    }
    drawn.sort_by(|a, b| b.pos.z.total_cmp(&a.pos.z));

    let mut ticked = None;
    let breathe = 0.75 + 0.25 * (now * 2.2).sin() as f32;
    for d in &drawn {
        let Some((center, k)) = project(d.pos) else {
            continue;
        };
        let alpha = d.alpha * fog(d.pos.z - cam.z);
        if alpha < 0.01 {
            continue;
        }
        let s = k * d.factor;
        // Laid out at a fixed text step and scaled, so zooming re-uses the layout.
        // Capped: titles are 66 pt at scale 1, and huge glyphs overflow the font atlas.
        let step = crate::nodes::text_step(s).min(MAX_CARD_TEXT_SCALE);
        let layout = layout_card(&ctx, map, theme, d.id, d.kind, step).scaled(s / step);
        let rect = Rect::from_center_size(center, layout.size);
        if !rect.intersects(area.expand(40.0)) {
            continue;
        }
        let accent = color_of(map, theme, d.id);
        // Glow around the card of the current stop
        if fx.glow && d.focus && finale_mix == 0.0 {
            let radius = d.kind.radius() * s;
            for i in 1..=12 {
                let grow = i as f32 * 3.0 * s;
                painter.rect_filled(
                    rect.expand(grow),
                    radius + grow,
                    accent.gamma_multiply(0.02 * breathe * spot * alpha),
                );
            }
        }
        let mut p = painter.clone();
        p.set_opacity(alpha);
        let check = paint_card(&p, &ctx, map, theme, d.id, d.kind, layout, rect, s);
        if let Some(r) = check {
            if alpha > 0.5 {
                let click = ui
                    .interact(
                        r.expand(6.0 * s),
                        ui.id().with(("flight_task", d.id)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if click.clicked() {
                    ticked = Some((d.id, r.center()));
                }
            }
        }
        // A ring that spreads from the card on arrival
        if fx.ripple && d.focus && (0.0..1.4).contains(&arrived) && finale_mix == 0.0 {
            for (delay, width) in [(0.0, 3.0_f32), (0.25, 2.0)] {
                let a = ((arrived - delay) / (1.4 - delay)).clamp(0.0, 1.0);
                if a > 0.0 {
                    let grow = 400.0 * s * ease_out_cubic(a);
                    painter.rect_stroke(
                        rect.expand(grow),
                        d.kind.radius() * s + grow,
                        Stroke::new(width, accent.gamma_multiply(0.5 * (1.0 - a))),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
    }

    stop_label(&painter, area, theme, map, &all, &doc.tour, false);
    if doc.tour.stop == 0 && arrived > 0.0 && last > 0 {
        // A small hint at the start
        let fade = (arrived / 0.6).clamp(0.0, 1.0) * 0.55;
        painter.text(
            Pos2::new(area.center().x, area.bottom() - 34.0),
            Align2::CENTER_CENTER,
            "Space · → to take off",
            FontId::proportional(15.0),
            theme.text.gamma_multiply(fade),
        );
    }
    if let Some(e) = controls(ui, area, doc, theme, fx, &all, &nav, Camera::default(), now) {
        event = Some(e);
    }
    if doc.tour.started == now {
        doc.tour.from3 = cam.to_array();
    }
    if let Some((id, pos)) = ticked {
        tick(&ctx, doc, theme, fx, id, pos, now);
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_flight_arcs_and_lands_exactly() {
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(1000.0, 0.0, 10_000.0);
        assert_eq!(fly3(from, to, 0.0), from);
        let end = fly3(from, to, 1.0);
        assert!(end.sub(to).length() < 1e-2);
        // Half way the camera has pulled back and risen a little
        let mid = fly3(from, to, 0.5);
        assert!(mid.z < 5000.0 - 1000.0);
        assert!(mid.y < 0.0);
        // Stations lie further and further in the depth
        assert!(station_center(3).z > station_center(2).z);
        assert_eq!(station_center(0), Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn every_branch_becomes_a_station_with_floating_sub_points() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let _ = ctx.run(Default::default(), |_| {});
        let map = MindMap::from_outline(include_str!("../assets/examples/universe.md")).unwrap();
        let theme = &crate::theme::THEMES[1];
        let all = stops(&map);
        let world = build_world(&ctx, &map, theme, &all);
        // Title plus 16 branches; the finale is no station of its own
        assert_eq!(world.len(), 17);
        assert_eq!(world[0].kind, Kind::Title);
        let solar = world
            .iter()
            .find(|s| {
                map.get(s.card)
                    .is_some_and(|n| n.text == "Our Solar System")
            })
            .unwrap();
        assert_eq!(solar.satellites.len(), 4);
        // The sub-points float left and right of the card, without overlapping it
        let card = layout_card(&ctx, &map, theme, solar.card, Kind::Main, 1.0).size;
        for sat in &solar.satellites {
            assert!(sat.offset.x.abs() > card.x / 2.0);
        }
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0));
        let fin = finale(&ctx, &map, theme, &world, area, 900.0);
        // Title and every branch get a place in the grid, in front of the camera
        assert_eq!(fin.places.len(), 17);
        assert!(fin.places.iter().all(|(_, p, _)| p.z > fin.camera.z));
    }
}
