//! rimelight_sound: short tones and a metronome for the apps, with one API on every target.
//!
//! - [`Tone`] and [`tone_samples`]: a sine tone with a 5 ms attack and a 10 ms release, so it starts and stops without a
//!   click. Pure arithmetic, no device.
//! - [`beat_period`], [`beat_start`] and [`is_accent`]: the metronome's schedule. Beat times are computed from the beat
//!   index in whole nanoseconds, so the tempo does not drift over a long session.
//! - [`play`]: plays one tone on the default output device without blocking the caller (desktop). A no-op elsewhere.
//! - [`Metronome`]: clicks at a tempo on a background thread until it is dropped or stopped. An accented click starts
//!   every `accent_every` beats (0 means no accents).
//!
//! Tests do not open an audio device. They check the sample math and the schedule.
//!
//! Known limits: each click opens the output device again (fine at metronome tempos, not for a sample-accurate score),
//! and there is no Android or wasm32 backend yet.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Length of the fade-in at the start of a tone, in milliseconds.
const ATTACK_MS: f32 = 5.0;
/// Length of the fade-out at the end of a tone, in milliseconds.
const RELEASE_MS: f32 = 10.0;

/// The slowest and fastest tempo the metronome accepts, in beats per minute.
pub const BPM_RANGE: (u32, u32) = (20, 300);

/// A sine tone: frequency in hertz, length in milliseconds and loudness from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tone {
    pub hz: f32,
    pub ms: u32,
    /// Peak amplitude, 0 to 1. Values outside the range are clamped.
    pub volume: f32,
}

/// The metronome's click on an ordinary beat.
pub const CLICK: Tone = Tone {
    hz: 800.0,
    ms: 20,
    volume: 0.4,
};

/// The metronome's click on an accented beat (the bar's first beat).
pub const ACCENT_CLICK: Tone = Tone {
    hz: 1200.0,
    ms: 30,
    volume: 0.6,
};

/// The number of samples a tone of `ms` milliseconds takes at `sample_rate`.
pub fn sample_count(ms: u32, sample_rate: u32) -> usize {
    (u64::from(ms) * u64::from(sample_rate) / 1000) as usize
}

/// The mono samples of `tone` at `sample_rate`, in -1 to 1. A tone of zero frequency is silent.
pub fn tone_samples(tone: Tone, sample_rate: u32) -> Vec<f32> {
    let count = sample_count(tone.ms, sample_rate);
    if count == 0 || sample_rate == 0 || tone.hz <= 0.0 {
        return vec![0.0; count];
    }
    let volume = tone.volume.clamp(0.0, 1.0);
    let rate = sample_rate as f32;
    let attack = (ATTACK_MS / 1000.0 * rate).max(1.0);
    let release = (RELEASE_MS / 1000.0 * rate).max(1.0);
    let total = count as f32;
    (0..count)
        .map(|i| {
            let n = i as f32;
            let phase = std::f32::consts::TAU * tone.hz * n / rate;
            // The envelope rises over the attack, holds, and falls over the release, so both ends are at 0.
            let fade_in = (n / attack).min(1.0);
            let fade_out = ((total - n) / release).min(1.0);
            phase.sin() * volume * fade_in * fade_out
        })
        .collect()
}

/// The time between two beats at `bpm` beats per minute (clamped to [`BPM_RANGE`]).
pub fn beat_period(bpm: u32) -> Duration {
    let bpm = bpm.clamp(BPM_RANGE.0, BPM_RANGE.1);
    Duration::from_nanos(60_000_000_000 / u64::from(bpm))
}

/// When beat `beat` (counting from 0) starts, measured from the first beat. Computed from the index, not by adding a
/// period each time, so the rounding never builds up.
pub fn beat_start(bpm: u32, beat: u64) -> Duration {
    let bpm = u128::from(bpm.clamp(BPM_RANGE.0, BPM_RANGE.1));
    let nanos = u128::from(beat) * 60_000_000_000 / bpm;
    Duration::from_nanos(nanos as u64)
}

/// True when beat `beat` is accented: every `accent_every`th beat, starting with beat 0. `accent_every` of 0 means no
/// accents.
pub fn is_accent(beat: u64, accent_every: u32) -> bool {
    accent_every > 0 && beat.is_multiple_of(u64::from(accent_every))
}

/// Plays `tone` on the default output device and returns at once. Errors (no device, an unsupported format) are
/// dropped: a missing sound does not stop the app.
pub fn play(tone: Tone) {
    imp::play(tone)
}

/// A running metronome. Dropping it, or calling [`Metronome::stop`], stops the clicks.
pub struct Metronome {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Metronome {
    /// Starts clicking at `bpm` beats per minute (clamped to [`BPM_RANGE`]). Beat 0 is accented when `accent_every`
    /// is above 0. The first click is immediate.
    pub fn start(bpm: u32, accent_every: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("rimelight-metronome".into())
            .spawn(move || {
                let origin = Instant::now();
                let mut beat: u64 = 0;
                while !flag.load(Ordering::Relaxed) {
                    let click = if is_accent(beat, accent_every) {
                        ACCENT_CLICK
                    } else {
                        CLICK
                    };
                    play(click);
                    beat += 1;
                    let due = origin + beat_start(bpm, beat);
                    // Sleep in short slices so a stop is seen within 10 ms.
                    while let Some(left) = due.checked_duration_since(Instant::now()) {
                        if flag.load(Ordering::Relaxed) {
                            return;
                        }
                        thread::sleep(left.min(Duration::from_millis(10)));
                    }
                }
            })
            .ok();
        Self { stop, thread }
    }

    /// Stops the clicks and waits for the scheduler thread to end.
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Metronome {
    fn drop(&mut self) {
        self.halt();
    }
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod imp {
    use std::thread;
    use std::time::Duration;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};

    use crate::{Tone, tone_samples};

    pub fn play(tone: Tone) {
        // A thread per tone: the caller never waits for the device.
        let _ = thread::Builder::new()
            .name("rimelight-tone".into())
            .spawn(move || {
                let _ = play_now(tone);
            });
    }

    /// Opens the default output, plays `tone` through it, and returns when the tone has ended.
    fn play_now(tone: Tone) -> Result<(), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("no audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = supported.sample_rate();
        let channels = usize::from(supported.channels()).max(1);
        let config: StreamConfig = supported.config();
        let samples = tone_samples(tone, rate);
        let stream = match supported.sample_format() {
            SampleFormat::F32 => build::<f32>(&device, &config, channels, samples)?,
            SampleFormat::I16 => build::<i16>(&device, &config, channels, samples)?,
            SampleFormat::U16 => build::<u16>(&device, &config, channels, samples)?,
            other => return Err(format!("unsupported sample format {other:?}")),
        };
        stream.play().map_err(|e| e.to_string())?;
        // The stream plays the samples and then silence. Wait out the tone plus a margin, then release the device.
        thread::sleep(Duration::from_millis(u64::from(tone.ms) + 50));
        drop(stream);
        Ok(())
    }

    fn build<T>(
        device: &cpal::Device,
        config: &StreamConfig,
        channels: usize,
        samples: Vec<f32>,
    ) -> Result<Stream, String>
    where
        T: SizedSample + FromSample<f32>,
    {
        let mut position = 0usize;
        device
            .build_output_stream(
                *config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let value = samples.get(position).copied().unwrap_or(0.0);
                        position = position.saturating_add(1);
                        let out = T::from_sample(value);
                        for slot in frame.iter_mut() {
                            *slot = out;
                        }
                    }
                },
                |err| eprintln!("rimelight_sound: output stream error: {err}"),
                None,
            )
            .map_err(|e| e.to_string())
    }
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
mod imp {
    use crate::Tone;

    /// No audio backend on this target yet: the tone is not played.
    pub fn play(_: Tone) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_count_matches_length() {
        assert_eq!(sample_count(1000, 48_000), 48_000);
        assert_eq!(sample_count(20, 44_100), 882);
        assert_eq!(sample_count(0, 48_000), 0);
    }

    #[test]
    fn tone_has_the_requested_length_and_silent_ends() {
        let tone = Tone {
            hz: 440.0,
            ms: 100,
            volume: 1.0,
        };
        let samples = tone_samples(tone, 48_000);
        assert_eq!(samples.len(), 4_800);
        assert!(samples[0].abs() < 1e-6, "starts at zero");
        assert!(samples[samples.len() - 1].abs() < 1e-3, "ends near zero");
    }

    #[test]
    fn tone_peak_stays_within_volume() {
        let tone = Tone {
            hz: 1000.0,
            ms: 50,
            volume: 0.5,
        };
        let peak = tone_samples(tone, 48_000)
            .iter()
            .fold(0.0_f32, |acc, s| acc.max(s.abs()));
        assert!(peak <= 0.5 + 1e-6);
        assert!(peak > 0.3, "the tone is audible, peak {peak}");
    }

    #[test]
    fn volume_is_clamped_and_degenerate_tones_are_silent() {
        let loud = tone_samples(
            Tone {
                hz: 1000.0,
                ms: 20,
                volume: 9.0,
            },
            48_000,
        );
        assert!(loud.iter().all(|s| s.abs() <= 1.0 + 1e-6));
        let zero_hz = tone_samples(
            Tone {
                hz: 0.0,
                ms: 20,
                volume: 1.0,
            },
            48_000,
        );
        assert!(zero_hz.iter().all(|s| *s == 0.0));
        let no_rate = tone_samples(
            Tone {
                hz: 440.0,
                ms: 20,
                volume: 1.0,
            },
            0,
        );
        assert!(no_rate.is_empty());
    }

    #[test]
    fn beat_period_follows_tempo_and_clamps() {
        assert_eq!(beat_period(60), Duration::from_secs(1));
        assert_eq!(beat_period(120), Duration::from_millis(500));
        assert_eq!(beat_period(1), beat_period(BPM_RANGE.0));
        assert_eq!(beat_period(10_000), beat_period(BPM_RANGE.1));
    }

    #[test]
    fn beat_starts_do_not_drift() {
        // Beat 7 at 97 BPM is the exact fraction of the minute, with no accumulated rounding.
        let exact = 7u128 * 60_000_000_000 / 97;
        assert_eq!(beat_start(97, 7), Duration::from_nanos(exact as u64));
        // An hour of beats at 60 BPM lands on exactly 3600 seconds.
        assert_eq!(beat_start(60, 3600), Duration::from_secs(3600));
    }

    #[test]
    fn accents_fall_on_the_bar_start() {
        assert!(is_accent(0, 4));
        assert!(!is_accent(1, 4));
        assert!(is_accent(4, 4));
        assert!(is_accent(8, 4));
        assert!(!is_accent(0, 0), "zero means no accents");
        assert!(is_accent(5, 1), "every beat accented");
    }
}
