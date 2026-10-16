//! Camera tour through the mind map.
//!
//! Stops: overview → every main branch → the whole mind map. Between the
//! stops the camera flies in an arc (first out a little, then in), the
//! current branch is in the spotlight and its sub-points grow out of it
//! – optionally level by level on a key press. The stop's note
//! appears as a subtitle. Optional extras: glow, a wave on arrival,
//! light pulses on the connections and a slight spring of the camera.
//!
//! The other tour style, *Free flight*, lives in `flight.rs`; it shares the stops,
//! the keyboard handling, the subtitle and the controls from here.

use crate::document::{Camera, Document, TourState};
use crate::model::MindMap;
use crate::nodes::{
    label_job, node_size, node_style, paint_label, paint_node, paint_task, paint_thumb,
};
use crate::present::{
    backdrop, color_of, confetti_burst, confetti_draw, corner_buttons, ease_in_out_cubic,
    ease_out_back, ease_out_cubic, filmstrip, laser, read_nav, strip_keys, vignette, Backdrop,
    Effects, Event, Nav, StripItem, StripKey, TourStyle,
};
use crate::theme::Theme;
use eframe::egui::{
    self, epaint::CubicBezierShape, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2,
};
use std::collections::{HashMap, HashSet};

/// Duration of a camera flight in seconds
pub(crate) const FLIGHT: f64 = 1.1;
/// When (after the flight starts) the sub-points start to grow
const GROW_DELAY: f64 = 0.7;
/// When revealing level by level it starts sooner
const LEVEL_DELAY: f64 = 0.3;
/// Delay between two growing sub-points
const GROW_STEP: f64 = 0.1;
/// How long a sub-point takes to grow
const GROW_TIME: f64 = 0.4;
/// Brightness outside the spotlight
const DIM: f32 = 0.22;
/// Room at the bottom for the subtitle
const BOTTOM_SPACE: f32 = 150.0;
/// Duration of the wave on arrival
const RIPPLE: f32 = 1.4;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Stop {
    Overview,
    Branch(u64),
    Finale,
}

pub(crate) fn stops(map: &MindMap) -> Vec<Stop> {
    let Some(root) = map.root() else {
        return vec![Stop::Overview];
    };
    let branches: Vec<Stop> = map.children(root).map(|n| Stop::Branch(n.id)).collect();
    if branches.is_empty() {
        return vec![Stop::Overview];
    }
    let mut out = vec![Stop::Overview];
    out.extend(branches);
    out.push(Stop::Finale);
    out
}

/// Level of a node below its main branch (children of the branch = 1)
fn level_in_branch(map: &MindMap, id: u64) -> usize {
    map.depth(id).saturating_sub(1)
}

/// Deepest visible level below a main branch
fn max_level(map: &MindMap, branch: u64) -> usize {
    map.subtree(branch)
        .into_iter()
        .filter(|id| !map.is_hidden(*id))
        .map(|id| level_in_branch(map, id))
        .max()
        .unwrap_or(0)
}

/// How visible a node is at the current stop (0 = not at all, 1 = fully).
/// `stepwise`: sub-points are revealed level by level (`tour.level`).
pub(crate) fn growth(
    map: &MindMap,
    all: &[Stop],
    tour: &TourState,
    id: u64,
    now: f64,
    stepwise: bool,
) -> f32 {
    if map.depth(id) <= 1 {
        return 1.0;
    }
    let Some(branch) = map.branch_of(id) else {
        return 1.0;
    };
    let Some(branch_stop) = all.iter().position(|s| *s == Stop::Branch(branch)) else {
        return 1.0;
    };
    let current = tour.stop;
    if all[current] == Stop::Finale || current > branch_stop {
        return 1.0;
    }
    if current < branch_stop {
        return 0.0;
    }

    let subtree = map.subtree(branch);
    if stepwise {
        let level = level_in_branch(map, id);
        if level > tour.level {
            return 0.0;
        }
        if level < tour.level || tour.backwards {
            return 1.0;
        }
        // The new level grows out one after another
        let order = subtree
            .iter()
            .filter(|n| level_in_branch(map, **n) == level)
            .position(|n| *n == id)
            .unwrap_or(0);
        let start = tour.started + LEVEL_DELAY + order as f64 * GROW_STEP;
        return ((now - start) / GROW_TIME).clamp(0.0, 1.0) as f32;
    }

    if tour.backwards {
        return 1.0;
    }
    // Order within the branch: level by level
    let order = subtree
        .iter()
        .position(|n| *n == id)
        .unwrap_or(1)
        .saturating_sub(1);
    let start = tour.started + GROW_DELAY + order as f64 * GROW_STEP;
    ((now - start) / GROW_TIME).clamp(0.0, 1.0) as f32
}

/// Nodes that should be in view at a stop. When revealing step by step,
/// only the visible levels plus the next one, so there is room for it.
fn framed(map: &MindMap, stop: Stop, level: Option<usize>) -> Vec<u64> {
    let visible = |id: &u64| !map.is_hidden(*id);
    match stop {
        Stop::Overview => map
            .nodes
            .iter()
            .filter(|n| map.depth(n.id) <= 1)
            .map(|n| n.id)
            .collect(),
        Stop::Branch(b) => map
            .subtree(b)
            .into_iter()
            .filter(visible)
            .filter(|id| level.is_none_or(|l| level_in_branch(map, *id) <= l + 1))
            .collect(),
        Stop::Finale => map.nodes.iter().map(|n| n.id).filter(visible).collect(),
    }
}

/// Camera that shows the nodes of a stop filling the screen
fn target(
    map: &MindMap,
    stop: Stop,
    level: Option<usize>,
    sizes: &HashMap<u64, Vec2>,
    area: Rect,
) -> Camera {
    let mut bounds = Rect::NOTHING;
    for id in framed(map, stop, level) {
        let size = sizes.get(&id).copied().unwrap_or(Vec2::splat(60.0));
        bounds = bounds.union(Rect::from_center_size(map.pos(id).to_pos2(), size));
    }
    if !bounds.is_positive() {
        return Camera {
            center: Vec2::ZERO,
            zoom: 1.0,
        };
    }
    let usable = Rect::from_min_max(
        area.min + Vec2::new(80.0, 70.0),
        area.max - Vec2::new(80.0, BOTTOM_SPACE),
    );
    let zoom = (usable.width() / bounds.width())
        .min(usable.height() / bounds.height())
        .clamp(0.15, 1.6);
    // The centre of the usable area is a little above the middle of the screen
    let shift = (usable.center() - area.center()) / zoom;
    Camera {
        center: bounds.center().to_vec2() - shift,
        zoom,
    }
}

/// Camera on the way: the centre glides, the zoom changes evenly, and on
/// long distances it first moves out a little in an arc. `bounce`: shortly before the
/// target a small zoom nudge that springs back softly.
fn fly(from: Camera, to: Camera, t: f32, area: Rect, bounce: bool) -> Camera {
    let e = ease_in_out_cubic(t);
    let center = from.center + (to.center - from.center) * e;
    let zoom = (from.zoom.ln() + (to.zoom.ln() - from.zoom.ln()) * e).exp();
    let distance = (to.center - from.center).length() * from.zoom.min(to.zoom) / area.width();
    let dip = 1.0 + distance.min(2.0) * 0.35 * (std::f32::consts::PI * e).sin();
    let punch = if bounce {
        let b = ((t - 0.7) / 0.3).clamp(0.0, 1.0);
        1.0 + 0.07 * (std::f32::consts::PI * b).sin()
    } else {
        1.0
    };
    Camera {
        center,
        zoom: zoom / dip * punch,
    }
}

/// Current camera: flight to the stop, optionally a slow drift afterwards
fn camera(tour: &TourState, to: Camera, now: f64, area: Rect, fx: &Effects) -> Camera {
    let t = ((now - tour.started) / FLIGHT).clamp(0.0, 1.0) as f32;
    let mut cam = fly(tour.from, to, t, area, fx.bounce);
    if fx.drift {
        // After arriving, zoom in very slightly over ~30 s and wander a little
        let d = ease_in_out_cubic(((now - tour.started - FLIGHT) / 30.0).clamp(0.0, 1.0) as f32);
        cam.zoom *= 1.0 + 0.06 * d;
        let wobble = Vec2::new((now * 0.11).sin() as f32, (now * 0.07).cos() as f32);
        cam.center += wobble * (10.0 / cam.zoom) * d;
    }
    cam
}

/// New flight: to another stop or (when revealing) to another level
fn fly_to(tour: &mut TourState, stop: usize, level: usize, backwards: bool, cam: Camera, now: f64) {
    if stop != tour.stop {
        crate::sound::play(crate::sound::Sound::Swish);
    }
    tour.from = cam;
    tour.backwards = backwards;
    tour.prev_stop = tour.stop;
    tour.stop = stop;
    tour.level = level;
    tour.started = now;
}

fn brightness(map: &MindMap, stop: Stop, id: u64, cache: &mut HashMap<u64, HashSet<u64>>) -> f32 {
    match stop {
        Stop::Branch(b) => {
            let lit = cache
                .entry(b)
                .or_insert_with(|| map.subtree(b).into_iter().collect());
            if lit.contains(&id) {
                1.0
            } else {
                DIM
            }
        }
        _ => 1.0,
    }
}

pub(crate) fn title_of(map: &MindMap, stop: Stop) -> String {
    match stop {
        Stop::Overview => map
            .root()
            .and_then(|r| map.get(r))
            .map(|n| n.text.clone())
            .unwrap_or_default(),
        Stop::Branch(b) => map.get(b).map(|n| n.text.clone()).unwrap_or_default(),
        Stop::Finale => "The whole picture".into(),
    }
}

fn note_of(map: &MindMap, stop: Stop) -> String {
    let id = match stop {
        Stop::Overview => map.root(),
        Stop::Branch(b) => Some(b),
        Stop::Finale => None,
    };
    id.and_then(|id| map.get(id))
        .map(|n| n.note.trim().to_string())
        .unwrap_or_default()
}

/// Point on a cubic Bézier curve (u = 0…1)
fn bezier(p: [Pos2; 4], u: f32) -> Pos2 {
    let v = 1.0 - u;
    let w = [v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u];
    Pos2::new(
        p.iter().zip(w).map(|(p, w)| p.x * w).sum(),
        p.iter().zip(w).map(|(p, w)| p.y * w).sum(),
    )
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    theme: &Theme,
    fx: &mut Effects,
) -> Option<Event> {
    if fx.tour_style == TourStyle::Free {
        return crate::flight::show(ui, doc, theme, fx);
    }
    let ctx = ui.ctx().clone();
    let area = ui.max_rect();
    let now = ui.input(|i| i.time);
    let stepwise = fx.step_levels;
    let map = &doc.map;
    let all = stops(map);
    let last = all.len() - 1;
    doc.tour.stop = doc.tour.stop.min(last);
    doc.tour.prev_stop = doc.tour.prev_stop.min(last);
    let level_opt = |tour: &TourState| stepwise.then_some(tour.level);

    let sizes: HashMap<u64, Vec2> = map
        .nodes
        .iter()
        .map(|n| (n.id, node_size(&ctx, map, theme, n)))
        .collect();

    if doc.tour.needs_init {
        doc.tour.from = target(map, all[doc.tour.stop], level_opt(&doc.tour), &sizes, area);
        doc.tour.prev_stop = doc.tour.stop;
        doc.tour.started = now - FLIGHT;
        doc.tour.needs_init = false;
    }

    let to = target(map, all[doc.tour.stop], level_opt(&doc.tour), &sizes, area);
    let cam = camera(&doc.tour, to, now, area, fx);
    let nav = read_nav(ui);
    let mut event = navigate(doc, &all, &nav, cam, now, stepwise);
    let map = &doc.map;

    // The camera may just have changed because of a key press
    let t = ((now - doc.tour.started) / FLIGHT).clamp(0.0, 1.0) as f32;
    let to = target(map, all[doc.tour.stop], level_opt(&doc.tour), &sizes, area);
    let cam = camera(&doc.tour, to, now, area, fx);
    let to_screen = |p: Vec2| area.center() + (p - cam.center) * cam.zoom;
    let arrived = (now - doc.tour.started - FLIGHT) as f32;
    ctx.request_repaint();

    // Background: animated (with parallax to the camera) or dot grid
    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);
    let plain = fx.background == Backdrop::Plain;
    // Camera movement since the last frame → warp streaks on fast flights
    let mut warp = Vec2::ZERO;
    if let Some((prev, then)) = doc.tour.last_cam {
        let dt = (now - then) as f32;
        if fx.warp && dt > 0.0 && dt < 0.2 {
            let velocity = (cam.center - prev.center) * cam.zoom / dt;
            let zooming = (cam.zoom / prev.zoom).ln() / dt;
            warp = (-velocity * 0.03 + Vec2::splat(zooming * 25.0))
                .clamp(Vec2::splat(-90.0), Vec2::splat(90.0));
        }
    }
    doc.tour.last_cam = Some((cam, now));
    if !plain {
        backdrop(
            &painter,
            area,
            theme,
            fx.background,
            now,
            -cam.center * 0.6,
            warp,
        );
    }
    let origin = to_screen(Vec2::ZERO);
    let mut spacing = 28.0 * cam.zoom;
    while spacing < 14.0 {
        spacing *= 2.0;
    }
    let mut y = area.top() + (origin.y - area.top()).rem_euclid(spacing);
    while plain && y < area.bottom() {
        let mut x = area.left() + (origin.x - area.left()).rem_euclid(spacing);
        while x < area.right() {
            painter.circle_filled(Pos2::new(x, y), 1.3, theme.grid);
            x += spacing;
        }
        y += spacing;
    }

    // For every node: how far grown, how bright, where right now
    let spot = ease_out_cubic(t);
    let mut lit_cache = HashMap::new();
    let focus = match all[doc.tour.stop] {
        Stop::Branch(b) => Some(b),
        _ => map.root(),
    };
    let in_spotlight: HashSet<u64> = match all[doc.tour.stop] {
        Stop::Branch(b) => map.subtree(b).into_iter().collect(),
        _ => HashSet::new(),
    };
    struct Drawn {
        id: u64,
        parent: Option<u64>,
        center: Pos2,
        grow: f32,
        alpha: f32,
    }
    let mut drawn: Vec<Drawn> = Vec::new();
    for n in &map.nodes {
        if map.is_hidden(n.id) {
            continue;
        }
        let grow = growth(map, &all, &doc.tour, n.id, now, stepwise);
        if grow <= 0.0 {
            continue;
        }
        let before = brightness(map, all[doc.tour.prev_stop], n.id, &mut lit_cache);
        let after = brightness(map, all[doc.tour.stop], n.id, &mut lit_cache);
        let light = before + (after - before) * spot;
        // Sub-points grow out of their parent node
        let pos = match n.parent {
            Some(p) if grow < 1.0 => {
                map.pos(p) + (map.pos(n.id) - map.pos(p)) * ease_out_cubic(grow)
            }
            _ => map.pos(n.id),
        };
        drawn.push(Drawn {
            id: n.id,
            parent: n.parent,
            center: to_screen(pos),
            grow,
            alpha: light * grow.min(1.0),
        });
    }
    let centers: HashMap<u64, Pos2> = drawn.iter().map(|d| (d.id, d.center)).collect();

    // Connections, optionally with light pulses on them
    for d in &drawn {
        let Some(a) = d.parent.and_then(|p| centers.get(&p)) else {
            continue;
        };
        let style = node_style(map, theme, d.id);
        let b = d.center;
        let dx = (b.x - a.x) * 0.5;
        let points = [*a, *a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0), b];
        painter.add(CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            Stroke::new(
                style.edge_width * cam.zoom.clamp(0.5, 2.0),
                style.accent.gamma_multiply(0.8 * d.alpha),
            ),
        ));
        let pulsing =
            in_spotlight.contains(&d.id) || (in_spotlight.is_empty() && map.depth(d.id) == 1);
        if fx.pulses && pulsing && d.grow >= 1.0 {
            let u = ((now * 0.45 + (d.id as f64 * 0.37).fract()) % 1.0) as f32;
            let p = bezier(points, u);
            let fade = (std::f32::consts::PI * u).sin() * d.alpha;
            let z = cam.zoom.clamp(0.5, 1.5);
            let glow = style.accent.lerp_to_gamma(Color32::WHITE, 0.5);
            painter.circle_filled(p, 9.0 * z, glow.gamma_multiply(0.18 * fade));
            painter.circle_filled(p, 3.5 * z, glow.gamma_multiply(0.95 * fade));
        }
    }

    // Wave that spreads around the focus on arrival
    if fx.ripple && (0.0..RIPPLE).contains(&arrived) {
        if let Some(c) = focus.and_then(|f| centers.get(&f)) {
            let accent = focus.map(|f| color_of(map, theme, f)).unwrap_or(theme.root);
            for (delay, width) in [(0.0, 3.0_f32), (0.25, 2.0)] {
                let a = ((arrived - delay) / (RIPPLE - delay)).clamp(0.0, 1.0);
                if a > 0.0 {
                    let radius = (40.0 + 380.0 * ease_out_cubic(a)) * cam.zoom.clamp(0.5, 1.5);
                    painter.circle_stroke(
                        *c,
                        radius,
                        Stroke::new(width, accent.gamma_multiply(0.55 * (1.0 - a))),
                    );
                }
            }
        }
    }

    // Nodes; a clicked checkbox is toggled after drawing
    let mut ticked: Option<(u64, Pos2)> = None;
    let breathe = 0.75 + 0.25 * (now * 2.2).sin() as f32;
    for d in &drawn {
        let style = node_style(map, theme, d.id);
        let pop = if d.grow < 1.0 {
            ease_out_back(d.grow).max(0.05)
        } else {
            1.0
        };
        // Depth of field: whatever isn't in the spotlight moves back a little
        let out_of_focus = !in_spotlight.is_empty() && !in_spotlight.contains(&d.id);
        let depth = if fx.depth_of_field && out_of_focus {
            1.0 - 0.12 * spot
        } else {
            1.0
        };
        let scale = cam.zoom * pop * depth;
        let size = sizes.get(&d.id).copied().unwrap_or(Vec2::splat(60.0)) * scale;
        let rect = Rect::from_center_size(d.center, size);

        // Glow around the branch in the spotlight
        if fx.glow && in_spotlight.contains(&d.id) {
            // Many thin layers instead of a few thick ones: a soft gradient without steps
            let radius = if node_has_image(map, d.id) {
                16.0 * scale
            } else {
                rect.height() / 2.0
            };
            for k in 1..=12 {
                let grow = k as f32 * 2.2 * scale;
                painter.rect_filled(
                    rect.expand(grow),
                    radius + grow,
                    style
                        .accent
                        .gamma_multiply(0.022 * breathe * spot * d.alpha),
                );
            }
        }

        let mut p = painter.clone();
        p.set_opacity(d.alpha);
        let node = map.get(d.id);
        paint_node(
            &p,
            theme,
            rect,
            &style,
            scale,
            false,
            false,
            node.is_some_and(|n| n.image.is_some()),
        );
        let label = match node {
            Some(n) => paint_thumb(&p, map, n, rect, scale),
            None => rect,
        };
        let task = node.and_then(|n| n.task);
        if let Some(done) = task {
            let r = paint_task(&p, label, &style, done, scale);
            if d.alpha > 0.5 {
                let click = ui
                    .interact(
                        r.expand(4.0 * scale),
                        ui.id().with(("tour_task", d.id)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if click.clicked() {
                    ticked = Some((d.id, r.center()));
                }
            }
        }
        let font = style.font_size * scale;
        if font >= 3.0 {
            if let Some(n) = map.get(d.id) {
                let galley =
                    crate::nodes::layout_stepped(&p, font, |size| label_job(n, &style, size));
                paint_label(&p, label, galley, &style, task, scale);
            }
        }
    }

    // Sparks circling the current branch
    if fx.orbit {
        if let Some((f, c)) = focus.and_then(|f| centers.get(&f).map(|c| (f, *c))) {
            let size = sizes.get(&f).copied().unwrap_or(Vec2::splat(60.0)) * cam.zoom;
            let appear = (arrived / 0.6).clamp(0.0, 1.0);
            let color = color_of(map, theme, f).lerp_to_gamma(Color32::WHITE, 0.5);
            let margin = cam.zoom.clamp(0.6, 1.5);
            for k in 0..3 {
                let angle = now as f32 * 1.3 + k as f32 * std::f32::consts::TAU / 3.0;
                let p = c + Vec2::new(
                    angle.cos() * (size.x / 2.0 + 18.0 * margin),
                    angle.sin() * (size.y / 2.0 + 14.0 * margin),
                );
                painter.circle_filled(p, 8.0, color.gamma_multiply(0.15 * appear));
                painter.circle_filled(p, 3.0, color.gamma_multiply(0.9 * appear));
            }
        }
    }

    subtitle(
        &painter,
        area,
        theme,
        map,
        all[doc.tour.stop],
        doc.tour.started,
        now,
        FLIGHT,
    );
    stop_label(&painter, area, theme, map, &all, &doc.tour, stepwise);
    if let Some(e) = controls(ui, area, doc, theme, fx, &all, &nav, cam, now) {
        event = Some(e);
    }
    if let Some((id, pos)) = ticked {
        tick(&ctx, doc, theme, fx, id, pos, now);
    }

    event
}

/// Subtitle with the note of the stop, fades in towards the end of the flight
#[allow(clippy::too_many_arguments)]
pub(crate) fn subtitle(
    painter: &egui::Painter,
    area: Rect,
    theme: &Theme,
    map: &MindMap,
    stop: Stop,
    started: f64,
    now: f64,
    flight: f64,
) {
    let note = note_of(map, stop);
    let fade = (((now - started) - flight * 0.6) / 0.4).clamp(0.0, 1.0) as f32;
    if !note.is_empty() && fade > 0.0 {
        let width = (area.width() - 160.0).min(900.0);
        let mut p = painter.clone();
        p.set_opacity(fade);
        let galley = p.layout(note, FontId::proportional(19.0), theme.text, width - 48.0);
        let card = Rect::from_center_size(
            Pos2::new(
                area.center().x,
                area.bottom() - 40.0 - (galley.size().y + 28.0) / 2.0 + (1.0 - fade) * 12.0,
            ),
            Vec2::new(galley.size().x + 48.0, galley.size().y + 28.0),
        );
        p.rect_filled(
            card.translate(Vec2::new(0.0, 4.0)),
            14.0,
            Color32::from_black_alpha(60),
        );
        p.rect_filled(card, 14.0, theme.card);
        let accent = match stop {
            Stop::Branch(b) => color_of(map, theme, b),
            _ => theme.root,
        };
        p.rect_filled(
            Rect::from_min_size(
                card.min + Vec2::new(0.0, 12.0),
                Vec2::new(4.0, card.height() - 24.0),
            ),
            2.0,
            accent,
        );
        p.galley(card.min + Vec2::new(24.0, 14.0), galley, theme.text);
    }
}

/// Which stop we are at, top left (with the level when revealing)
pub(crate) fn stop_label(
    painter: &egui::Painter,
    area: Rect,
    theme: &Theme,
    map: &MindMap,
    all: &[Stop],
    tour: &TourState,
    stepwise: bool,
) {
    let last = all.len() - 1;
    let mut label = match all[tour.stop] {
        Stop::Overview => "Start".to_string(),
        Stop::Finale => "End".to_string(),
        Stop::Branch(_) => format!("{} / {}", tour.stop, last - 1),
    };
    if let (true, Stop::Branch(b)) = (stepwise, all[tour.stop]) {
        let max = max_level(map, b);
        if max > 0 {
            label.push_str(&format!("   ·   level {} of {max}", tour.level));
        }
    }
    painter.text(
        area.left_top() + Vec2::new(24.0, 22.0),
        Align2::LEFT_TOP,
        format!("{label}   ·   {}", title_of(map, all[tour.stop])),
        FontId::proportional(15.0),
        theme.text.gamma_multiply(0.7),
    );
}

/// Vignette, the buttons at the top right, the quick-jump strip, confetti and laser pointer
#[allow(clippy::too_many_arguments)]
pub(crate) fn controls(
    ui: &mut egui::Ui,
    area: Rect,
    doc: &mut Document,
    theme: &Theme,
    fx: &mut Effects,
    all: &[Stop],
    nav: &Nav,
    cam: Camera,
    now: f64,
) -> Option<Event> {
    let ctx = ui.ctx().clone();
    let painter = ui.painter().clone();
    let last = all.len() - 1;
    let mut event = None;
    if fx.vignette {
        vignette(&painter, area);
    }

    let corner = corner_buttons(ui, area, "End tour (Esc)", fx, true);
    let (corner_event, strip_clicked) = (corner.event, corner.strip);
    if corner_event.is_some() {
        event = corner_event;
    }
    if strip_clicked {
        doc.strip = !doc.strip;
        doc.strip_cursor = doc.tour.stop;
    }

    // Stops as quick jump, confetti and laser pointer on top
    let map = &doc.map;
    let items: Vec<StripItem> = all
        .iter()
        .enumerate()
        .map(|(i, stop)| StripItem {
            label: match stop {
                Stop::Overview => "Start".into(),
                Stop::Finale => "End".into(),
                Stop::Branch(_) => format!("{i} / {}", last - 1),
            },
            title: title_of(map, *stop),
            color: match stop {
                Stop::Branch(b) => color_of(map, theme, *b),
                _ => theme.root,
            },
        })
        .collect();
    let picked = filmstrip(
        ui,
        area,
        theme,
        &items,
        doc.tour.stop,
        doc.strip_cursor,
        doc.strip,
    );
    if let Some(i) = picked {
        if i != doc.tour.stop {
            let backwards = i < doc.tour.stop;
            fly_to(&mut doc.tour, i, 0, backwards, cam, now);
        }
        doc.strip = false;
    }
    confetti_draw(&ctx, now);
    laser(ui, nav.laser, now);
    event
}

/// Tick a checkbox clicked during the tour, with confetti if wanted
pub(crate) fn tick(
    ctx: &egui::Context,
    doc: &mut Document,
    theme: &Theme,
    fx: &Effects,
    id: u64,
    pos: Pos2,
    now: f64,
) {
    doc.checkpoint();
    if let Some(n) = doc.map.get_mut(id) {
        n.task = n.task.map(|done| !done);
        if n.task == Some(true) {
            crate::sound::play(crate::sound::Sound::Done);
            if fx.confetti {
                confetti_burst(ctx, pos, &theme.branches, now);
            }
        }
    }
}

/// Keys during the tour (both styles): next/back (level by level when revealing),
/// first/last, quick jump, end and full screen
pub(crate) fn navigate(
    doc: &mut Document,
    all: &[Stop],
    nav: &Nav,
    cam: Camera,
    now: f64,
    stepwise: bool,
) -> Option<Event> {
    let map = &doc.map;
    let last = all.len() - 1;
    let mut event = None;
    // Level a stop starts with on arrival
    let entry_level = |stop: usize, backwards: bool| match all[stop] {
        Stop::Branch(b) if stepwise && backwards => max_level(map, b),
        _ => 0,
    };

    // Flying is keyboard only – clicks are for ticking off
    let current = doc.tour.stop;
    let branch_here = match all[current] {
        Stop::Branch(b) => Some(b),
        _ => None,
    };
    if doc.strip {
        match strip_keys(nav, &mut doc.strip_cursor, last) {
            StripKey::Jump(i) => {
                if i != current {
                    fly_to(&mut doc.tour, i, 0, i < current, cam, now);
                }
                doc.strip = false;
            }
            StripKey::Close => doc.strip = false,
            StripKey::Stay => {}
        }
    } else {
        if nav.next() {
            match branch_here {
                // First reveal the next level, then fly on
                Some(b) if stepwise && doc.tour.level < max_level(map, b) => {
                    let level = doc.tour.level + 1;
                    fly_to(&mut doc.tour, current, level, false, cam, now);
                }
                _ if current < last => {
                    fly_to(
                        &mut doc.tour,
                        current + 1,
                        entry_level(current + 1, false),
                        false,
                        cam,
                        now,
                    );
                }
                _ => {}
            }
        }
        if nav.left {
            if stepwise && branch_here.is_some() && doc.tour.level > 0 {
                let level = doc.tour.level - 1;
                fly_to(&mut doc.tour, current, level, true, cam, now);
            } else if current > 0 {
                fly_to(
                    &mut doc.tour,
                    current - 1,
                    entry_level(current - 1, true),
                    true,
                    cam,
                    now,
                );
            }
        }
        if nav.home && current != 0 {
            fly_to(&mut doc.tour, 0, 0, true, cam, now);
        }
        if nav.end && current != last {
            fly_to(&mut doc.tour, last, 0, false, cam, now);
        }
        if nav.escape {
            event = Some(Event::Exit);
        }
        if nav.strip {
            doc.strip = true;
            doc.strip_cursor = doc.tour.stop;
        }
    }
    if nav.fullscreen {
        event = Some(Event::ToggleFullscreen);
    }

    event
}

/// Nodes with a picture are drawn as a card
fn node_has_image(map: &MindMap, id: u64) -> bool {
    map.get(id).is_some_and(|n| n.image.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (MindMap, u64, u64, u64) {
        let mut map = MindMap::new("Center");
        let a = map.add_child_text(0, "A");
        let b = map.add_child_text(0, "B");
        let a1 = map.add_child_text(a, "A1");
        (map, a, b, a1)
    }

    fn at(stop: usize, level: usize, backwards: bool) -> TourState {
        TourState {
            stop,
            level,
            backwards,
            started: 0.0,
            ..TourState::default()
        }
    }

    #[test]
    fn stops_are_overview_branches_finale() {
        let (map, a, b, _) = sample();
        let all = stops(&map);
        assert_eq!(
            all,
            vec![
                Stop::Overview,
                Stop::Branch(a),
                Stop::Branch(b),
                Stop::Finale
            ]
        );
        assert_eq!(stops(&MindMap::new("Alone")), vec![Stop::Overview]);
    }

    #[test]
    fn children_appear_only_from_their_branch_on() {
        let (map, _, _, a1) = sample();
        let all = stops(&map);
        // Overview: sub-point of A still hidden
        assert_eq!(growth(&map, &all, &at(0, 0, false), a1, 100.0, false), 0.0);
        // At A: grows out after the delay
        assert_eq!(growth(&map, &all, &at(1, 0, false), a1, 0.0, false), 0.0);
        assert_eq!(growth(&map, &all, &at(1, 0, false), a1, 100.0, false), 1.0);
        // Paged back to A: fully there at once
        assert_eq!(growth(&map, &all, &at(1, 0, true), a1, 0.0, false), 1.0);
        // Main branches and the centre are always visible
        assert_eq!(growth(&map, &all, &at(0, 0, false), 0, 0.0, false), 1.0);
    }

    #[test]
    fn levels_are_revealed_one_by_one() {
        let (mut map, a, _, a1) = sample();
        let a1x = map.add_child_text(a1, "A1x");
        let all = stops(&map);
        assert_eq!(max_level(&map, a), 2);
        // Level 0: only the branch itself
        assert_eq!(growth(&map, &all, &at(1, 0, false), a1, 100.0, true), 0.0);
        // Level 1: children yes, grandchildren no
        assert_eq!(growth(&map, &all, &at(1, 1, false), a1, 100.0, true), 1.0);
        assert_eq!(growth(&map, &all, &at(1, 1, false), a1x, 100.0, true), 0.0);
        // Level 2: everything
        assert_eq!(growth(&map, &all, &at(1, 2, false), a1x, 100.0, true), 1.0);
        // The camera frames the visible levels plus the next one
        assert_eq!(framed(&map, Stop::Branch(a), Some(0)).len(), 2);
        assert_eq!(framed(&map, Stop::Branch(a), Some(1)).len(), 3);
    }

    #[test]
    fn flight_starts_and_ends_on_target_and_arcs_out() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        let from = Camera {
            center: Vec2::ZERO,
            zoom: 1.0,
        };
        let to = Camera {
            center: Vec2::new(3000.0, 0.0),
            zoom: 1.0,
        };
        assert_eq!(fly(from, to, 0.0, area, false).center, from.center);
        assert_eq!(fly(from, to, 1.0, area, false).center, to.center);
        assert!((fly(from, to, 1.0, area, false).zoom - 1.0).abs() < 1e-4);
        // In the middle of the flight the camera is zoomed out (arc)
        assert!(fly(from, to, 0.5, area, false).zoom < 0.8);
        // With spring: a little closer shortly before the target, exact at the target
        assert!(fly(from, from, 0.85, area, true).zoom > 1.0);
        assert!((fly(from, from, 1.0, area, true).zoom - 1.0).abs() < 1e-4);
    }
}
