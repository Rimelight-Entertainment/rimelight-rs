//! Playback speed without changing pitch: WSOLA (waveform-similarity overlap-add) on interleaved f32 sound. Each output
//! block overlaps the previous one by half; the next input block is taken near `speed` times further on, at the offset
//! whose waveform best continues what was just played, so speech and music stay smooth.

/// Frames per analysis block (about 21 ms at 48 kHz).
const BLOCK: usize = 1024;
const HALF: usize = BLOCK / 2;
/// How far (frames) the search may move a block to find the best match.
const SEEK: usize = 256;

pub struct Stretch {
    channels: usize,
    speed: f64,
    window: Vec<f32>,
    /// Waiting input, interleaved.
    input: Vec<f32>,
    /// Where the next block starts in `input` (frames, fractional).
    pos: f64,
    /// Second half of the last windowed block, waiting to overlap the next one.
    tail: Vec<f32>,
    /// What naturally followed the last block (mono), the target for the similarity search.
    natural: Vec<f32>,
    started: bool,
}

impl Stretch {
    pub fn new(channels: usize, speed: f64) -> Stretch {
        let window = (0..BLOCK)
            .map(|n| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / BLOCK as f32).cos())
            .collect();
        Stretch {
            channels: channels.max(1),
            speed: speed.clamp(0.25, 4.0),
            window,
            input: Vec::new(),
            pos: 0.0,
            tail: vec![0.0; HALF * channels.max(1)],
            natural: Vec::new(),
            started: false,
        }
    }

    fn frames(&self) -> usize {
        self.input.len() / self.channels
    }

    fn mono(&self, start: usize, len: usize) -> Vec<f32> {
        let ch = self.channels;
        (start..start + len)
            .map(|f| self.input[f * ch..f * ch + ch].iter().sum::<f32>() / ch as f32)
            .collect()
    }

    /// Feeds input and returns whatever output is ready.
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        self.input.extend_from_slice(samples);
        let ch = self.channels;
        let hop_in = HALF as f64 * self.speed;
        let mut out = Vec::new();
        loop {
            let base = self.pos.floor() as usize;
            if base + BLOCK + SEEK >= self.frames() {
                break;
            }
            // Best offset around `base` (coarse step, then exact around the winner).
            let start = if !self.started {
                self.started = true;
                base
            } else {
                let lo = base.saturating_sub(SEEK);
                let hi = base + SEEK;
                let target = &self.natural;
                let score = |s: usize, me: &Self| -> f32 {
                    let m = me.mono(s, HALF);
                    m.iter().zip(target).map(|(a, b)| a * b).sum()
                };
                let mut best = (base, f32::MIN);
                let mut s = lo;
                while s <= hi {
                    let v = score(s, self);
                    if v > best.1 {
                        best = (s, v);
                    }
                    s += 8;
                }
                let (c, _) = best;
                for s in c.saturating_sub(7).max(lo)..=(c + 7).min(hi) {
                    let v = score(s, self);
                    if v > best.1 {
                        best = (s, v);
                    }
                }
                best.0
            };
            // Overlap-add: first half with the previous tail goes out, second half waits.
            for i in 0..HALF {
                let w = self.window[i];
                for c in 0..ch {
                    out.push(self.tail[i * ch + c] + self.input[(start + i) * ch + c] * w);
                }
            }
            for i in 0..HALF {
                let w = self.window[HALF + i];
                for c in 0..ch {
                    self.tail[i * ch + c] = self.input[(start + HALF + i) * ch + c] * w;
                }
            }
            self.natural = self.mono(start + HALF, HALF);
            self.pos += hop_in;
            // Drop input nobody will look at again.
            let keep_from = (self.pos.floor() as usize).saturating_sub(SEEK);
            if keep_from > 4 * BLOCK {
                self.input.drain(..keep_from * ch);
                self.pos -= keep_from as f64;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_follows_speed_and_tone_stays() {
        let rate = 48_000.0;
        let input: Vec<f32> = (0..96_000)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate).sin() * 0.5)
            .collect();
        for speed in [0.5, 1.5, 2.0] {
            let mut s = Stretch::new(1, speed);
            let out = s.process(&input);
            let expected = input.len() as f64 / speed;
            assert!(
                (out.len() as f64 - expected).abs() < expected * 0.08,
                "speed {speed}: {} vs {expected}",
                out.len()
            );
            // Same pitch: count zero crossings per second in the middle.
            let mid = &out[out.len() / 4..out.len() * 3 / 4];
            let crossings = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f64;
            let hz = crossings / (mid.len() as f64 / f64::from(rate));
            assert!((hz - 440.0).abs() < 15.0, "speed {speed}: {hz} Hz");
        }
    }
}
