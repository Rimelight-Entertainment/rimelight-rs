//! Poster frames for the public `poster` call: one frame near a fraction of the video, scaled to fit a square box.
//! Kept from rimelight-space's backend (its keyframe seek and search limit); the player does not use it.

use std::path::Path;

use ffmpeg_next as ff;

use super::player::{copy_into, fit, open_input};
use crate::{MediaError, Rgba, new_pixels, pixel_bytes_mut};

type Scaler = ff::software::scaling::Context;

/// How far past the wanted time a poster may search for a frame.
const POSTER_SEARCH_SECS: f64 = 3.0;

fn to_av_ts(secs: f64) -> i64 {
    (secs * f64::from(ff::ffi::AV_TIME_BASE)) as i64
}

fn duration_secs(ictx: &ff::format::context::Input) -> f64 {
    let d = ictx.duration();
    if d <= 0 {
        0.0
    } else {
        d as f64 / f64::from(ff::ffi::AV_TIME_BASE)
    }
}

fn pts_secs(ts: Option<i64>, time_base: f64) -> f64 {
    ts.unwrap_or(0) as f64 * time_base
}

fn decode_err(e: ff::Error) -> MediaError {
    MediaError::Decode(e.to_string())
}

/// One frame at `fraction` (0..1) of the video, scaled so its longer side is at most `max_side` (never enlarged).
/// Seeks to the keyframe before that time, then decodes forward to the first frame at or after it. Gives up when
/// packets run more than 3 s past the wanted time.
pub fn poster(path: &Path, fraction: f64, max_side: u32) -> Result<Rgba, MediaError> {
    let mut ictx = open_input(path).map_err(MediaError::Open)?;
    let (idx, tb, ctx) = {
        let st = ictx
            .streams()
            .best(ff::media::Type::Video)
            .ok_or(MediaError::NoVideo)?;
        let ctx =
            ff::codec::context::Context::from_parameters(st.parameters()).map_err(decode_err)?;
        (st.index(), f64::from(st.time_base()), ctx)
    };
    let mut dec = ctx.decoder().video().map_err(decode_err)?;
    let target = duration_secs(&ictx) * fraction.clamp(0.0, 1.0);
    if target > 0.0 {
        ictx.seek(to_av_ts(target), ..).map_err(decode_err)?;
    }
    let size = fit(dec.width(), dec.height(), (max_side, max_side));
    let mut sc: Scaler = Scaler::get(
        dec.format(),
        dec.width(),
        dec.height(),
        ff::format::Pixel::RGBA,
        size.0,
        size.1,
        ff::software::scaling::Flags::BILINEAR,
    )
    .map_err(decode_err)?;
    let mut decoded = ff::frame::Video::empty();
    let mut take = |frame: &ff::frame::Video| -> Result<Option<Rgba>, MediaError> {
        if pts_secs(frame.timestamp(), tb) < target {
            return Ok(None);
        }
        let mut rgba = ff::frame::Video::empty();
        sc.run(frame, &mut rgba).map_err(decode_err)?;
        let mut pixels = new_pixels(size.0, size.1);
        copy_into(&rgba, size, pixel_bytes_mut(&mut pixels));
        Ok(Some(Rgba {
            width: size.0,
            height: size.1,
            pixels,
        }))
    };
    for (st, pkt) in ictx.packets() {
        if st.index() != idx {
            continue;
        }
        if pkt
            .pts()
            .is_some_and(|p| pts_secs(Some(p), tb) > target + POSTER_SEARCH_SECS)
        {
            break;
        }
        dec.send_packet(&pkt).map_err(decode_err)?;
        while dec.receive_frame(&mut decoded).is_ok() {
            if let Some(found) = take(&decoded)? {
                return Ok(found);
            }
        }
    }
    dec.send_eof().map_err(decode_err)?;
    while dec.receive_frame(&mut decoded).is_ok() {
        if let Some(found) = take(&decoded)? {
            return Ok(found);
        }
    }
    Err(MediaError::Decode("no frame near that time".into()))
}
