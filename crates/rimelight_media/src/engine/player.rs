//! FFmpeg (decoding) and cpal (sound) player: the same generation-per-seek threads and sound-device clock as the
//! public `Player`, plus a chosen audio track, subtitle tracks (text and ASS, decoded while the video is read) and
//! files with sound only. Frames are `crate::VideoFrame` (RGBA bytes).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ff;

use super::bitmap_sub::{self, SubPicture};
use super::files;
use super::stretch::Stretch;
use super::subs::{self, Cue};
use crate::{Pixels, Rgba, VideoFrame, new_pixels, pixel_bytes_mut};

type Input = ff::format::context::Input;
type Scaler = ff::software::scaling::Context;
type Resampler = ff::software::resampling::Context;

// Decoded frames waiting to be shown. Each is a full RGBA picture at the window's size (8 MB at 1080p, 33 MB at 4K),
// so this is the biggest part of the memory while a video plays. 5 covers the decoder's hiccups; the clock drops late
// frames anyway, so a longer queue only adds memory.
const MAX_QUEUED_FRAMES: usize = 5;
/// The sound output is retried at most this often after it was lost.
const OUTPUT_RETRY: Duration = Duration::from_secs(1);
const NO_DEVICE_RATE: u32 = 48_000;
/// `sub_stream` value for "subtitles from a file" (the cues are set from outside).
pub const SUB_EXTERNAL: i64 = -2;
pub const SUB_OFF: i64 = -1;

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn init() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        ff::init().map_err(|e| e.to_string())?;
        ff::util::log::set_level(ff::util::log::Level::Error);
        Ok(())
    })
    .clone()
}

pub fn open_input(path: &Path) -> Result<Input, String> {
    init()?;
    ff::format::input(&path).map_err(|e| format!("Could not open the file: {e}"))
}

fn to_av_ts(secs: f64) -> i64 {
    (secs * f64::from(ff::ffi::AV_TIME_BASE)) as i64
}

fn duration_ms(ictx: &Input) -> u64 {
    let d = ictx.duration();
    if d <= 0 {
        0
    } else {
        (d as f64 / f64::from(ff::ffi::AV_TIME_BASE) * 1000.0) as u64
    }
}

fn secs_to_ms(secs: f64) -> u64 {
    (secs.max(0.0) * 1000.0) as u64
}

fn pts_secs(ts: Option<i64>, tb: f64) -> f64 {
    ts.unwrap_or(0) as f64 * tb
}

fn micros(d: Duration) -> u64 {
    d.as_micros() as u64
}

fn pack_size(w: u32, h: u32) -> u64 {
    (u64::from(w) << 32) | u64::from(h)
}

fn unpack_size(v: u64) -> (u32, u32) {
    ((v >> 32) as u32, v as u32)
}

/// Fits (w, h) inside `max`, keeping the shape, never enlarging. Even sides, at least 2 px.
pub fn fit(w: u32, h: u32, max: (u32, u32)) -> (u32, u32) {
    let (w, h) = (f64::from(w.max(1)), f64::from(h.max(1)));
    let scale = (f64::from(max.0.max(2)) / w)
        .min(f64::from(max.1.max(2)) / h)
        .min(1.0);
    let even = |v: f64| ((v.round() as u32) & !1).max(2);
    (even(w * scale), even(h * scale))
}

/// Copies the scaled frame's rows into `bytes` (w x h RGBA, no padding between rows).
pub(super) fn copy_into(f: &ff::frame::Video, (w, h): (u32, u32), bytes: &mut [u8]) {
    let row = w as usize * 4;
    let stride = f.stride(0);
    let data = f.data(0);
    for y in 0..h as usize {
        bytes[y * row..(y + 1) * row].copy_from_slice(&data[y * stride..y * stride + row]);
    }
}

/// Copies the scaled frame into a new picture in the current pixel storage (one copy, straight into it).
fn copy_rgba(f: &ff::frame::Video, size: (u32, u32)) -> Pixels {
    let mut out = new_pixels(size.0, size.1);
    copy_into(f, size, pixel_bytes_mut(&mut out));
    out
}

fn scaler(dec: &ff::decoder::Video, (w, h): (u32, u32)) -> Result<Scaler, String> {
    Scaler::get(
        dec.format(),
        dec.width(),
        dec.height(),
        ff::format::Pixel::RGBA,
        w,
        h,
        ff::software::scaling::Flags::BILINEAR,
    )
    .map_err(|e| e.to_string())
}

/// An audio or subtitle track of a file.
#[derive(Debug, Clone)]
pub struct Track {
    pub index: usize,
    pub label: String,
    pub detail: String,
    /// Picture-based subtitles (DVD, Blu-ray): listed but not drawn.
    pub bitmap: bool,
    pub default: bool,
}

/// What a file holds.
#[derive(Debug, Clone, Default)]
pub struct MediaInfo {
    pub duration_ms: u64,
    pub video: Option<usize>,
    pub width: u32,
    pub height: u32,
    pub audio: Vec<Track>,
    pub subtitles: Vec<Track>,
    pub title: String,
    pub detail: String,
    /// Container bitrate in bits per second (0 if unknown).
    pub bitrate: u64,
    pub video_codec: String,
    pub fps: f64,
    pub audio_codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// The container's short name from FFmpeg (for example "mp3" or "matroska").
    pub container: String,
    /// The file's tags that are set: (label, value), for the Details panel.
    pub tags: Vec<(String, String)>,
}

fn language_name(code: &str) -> String {
    let name = match code {
        "eng" | "en" => "English",
        "por" | "pt" => "Portuguese",
        "spa" | "es" => "Spanish",
        "fre" | "fra" | "fr" => "French",
        "ger" | "deu" | "de" => "German",
        "ita" | "it" => "Italian",
        "jpn" | "ja" => "Japanese",
        "kor" | "ko" => "Korean",
        "chi" | "zho" | "zh" => "Chinese",
        "rus" | "ru" => "Russian",
        "dut" | "nld" | "nl" => "Dutch",
        "pol" | "pl" => "Polish",
        "swe" | "sv" => "Swedish",
        "ara" | "ar" => "Arabic",
        "hin" | "hi" => "Hindi",
        "tur" | "tr" => "Turkish",
        "und" | "" => "",
        other => other,
    };
    name.to_string()
}

pub fn probe(path: &Path) -> Result<MediaInfo, String> {
    let ictx = open_input(path)?;
    let mut info = MediaInfo {
        duration_ms: duration_ms(&ictx),
        ..MediaInfo::default()
    };
    let best_video = ictx
        .streams()
        .best(ff::media::Type::Video)
        .map(|s| s.index());
    let mut n_audio = 0;
    let mut n_subs = 0;
    for st in ictx.streams() {
        let params = st.parameters();
        let kind = params.medium();
        let disp = st.disposition();
        let meta = st.metadata();
        let lang = language_name(meta.get("language").unwrap_or_default());
        let title = meta.get("title").unwrap_or_default().to_string();
        let codec = params.id().name().to_string();
        let default = disp.contains(ff::format::stream::Disposition::DEFAULT);
        match kind {
            ff::media::Type::Video => {
                if Some(st.index()) == best_video
                    && !disp.contains(ff::format::stream::Disposition::ATTACHED_PIC)
                {
                    info.video = Some(st.index());
                    if let Ok(ctx) = ff::codec::context::Context::from_parameters(params.clone())
                        && let Ok(dec) = ctx.decoder().video()
                    {
                        info.width = dec.width();
                        info.height = dec.height();
                    }
                    info.video_codec = codec.to_uppercase();
                    let r = st.avg_frame_rate();
                    info.fps = if r.denominator() > 0 {
                        f64::from(r.numerator()) / f64::from(r.denominator())
                    } else {
                        0.0
                    };
                }
            }
            ff::media::Type::Audio => {
                n_audio += 1;
                let mut label = if !title.is_empty() {
                    title.clone()
                } else {
                    format!("Track {n_audio}")
                };
                if !lang.is_empty() && !label.contains(&lang) {
                    label = format!("{label} ({lang})");
                }
                let (channels, rate) = ff::codec::context::Context::from_parameters(params.clone())
                    .ok()
                    .and_then(|c| c.decoder().audio().ok())
                    .map(|d| (d.channels(), d.rate()))
                    .unwrap_or((0, 0));
                if info.audio.is_empty() {
                    info.audio_codec = codec.to_uppercase();
                    info.sample_rate = rate;
                    info.channels = channels;
                }
                let layout = match channels {
                    1 => "mono".to_string(),
                    2 => "stereo".to_string(),
                    6 => "5.1".to_string(),
                    8 => "7.1".to_string(),
                    0 => String::new(),
                    n => format!("{n} channels"),
                };
                info.audio.push(Track {
                    index: st.index(),
                    label,
                    detail: format!("{} {layout}", codec.to_uppercase())
                        .trim()
                        .to_string(),
                    bitmap: false,
                    default,
                });
            }
            ff::media::Type::Subtitle => {
                n_subs += 1;
                let bitmap = matches!(
                    codec.as_str(),
                    "hdmv_pgs_subtitle" | "dvd_subtitle" | "dvb_subtitle" | "xsub"
                );
                let mut label = if !title.is_empty() {
                    title.clone()
                } else if !lang.is_empty() {
                    lang.clone()
                } else {
                    format!("Track {n_subs}")
                };
                if !lang.is_empty() && !label.contains(&lang) {
                    label = format!("{label} ({lang})");
                }
                let detail = if bitmap {
                    format!(
                        "{} - picture subtitles, not shown yet",
                        codec.to_uppercase()
                    )
                } else {
                    codec.to_uppercase()
                };
                info.subtitles.push(Track {
                    index: st.index(),
                    label,
                    detail,
                    bitmap,
                    default,
                });
            }
            _ => {}
        }
    }
    info.bitrate = ictx.bit_rate().max(0) as u64;
    info.container = ictx
        .format()
        .name()
        .split(',')
        .next()
        .unwrap_or_default()
        .to_string();
    let meta = ictx.metadata();
    // The tags for the Details panel: only the ones the file sets.
    let year = meta.get("date").or_else(|| meta.get("year"));
    for (key, label) in [
        ("title", "Title"),
        ("artist", "Artist"),
        ("album", "Album"),
        ("track", "Track"),
        ("genre", "Genre"),
    ] {
        if let Some(v) = meta.get(key).map(str::trim).filter(|v| !v.is_empty()) {
            info.tags.push((label.to_string(), v.to_string()));
        }
    }
    if let Some(v) = year.map(str::trim).filter(|v| !v.is_empty()) {
        info.tags.push(("Year".to_string(), v.to_string()));
    }
    info.title = meta
        .get("title")
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| files::file_name(path));
    let artist = meta
        .get("artist")
        .or_else(|| meta.get("album_artist"))
        .unwrap_or_default();
    let album = meta.get("album").unwrap_or_default();
    info.detail = [artist, album]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("  ·  ");
    if info.video.is_none() && info.audio.is_empty() {
        return Err("This file has no video or sound that Rimelight Media can play.".into());
    }
    Ok(info)
}

/// Picture of the video at `ms` (full size). Seeks to the keyframe before, then decodes forward.
pub fn frame_at(path: &Path, ms: u64) -> Result<Rgba, String> {
    let mut ictx = open_input(path)?;
    let (idx, tb, ctx) = {
        let st = ictx
            .streams()
            .best(ff::media::Type::Video)
            .ok_or("no video")?;
        (
            st.index(),
            f64::from(st.time_base()),
            ff::codec::context::Context::from_parameters(st.parameters())
                .map_err(|e| e.to_string())?,
        )
    };
    let mut dec = ctx.decoder().video().map_err(|e| e.to_string())?;
    let target = ms as f64 / 1000.0;
    if target > 0.0 {
        ictx.seek(to_av_ts(target), ..).map_err(|e| e.to_string())?;
    }
    let size = (dec.width(), dec.height());
    let mut sc = scaler(&dec, size)?;
    let mut decoded = ff::frame::Video::empty();
    let mut last: Option<Rgba> = None;
    let grab = |f: &ff::frame::Video, sc: &mut Scaler| -> Result<Rgba, String> {
        let mut rgba = ff::frame::Video::empty();
        sc.run(f, &mut rgba).map_err(|e| e.to_string())?;
        Ok(Rgba {
            width: size.0,
            height: size.1,
            pixels: copy_rgba(&rgba, size),
        })
    };
    for (st, pkt) in ictx.packets() {
        if st.index() != idx {
            continue;
        }
        dec.send_packet(&pkt).map_err(|e| e.to_string())?;
        while dec.receive_frame(&mut decoded).is_ok() {
            let pic = grab(&decoded, &mut sc)?;
            if pts_secs(decoded.timestamp(), tb) + 0.001 >= target {
                return Ok(pic);
            }
            last = Some(pic);
        }
    }
    last.ok_or_else(|| "no frame there".into())
}

/// The loudest sample in each of `buckets` slices of one sound track, for the waveform (0..1). Runs on a worker thread.
/// The track is decoded at a low rate and first reduced to one peak per 10 ms, so the track's length need not be known
/// (the length in the file header can be missing or wrong). An empty result means no sound was decoded.
pub fn peaks(
    path: &Path,
    idx: usize,
    buckets: usize,
    cancel: &AtomicBool,
) -> Result<Vec<f32>, String> {
    const RATE: u32 = 8_000;
    const HOP: usize = 80; // samples per peak: 10 ms at RATE
    let buckets = buckets.max(1);
    let mut ictx = open_input(path)?;
    let ctx = {
        let st = ictx.stream(idx).ok_or("the sound track is gone")?;
        ff::codec::context::Context::from_parameters(st.parameters()).map_err(|e| e.to_string())?
    };
    let mut dec = ctx.decoder().audio().map_err(|e| e.to_string())?;
    // The peaks so far, the largest sample in the current 10 ms, and how many samples it holds.
    let mut acc: (Vec<f32>, f32, usize) = (Vec::new(), 0.0, 0);
    let mut rs: Option<Resampler> = None;
    let mut decoded = ff::frame::Audio::empty();

    let take = |frame: &ff::frame::Audio,
                rs: &mut Option<Resampler>,
                acc: &mut (Vec<f32>, f32, usize)|
     -> Result<(), String> {
        if rs.is_none() {
            let layout = if frame.channel_layout().channels() > 0 {
                frame.channel_layout()
            } else {
                ff::ChannelLayout::default(i32::from(frame.channels()))
            };
            *rs = Some(
                Resampler::get(
                    frame.format(),
                    layout,
                    frame.rate(),
                    ff::format::Sample::F32(ff::format::sample::Type::Packed),
                    ff::ChannelLayout::default(1),
                    RATE,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        let Some(r) = rs.as_mut() else { return Ok(()) };
        let mut res = ff::frame::Audio::empty();
        r.run(frame, &mut res).map_err(|e| e.to_string())?;
        let bytes = res.data(0).get(..res.samples() * 4).unwrap_or_default();
        for b in bytes.as_chunks::<4>().0 {
            let v = f32::from_le_bytes(*b).abs();
            acc.1 = acc.1.max(v);
            acc.2 += 1;
            if acc.2 == HOP {
                acc.0.push(acc.1);
                acc.1 = 0.0;
                acc.2 = 0;
            }
        }
        Ok(())
    };

    for (st, pkt) in ictx.packets() {
        if cancel.load(Relaxed) {
            return Ok(Vec::new());
        }
        if st.index() != idx || dec.send_packet(&pkt).is_err() {
            continue;
        }
        while dec.receive_frame(&mut decoded).is_ok() {
            take(&decoded, &mut rs, &mut acc)?;
        }
    }
    let _ = dec.send_eof();
    while dec.receive_frame(&mut decoded).is_ok() {
        take(&decoded, &mut rs, &mut acc)?;
    }
    if acc.2 > 0 {
        acc.0.push(acc.1);
    }
    let cols = acc.0;
    let n = cols.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    // Each bucket takes the loudest of its share of the peaks.
    let mut out: Vec<f32> = (0..buckets)
        .map(|i| {
            let a = i * n / buckets;
            let b = ((i + 1) * n / buckets).max(a + 1).min(n);
            cols[a..b].iter().copied().fold(0f32, f32::max)
        })
        .collect();
    let max = out.iter().copied().fold(0f32, f32::max);
    if max > 0.0 {
        for v in &mut out {
            *v /= max;
        }
    }
    Ok(out)
}

struct Shared {
    path: PathBuf,
    duration_ms: u64,
    video_idx: Option<usize>,
    audio_idx: Option<usize>,
    out_rate: u32,
    out_channels: u16,
    frames: Mutex<VecDeque<VideoFrame>>,
    audio: Mutex<VecDeque<f32>>,
    played: AtomicU64,
    clock_base_us: AtomicU64,
    wall: Mutex<Option<Instant>>,
    playing: AtomicBool,
    prerolled: AtomicBool,
    /// Set once the first picture or sound is ready. Seeks clear `prerolled` again, but only the first open shows
    /// "Opening...".
    opened: AtomicBool,
    video_done: AtomicBool,
    audio_done: AtomicBool,
    audio_on: AtomicBool,
    /// Set by the output stream's error callback (device removed or changed); `Player::check_output` rebuilds it.
    stream_lost: AtomicBool,
    stop: AtomicBool,
    ended: AtomicBool,
    looping: AtomicBool,
    muted: AtomicBool,
    volume: AtomicU32,
    generation: AtomicU64,
    output_size: AtomicU64,
    sub_stream: AtomicI64,
    cues: Mutex<Vec<Cue>>,
    /// Playback speed (f32 bits); the sound is time-stretched, the clock runs this much faster.
    speed: AtomicU32,
    /// Subtitle delay in ms (positive shows them later).
    sub_delay_ms: AtomicI64,
    error: Mutex<Option<String>>,
    /// The decoding threads of the current and earlier spots (a seek starts new ones; old ones stop by themselves).
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

/// Waits up to `limit` for the threads to finish, joining the ones that did. Threads still busy after that (a read
/// stuck in the OS) are left to end on their own; they hold no UI state.
pub fn join_within(handles: Vec<std::thread::JoinHandle<()>>, limit: Duration) {
    let until = Instant::now() + limit;
    let mut left = handles;
    loop {
        let (done, busy): (Vec<_>, Vec<_>) = left.into_iter().partition(|h| h.is_finished());
        for h in done {
            let _ = h.join();
        }
        left = busy;
        if left.is_empty() || Instant::now() >= until {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

impl Shared {
    fn is_stale(&self, epoch: u64) -> bool {
        self.stop.load(Relaxed) || self.generation.load(Relaxed) != epoch
    }

    fn audio_limit(&self) -> usize {
        self.out_rate as usize * usize::from(self.out_channels) / 2
    }

    fn set_error(&self, message: String) {
        *lock(&self.error) = Some(message);
    }

    /// Clears a sound output error (only that kind: decoding errors stay until the next file).
    fn clear_output_error(&self) {
        let mut e = lock(&self.error);
        if e.as_deref().is_some_and(|m| m.starts_with("Sound output")) {
            *e = None;
        }
    }

    fn finish_video(&self, epoch: u64) {
        let _q = lock(&self.frames);
        if self.generation.load(Relaxed) == epoch {
            self.video_done.store(true, Relaxed);
        }
    }

    fn finish_audio(&self, epoch: u64) {
        let _q = lock(&self.audio);
        if self.generation.load(Relaxed) == epoch {
            self.audio_done.store(true, Relaxed);
        }
    }

    fn speed(&self) -> f64 {
        f64::from(f32::from_bits(self.speed.load(Relaxed)))
    }

    fn clock_us(&self) -> u64 {
        let base = self.clock_base_us.load(Relaxed);
        let speed = self.speed();
        if self.audio_on.load(Relaxed) {
            base + (self.played.load(Relaxed) as f64 * 1_000_000.0 / f64::from(self.out_rate)
                * speed) as u64
        } else {
            base + lock(&self.wall).map_or(0, |t| (micros(t.elapsed()) as f64 * speed) as u64)
        }
    }

    fn start_wall(&self) {
        if self.audio_on.load(Relaxed)
            || !self.playing.load(Relaxed)
            || !self.prerolled.load(Relaxed)
        {
            return;
        }
        let mut wall = lock(&self.wall);
        if wall.is_none() {
            *wall = Some(Instant::now());
        }
    }

    fn pause(&self) {
        self.playing.store(false, Relaxed);
        if !self.audio_on.load(Relaxed)
            && let Some(t) = lock(&self.wall).take()
        {
            self.clock_base_us
                .fetch_add((micros(t.elapsed()) as f64 * self.speed()) as u64, Relaxed);
        }
    }

    fn update_preroll(&self) {
        if self.prerolled.load(Relaxed) {
            return;
        }
        if self.video_idx.is_some()
            && lock(&self.frames).is_empty()
            && !self.video_done.load(Relaxed)
        {
            return;
        }
        let need = self.out_rate as usize * usize::from(self.out_channels) / 10;
        if self.audio_on.load(Relaxed)
            && !self.audio_done.load(Relaxed)
            && lock(&self.audio).len() < need
        {
            return;
        }
        self.prerolled.store(true, Relaxed);
        self.opened.store(true, Relaxed);
        self.start_wall();
    }

    fn add_cue(&self, cue: Cue) {
        let mut cues = lock(&self.cues);
        if !cues.iter().any(|c| {
            c.start_ms == cue.start_ms
                && c.text == cue.text
                && c.picture.is_some() == cue.picture.is_some()
        }) {
            cues.push(cue);
        }
    }

    /// A picture subtitle event: ends the picture still showing, then adds the new one (if it has a picture).
    fn picture_event(&self, start_ms: u64, end_ms: Option<u64>, picture: Option<SubPicture>) {
        let mut cues = lock(&self.cues);
        for c in cues
            .iter_mut()
            .filter(|c| c.open && c.start_ms < start_ms && c.end_ms > start_ms)
        {
            c.end_ms = start_ms;
            c.open = false;
        }
        if let Some(p) = picture
            && !cues
                .iter()
                .any(|c| c.picture.is_some() && c.start_ms == start_ms)
        {
            let open = end_ms.is_none();
            cues.push(Cue {
                start_ms,
                end_ms: end_ms.unwrap_or(start_ms + 10_000),
                text: String::new(),
                picture: Some(Arc::new(p)),
                open,
            });
        }
    }
}

fn push_frame(sh: &Shared, epoch: u64, frame: VideoFrame) -> bool {
    loop {
        {
            let mut q = lock(&sh.frames);
            if sh.is_stale(epoch) {
                return false;
            }
            if q.len() < MAX_QUEUED_FRAMES {
                q.push_back(frame);
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn push_samples(sh: &Shared, epoch: u64, samples: Vec<f32>) -> bool {
    let limit = sh.audio_limit();
    loop {
        {
            let mut q = lock(&sh.audio);
            if sh.is_stale(epoch) {
                return false;
            }
            if q.len() < limit {
                q.extend(samples);
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Subtitle decoding inside the video thread.
struct SubPipe {
    stream: i64,
    dec: Option<ff::decoder::Subtitle>,
    tb: f64,
    bitmap: bool,
}

/// Several picture rectangles of one event (top and bottom lines) as one picture.
fn merge_pictures(parts: Vec<SubPicture>) -> Option<SubPicture> {
    if parts.len() <= 1 {
        return parts.into_iter().next();
    }
    let x0 = parts.iter().map(|p| p.x).min()?;
    let y0 = parts.iter().map(|p| p.y).min()?;
    let x1 = parts.iter().map(|p| p.x + p.w as i32).max()?;
    let y1 = parts.iter().map(|p| p.y + p.h as i32).max()?;
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for p in &parts {
        for row in 0..p.h as usize {
            let src = &p.rgba[row * p.w as usize * 4..(row + 1) * p.w as usize * 4];
            let dy = (p.y - y0) as usize + row;
            let dx = (p.x - x0) as usize;
            let at = (dy * w as usize + dx) * 4;
            rgba[at..at + src.len()].copy_from_slice(src);
        }
    }
    Some(SubPicture {
        x: x0,
        y: y0,
        w,
        h,
        rgba,
    })
}

impl SubPipe {
    fn handle(&mut self, ictx_stream: &ff::format::stream::Stream, pkt: &ff::Packet, sh: &Shared) {
        let want = sh.sub_stream.load(Relaxed);
        if want < 0 || ictx_stream.index() as i64 != want {
            return;
        }
        if self.stream != want || self.dec.is_none() {
            self.stream = want;
            self.tb = f64::from(ictx_stream.time_base());
            let codec = ictx_stream.parameters().id().name().to_string();
            self.bitmap = matches!(
                codec.as_str(),
                "hdmv_pgs_subtitle" | "dvd_subtitle" | "dvb_subtitle" | "xsub"
            );
            self.dec = ff::codec::context::Context::from_parameters(ictx_stream.parameters())
                .ok()
                .and_then(|c| c.decoder().subtitle().ok());
        }
        let Some(dec) = self.dec.as_mut() else { return };
        let mut sub = ff::Subtitle::new();
        if !matches!(dec.decode(pkt, &mut sub), Ok(true)) {
            return;
        }
        let base = pts_secs(pkt.pts().or(pkt.dts()), self.tb) * 1000.0;
        let start = base + f64::from(sub.start());
        let end = if sub.end() > sub.start() {
            base + f64::from(sub.end())
        } else if pkt.duration() > 0 {
            base + pkt.duration() as f64 * self.tb * 1000.0
        } else {
            start + 4000.0
        };
        let mut text = Vec::new();
        let mut pictures = Vec::new();
        for rect in sub.rects() {
            match rect {
                ff::subtitle::Rect::Text(t) => text.push(subs::strip_tags(t.get())),
                ff::subtitle::Rect::Ass(a) => text.push(subs::ass_event_text(a.get())),
                ff::subtitle::Rect::Bitmap(b) => pictures.extend(bitmap_sub::read(&b)),
                _ => {}
            }
        }
        let start_ms = start.max(0.0) as u64;
        if self.bitmap {
            // Picture subtitles: an event without a picture clears the screen; an unknown end lasts until the next.
            let known_end =
                (sub.end() > sub.start() && sub.end() != u32::MAX).then(|| end.max(start) as u64);
            sh.picture_event(start_ms, known_end, merge_pictures(pictures));
            return;
        }
        let text = text.join("\n");
        if !text.trim().is_empty() {
            sh.add_cue(Cue {
                start_ms,
                end_ms: end.max(start) as u64,
                text,
                picture: None,
                open: false,
            });
        }
    }
}

fn video_run(sh: &Shared, start_ms: u64, epoch: u64, idx: usize) -> Result<(), String> {
    let mut ictx = open_input(&sh.path)?;
    let (tb, ctx) = {
        let st = ictx.stream(idx).ok_or("the video stream is gone")?;
        (
            f64::from(st.time_base()),
            ff::codec::context::Context::from_parameters(st.parameters())
                .map_err(|e| e.to_string())?,
        )
    };
    let mut dec = ctx.decoder().video().map_err(|e| e.to_string())?;
    let start = start_ms as f64 / 1000.0;
    if start > 0.0 {
        ictx.seek(to_av_ts(start), ..).map_err(|e| e.to_string())?;
    }
    let mut size = fit(
        dec.width(),
        dec.height(),
        unpack_size(sh.output_size.load(Relaxed)),
    );
    let mut sc = scaler(&dec, size)?;
    let mut decoded = ff::frame::Video::empty();
    let mut subs = SubPipe {
        stream: SUB_OFF,
        dec: None,
        tb: 0.0,
        bitmap: false,
    };
    // The scaler's output frame is kept from one frame to the next (not a new frame object per picture).
    let mut scaled = ff::frame::Video::empty();

    let mut drain = |dec: &mut ff::decoder::Video,
                     sc: &mut Scaler,
                     size: &mut (u32, u32)|
     -> Result<bool, String> {
        while dec.receive_frame(&mut decoded).is_ok() {
            if sh.is_stale(epoch) {
                return Ok(false);
            }
            let pts = pts_secs(decoded.timestamp(), tb);
            if pts + 0.001 < start {
                continue;
            }
            let want = fit(
                dec.width(),
                dec.height(),
                unpack_size(sh.output_size.load(Relaxed)),
            );
            if want != *size {
                *sc = scaler(dec, want)?;
                *size = want;
            }
            sc.run(&decoded, &mut scaled).map_err(|e| e.to_string())?;
            let frame = VideoFrame {
                picture: Rgba {
                    width: size.0,
                    height: size.1,
                    pixels: copy_rgba(&scaled, *size),
                },
                pts_ms: secs_to_ms(pts),
            };
            if !push_frame(sh, epoch, frame) {
                return Ok(false);
            }
        }
        Ok(true)
    };

    for (st, pkt) in ictx.packets() {
        if sh.is_stale(epoch) {
            return Ok(());
        }
        if st.index() != idx {
            if st.parameters().medium() == ff::media::Type::Subtitle {
                subs.handle(&st, &pkt, sh);
            }
            continue;
        }
        // A broken packet is skipped, as players do.
        if dec.send_packet(&pkt).is_err() {
            continue;
        }
        if !drain(&mut dec, &mut sc, &mut size)? {
            return Ok(());
        }
    }
    let _ = dec.send_eof();
    drain(&mut dec, &mut sc, &mut size)?;
    Ok(())
}

fn audio_run(sh: &Shared, start_ms: u64, epoch: u64, idx: usize) -> Result<(), String> {
    let mut ictx = open_input(&sh.path)?;
    let (tb, ctx) = {
        let st = ictx.stream(idx).ok_or("the sound track is gone")?;
        (
            f64::from(st.time_base()),
            ff::codec::context::Context::from_parameters(st.parameters())
                .map_err(|e| e.to_string())?,
        )
    };
    let mut dec = ctx.decoder().audio().map_err(|e| e.to_string())?;
    let start = start_ms as f64 / 1000.0;
    if start > 0.0 {
        ictx.seek(to_av_ts(start), ..).map_err(|e| e.to_string())?;
    }
    let mut rs: Option<Resampler> = None;
    let mut decoded = ff::frame::Audio::empty();
    let channels = usize::from(sh.out_channels);
    let speed = sh.speed();
    let mut stretch = ((speed - 1.0).abs() > 0.001).then(|| Stretch::new(channels, speed));

    let mut drain = |dec: &mut ff::decoder::Audio,
                     rs: &mut Option<Resampler>,
                     stretch: &mut Option<Stretch>|
     -> Result<bool, String> {
        while dec.receive_frame(&mut decoded).is_ok() {
            if sh.is_stale(epoch) {
                return Ok(false);
            }
            let pts = pts_secs(decoded.timestamp(), tb);
            if decoded.timestamp().is_some() && pts + 0.05 < start {
                continue;
            }
            if rs.is_none() {
                let layout = if decoded.channel_layout().channels() > 0 {
                    decoded.channel_layout()
                } else {
                    ff::ChannelLayout::default(i32::from(decoded.channels()))
                };
                *rs = Some(
                    Resampler::get(
                        decoded.format(),
                        layout,
                        decoded.rate(),
                        ff::format::Sample::F32(ff::format::sample::Type::Packed),
                        ff::ChannelLayout::default(channels as i32),
                        sh.out_rate,
                    )
                    .map_err(|e| e.to_string())?,
                );
            }
            let Some(r) = rs.as_mut() else { continue };
            let mut out = ff::frame::Audio::empty();
            r.run(&decoded, &mut out).map_err(|e| e.to_string())?;
            let count = out.samples() * channels;
            let bytes = out.data(0).get(..count * 4).unwrap_or_default();
            let mut samples: Vec<f32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            if let Some(st) = stretch.as_mut() {
                samples = st.process(&samples);
            }
            if !push_samples(sh, epoch, samples) {
                return Ok(false);
            }
        }
        Ok(true)
    };

    for (st, pkt) in ictx.packets() {
        if sh.is_stale(epoch) {
            return Ok(());
        }
        if st.index() != idx {
            continue;
        }
        if dec.send_packet(&pkt).is_err() {
            continue;
        }
        if !drain(&mut dec, &mut rs, &mut stretch)? {
            return Ok(());
        }
    }
    let _ = dec.send_eof();
    drain(&mut dec, &mut rs, &mut stretch)?;
    Ok(())
}

fn spawn_workers(sh: &Arc<Shared>, start_ms: u64, epoch: u64) {
    let mut handles = Vec::new();
    if let Some(idx) = sh.video_idx {
        let v = Arc::clone(sh);
        handles.push(std::thread::spawn(move || {
            if let Err(e) = video_run(&v, start_ms, epoch, idx)
                && !v.is_stale(epoch)
            {
                v.set_error(e);
            }
            v.finish_video(epoch);
        }));
    } else {
        sh.video_done.store(true, Relaxed);
    }
    if let (Some(idx), true) = (sh.audio_idx, sh.audio_on.load(Relaxed)) {
        let a = Arc::clone(sh);
        handles.push(std::thread::spawn(move || {
            if let Err(e) = audio_run(&a, start_ms, epoch, idx)
                && !a.is_stale(epoch)
            {
                a.set_error(e);
            }
            a.finish_audio(epoch);
        }));
    } else {
        sh.audio_done.store(true, Relaxed);
    }
    // Earlier threads that have ended are forgotten; the rest stay listed so release() can wait for them.
    let mut workers = lock(&sh.workers);
    workers.retain(|h| !h.is_finished());
    workers.extend(handles);
}

fn fill_audio(sh: &Shared, data: &mut [f32]) {
    if !sh.playing.load(Relaxed) || !sh.prerolled.load(Relaxed) {
        data.fill(0.0);
        return;
    }
    let gain = if sh.muted.load(Relaxed) {
        0.0
    } else {
        f32::from_bits(sh.volume.load(Relaxed)).powi(2)
    };
    let channels = usize::from(sh.out_channels).max(1);
    let mut q = lock(&sh.audio);
    let have = q.len().min(data.len());
    for (i, s) in data.iter_mut().enumerate() {
        *s = if i < have {
            q.pop_front().unwrap_or(0.0) * gain
        } else {
            0.0
        };
    }
    drop(q);
    sh.played.fetch_add((have / channels) as u64, Relaxed);
}

fn open_device() -> Option<(cpal::Device, cpal::SupportedStreamConfig)> {
    let device = cpal::default_host().default_output_device()?;
    let config = device.default_output_config().ok()?;
    (config.sample_format() == cpal::SampleFormat::F32).then_some((device, config))
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    sh: &Arc<Shared>,
) -> Option<cpal::Stream> {
    let cb = Arc::clone(sh);
    let err = Arc::clone(sh);
    let stream = device
        .build_output_stream(
            config.config(),
            move |data: &mut [f32], _| fill_audio(&cb, data),
            move |e| {
                err.stream_lost.store(true, Relaxed);
                err.set_error(format!("Sound output: {e}"));
            },
            None,
        )
        .ok()?;
    stream.play().ok()?;
    Some(stream)
}

pub struct Player {
    shared: Arc<Shared>,
    stream: Mutex<Option<cpal::Stream>>,
    /// When the next rebuild may be tried (see `check_output`).
    retry_at: Mutex<Option<Instant>>,
}

impl Player {
    /// Opens a file and starts decoding at `start_ms`, paused. `audio` picks the sound track (stream index).
    pub fn open(
        path: &Path,
        info: &MediaInfo,
        audio: Option<usize>,
        start_ms: u64,
        max_size: (u32, u32),
    ) -> Result<Player, String> {
        let audio_idx = audio.or_else(|| {
            info.audio
                .iter()
                .find(|t| t.default)
                .or(info.audio.first())
                .map(|t| t.index)
        });
        let device = if audio_idx.is_some() {
            open_device()
        } else {
            None
        };
        let (out_rate, out_channels) = device.as_ref().map_or((NO_DEVICE_RATE, 2), |(_, c)| {
            (c.sample_rate(), c.channels())
        });
        let shared = Arc::new(Shared {
            path: path.to_path_buf(),
            duration_ms: info.duration_ms,
            video_idx: info.video,
            audio_idx,
            out_rate,
            out_channels,
            frames: Mutex::new(VecDeque::new()),
            audio: Mutex::new(VecDeque::new()),
            played: AtomicU64::new(0),
            clock_base_us: AtomicU64::new(start_ms * 1000),
            wall: Mutex::new(None),
            playing: AtomicBool::new(false),
            prerolled: AtomicBool::new(false),
            opened: AtomicBool::new(false),
            video_done: AtomicBool::new(false),
            audio_done: AtomicBool::new(false),
            audio_on: AtomicBool::new(false),
            stream_lost: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            looping: AtomicBool::new(false),
            muted: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
            generation: AtomicU64::new(1),
            output_size: AtomicU64::new(pack_size(max_size.0, max_size.1)),
            sub_stream: AtomicI64::new(SUB_OFF),
            cues: Mutex::new(Vec::new()),
            speed: AtomicU32::new(1.0f32.to_bits()),
            sub_delay_ms: AtomicI64::new(0),
            error: Mutex::new(None),
            workers: Mutex::new(Vec::new()),
        });
        let stream = device.and_then(|(d, c)| build_stream(&d, &c, &shared));
        shared.audio_on.store(stream.is_some(), Relaxed);
        spawn_workers(&shared, start_ms, 1);
        Ok(Player {
            shared,
            stream: Mutex::new(stream),
            retry_at: Mutex::new(None),
        })
    }

    /// Rebuilds the sound output on the default device after it was lost (the device changed or the stream was
    /// invalidated). Called from the UI tick; tries at most once a second. A rebuilt stream clears the error. If the
    /// new device runs at another rate or channel count, the rebuild is skipped and the error stays.
    pub fn check_output(&self) {
        let sh = &self.shared;
        if !sh.stream_lost.load(Relaxed) {
            return;
        }
        {
            let mut next = lock(&self.retry_at);
            if next.is_some_and(|t| Instant::now() < t) {
                return;
            }
            *next = Some(Instant::now() + OUTPUT_RETRY);
        }
        sh.stream_lost.store(false, Relaxed);
        let rebuilt = open_device()
            .filter(|(_, c)| c.sample_rate() == sh.out_rate && c.channels() == sh.out_channels)
            .and_then(|(d, c)| build_stream(&d, &c, sh));
        match rebuilt {
            Some(stream) => {
                // Replacing the stream drops the invalid one.
                *lock(&self.stream) = Some(stream);
                *lock(&self.retry_at) = None;
                sh.clear_output_error();
            }
            None => sh.stream_lost.store(true, Relaxed),
        }
    }

    /// Stops every decoding thread and waits up to 500 ms for them to close their files (so the file can be renamed or
    /// deleted). Safe to call more than once.
    pub fn release(&self) {
        self.shared.stop.store(true, Relaxed);
        self.shared.playing.store(false, Relaxed);
        join_within(
            std::mem::take(&mut *lock(&self.shared.workers)),
            Duration::from_millis(500),
        );
    }

    pub fn play(&self) {
        if self.shared.ended.load(Relaxed) {
            self.seek(0);
        }
        self.shared.playing.store(true, Relaxed);
        self.shared.start_wall();
    }

    pub fn pause(&self) {
        self.shared.pause();
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Relaxed)
    }

    /// True when the file has sound and it is played through a device.
    pub fn has_audio(&self) -> bool {
        self.shared.audio_on.load(Relaxed)
    }

    pub fn seek(&self, ms: u64) {
        let sh = &self.shared;
        let ms = ms.min(sh.duration_ms.max(1));
        let epoch = {
            let mut frames = lock(&sh.frames);
            let mut audio = lock(&sh.audio);
            frames.clear();
            audio.clear();
            sh.video_done.store(false, Relaxed);
            sh.audio_done.store(false, Relaxed);
            sh.clock_base_us.store(ms * 1000, Relaxed);
            sh.played.store(0, Relaxed);
            sh.prerolled.store(false, Relaxed);
            sh.ended.store(false, Relaxed);
            sh.generation.fetch_add(1, Relaxed) + 1
        };
        *lock(&sh.wall) = None;
        spawn_workers(sh, ms, epoch);
    }

    pub fn set_volume(&self, v: f32) {
        self.shared
            .volume
            .store(v.clamp(0.0, 1.0).to_bits(), Relaxed);
    }

    pub fn set_muted(&self, m: bool) {
        self.shared.muted.store(m, Relaxed);
    }

    pub fn set_looping(&self, l: bool) {
        self.shared.looping.store(l, Relaxed);
    }

    pub fn set_output_size(&self, w: u32, h: u32) {
        self.shared.output_size.store(pack_size(w, h), Relaxed);
    }

    /// Picks an embedded subtitle stream (or SUB_OFF / SUB_EXTERNAL with `cues`). Re-reads from the current spot.
    pub fn set_subtitles(&self, stream: i64, cues: Vec<Cue>) {
        *lock(&self.shared.cues) = cues;
        self.shared.sub_stream.store(stream, Relaxed);
        if stream >= 0 {
            self.seek(self.position_ms());
        }
    }

    /// The subtitle text and picture showing now (the delay applied).
    pub fn subtitle_now(&self) -> (String, Option<Arc<SubPicture>>) {
        if self.shared.sub_stream.load(Relaxed) == SUB_OFF {
            return (String::new(), None);
        }
        let at = (self.position_ms() as i64 - self.shared.sub_delay_ms.load(Relaxed)).max(0) as u64;
        let cues = lock(&self.shared.cues);
        (subs::text_at(&cues, at), subs::picture_at(&cues, at))
    }

    pub fn set_sub_delay(&self, ms: i64) {
        self.shared.sub_delay_ms.store(ms, Relaxed);
    }

    /// Playback speed (0.25 to 4). The sound keeps its pitch. Takes effect from the current spot.
    pub fn set_speed(&self, speed: f32) {
        let speed = speed.clamp(0.25, 4.0);
        if (f32::from_bits(self.shared.speed.load(Relaxed)) - speed).abs() < 0.001 {
            return;
        }
        let pos = self.position_ms();
        self.shared.speed.store(speed.to_bits(), Relaxed);
        self.seek(pos);
    }

    pub fn position_ms(&self) -> u64 {
        (self.shared.clock_us() / 1000).min(self.shared.duration_ms.max(1))
    }

    pub fn duration_ms(&self) -> u64 {
        self.shared.duration_ms
    }

    pub fn ended(&self) -> bool {
        self.shared.ended.load(Relaxed)
    }

    /// True only while the file is first being opened. A seek does not count (it shows the old picture until the new one).
    pub fn is_buffering(&self) -> bool {
        !self.shared.prerolled.load(Relaxed) && !self.shared.opened.load(Relaxed)
    }

    /// The newest frame that is due (older ones are dropped). Also ends or loops playback when everything ran out.
    pub fn take_frame(&self) -> Option<VideoFrame> {
        let sh = &self.shared;
        sh.update_preroll();
        let clock = sh.clock_us();
        let picked = {
            let mut q = lock(&sh.frames);
            let mut pick = None;
            // While paused before the first frame is shown, show the first one at once.
            while q.front().is_some_and(|f| {
                f.pts_ms.saturating_mul(1000) <= clock
                    || (pick.is_none()
                        && !sh.playing.load(Relaxed)
                        && f.pts_ms.saturating_mul(1000) <= clock + 60_000)
            }) {
                pick = q.pop_front();
                if !sh.playing.load(Relaxed) {
                    break;
                }
            }
            pick
        };
        if picked.is_none() {
            self.check_end();
        }
        picked
    }

    fn check_end(&self) {
        let sh = &self.shared;
        if !sh.playing.load(Relaxed) || !sh.video_done.load(Relaxed) || !lock(&sh.frames).is_empty()
        {
            return;
        }
        if sh.audio_on.load(Relaxed) && !(sh.audio_done.load(Relaxed) && lock(&sh.audio).is_empty())
        {
            return;
        }
        if sh.video_idx.is_none()
            && !sh.audio_on.load(Relaxed)
            && self.position_ms() < sh.duration_ms
        {
            return;
        }
        if sh.looping.load(Relaxed) {
            self.seek(0);
        } else {
            self.pause();
            sh.ended.store(true, Relaxed);
        }
    }

    pub fn error(&self) -> Option<String> {
        lock(&self.shared.error).clone()
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.release();
    }
}
