//! Optional sound feedback: short, quiet, synthesized sounds – a soft pop when a
//! node is added, a chime when a task is ticked off, a whoosh when switching views.
//!
//! Nothing is loaded from files: every sound is computed here (sine tones with
//! pitch drops, band-passed noise). Playback runs on a thread of its own with
//! `rodio`, started the first time a sound plays. Off by default.

use std::f32::consts::{PI, TAU};
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const RATE: u32 = 44_100;

static ENABLED: AtomicBool = AtomicBool::new(false);
/// Volume 0…1 as `f32` bits
static VOLUME: AtomicU32 = AtomicU32::new(0x3F00_0000); // 0.5
/// Channel to the audio thread (started on the first sound)
static PLAYER: Mutex<Option<Sender<Sound>>> = Mutex::new(None);
/// Last time each sound played – so held keys don't stack up a wall of noise
static LAST: Mutex<Vec<(Sound, Instant)>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sound {
    /// A node was added: soft pop with a falling pitch
    Create,
    /// A node was deleted: a lower, duller pop
    Delete,
    /// A task was ticked off: two-tone chime (a fifth)
    Done,
    /// Switching between views: band-passed noise sweeping up, panning left to right
    Whoosh,
    /// Next slide or tour stop: a short, light swish
    Swish,
}

/// Called every frame with the settings
pub fn configure(enabled: bool, volume: f32) {
    ENABLED.store(enabled, Ordering::Relaxed);
    VOLUME.store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

pub fn play(sound: Sound) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    if let Ok(mut last) = LAST.lock() {
        let now = Instant::now();
        if let Some((_, at)) = last.iter_mut().find(|(s, _)| *s == sound) {
            if now.duration_since(*at) < Duration::from_millis(60) {
                return;
            }
            *at = now;
        } else {
            last.push((sound, now));
        }
    }
    let Ok(mut player) = PLAYER.lock() else {
        return;
    };
    let sender = player.get_or_insert_with(start);
    if sender.send(sound).is_err() {
        // No audio device: stay quiet instead of retrying on every click
        ENABLED.store(false, Ordering::Relaxed);
    }
}

/// Starts the audio thread. If no output device can be opened, the thread ends
/// and later sends fail (see `play`).
fn start() -> Sender<Sound> {
    let (tx, rx) = channel::<Sound>();
    let _ = std::thread::Builder::new()
        .name("mindful-sound".into())
        .spawn(move || {
            let Ok(mut sink) = rodio::DeviceSinkBuilder::open_default_sink() else {
                return;
            };
            sink.log_on_drop(false);
            for sound in rx {
                let volume = f32::from_bits(VOLUME.load(Ordering::Relaxed));
                let samples: Vec<f32> = render(sound).into_iter().map(|s| s * volume).collect();
                let buffer = rodio::buffer::SamplesBuffer::new(
                    NonZero::new(2).unwrap(),
                    NonZero::new(RATE).unwrap(),
                    samples,
                );
                sink.mixer().add(buffer);
            }
        });
    tx
}

/// The sound as interleaved stereo samples (left, right, left, …)
pub fn render(sound: Sound) -> Vec<f32> {
    let mono = |samples: Vec<f32>| samples.iter().flat_map(|s| [*s, *s]).collect::<Vec<_>>();
    match sound {
        Sound::Create => mono(pop(1250.0, 720.0, 0.09, 0.028, 0.22)),
        Sound::Delete => mono(pop(520.0, 260.0, 0.12, 0.04, 0.22)),
        Sound::Done => mono(chime()),
        Sound::Whoosh => whoosh(0.42, 350.0, 2600.0, 0.16, (-0.7, 0.7)),
        Sound::Swish => whoosh(0.2, 900.0, 3800.0, 0.09, (-0.3, 0.3)),
    }
}

fn seconds(len: f32) -> usize {
    (len * RATE as f32) as usize
}

/// Short fade at the end, so no sound stops with a click
fn fade_out(samples: &mut [f32]) {
    let n = seconds(0.006).min(samples.len());
    let len = samples.len();
    for (i, s) in samples[len - n..].iter_mut().enumerate() {
        *s *= 1.0 - i as f32 / n as f32;
    }
}

/// Sine that glides from `from` to `to` Hz and dies away with `decay` seconds
fn pop(from: f32, to: f32, len: f32, decay: f32, gain: f32) -> Vec<f32> {
    let mut phase = 0.0f32;
    let mut out: Vec<f32> = (0..seconds(len))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let freq = to + (from - to) * (-t / 0.012).exp();
            phase = (phase + TAU * freq / RATE as f32) % TAU;
            let attack = (t / 0.0015).min(1.0);
            // A little second harmonic makes it rounder, like a wooden click
            let tone = phase.sin() + 0.18 * (2.0 * phase).sin();
            tone * attack * (-t / decay).exp() * gain
        })
        .collect();
    fade_out(&mut out);
    out
}

/// Two bell-like tones a fifth apart, the second one slightly later
fn chime() -> Vec<f32> {
    let len = 0.55;
    let mut out: Vec<f32> = (0..seconds(len))
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let bell = |freq: f32, start: f32| {
                let t = t - start;
                if t < 0.0 {
                    return 0.0;
                }
                let attack = (t / 0.004).min(1.0);
                let body = (TAU * freq * t).sin() + 0.25 * (TAU * freq * 2.01 * t).sin();
                body * attack * (-t / 0.16).exp()
            };
            (bell(880.0, 0.0) + 0.8 * bell(1318.5, 0.07)) * 0.11
        })
        .collect();
    fade_out(&mut out);
    out
}

/// Noise through a band-pass filter whose centre sweeps from `from` to `to` Hz,
/// swelling and fading, and panning between `pan.0` and `pan.1` (−1 left … 1 right)
fn whoosh(len: f32, from: f32, to: f32, gain: f32, pan: (f32, f32)) -> Vec<f32> {
    let n = seconds(len);
    let mut seed: u32 = 0x9E37_79B9;
    let mut noise = move || {
        // xorshift: fast, good enough for noise
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    // State-variable filter
    let (mut low, mut band) = (0.0f32, 0.0f32);
    let q = 1.0 / 1.4;
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let x = i as f32 / n as f32;
        let centre = from * (to / from).powf(x);
        let f = 2.0 * (PI * centre / RATE as f32).sin();
        let high = noise() - low - q * band;
        band += f * high;
        low += f * band;
        // Rises quickly, falls slowly
        let swell = (PI * x.powf(0.6)).sin().powi(2);
        let s = band * swell * gain;
        let p = pan.0 + (pan.1 - pan.0) * x;
        let angle = (p + 1.0) * PI / 4.0;
        out.push(s * angle.cos());
        out.push(s * angle.sin());
    }
    fade_out(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_short_quiet_and_end_softly() {
        for sound in [
            Sound::Create,
            Sound::Delete,
            Sound::Done,
            Sound::Whoosh,
            Sound::Swish,
        ] {
            let samples = render(sound);
            let secs = samples.len() as f32 / 2.0 / RATE as f32;
            assert!(secs > 0.05 && secs < 0.6, "{sound:?}: {secs} s");
            let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.02 && peak < 0.5, "{sound:?}: peak {peak}");
            assert!(
                samples.last().unwrap().abs() < 1e-3,
                "{sound:?} ends with a click"
            );
            assert!(samples.iter().all(|s| s.is_finite()));
        }
        // The whoosh moves from left to right
        let w = render(Sound::Whoosh);
        let energy =
            |part: &[f32], ch: usize| -> f32 { part.chunks(2).map(|f| f[ch] * f[ch]).sum() };
        let (start, end) = w.split_at(w.len() / 2);
        assert!(energy(start, 0) > energy(start, 1));
        assert!(energy(end, 1) > energy(end, 0));
        // Off by default: playing does nothing (and starts no thread)
        play(Sound::Create);
        assert!(PLAYER.lock().unwrap().is_none());
    }

    /// Writes every sound as a WAV file to listen to:
    /// `cargo test --release render_ -- --ignored` → target/snapshots/sounds/*.wav
    #[test]
    #[ignore]
    fn render_sounds() {
        let dir = std::path::Path::new("target/snapshots/sounds");
        std::fs::create_dir_all(dir).unwrap();
        for (name, sound) in [
            ("create", Sound::Create),
            ("delete", Sound::Delete),
            ("done", Sound::Done),
            ("whoosh", Sound::Whoosh),
            ("swish", Sound::Swish),
        ] {
            let pcm: Vec<u8> = render(sound)
                .iter()
                .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
                .collect();
            // 16-bit stereo PCM
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&2u16.to_le_bytes());
            wav.extend_from_slice(&RATE.to_le_bytes());
            wav.extend_from_slice(&(RATE * 4).to_le_bytes());
            wav.extend_from_slice(&4u16.to_le_bytes());
            wav.extend_from_slice(&16u16.to_le_bytes());
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
            wav.extend_from_slice(&pcm);
            std::fs::write(dir.join(format!("{name}.wav")), wav).unwrap();
        }
    }
}
