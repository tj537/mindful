//! Folien-Layouts im Stil von SmartArt: Die Stichpunkte einer Folie werden
//! als Zeitstrahl, Kreislauf, Pyramide, Trichter, Vergleich, Matrix,
//! Venn-Diagramm, große Zahl oder Zitat gezeichnet.
//!
//! `reveal(i)` liefert für Punkt `i` den Fortschritt seines Erscheinens
//! (0 = noch verborgen, 1 = ganz da) – so bauen sich die Diagramme genauso
//! schrittweise auf wie normale Stichpunkte.

use crate::model::SlideLayout;
use crate::present::{ease_out_back, ease_out_cubic};
use crate::theme::Theme;
use eframe::egui::{
    self, text::LayoutJob, Align, Color32, FontId, Galley, Painter, Pos2, Rect, Shape, Stroke,
    TextFormat, Vec2,
};
use egui_phosphor::regular as icon;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

/// Ein Punkt der obersten Ebene mit seinen Unterpunkten
pub struct Item {
    pub text: String,
    pub children: Vec<String>,
}

impl SlideLayout {
    pub fn label(self) -> &'static str {
        match self {
            SlideLayout::Timeline => "Timeline",
            SlideLayout::Cycle => "Cycle",
            SlideLayout::Pyramid => "Pyramid",
            SlideLayout::Funnel => "Funnel",
            SlideLayout::Compare => "Comparison",
            SlideLayout::Matrix => "2×2 matrix",
            SlideLayout::Venn => "Venn diagram",
            SlideLayout::Number => "Big number",
            SlideLayout::Quote => "Quote",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            SlideLayout::Timeline => icon::ARROW_RIGHT,
            SlideLayout::Cycle => icon::ARROWS_CLOCKWISE,
            SlideLayout::Pyramid => icon::TRIANGLE,
            SlideLayout::Funnel => icon::FUNNEL,
            SlideLayout::Compare => icon::COLUMNS,
            SlideLayout::Matrix => icon::SQUARES_FOUR,
            SlideLayout::Venn => icon::INTERSECT,
            SlideLayout::Number => icon::HASH,
            SlideLayout::Quote => icon::QUOTES,
        }
    }

    /// Wie viele Punkte das Layout zeigt (= Schritte beim Aufbauen)
    pub fn steps(self, items: usize) -> usize {
        match self {
            SlideLayout::Number | SlideLayout::Quote => 0,
            SlideLayout::Compare | SlideLayout::Matrix | SlideLayout::Venn => items.min(4),
            _ => items,
        }
    }
}

/// Auswahlmenü für das Layout. Gibt `Some(Wahl)` zurück, wenn etwas angeklickt wurde
/// (`Some(None)` = normale Aufzählung).
pub fn layout_menu(ui: &mut egui::Ui, current: Option<SlideLayout>) -> Option<Option<SlideLayout>> {
    let mut picked = None;
    ui.set_min_width(200.0);
    if ui
        .selectable_label(
            current.is_none(),
            format!("{}  Bullet points", icon::LIST_BULLETS),
        )
        .clicked()
    {
        picked = Some(None);
    }
    ui.separator();
    for layout in SlideLayout::ALL {
        let label = format!("{}  {}", layout.icon(), layout.label());
        if ui
            .selectable_label(current == Some(layout), label)
            .clicked()
        {
            picked = Some(Some(layout));
        }
    }
    if picked.is_some() {
        ui.close_menu();
    }
    picked
}

// ---------------------------------------------------------------------------
// Text-Helfer
// ---------------------------------------------------------------------------

fn galley(
    p: &Painter,
    text: &str,
    size: f32,
    color: Color32,
    wrap: f32,
    centered: bool,
) -> Arc<Galley> {
    let mut job = LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(size),
        color,
        wrap.max(20.0),
    );
    if centered {
        job.halign = Align::Center;
    }
    p.layout_job(job)
}

/// Text mittig unter `top` (Mitte oben); gibt die belegte Fläche zurück
fn text_centered(p: &Painter, text: &str, size: f32, color: Color32, wrap: f32, top: Pos2) -> Rect {
    let g = galley(p, text, size, color, wrap, true);
    let rect = g.rect.translate(top.to_vec2());
    p.galley(top, g, color);
    rect
}

/// Text links oben an `top_left`; gibt die belegte Fläche zurück
fn text_left(
    p: &Painter,
    text: &str,
    size: f32,
    color: Color32,
    wrap: f32,
    top_left: Pos2,
) -> Rect {
    let g = galley(p, text, size, color, wrap, false);
    let rect = Rect::from_min_size(top_left, g.size());
    p.galley(top_left, g, color);
    rect
}

/// Unterpunkte als Liste mit Punkten (ein einzelner Unterpunkt ohne Punkt)
fn child_lines(item: &Item) -> String {
    if let [only] = item.children.as_slice() {
        return only.clone();
    }
    item.children
        .iter()
        .map(|c| format!("• {c}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn faded(p: &Painter, alpha: f32) -> Painter {
    let mut p = p.clone();
    p.multiply_opacity(alpha.clamp(0.0, 1.0));
    p
}

fn palette(theme: &Theme, i: usize) -> Color32 {
    theme.branches[i % theme.branches.len()]
}

/// Schrift etwas kleiner, wenn es viele Punkte sind
fn fit(n: usize, comfortable: usize) -> f32 {
    (comfortable as f32 / n.max(comfortable) as f32).max(0.6)
}

// ---------------------------------------------------------------------------
// Zeichnen
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &Painter,
    rect: Rect,
    layout: SlideLayout,
    items: &[Item],
    note: &str,
    theme: &Theme,
    color: Color32,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
    count_up: f32,
) {
    if items.is_empty() && layout != SlideLayout::Quote {
        return;
    }
    match layout {
        SlideLayout::Timeline => timeline(p, rect, items, theme, color, s, reveal),
        SlideLayout::Cycle => cycle(p, rect, items, theme, s, reveal),
        SlideLayout::Pyramid => stack(p, rect, items, theme, color, s, reveal, false),
        SlideLayout::Funnel => stack(p, rect, items, theme, color, s, reveal, true),
        SlideLayout::Compare => compare(p, rect, items, theme, s, reveal),
        SlideLayout::Matrix => matrix(p, rect, items, theme, s, reveal),
        SlideLayout::Venn => venn(p, rect, items, theme, s, reveal),
        SlideLayout::Number => number(p, rect, items, theme, color, s, reveal, count_up),
        SlideLayout::Quote => quote(p, rect, items, note, theme, color, s, count_up),
    }
}

/// Schritte auf einer Linie, nummeriert, Text darunter
fn timeline(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    color: Color32,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let n = items.len();
    let f = fit(n, 5);
    // Etwas über der Mitte, damit Linie und Texte die Fläche füllen
    let y = r.top() + ((r.height() - 190.0 * s) * 0.4).max(40.0 * s);
    let margin = 70.0 * s;
    let span = (r.width() - 2.0 * margin).max(1.0);
    let step = if n > 1 { span / (n - 1) as f32 } else { 0.0 };
    let x_of = |i: usize| {
        if n > 1 {
            r.left() + margin + step * i as f32
        } else {
            r.center().x
        }
    };
    let shown: Vec<f32> = (0..n).map(reveal).collect();

    // Linie: grau, darüber farbig bis zum zuletzt erschienenen Schritt
    let width = 6.0 * s;
    p.line_segment(
        [Pos2::new(x_of(0), y), Pos2::new(x_of(n - 1), y)],
        Stroke::new(width, theme.grid),
    );
    let mut end = x_of(0);
    for (i, e) in shown.iter().enumerate() {
        if *e > 0.0 && i > 0 {
            end = x_of(i - 1) + (x_of(i) - x_of(i - 1)) * ease_out_cubic(*e);
        }
    }
    p.line_segment(
        [Pos2::new(x_of(0), y), Pos2::new(end, y)],
        Stroke::new(width, color),
    );

    let wrap = if n > 1 { step * 0.92 } else { r.width() * 0.6 }.max(120.0 * s);
    for (i, item) in items.iter().enumerate() {
        let e = shown[i];
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let c = Pos2::new(x_of(i), y);
        let radius = 24.0 * s * ease_out_back(e).max(0.0);
        q.circle_filled(c, radius + 6.0 * s, theme.card);
        q.circle_filled(c, radius, color);
        q.text(
            c,
            egui::Align2::CENTER_CENTER,
            (i + 1).to_string(),
            FontId::proportional(22.0 * s),
            theme.on_accent,
        );
        let top = Pos2::new(c.x, y + 46.0 * s + (1.0 - e) * 16.0 * s);
        let title = text_centered(&q, &item.text, 30.0 * s * f, theme.text, wrap, top);
        if !item.children.is_empty() {
            text_centered(
                &q,
                &child_lines(item),
                21.0 * s * f,
                theme.text.gamma_multiply(0.7),
                wrap,
                Pos2::new(c.x, title.bottom() + 10.0 * s),
            );
        }
    }
}

/// Punkte auf einer Ellipse, verbunden durch Pfeile; Unterpunkte im Kästchen
fn cycle(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let n = items.len();
    let f = fit(n, 5);
    let c = r.center();
    let rx = (r.width() * 0.32).max(80.0 * s);
    let ry = (r.height() * 0.34).max(60.0 * s);
    let angle = |i: usize| -PI / 2.0 + TAU * i as f32 / n as f32;
    let at = |a: f32| c + Vec2::new(a.cos() * rx, a.sin() * ry);
    let shown: Vec<f32> = (0..n).map(reveal).collect();

    // Kästchen vorab setzen (Titel + Unterpunkte), damit die Pfeile ausweichen können
    let wrap = 240.0 * s * f;
    let boxes: Vec<(Arc<Galley>, Option<Arc<Galley>>, Vec2)> = items
        .iter()
        .map(|item| {
            let title = galley(p, &item.text, 21.0 * s * f, theme.on_accent, wrap, true);
            let sub = (!item.children.is_empty()).then(|| {
                galley(
                    p,
                    &child_lines(item),
                    15.0 * s * f,
                    theme.on_accent,
                    wrap,
                    true,
                )
            });
            let w = title.size().x.max(sub.as_ref().map_or(0.0, |g| g.size().x));
            let h = title.size().y + sub.as_ref().map_or(0.0, |g| g.size().y + 6.0 * s);
            (title, sub, Vec2::new(w, h) + Vec2::new(30.0, 18.0) * s)
        })
        .collect();
    // Wo der Bogen ein Kästchen verlässt: vom Mittelpunkt aus schrittweise suchen
    let rects: Vec<Rect> = (0..n)
        .map(|i| Rect::from_center_size(at(angle(i)), boxes[i].2).expand(10.0 * s))
        .collect();
    let leave = |i: usize, direction: f32| {
        let mut a = angle(i);
        for _ in 0..400 {
            if !rects[i].contains(at(a)) {
                break;
            }
            a += direction * 0.005;
        }
        a
    };

    // Pfeile zum jeweils nächsten Punkt (zurück zum ersten erst, wenn alles da ist)
    if n > 1 {
        for i in 0..n {
            let next = (i + 1) % n;
            let e = if next == 0 {
                shown.iter().copied().fold(1.0, f32::min)
            } else {
                shown[next]
            };
            if e <= 0.0 || shown[i] <= 0.0 {
                continue;
            }
            let a0 = leave(i, 1.0);
            // Der nächste Punkt liegt eine volle Runde weiter, wenn es zurück zum ersten geht
            let wrap_around = if next == 0 { TAU } else { 0.0 };
            let mut a1 = leave(next, -1.0) + wrap_around;
            if a1 <= a0 {
                continue;
            }
            a1 = a0 + (a1 - a0) * ease_out_cubic(e);
            let steps = 28;
            let points: Vec<Pos2> = (0..=steps)
                .map(|k| at(a0 + (a1 - a0) * k as f32 / steps as f32))
                .collect();
            let col = theme.text.gamma_multiply(0.35);
            p.add(Shape::line(points, Stroke::new(3.0 * s, col)));
            let tip = at(a1);
            let dir = Vec2::new(-a1.sin() * rx, a1.cos() * ry).normalized();
            let side = Vec2::new(-dir.y, dir.x);
            p.add(Shape::convex_polygon(
                vec![
                    tip + dir * 4.0 * s,
                    tip - dir * 10.0 * s + side * 7.0 * s,
                    tip - dir * 10.0 * s - side * 7.0 * s,
                ],
                col,
                Stroke::NONE,
            ));
        }
    }

    for i in 0..n {
        let e = shown[i];
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let center = at(angle(i));
        let (title, sub, size) = &boxes[i];
        let rect = Rect::from_center_size(center, *size * ease_out_back(e).max(0.0));
        let radius = if sub.is_some() {
            16.0 * s
        } else {
            rect.height() / 2.0
        };
        q.rect_filled(rect, radius, palette(theme, i));
        if e > 0.5 {
            let top = center.y - (size.y - 18.0 * s) / 2.0;
            q.galley(Pos2::new(center.x, top), title.clone(), theme.on_accent);
            if let Some(sub) = sub {
                q.galley(
                    Pos2::new(center.x, top + title.size().y + 6.0 * s),
                    sub.clone(),
                    theme.on_accent.gamma_multiply(0.85),
                );
            }
        }
    }
}

/// Pyramide (schmal oben) oder Trichter (breit oben), Unterpunkte daneben
#[allow(clippy::too_many_arguments)]
fn stack(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    color: Color32,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
    funnel: bool,
) {
    let n = items.len();
    let f = fit(n, 5);
    let gap = 8.0 * s;
    let level = ((r.height() - gap * (n - 1) as f32) / n as f32).min(110.0 * s);
    let total = level * n as f32 + gap * (n - 1) as f32;
    let top = r.top() + (r.height() - total) / 2.0;
    let full = r.width() * 0.6;
    let cx = r.left() + full / 2.0;
    let width_at = |t: f32| {
        if funnel {
            full * (1.0 - 0.7 * t)
        } else {
            full * (0.3 + 0.7 * t)
        }
    };
    for (i, item) in items.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let y0 = top + i as f32 * (level + gap) - (1.0 - e) * 14.0 * s;
        let y1 = y0 + level;
        let (wt, wb) = (
            width_at(i as f32 / n as f32),
            width_at((i + 1) as f32 / n as f32),
        );
        let fill = color.lerp_to_gamma(theme.background, 0.12 * i as f32);
        q.add(Shape::convex_polygon(
            vec![
                Pos2::new(cx - wt / 2.0, y0),
                Pos2::new(cx + wt / 2.0, y0),
                Pos2::new(cx + wb / 2.0, y1),
                Pos2::new(cx - wb / 2.0, y1),
            ],
            fill,
            Stroke::NONE,
        ));
        let g = galley(
            &q,
            &item.text,
            22.0 * s * f,
            theme.on_accent,
            wt.min(wb) * 0.85,
            true,
        );
        let text_top = Pos2::new(cx, (y0 + y1) / 2.0 - g.size().y / 2.0);
        q.galley(text_top, g, theme.on_accent);
        if !item.children.is_empty() {
            let x = cx + wt.max(wb) / 2.0 + 28.0 * s;
            let g = galley(
                &q,
                &child_lines(item),
                18.0 * s * f,
                theme.text.gamma_multiply(0.7),
                r.right() - x,
                false,
            );
            q.galley(
                Pos2::new(x, (y0 + y1) / 2.0 - g.size().y / 2.0),
                g,
                theme.text,
            );
        }
    }
}

/// Spalten nebeneinander: Überschrift oben, Unterpunkte darunter
fn compare(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let k = items.len().min(4);
    let gap = 18.0 * s;
    let col_w = (r.width() - gap * (k - 1) as f32) / k as f32;
    let text_w = col_w - 52.0 * s;
    // Alle Kästen so hoch wie der längste Inhalt, aber nicht höher als nötig
    let content = items
        .iter()
        .take(k)
        .map(|item| {
            item.children
                .iter()
                .map(|c| galley(p, c, 20.0 * s, theme.text, text_w, false).size().y + 10.0 * s)
                .sum::<f32>()
        })
        .fold(0.0, f32::max);
    let header_h = 58.0 * s;
    let body_h = (content + 26.0 * s).min(r.height() - header_h - 10.0 * s);
    for (i, item) in items.iter().take(k).enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let x = r.left() + i as f32 * (col_w + gap);
        let rise = (1.0 - e) * 20.0 * s;
        let header = Rect::from_min_size(Pos2::new(x, r.top() + rise), Vec2::new(col_w, header_h));
        q.rect_filled(header, 12.0 * s, palette(theme, i));
        let g = galley(
            &q,
            &item.text,
            24.0 * s,
            theme.on_accent,
            col_w - 24.0 * s,
            true,
        );
        q.galley(
            Pos2::new(header.center().x, header.center().y - g.size().y / 2.0),
            g,
            theme.on_accent,
        );
        if item.children.is_empty() {
            continue;
        }
        let body = Rect::from_min_size(
            Pos2::new(x, header.bottom() + 10.0 * s),
            Vec2::new(col_w, body_h),
        );
        q.rect_filled(body, 12.0 * s, theme.background);
        let mut y = body.top() + 16.0 * s;
        for child in &item.children {
            q.circle_filled(
                Pos2::new(body.left() + 22.0 * s, y + 12.0 * s),
                4.0 * s,
                palette(theme, i),
            );
            let line = text_left(
                &q,
                child,
                20.0 * s,
                theme.text,
                text_w,
                Pos2::new(body.left() + 36.0 * s, y),
            );
            y = line.bottom() + 10.0 * s;
        }
    }
}

/// Vier Felder im 2×2-Raster
fn matrix(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let gap = 14.0 * s;
    let cell = Vec2::new((r.width() - gap) / 2.0, (r.height() - gap) / 2.0);
    for i in 0..4 {
        let origin = r.min
            + Vec2::new(
                (i % 2) as f32 * (cell.x + gap),
                (i / 2) as f32 * (cell.y + gap),
            );
        let full = Rect::from_min_size(origin, cell);
        let col = palette(theme, i);
        let Some(item) = items.get(i) else {
            // Leere Felder nur angedeutet
            p.rect_stroke(
                full,
                14.0 * s,
                Stroke::new(1.5 * s, theme.grid),
                egui::StrokeKind::Inside,
            );
            continue;
        };
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let rect =
            Rect::from_center_size(full.center(), full.size() * (0.9 + 0.1 * ease_out_back(e)));
        q.rect_filled(rect, 14.0 * s, col.gamma_multiply(0.16));
        q.rect_stroke(
            rect,
            14.0 * s,
            Stroke::new(2.0 * s, col),
            egui::StrokeKind::Inside,
        );
        let title = text_left(
            &q,
            &item.text,
            25.0 * s,
            col,
            rect.width() - 40.0 * s,
            rect.min + Vec2::new(20.0, 16.0) * s,
        );
        if !item.children.is_empty() {
            text_left(
                &q,
                &child_lines(item),
                19.0 * s,
                theme.text,
                rect.width() - 40.0 * s,
                Pos2::new(rect.left() + 20.0 * s, title.bottom() + 10.0 * s),
            );
        }
    }
}

/// Zwei oder drei überlappende Kreise. Ein weiterer Punkt beschreibt, was alle
/// gemeinsam haben, und steht als Unterschrift darunter.
fn venn(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let k = items.len().clamp(1, 3);
    let shared = items.get(k).filter(|_| k > 1);
    let area = if shared.is_some() {
        Rect::from_min_max(r.min, r.max - Vec2::new(0.0, 50.0 * s))
    } else {
        r
    };
    // Größe so wählen, dass alle Kreise in die Fläche passen
    let (radius, centers): (f32, Vec<Pos2>) = match k {
        1 => {
            let radius = area.width().min(area.height()) * 0.45;
            (radius, vec![area.center()])
        }
        2 => {
            let radius = (area.width() / 3.4).min(area.height() / 2.1);
            let offset = radius * 0.62;
            let c = area.center();
            (radius, vec![c - Vec2::X * offset, c + Vec2::X * offset])
        }
        _ => {
            let radius = (area.width() / 3.4).min(area.height() / 2.95);
            let offset = radius * 0.62;
            // Die drei Kreise reichen oben weiter als unten: Mitte etwas nach unten
            let c = area.center() + Vec2::Y * offset * 0.25;
            let centers = (0..3)
                .map(|i| {
                    let a = -PI / 2.0 + TAU * i as f32 / 3.0;
                    c + Vec2::new(a.cos(), a.sin()) * offset
                })
                .collect();
            (radius, centers)
        }
    };
    let middle = centers.iter().fold(Vec2::ZERO, |acc, c| acc + c.to_vec2()) / centers.len() as f32;
    for (i, center) in centers.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let col = palette(theme, i);
        let rad = radius * ease_out_back(e).max(0.0);
        p.circle_filled(*center, rad, col.gamma_multiply(0.26 * e));
        p.circle_stroke(*center, rad, Stroke::new(3.0 * s, col.gamma_multiply(e)));
    }
    // Beschriftungen in den äußeren Teil des Kreises, mittig um ihren Ankerpunkt
    for (i, center) in centers.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let dir = (*center - middle.to_pos2()).normalized();
        let anchor = if k == 1 {
            *center
        } else {
            *center + dir * radius * 0.32
        };
        let item = &items[i];
        // Lange Beschriftungen etwas kleiner, damit Wörter nicht mittendrin umbrechen
        let wrap = radius * 1.15;
        let size = if item.text.chars().count() > 18 {
            17.0
        } else {
            22.0
        } * s;
        let title = galley(&q, &item.text, size, theme.text, wrap, true);
        let sub = (k < 3 && !item.children.is_empty()).then(|| {
            galley(
                &q,
                &child_lines(item),
                16.0 * s,
                theme.text.gamma_multiply(0.7),
                wrap,
                true,
            )
        });
        let height = title.size().y + sub.as_ref().map_or(0.0, |g| g.size().y + 6.0 * s);
        let top = anchor.y - height / 2.0;
        let title_h = title.size().y;
        q.galley(Pos2::new(anchor.x, top), title, theme.text);
        if let Some(sub) = sub {
            q.galley(
                Pos2::new(anchor.x, top + title_h + 6.0 * s),
                sub,
                theme.text,
            );
        }
    }
    // Was alle gemeinsam haben: als Unterschrift mit Schnittmengen-Symbol
    if let Some(shared) = shared {
        let e = reveal(k);
        if e > 0.0 {
            let q = faded(p, e);
            let c = middle.to_pos2();
            let caption_top = r.bottom() - 40.0 * s;
            text_centered(
                &q,
                &format!("{}  {}", icon::INTERSECT, shared.text),
                19.0 * s,
                theme.text,
                r.width() * 0.8,
                Pos2::new(c.x, caption_top),
            );
        }
    }
}

/// Zerlegt „13.8 billion years“ in Zahl und Rest
fn split_number(text: &str) -> Option<(String, String, String)> {
    // Ein kurzes Wort davor ist erlaubt („Vor 13,8 …“, „ca. 5 …“)
    let start = text.find(|c: char| c.is_ascii_digit())?;
    if start > 6 {
        return None;
    }
    let rest = &text[start..];
    let len = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(rest.len());
    let number = rest[..len].trim_end_matches(['.', ',']);
    let after = text[start + number.len()..].trim_start_matches([' ', ':']);
    Some((
        text[..start].to_string(),
        number.to_string(),
        after.to_string(),
    ))
}

/// Zahl wie im Original schreiben (Dezimalstellen, Tausendertrenner), aber mit Wert `v`
fn format_like(original: &str, v: f64) -> String {
    // Ein Trenner, dem genau drei Ziffern folgen, gilt als Tausendertrenner
    let last_sep = original.rfind(['.', ',']);
    let (decimal_sep, decimals) = match last_sep {
        Some(i) if original.len() - i - 1 != 3 => {
            (original[i..=i].chars().next(), original.len() - i - 1)
        }
        _ => (None, 0),
    };
    let thousands = original
        .chars()
        .find(|c| (*c == '.' || *c == ',') && Some(*c) != decimal_sep);
    let text = format!("{v:.decimals$}");
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let mut grouped = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            if let Some(t) = thousands {
                grouped.push(t);
            }
        }
        grouped.push(ch);
    }
    match decimal_sep {
        Some(sep) if !frac.is_empty() => format!("{grouped}{sep}{frac}"),
        _ => grouped,
    }
}

fn parse_number(original: &str) -> Option<f64> {
    let last_sep = original.rfind(['.', ',']);
    let mut cleaned = String::new();
    for (i, ch) in original.char_indices() {
        if ch.is_ascii_digit() {
            cleaned.push(ch);
        } else if Some(i) == last_sep && original.len() - i - 1 != 3 {
            cleaned.push('.');
        }
    }
    cleaned.parse().ok()
}

/// Eine Kennzahl riesig, die beim Erscheinen hochzählt; weitere Punkte darunter
#[allow(clippy::too_many_arguments)]
fn number(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    color: Color32,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
    count_up: f32,
) {
    let first = &items[0];
    let e = ease_out_cubic(count_up);
    let q = faded(p, (count_up * 3.0).min(1.0));
    let top = Pos2::new(r.center().x, r.top() + 10.0 * s);
    let below = match split_number(&first.text) {
        Some((prefix, digits, after)) => {
            let shown = parse_number(&digits)
                .map(|v| format_like(&digits, v * e as f64))
                .unwrap_or(digits);
            let big = text_centered(
                &q,
                &format!("{prefix}{shown}"),
                130.0 * s,
                color,
                r.width(),
                top,
            );
            text_centered(
                &q,
                &after,
                32.0 * s,
                theme.text,
                r.width() * 0.8,
                Pos2::new(top.x, big.bottom() + 4.0 * s),
            )
        }
        None => text_centered(&q, &first.text, 64.0 * s, color, r.width() * 0.9, top),
    };
    // Unterpunkte und weitere Punkte als kleine Fakten
    let facts: Vec<&str> = first
        .children
        .iter()
        .map(String::as_str)
        .chain(items[1..].iter().map(|i| i.text.as_str()))
        .take(3)
        .collect();
    if facts.is_empty() {
        return;
    }
    let gap = 16.0 * s;
    let w = (r.width() - gap * (facts.len() - 1) as f32) / facts.len() as f32;
    let y = (below.bottom() + 40.0 * s).max(r.bottom() - 120.0 * s);
    // Alle Kärtchen gleich hoch
    let card_h = facts
        .iter()
        .map(|f| {
            galley(p, f, 20.0 * s, theme.text, w - 32.0 * s, true)
                .size()
                .y
        })
        .fold(0.0, f32::max)
        + 28.0 * s;
    for (i, fact) in facts.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let x = r.left() + i as f32 * (w + gap);
        let g = galley(&q, fact, 20.0 * s, theme.text, w - 32.0 * s, true);
        let card =
            Rect::from_min_size(Pos2::new(x, y + (1.0 - e) * 14.0 * s), Vec2::new(w, card_h));
        q.rect_filled(card, 12.0 * s, theme.background);
        q.rect_filled(
            Rect::from_min_size(card.min, Vec2::new(w, 4.0 * s)),
            2.0 * s,
            color,
        );
        q.galley(
            Pos2::new(card.center().x, card.top() + 14.0 * s),
            g,
            theme.text,
        );
    }
}

/// Großes Zitat mit Quelle
#[allow(clippy::too_many_arguments)]
fn quote(
    p: &Painter,
    r: Rect,
    items: &[Item],
    note: &str,
    theme: &Theme,
    color: Color32,
    s: f32,
    appear: f32,
) {
    let text = items
        .first()
        .map(|i| i.text.as_str())
        .unwrap_or(note)
        .trim_matches(['"', '“', '”', '„']);
    if text.is_empty() {
        return;
    }
    let author = items
        .first()
        .and_then(|i| i.children.first())
        .or(items.get(1).map(|i| &i.text));
    let e = ease_out_cubic(appear);
    let q = faded(p, e);
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: FontId::proportional(42.0 * s * fit(text.len() / 40, 3)),
            color: theme.text,
            italics: true,
            ..Default::default()
        },
    );
    job.wrap.max_width = r.width() * 0.85;
    job.halign = Align::Center;
    let g = q.layout_job(job);
    let top = Pos2::new(
        r.center().x,
        r.center().y - g.size().y / 2.0 - 20.0 * s + (1.0 - e) * 16.0 * s,
    );
    let used = g.rect.translate(top.to_vec2());
    q.galley(top, g, theme.text);
    // Großes Anführungszeichen links über dem Text
    // Das Zeichen sitzt weit oben in seiner Zeile, darum oben ausrichten
    q.text(
        used.left_top() + Vec2::new(-16.0, -36.0) * s,
        egui::Align2::RIGHT_TOP,
        "“",
        FontId::proportional(150.0 * s),
        color.gamma_multiply(0.45),
    );
    if let Some(author) = author {
        q.text(
            Pos2::new(used.right(), used.bottom() + 24.0 * s),
            egui::Align2::RIGHT_TOP,
            format!("— {author}"),
            FontId::proportional(24.0 * s),
            theme.text.gamma_multiply(0.65),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_split_and_formatted_like_the_original() {
        let (prefix, digits, rest) = split_number("13.8 billion years ago: the Big Bang").unwrap();
        assert_eq!(
            (prefix.as_str(), digits.as_str(), rest.as_str()),
            ("", "13.8", "billion years ago: the Big Bang")
        );
        assert_eq!(parse_number("13.8"), Some(13.8));
        assert_eq!(format_like("13.8", 13.8), "13.8");
        assert_eq!(format_like("13.8", 6.9), "6.9");
        // Tausendertrenner bleiben erhalten, auch mit Komma oder Punkt
        assert_eq!(parse_number("380,000"), Some(380000.0));
        assert_eq!(format_like("380,000", 190000.0), "190,000");
        assert_eq!(format_like("100.000", 2500.0), "2.500");
        // Deutsches Dezimalkomma
        assert_eq!(parse_number("13,8"), Some(13.8));
        assert_eq!(format_like("13,8", 13.8), "13,8");
        assert!(split_number("No number here").is_none());
        let (prefix, digits, _) = split_number("Vor 13,8 Milliarden Jahren").unwrap();
        assert_eq!((prefix.as_str(), digits.as_str()), ("Vor ", "13,8"));
    }
}
