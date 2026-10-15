//! Kamerafahrt durch die Mindmap.
//!
//! Stationen: Übersicht → jeder Hauptzweig → ganze Mindmap. Zwischen den
//! Stationen fliegt die Kamera im Bogen (erst etwas raus, dann rein), der
//! aktuelle Zweig steht im Scheinwerfer und seine Unterpunkte wachsen
//! nacheinander aus ihm heraus. Die Notiz der Station erscheint als Untertitel.

use crate::document::{Camera, Document, TourState};
use crate::model::MindMap;
use crate::nodes::{node_size, node_style, paint_node};
use crate::present::{
    backdrop, color_of, corner_buttons, ease_in_out_cubic, ease_out_back, ease_out_cubic,
    filmstrip, hint, laser, read_nav, Backdrop, Effects, Event, StripItem,
};
use crate::theme::Theme;
use eframe::egui::{
    self, epaint::CubicBezierShape, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2,
};
use std::collections::{HashMap, HashSet};

/// Dauer eines Kameraflugs in Sekunden
const FLIGHT: f64 = 1.1;
/// Ab wann (nach Flugbeginn) die Unterpunkte zu wachsen beginnen
const GROW_DELAY: f64 = 0.7;
/// Abstand zwischen zwei aufwachsenden Unterpunkten
const GROW_STEP: f64 = 0.1;
/// Wie lange ein Unterpunkt zum Aufwachsen braucht
const GROW_TIME: f64 = 0.4;
/// Helligkeit außerhalb des Scheinwerfers
const DIM: f32 = 0.22;
/// Platz unten für den Untertitel
const BOTTOM_SPACE: f32 = 150.0;

#[derive(Clone, Copy, PartialEq)]
enum Stop {
    Overview,
    Branch(u64),
    Finale,
}

fn stops(map: &MindMap) -> Vec<Stop> {
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

/// Wie weit ein Knoten an Station `stop` sichtbar ist (0 = gar nicht, 1 = ganz)
fn growth(map: &MindMap, all: &[Stop], tour: &TourState, id: u64, now: f64) -> f32 {
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
    if tour.backwards {
        return 1.0;
    }
    // Reihenfolge innerhalb des Zweigs: Ebene für Ebene
    let order = map
        .subtree(branch)
        .iter()
        .position(|n| *n == id)
        .unwrap_or(1)
        .saturating_sub(1);
    let start = tour.started + GROW_DELAY + order as f64 * GROW_STEP;
    ((now - start) / GROW_TIME).clamp(0.0, 1.0) as f32
}

/// Knoten, die an einer Station ins Bild sollen
fn framed(map: &MindMap, stop: Stop) -> Vec<u64> {
    let visible = |id: &u64| !map.is_hidden(*id);
    match stop {
        Stop::Overview => map
            .nodes
            .iter()
            .filter(|n| map.depth(n.id) <= 1)
            .map(|n| n.id)
            .collect(),
        Stop::Branch(b) => map.subtree(b).into_iter().filter(visible).collect(),
        Stop::Finale => map.nodes.iter().map(|n| n.id).filter(visible).collect(),
    }
}

/// Kamera, die die Knoten einer Station formatfüllend zeigt
fn target(map: &MindMap, stop: Stop, sizes: &HashMap<u64, Vec2>, area: Rect) -> Camera {
    let mut bounds = Rect::NOTHING;
    for id in framed(map, stop) {
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
    // Mitte der nutzbaren Fläche liegt etwas über der Bildschirmmitte
    let shift = (usable.center() - area.center()) / zoom;
    Camera {
        center: bounds.center().to_vec2() - shift,
        zoom,
    }
}

/// Kamera unterwegs: Mitte gleitet, Zoom ändert sich gleichmäßig, und bei
/// weiten Wegen geht es im Bogen erst etwas heraus.
fn fly(from: Camera, to: Camera, t: f32, area: Rect) -> Camera {
    let e = ease_in_out_cubic(t);
    let center = from.center + (to.center - from.center) * e;
    let zoom = (from.zoom.ln() + (to.zoom.ln() - from.zoom.ln()) * e).exp();
    let distance = (to.center - from.center).length() * from.zoom.min(to.zoom) / area.width();
    let dip = 1.0 + distance.min(2.0) * 0.35 * (std::f32::consts::PI * e).sin();
    Camera {
        center,
        zoom: zoom / dip,
    }
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

fn title_of(map: &MindMap, stop: Stop) -> String {
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

/// Aktuelle Kamera: Flug zur Station, danach optional langsames Schweben
fn camera(tour: &TourState, to: Camera, now: f64, area: Rect, drift: bool) -> Camera {
    let t = ((now - tour.started) / FLIGHT).clamp(0.0, 1.0) as f32;
    let mut cam = fly(tour.from, to, t, area);
    if drift {
        // Nach der Ankunft über ~30 s ganz leicht hineinzoomen und etwas wandern
        let d = ease_in_out_cubic(((now - tour.started - FLIGHT) / 30.0).clamp(0.0, 1.0) as f32);
        cam.zoom *= 1.0 + 0.06 * d;
        let wobble = Vec2::new((now * 0.11).sin() as f32, (now * 0.07).cos() as f32);
        cam.center += wobble * (10.0 / cam.zoom) * d;
    }
    cam
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
    let map = &doc.map;
    let all = stops(map);
    let last = all.len() - 1;
    doc.tour.stop = doc.tour.stop.min(last);
    doc.tour.prev_stop = doc.tour.prev_stop.min(last);

    let sizes: HashMap<u64, Vec2> = map
        .nodes
        .iter()
        .map(|n| (n.id, node_size(&ctx, map, theme, n)))
        .collect();

    if doc.tour.needs_init {
        doc.tour.from = target(map, all[doc.tour.stop], &sizes, area);
        doc.tour.prev_stop = doc.tour.stop;
        doc.tour.started = now - FLIGHT;
        doc.tour.needs_init = false;
    }

    let to = target(map, all[doc.tour.stop], &sizes, area);
    let cam = camera(&doc.tour, to, now, area, fx.drift);

    // Zu einer anderen Station: Flug beginnt dort, wo die Kamera gerade ist
    let go_to = |tour: &mut TourState, stop: usize| {
        if stop != tour.stop {
            tour.from = cam;
            tour.backwards = stop < tour.stop;
            tour.prev_stop = tour.stop;
            tour.stop = stop;
            tour.started = now;
        }
    };

    let nav = read_nav(ui);
    let mut event = None;

    let background = ui.allocate_rect(area, Sense::click());
    let current = doc.tour.stop;
    if (nav.next || background.clicked()) && current < last {
        go_to(&mut doc.tour, current + 1);
    }
    if (nav.prev || background.secondary_clicked()) && current > 0 {
        go_to(&mut doc.tour, current - 1);
    }
    if nav.home {
        go_to(&mut doc.tour, 0);
    }
    if nav.end {
        go_to(&mut doc.tour, last);
    }
    if nav.escape {
        event = Some(Event::Exit);
    }
    if nav.fullscreen {
        event = Some(Event::ToggleFullscreen);
    }
    if nav.strip {
        doc.strip = !doc.strip;
    }

    // Läuft noch eine Animation? Dann weiter neu zeichnen.
    let busy_until = doc.tour.started + FLIGHT + GROW_DELAY + map.nodes.len() as f64 * GROW_STEP;
    if now < busy_until + GROW_TIME || fx.drift {
        ctx.request_repaint();
    }

    // Kamera kann sich durch einen Tastendruck gerade geändert haben
    let t = ((now - doc.tour.started) / FLIGHT).clamp(0.0, 1.0) as f32;
    let to = target(map, all[doc.tour.stop], &sizes, area);
    let cam = camera(&doc.tour, to, now, area, fx.drift);
    let to_screen = |p: Vec2| area.center() + (p - cam.center) * cam.zoom;

    // Hintergrund: animiert oder Punktraster, das mit der Kamera mitzieht
    let painter = ui.painter().clone();
    painter.rect_filled(area, 0.0, theme.background);
    let plain = fx.background == Backdrop::Plain;
    if !plain && backdrop(&painter, area, theme, fx.background, now) {
        ctx.request_repaint();
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

    // Für jeden Knoten: wie weit gewachsen, wie hell, wo gerade
    let spot = ease_out_cubic(t);
    let mut lit_cache = HashMap::new();
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
        let grow = growth(map, &all, &doc.tour, n.id, now);
        if grow <= 0.0 {
            continue;
        }
        let before = brightness(map, all[doc.tour.prev_stop], n.id, &mut lit_cache);
        let after = brightness(map, all[doc.tour.stop], n.id, &mut lit_cache);
        let light = before + (after - before) * spot;
        // Unterpunkte wachsen aus ihrem Elternknoten heraus
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

    // Verbindungen
    for d in &drawn {
        let Some(a) = d.parent.and_then(|p| centers.get(&p)) else {
            continue;
        };
        let style = node_style(map, theme, d.id);
        let b = d.center;
        let dx = (b.x - a.x) * 0.5;
        painter.add(CubicBezierShape::from_points_stroke(
            [*a, *a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0), b],
            false,
            Color32::TRANSPARENT,
            Stroke::new(
                style.edge_width * cam.zoom.clamp(0.5, 2.0),
                style.accent.gamma_multiply(0.8 * d.alpha),
            ),
        ));
    }

    // Knoten
    for d in &drawn {
        let style = node_style(map, theme, d.id);
        let pop = if d.grow < 1.0 {
            ease_out_back(d.grow).max(0.05)
        } else {
            1.0
        };
        let scale = cam.zoom * pop;
        let size = sizes.get(&d.id).copied().unwrap_or(Vec2::splat(60.0)) * scale;
        let rect = Rect::from_center_size(d.center, size);
        let mut p = painter.clone();
        p.set_opacity(d.alpha);
        paint_node(&p, theme, rect, &style, scale, false, false);
        let font = style.font_size * scale;
        if font >= 3.0 {
            let text = map.get(d.id).map(|n| n.text.clone()).unwrap_or_default();
            let galley = p.layout_no_wrap(text, FontId::proportional(font), style.text);
            p.galley(rect.center() - galley.size() / 2.0, galley, style.text);
        }
    }

    // Untertitel mit der Notiz, blendet nach der Ankunft ein
    let note = note_of(map, all[doc.tour.stop]);
    let has_note = !note.is_empty();
    let fade = (((now - doc.tour.started) - FLIGHT * 0.6) / 0.4).clamp(0.0, 1.0) as f32;
    if has_note && fade > 0.0 {
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
        let accent = match all[doc.tour.stop] {
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

    // Station oben links, Hinweise unten
    let label = match all[doc.tour.stop] {
        Stop::Overview => "Start".to_string(),
        Stop::Finale => "End".to_string(),
        Stop::Branch(_) => format!("{} / {}", doc.tour.stop, last - 1),
    };
    painter.text(
        area.left_top() + Vec2::new(24.0, 22.0),
        Align2::LEFT_TOP,
        format!("{label}   ·   {}", title_of(map, all[doc.tour.stop])),
        FontId::proportional(15.0),
        theme.text.gamma_multiply(0.7),
    );
    if !has_note {
        hint(
            &painter,
            Pos2::new(area.center().x, area.bottom() - 24.0),
            theme,
            "←  →  navigate   ·   G  stops   ·   L  laser   ·   F  full screen   ·   Esc  exit",
        );
    }

    let (corner_event, strip_clicked) = corner_buttons(ui, area, "End tour (Esc)", fx, true);
    if corner_event.is_some() {
        event = corner_event;
    }
    if strip_clicked {
        doc.strip = !doc.strip;
    }

    // Stationen als Folienstreifen, Laserpointer obenauf
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
    if let Some(i) = filmstrip(ui, area, theme, &items, doc.tour.stop, doc.strip) {
        let cam = camera(&doc.tour, to, now, area, fx.drift);
        let tour = &mut doc.tour;
        if i != tour.stop {
            tour.from = cam;
            tour.backwards = i < tour.stop;
            tour.prev_stop = tour.stop;
            tour.stop = i;
            tour.started = now;
        }
    }
    laser(ui, nav.laser, now);

    event
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

    #[test]
    fn stops_are_overview_branches_finale() {
        let (map, a, b, _) = sample();
        let all = stops(&map);
        assert!(
            all == vec![
                Stop::Overview,
                Stop::Branch(a),
                Stop::Branch(b),
                Stop::Finale
            ]
        );
        assert!(stops(&MindMap::new("Alone")) == vec![Stop::Overview]);
    }

    #[test]
    fn children_appear_only_from_their_branch_on() {
        let (map, _, _, a1) = sample();
        let all = stops(&map);
        let at = |stop: usize, backwards: bool| TourState {
            stop,
            backwards,
            started: 0.0,
            ..TourState::default()
        };
        // Übersicht: Unterpunkt von A noch verborgen
        assert_eq!(growth(&map, &all, &at(0, false), a1, 100.0), 0.0);
        // An A: wächst nach der Verzögerung heraus
        assert_eq!(growth(&map, &all, &at(1, false), a1, 0.0), 0.0);
        assert_eq!(growth(&map, &all, &at(1, false), a1, 100.0), 1.0);
        // Rückwärts zu A geblättert: sofort ganz da
        assert_eq!(growth(&map, &all, &at(1, true), a1, 0.0), 1.0);
        // Hauptzweige und Mitte sind immer sichtbar
        assert_eq!(growth(&map, &all, &at(0, false), 0, 0.0), 1.0);
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
        assert_eq!(fly(from, to, 0.0, area).center, from.center);
        assert_eq!(fly(from, to, 1.0, area).center, to.center);
        assert!((fly(from, to, 1.0, area).zoom - 1.0).abs() < 1e-4);
        // In der Mitte des Flugs ist die Kamera herausgezoomt (Bogen)
        assert!(fly(from, to, 0.5, area).zoom < 0.8);
    }
}
