# rimelight_sound

Short tones and a metronome for the Rimelight apps (the web's metronome, fasting stage cues and stretch-routine tones).

## API

| Item | What it does |
|---|---|
| `Tone { hz, ms, volume }` | A sine tone. `volume` is clamped to 0 to 1. A 5 ms attack and a 10 ms release stop clicks. |
| `tone_samples(tone, rate)` | The mono samples at a sample rate (pure, no device). |
| `play(tone)` | Plays one tone on the default output and returns at once. |
| `CLICK`, `ACCENT_CLICK` | The metronome's clicks (800 Hz and 1200 Hz). |
| `Metronome::start(bpm, accent_every)` | Clicks on a background thread. Beat 0 is accented when `accent_every > 0`. Stop it with `stop()` or by dropping it. |
| `beat_period(bpm)`, `beat_start(bpm, beat)`, `is_accent(beat, every)` | The schedule. Beat times come from the index in whole nanoseconds, so there is no drift. |

The tempo is clamped to `BPM_RANGE` (20 to 300 BPM).

## Targets

- **Windows, macOS, Linux** (`cpal` 0.15): `play` opens the default output device for each tone, on a thread of its
  own, and releases it when the tone has ended. Sample formats `f32`, `i16` and `u16` are supported.
- **Android and wasm32**: `play` does nothing (no backend yet). The schedule and sample math still work and are tested.

## Limits

- One device open per click. That is fine at metronome tempos. A sample-accurate score needs one long-lived stream,
  which is a later change.
- No Android backend (it would go through AAudio or Oboe).
- Tests never open a device. Run them with the other crate tests.
