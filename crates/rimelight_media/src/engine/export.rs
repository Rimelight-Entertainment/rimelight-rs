//! Trimming without re-encoding: copies the packets between two times into a new file of the same kind. Video cuts
//! start at the keyframe at or before the chosen start (like the Photos app's fast trim).

use std::path::Path;

use ffmpeg_next as ff;

use super::player::open_input;

pub fn trim(input: &Path, output: &Path, start_ms: u64, end_ms: u64) -> Result<(), String> {
    let mut ictx = open_input(input)?;
    let mut octx =
        ff::format::output(&output).map_err(|e| format!("Could not create the file: {e}"))?;

    // Map input streams to output streams (video, audio and subtitles; data streams and attachments are left out).
    let mut map: Vec<Option<usize>> = vec![None; ictx.nb_streams() as usize];
    let mut next = 0;
    for ist in ictx.streams() {
        let medium = ist.parameters().medium();
        if !matches!(
            medium,
            ff::media::Type::Video | ff::media::Type::Audio | ff::media::Type::Subtitle
        ) {
            continue;
        }
        if ist
            .disposition()
            .contains(ff::format::stream::Disposition::ATTACHED_PIC)
        {
            continue;
        }
        let mut ost = octx
            .add_stream(ff::encoder::find(ff::codec::Id::None))
            .map_err(|e| e.to_string())?;
        ost.set_parameters(ist.parameters());
        ost.set_time_base(ist.time_base());
        map[ist.index()] = Some(next);
        next += 1;
    }
    if next == 0 {
        return Err("nothing to copy".into());
    }
    octx.set_metadata(ictx.metadata().to_owned());
    octx.write_header()
        .map_err(|e| format!("This kind of file can't be trimmed here: {e}"))?;

    let start = start_ms as f64 / 1000.0;
    let end = end_ms as f64 / 1000.0;
    if start > 0.0 {
        ictx.seek((start * f64::from(ff::ffi::AV_TIME_BASE)) as i64, ..)
            .map_err(|e| e.to_string())?;
    }
    // The first copied packet's time becomes 0 for every stream, so they stay in sync.
    let mut offset: Option<f64> = None;
    let mut done = vec![false; map.len()];
    let out_tbs: Vec<ff::Rational> = octx.streams().map(|s| s.time_base()).collect();

    for (ist, mut pkt) in ictx.packets() {
        let i = ist.index();
        let Some(o) = map[i] else { continue };
        if done[i] {
            if done.iter().zip(&map).all(|(d, m)| *d || m.is_none()) {
                break;
            }
            continue;
        }
        let tb = f64::from(ist.time_base());
        let Some(ts) = pkt.pts().or(pkt.dts()) else {
            continue;
        };
        let t = ts as f64 * tb;
        if t > end {
            done[i] = true;
            continue;
        }
        let base = *offset.get_or_insert(t.min(start));
        let shift = (base / tb).round() as i64;
        pkt.set_pts(pkt.pts().map(|p| p - shift));
        pkt.set_dts(pkt.dts().map(|d| d - shift));
        pkt.rescale_ts(ist.time_base(), out_tbs[o]);
        pkt.set_position(-1);
        pkt.set_stream(o);
        if pkt.dts().is_some_and(|d| d < 0) && ist.parameters().medium() != ff::media::Type::Video {
            continue;
        }
        pkt.write_interleaved(&mut octx)
            .map_err(|e| e.to_string())?;
    }
    octx.write_trailer().map_err(|e| e.to_string())?;
    Ok(())
}
