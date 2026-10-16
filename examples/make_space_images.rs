//! Draws the pictures for the space example (assets/examples/images/*.jpg).
//! Everything is computed – no photos, no licensing questions.
//!
//!     cargo run --release --example make_space_images

use image::{Rgb, RgbImage};

type Color = [f32; 3];

// ---------------------------------------------------------------------------
// Randomness and noise
// ---------------------------------------------------------------------------

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let h = hash(
        (x as u32).wrapping_mul(73_856_093)
            ^ (y as u32).wrapping_mul(19_349_663)
            ^ (z as u32).wrapping_mul(83_492_791)
            ^ seed.wrapping_mul(2_654_435_761),
    );
    h as f32 / u32::MAX as f32
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Value noise in 3D, 0…1
fn noise(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (xi, yi, zi) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    let (fx, fy, fz) = (
        smooth(x - x.floor()),
        smooth(y - y.floor()),
        smooth(z - z.floor()),
    );
    let c = |dx, dy, dz| rand3(xi + dx, yi + dy, zi + dz, seed);
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), fx);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), fx);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), fx);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), fx);
    lerp(lerp(x00, x10, fy), lerp(x01, x11, fy), fz)
}

/// Several noise layers on top of each other, 0…1
fn fbm(x: f32, y: f32, z: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * noise(x * freq, y * freq, z * freq, seed + o * 31);
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

fn add(a: Color, b: Color) -> Color {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Color, s: f32) -> Color {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// Colour gradient over several stops
fn ramp(stops: &[(f32, Color)], t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        if t <= w[1].0 {
            return mix(w[0].1, w[1].1, (t - w[0].0) / (w[1].0 - w[0].0));
        }
    }
    stops.last().unwrap().1
}

// ---------------------------------------------------------------------------
// Image with anti-aliasing (2×2 samples per pixel)
// ---------------------------------------------------------------------------

fn render(w: u32, h: u32, f: impl Fn(f32, f32) -> Color + Sync) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    let offsets = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
    for y in 0..h {
        for x in 0..w {
            let mut c = [0.0; 3];
            for (ox, oy) in offsets {
                c = add(c, f(x as f32 + ox, y as f32 + oy));
            }
            let c = scale(c, 0.25);
            // Soft saturation instead of hard clipping
            let tone = |v: f32| 1.0 - (-v.max(0.0) * 1.4).exp();
            img.put_pixel(
                x,
                y,
                Rgb(c.map(|v| (tone(v) * 255.0).round().clamp(0.0, 255.0) as u8)),
            );
        }
    }
    img
}

/// Stars: many faint ones, a few bright ones with a glow
fn stars(x: f32, y: f32, seed: u32, density: f32) -> Color {
    let cell = 6.0;
    let (cx, cy) = ((x / cell).floor() as i32, (y / cell).floor() as i32);
    let mut c = [0.0; 3];
    for dy in -2..=2 {
        for dx in -2..=2 {
            let (gx, gy) = (cx + dx, cy + dy);
            if rand3(gx, gy, 0, seed) > density {
                continue;
            }
            let px = (gx as f32 + rand3(gx, gy, 1, seed)) * cell;
            let py = (gy as f32 + rand3(gx, gy, 2, seed)) * cell;
            let bright = rand3(gx, gy, 3, seed).powf(12.0) * 2.5 + 0.12;
            let d2 = (x - px).powi(2) + (y - py).powi(2);
            let core = (-d2 / 0.7).exp() * bright;
            let halo = (-d2 / (5.0 * bright + 1.0)).exp() * bright * 0.05;
            let tint = mix([0.75, 0.85, 1.0], [1.0, 0.9, 0.75], rand3(gx, gy, 4, seed));
            c = add(c, scale(tint, core + halo));
        }
    }
    c
}

fn save(img: &RgbImage, name: &str) {
    let path = format!("assets/examples/images/{name}");
    let mut file = std::fs::File::create(&path).unwrap();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 86)
        .encode_image(img)
        .unwrap();
    println!(
        "{path}: {} KB",
        std::fs::metadata(&path).unwrap().len() / 1024
    );
}

// ---------------------------------------------------------------------------
// Subjects
// ---------------------------------------------------------------------------

fn nebula() -> RgbImage {
    let (w, h) = (1600, 900);
    render(w, h, |x, y| {
        let (u, v) = (x / w as f32 * 3.2, y / h as f32 * 1.8);
        // Warp for flowing shapes
        let q = fbm(u, v, 0.3, 4, 1);
        let r = fbm(u + 2.0 * q, v + 1.7 * q, 1.1, 6, 2);
        let gas = ((r - 0.38).max(0.0) * 2.4).powf(1.3);
        let dust = smooth(((fbm(u * 1.6 + 5.0, v * 1.6, 2.0, 5, 3) - 0.45) * 3.0).clamp(0.0, 1.0));
        let color = ramp(
            &[
                (0.0, [0.0, 0.0, 0.02]),
                (0.3, [0.25, 0.04, 0.35]),
                (0.55, [0.85, 0.2, 0.45]),
                (0.75, [1.0, 0.55, 0.3]),
                (1.0, [1.3, 1.1, 0.8]),
            ],
            gas * (0.6 + 0.6 * q),
        );
        let blue = scale(
            [0.08, 0.3, 0.85],
            ((fbm(u * 0.8 + 9.0, v * 0.8, 4.0, 5, 4) - 0.45).max(0.0) * 2.2).powi(2),
        );
        let glow = add(color, blue);
        let lit = scale(glow, 1.0 - 0.85 * dust);
        add(lit, scale(stars(x, y, 7, 0.05), 1.0 - 0.7 * dust))
    })
}

fn black_hole() -> RgbImage {
    let (w, h) = (1200, 900);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0 + 10.0);
    let shadow = 120.0;
    let tilt = 0.22; // How flat the disc looks from the side
    render(w, h, |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let dist = (dx * dx + dy * dy).sqrt();
        let mut c = scale(stars(x, y, 11, 0.04), 0.8);

        // Accretion disc in its own plane
        let disk = |dx: f32, dy: f32| -> Color {
            let r = (dx * dx + (dy / tilt).powi(2)).sqrt() / shadow;
            if !(1.35..5.2).contains(&r) {
                return [0.0; 3];
            }
            let angle = (dy / tilt).atan2(dx);
            // Noise on the circle instead of over the angle, otherwise there is a seam at ±180°
            let swirl = fbm(r * 6.0, angle.cos() * 1.6 + r, angle.sin() * 1.6, 5, 5);
            let falloff = ((5.2 - r) / 3.8).powf(1.6) * smooth(((r - 1.35) / 0.3).clamp(0.0, 1.0));
            // The side coming towards us glows brighter
            let beam = 1.0 + 0.55 * (-dx / (r * shadow)).clamp(-1.0, 1.0);
            let heat = ramp(
                &[
                    (0.0, [0.5, 0.08, 0.0]),
                    (0.5, [1.2, 0.45, 0.08]),
                    (0.85, [1.8, 1.1, 0.55]),
                    (1.0, [2.4, 2.0, 1.6]),
                ],
                falloff * (0.55 + 0.6 * swirl),
            );
            scale(heat, falloff * beam * (0.6 + 0.8 * swirl))
        };

        // Light from the far side, bent around the hole: a ring
        let ring_r = shadow * 1.18;
        let ring = (-((dist - ring_r) / 9.0).powi(2)).exp() * 1.6
            + (-((dist - ring_r) / 40.0).powi(2)).exp() * 0.35;
        let beam = 1.0 + 0.4 * (-dx / dist.max(1.0));
        c = add(c, scale([1.6, 0.9, 0.4], ring * beam));

        if dist < shadow {
            c = [0.0; 3];
        }
        // Front part of the disc lies in front of the hole, back part behind it
        if dy > 0.0 || dist >= shadow {
            c = add(c, disk(dx, dy));
        }
        // Slight glow around the hole
        if dist >= shadow {
            c = add(
                c,
                scale(
                    [0.9, 0.4, 0.15],
                    0.12 * (-(dist / (shadow * 2.5)).powi(2)).exp(),
                ),
            );
        }
        c
    })
}

/// Lit sphere; `surface` gets the point on the unit sphere
fn planet(
    w: u32,
    h: u32,
    radius: f32,
    atmosphere: Color,
    surface: impl Fn([f32; 3]) -> Color + Sync,
    extra: impl Fn(f32, f32, Color, Option<f32>) -> Color + Sync,
) -> RgbImage {
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let light = {
        let l = [-0.55_f32, -0.35, 0.76];
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        l.map(|v| v / n)
    };
    render(w, h, |x, y| {
        let (nx, ny) = ((x - cx) / radius, (y - cy) / radius);
        let d2 = nx * nx + ny * ny;
        let mut c = scale(stars(x, y, 23, 0.035), 0.7);
        let mut depth = None;
        if d2 < 1.0 {
            let nz = (1.0 - d2).sqrt();
            let n = [nx, ny, nz];
            let lambert = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
            let limb = 0.55 + 0.45 * nz.sqrt();
            let shade = lambert.powf(0.8) * 1.25 * limb + 0.01;
            c = scale(surface(n), shade);
            // Thin haze at the edge of the day side
            let rim = (1.0 - nz).powi(3);
            c = add(c, scale(atmosphere, rim * lambert.sqrt() * 0.9));
            depth = Some(nz);
        } else {
            let edge = d2.sqrt() - 1.0;
            let len = d2.sqrt();
            let toward_light = ((nx * light[0] + ny * light[1]) / len).max(0.0);
            c = add(
                c,
                scale(atmosphere, (-edge / 0.02).exp() * 0.45 * toward_light),
            );
        }
        extra(x, y, c, depth)
    })
}

fn rocky_planet() -> RgbImage {
    planet(
        1200,
        900,
        330.0,
        [0.9, 0.5, 0.35],
        |n| {
            let (x, y, z) = (n[0] * 2.2 + 3.0, n[1] * 2.2, n[2] * 2.2);
            let land = fbm(x, y, z, 6, 41);
            let detail = fbm(x * 4.0, y * 4.0, z * 4.0, 4, 42);
            let mut c = ramp(
                &[
                    (0.0, [0.25, 0.08, 0.04]),
                    (0.4, [0.55, 0.2, 0.08]),
                    (0.6, [0.8, 0.38, 0.16]),
                    (1.0, [0.95, 0.65, 0.4]),
                ],
                land * 0.8 + detail * 0.3,
            );
            // Polar cap
            let cap = smooth(((-n[1] - 0.84 + 0.25 * (land - 0.5)) / 0.04).clamp(0.0, 1.0));
            c = mix(c, [1.1, 1.05, 1.0], cap);
            c
        },
        |_, _, c, _| c,
    )
}

fn gas_giant() -> RgbImage {
    planet(
        1200,
        900,
        340.0,
        [0.8, 0.7, 0.55],
        |n| {
            let lat = n[1];
            let turb = fbm(n[0] * 3.0, n[1] * 9.0, n[2] * 3.0, 5, 51);
            let band = (lat * 14.0 + turb * 2.2).sin() * 0.5 + 0.5;
            let fine = (lat * 41.0 + turb * 5.0).sin() * 0.5 + 0.5;
            let mut c = ramp(
                &[
                    (0.0, [0.55, 0.3, 0.18]),
                    (0.45, [0.85, 0.65, 0.45]),
                    (0.7, [0.95, 0.88, 0.75]),
                    (1.0, [0.75, 0.5, 0.32]),
                ],
                band * 0.8 + fine * 0.2,
            );
            // Great storm
            let (sx, sy) = (n[0] - 0.25, (lat - 0.32) * 1.8);
            let spot = (-(sx * sx + sy * sy) / 0.012).exp();
            c = mix(c, [0.8, 0.32, 0.18], spot * 0.9);
            c
        },
        |_, _, c, _| c,
    )
}

fn ringed_planet() -> RgbImage {
    let (w, h) = (1200, 900);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let radius = 230.0;
    let tilt = 0.3_f32;
    let rotate = -0.32_f32; // Rings slightly tilted
    let (sin, cos) = rotate.sin_cos();
    let ring_at = move |x: f32, y: f32| -> (f32, f32) {
        // Rotate back into the ring plane
        let (dx, dy) = (x - cx, y - cy);
        let (rx, ry) = (dx * cos + dy * sin, -dx * sin + dy * cos);
        ((rx * rx + (ry / tilt).powi(2)).sqrt() / radius, ry)
    };
    let ring_color = |r: f32| -> Option<(Color, f32)> {
        if !(1.25..2.35).contains(&r) {
            return None;
        }
        let gap = if (1.78..1.86).contains(&r) { 0.1 } else { 1.0 };
        let bands = 0.65 + 0.35 * (r * 60.0).sin() * (r * 23.0).cos();
        let alpha = gap
            * bands
            * smooth(((r - 1.25) / 0.08).min(1.0))
            * smooth(((2.35 - r) / 0.1).min(1.0));
        Some((
            mix([0.75, 0.65, 0.5], [0.95, 0.88, 0.72], (r - 1.25) / 1.1),
            alpha * 0.9,
        ))
    };
    planet(
        w,
        h,
        radius,
        [0.9, 0.85, 0.6],
        |n| {
            let turb = fbm(n[0] * 2.0, n[1] * 7.0, n[2] * 2.0, 4, 61);
            let band = (n[1] * 11.0 + turb * 1.2).sin() * 0.5 + 0.5;
            ramp(
                &[
                    (0.0, [0.7, 0.58, 0.38]),
                    (0.5, [0.92, 0.82, 0.6]),
                    (1.0, [0.82, 0.7, 0.48]),
                ],
                band,
            )
        },
        move |x, y, c, depth| {
            let (r, ry) = ring_at(x, y);
            let Some((col, alpha)) = ring_color(r) else {
                return c;
            };
            // Behind the sphere (upper half of the ring) the ring is hidden
            if depth.is_some() && ry < 0.0 {
                return c;
            }
            let lit = scale(col, 0.95);
            mix(c, lit, alpha)
        },
    )
}

fn main() {
    std::fs::create_dir_all("assets/examples/images").unwrap();
    save(&nebula(), "nebula.jpg");
    save(&black_hole(), "black-hole.jpg");
    save(&rocky_planet(), "rocky-planet.jpg");
    save(&gas_giant(), "gas-giant.jpg");
    save(&ringed_planet(), "ringed-planet.jpg");
}
