//! Folien-Layouts im Stil von SmartArt: Die Stichpunkte einer Folie werden
//! als Zeitstrahl, Kreislauf, Pyramide, Trichter, Vergleich, Matrix,
//! Venn-Diagramm, große Zahl, Zitat oder als Balken-, Kreis- und
//! Liniendiagramm aus Punkten wie „Solar: 42 %“ gezeichnet.
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
            SlideLayout::Bars => "Bar chart",
            SlideLayout::Pie => "Pie chart",
            SlideLayout::Line => "Line chart",
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
            SlideLayout::Bars => icon::CHART_BAR,
            SlideLayout::Pie => icon::CHART_DONUT,
            SlideLayout::Line => icon::CHART_LINE_UP,
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
        SlideLayout::Bars => bars(p, rect, items, theme, s, reveal),
        SlideLayout::Pie => pie(p, rect, items, theme, s, reveal),
        SlideLayout::Line => line(p, rect, items, theme, color, s, reveal),
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

// ---------------------------------------------------------------------------
// Diagramme mit Zahlen
// ---------------------------------------------------------------------------

/// Ein Datenpunkt aus einem Stichpunkt wie „Solar: 42 %“
#[derive(Debug, PartialEq)]
pub struct Datum {
    pub label: String,
    pub value: f64,
    /// Die Zahl, wie sie geschrieben wurde („42“, „1,250.5“)
    pub number: String,
    /// Was danach kommt („ %“, „ km“)
    pub unit: String,
}

impl Datum {
    /// Wert schreiben wie im Original, beim Hochzählen mit `share` (0…1) des Werts
    fn shown(&self, share: f32) -> String {
        format!(
            "{}{}",
            format_like(&self.number, self.value * share as f64),
            self.unit
        )
    }
}

/// Die letzte Zahl im Text ist der Wert, davor steht die Beschriftung
pub fn parse_datum(text: &str) -> Option<Datum> {
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    let last_digit = bytes.iter().rposition(|(_, c)| c.is_ascii_digit())?;
    let mut first = last_digit;
    while first > 0 {
        let (_, c) = bytes[first - 1];
        let separator = (c == '.' || c == ',') && first >= 2 && bytes[first - 2].1.is_ascii_digit();
        if c.is_ascii_digit() || separator {
            first -= 1;
        } else {
            break;
        }
    }
    let start = bytes[first].0;
    let end = bytes[last_digit].0 + 1;
    let number = &text[start..end];
    let value = parse_number(number)?;
    let label = text[..start]
        .trim_end()
        .trim_end_matches([':', '=', '–', '-'])
        .trim()
        .to_string();
    Some(Datum {
        label,
        value,
        number: number.to_string(),
        unit: text[end..].trim_end().to_string(),
    })
}

fn data(items: &[Item]) -> Vec<Datum> {
    items.iter().filter_map(|i| parse_datum(&i.text)).collect()
}

/// Hinweis, wenn keine Zahlen gefunden wurden
fn no_data(p: &Painter, r: Rect, theme: &Theme, s: f32) {
    text_centered(
        p,
        "Add points with numbers, e.g. “Solar: 42 %”",
        20.0 * s,
        theme.text.gamma_multiply(0.5),
        r.width(),
        Pos2::new(r.center().x, r.center().y - 12.0 * s),
    );
}

/// Feine Hilfslinien mit „runden“ Werten; gibt den Höchstwert der Skala zurück
fn grid_lines(p: &Painter, plot: Rect, data: &[Datum], theme: &Theme, s: f32) -> f64 {
    let max = data
        .iter()
        .map(|d| d.value)
        .fold(0.0, f64::max)
        .max(f64::EPSILON);
    // Tausendertrenner wie in den Daten (Komma oder Punkt)
    let sep = data.iter().find_map(|d| {
        let i = d.number.rfind([',', '.'])?;
        (d.number.len() - i - 1 == 3).then(|| d.number[i..=i].to_string())
    });
    // Schöne Schrittweite: 1, 2 oder 5 mal einer Zehnerpotenz
    let raw = (max / 4.0).max(f64::EPSILON);
    let pow = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * pow)
        .find(|st| *st >= raw)
        .unwrap_or(raw);
    let top = (max / step).ceil() * step;
    let mut v = step;
    while v <= top + step * 0.01 {
        let y = plot.bottom() - (v / top) as f32 * plot.height();
        p.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            Stroke::new(1.0 * s, theme.grid.gamma_multiply(0.7)),
        );
        p.text(
            Pos2::new(plot.left() - 10.0 * s, y),
            egui::Align2::RIGHT_CENTER,
            match (&sep, step.fract() == 0.0) {
                (Some(sep), true) if v >= 1000.0 => format_like(&format!("1{sep}000"), v),
                (_, true) => format_like("1", v),
                _ => format_like("1.0", v),
            },
            FontId::proportional(14.0 * s),
            theme.text.gamma_multiply(0.45),
        );
        v += step;
    }
    p.line_segment(
        [
            Pos2::new(plot.left(), plot.bottom()),
            Pos2::new(plot.right(), plot.bottom()),
        ],
        Stroke::new(2.0 * s, theme.grid),
    );
    top
}

/// Säulen, die nacheinander hochwachsen; Werte zählen dabei hoch
fn bars(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let data = data(items);
    if data.is_empty() {
        return no_data(p, r, theme, s);
    }
    let n = data.len();
    let f = fit(n, 6);
    let label_h = 48.0 * s;
    let plot = Rect::from_min_max(
        r.min + Vec2::new(56.0 * s, 34.0 * s),
        r.max - Vec2::new(0.0, label_h),
    );
    let top = grid_lines(p, plot, &data, theme, s);
    let slot = plot.width() / n as f32;
    let bar_w = (slot * 0.62).min(120.0 * s);
    for (i, d) in data.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let grow = ease_out_cubic(e);
        let q = faded(p, (e * 2.0).min(1.0));
        let cx = plot.left() + slot * (i as f32 + 0.5);
        let h = (d.value.max(0.0) / top) as f32 * plot.height() * grow;
        let bar = Rect::from_min_max(
            Pos2::new(cx - bar_w / 2.0, plot.bottom() - h),
            Pos2::new(cx + bar_w / 2.0, plot.bottom()),
        );
        let col = palette(theme, i);
        q.rect_filled(
            bar,
            egui::CornerRadius {
                nw: (8.0 * s) as u8,
                ne: (8.0 * s) as u8,
                sw: 0,
                se: 0,
            },
            col,
        );
        q.text(
            Pos2::new(cx, bar.top() - 6.0 * s),
            egui::Align2::CENTER_BOTTOM,
            d.shown(grow),
            FontId::proportional(20.0 * s * f),
            theme.text,
        );
        text_centered(
            &q,
            &d.label,
            17.0 * s * f,
            theme.text.gamma_multiply(0.75),
            slot * 0.95,
            Pos2::new(cx, plot.bottom() + 10.0 * s),
        );
    }
}

/// Ring aus Segmenten, die sich nacheinander schließen; Legende rechts
fn pie(p: &Painter, r: Rect, items: &[Item], theme: &Theme, s: f32, reveal: &dyn Fn(usize) -> f32) {
    let data = data(items);
    if data.is_empty() {
        return no_data(p, r, theme, s);
    }
    let total: f64 = data.iter().map(|d| d.value.max(0.0)).sum();
    if total <= 0.0 {
        return no_data(p, r, theme, s);
    }
    let radius = (r.height() / 2.0 - 8.0 * s).min(r.width() * 0.22);
    let inner = radius * 0.6;
    let center = Pos2::new(r.left() + r.width() * 0.3, r.center().y);
    let gap = if data.len() > 1 { 0.012 } else { 0.0 };
    let mut angle = -PI / 2.0;
    let mut largest: Option<(usize, f32)> = None;
    for (i, d) in data.iter().enumerate() {
        let sweep = (d.value.max(0.0) / total) as f32 * TAU;
        let e = ease_out_cubic(reveal(i));
        if e > 0.0 && sweep > 0.0 {
            let shown = (sweep - gap).max(0.0) * e;
            let a0 = angle + gap / 2.0;
            let steps = ((shown / TAU) * 120.0).ceil().max(2.0) as usize;
            let col = palette(theme, i);
            let mut mesh = egui::Mesh::default();
            for k in 0..=steps {
                let a = a0 + shown * k as f32 / steps as f32;
                let dir = Vec2::new(a.cos(), a.sin());
                mesh.colored_vertex(center + dir * inner, col);
                mesh.colored_vertex(center + dir * radius, col);
                if k > 0 {
                    let j = 2 * k as u32;
                    mesh.add_triangle(j - 2, j - 1, j);
                    mesh.add_triangle(j - 1, j + 1, j);
                }
            }
            p.add(Shape::mesh(mesh));
            if largest.is_none_or(|(_, w)| sweep > w) {
                largest = Some((i, sweep));
            }
        }
        angle += sweep;
    }
    // In der Mitte der größte Anteil
    if let Some((i, _)) = largest {
        let d = &data[i];
        let e = reveal(i);
        let q = faded(p, e);
        let share = d.value / total * 100.0;
        let big = if d.unit.trim() == "%" {
            d.shown(1.0)
        } else {
            format!("{share:.0} %")
        };
        q.text(
            center - Vec2::new(0.0, 6.0 * s),
            egui::Align2::CENTER_BOTTOM,
            big,
            FontId::proportional(40.0 * s),
            palette(theme, i),
        );
        text_centered(
            &q,
            &d.label,
            17.0 * s,
            theme.text.gamma_multiply(0.75),
            inner * 1.6,
            center + Vec2::new(0.0, 2.0 * s),
        );
    }
    // Legende
    let x = center.x + radius + 60.0 * s;
    let row = (r.height() / data.len() as f32).min(54.0 * s);
    let top = r.center().y - row * data.len() as f32 / 2.0;
    for (i, d) in data.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let y = top + row * (i as f32 + 0.5) + (1.0 - e) * 10.0 * s;
        q.circle_filled(Pos2::new(x, y), 8.0 * s, palette(theme, i));
        let value = if d.unit.trim() == "%" {
            d.shown(1.0)
        } else {
            format!("{}  ·  {:.0} %", d.shown(1.0), d.value / total * 100.0)
        };
        let g = galley(
            &q,
            &d.label,
            21.0 * s,
            theme.text,
            r.right() - x - 160.0 * s,
            false,
        );
        let h = g.size().y;
        q.galley(Pos2::new(x + 22.0 * s, y - h / 2.0), g, theme.text);
        q.text(
            Pos2::new(r.right(), y),
            egui::Align2::RIGHT_CENTER,
            value,
            FontId::proportional(20.0 * s),
            theme.text.gamma_multiply(0.7),
        );
    }
}

/// Linie, die von Punkt zu Punkt weiterwächst, mit leichter Fläche darunter
#[allow(clippy::too_many_arguments)]
fn line(
    p: &Painter,
    r: Rect,
    items: &[Item],
    theme: &Theme,
    color: Color32,
    s: f32,
    reveal: &dyn Fn(usize) -> f32,
) {
    let data = data(items);
    if data.is_empty() {
        return no_data(p, r, theme, s);
    }
    let n = data.len();
    let f = fit(n, 7);
    let plot = Rect::from_min_max(
        r.min + Vec2::new(64.0 * s, 40.0 * s),
        r.max - Vec2::new(20.0 * s, 44.0 * s),
    );
    let top = grid_lines(p, plot, &data, theme, s);
    let step = if n > 1 {
        plot.width() / (n - 1) as f32
    } else {
        0.0
    };
    let at = |i: usize| {
        let x = if n > 1 {
            plot.left() + step * i as f32
        } else {
            plot.center().x
        };
        Pos2::new(
            x,
            plot.bottom() - (data[i].value.max(0.0) / top) as f32 * plot.height(),
        )
    };
    // Linie und Fläche, Abschnitt für Abschnitt
    for i in 1..n {
        let e = ease_out_cubic(reveal(i));
        if e <= 0.0 {
            continue;
        }
        let (a, b) = (at(i - 1), at(i));
        let end = a + (b - a) * e;
        p.add(Shape::convex_polygon(
            vec![
                a,
                end,
                Pos2::new(end.x, plot.bottom()),
                Pos2::new(a.x, plot.bottom()),
            ],
            color.gamma_multiply(0.14),
            Stroke::NONE,
        ));
        p.line_segment([a, end], Stroke::new(4.0 * s, color));
    }
    for (i, d) in data.iter().enumerate() {
        let e = reveal(i);
        if e <= 0.0 {
            continue;
        }
        let q = faded(p, e);
        let pt = at(i);
        let size = 7.0 * s * ease_out_back(e).max(0.0);
        q.circle_filled(pt, size + 3.0 * s, theme.card);
        q.circle_filled(pt, size, color);
        q.text(
            pt - Vec2::new(0.0, 14.0 * s),
            egui::Align2::CENTER_BOTTOM,
            d.shown(ease_out_cubic(e)),
            FontId::proportional(18.0 * s * f),
            theme.text,
        );
        text_centered(
            &q,
            &d.label,
            16.0 * s * f,
            theme.text.gamma_multiply(0.7),
            (step * 0.95).max(80.0 * s),
            Pos2::new(pt.x, plot.bottom() + 10.0 * s),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_points_are_read_from_bullets() {
        let d = parse_datum("Solar: 42 %").unwrap();
        assert_eq!(
            (d.label.as_str(), d.value, d.unit.as_str()),
            ("Solar", 42.0, " %")
        );
        assert_eq!(d.shown(0.5), "21 %");
        let d = parse_datum("2010: 1,250").unwrap();
        assert_eq!((d.label.as_str(), d.value), ("2010", 1250.0));
        let d = parse_datum("Jupiter – 139.8 thousand km").unwrap();
        assert_eq!(
            (d.label.as_str(), d.value, d.unit.as_str()),
            ("Jupiter", 139.8, " thousand km")
        );
        assert!(parse_datum("No number").is_none());
    }

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
