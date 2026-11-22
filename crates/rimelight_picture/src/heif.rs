//! HEIC, HEIF, HIF and AVIF pictures, read by FFmpeg (the mov demuxer, the HEVC and AV1 decoders). Phone HEICs are tile
//! grids: every tile is decoded to RGBA, placed on the canvas at its offset, and the final crop is taken. Viewing only:
//! `picture::can_save` refuses these extensions.
//!
//! The only unsafe code in this crate: reading FFmpeg's stream groups and display matrix through the raw pointers
//! FFmpeg exposes (the safe ffmpeg-next API has no accessor for either). Every use is listed in the `// SAFETY:` notes.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use ffmpeg_next as ff;

use crate::edit::{crop_px, rotate_cw};
use crate::picture::Picture;

/// The tile grid of a HEIF picture: where each tile goes on the canvas, then the crop of the final image.
struct Grid {
    canvas_w: u32,
    canvas_h: u32,
    background: [u8; 4],
    /// (stream index, x, y) in the order they are drawn (later ones on top).
    tiles: Vec<(usize, i64, i64)>,
    /// The final image: x, y, w, h on the canvas.
    crop: (u32, u32, u32, u32),
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// FFmpeg's global setup, once.
fn init() -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| ff::init().map_err(|e| e.to_string()))
        .clone()
}

/// Opens a HEIC, HEIF, HIF or AVIF file and returns its picture, upright.
pub fn load(path: &Path) -> Result<Picture, String> {
    init()?;
    let mut ictx =
        ff::format::input(&path).map_err(|e| format!("Could not open the picture: {e}"))?;
    let turns = quarter_turns(&ictx);
    // Files without a tile grid (single-image HEIC and AVIF) take the first video stream's first picture.
    // TODO: a multi-image HEIF with no tile grid shows only its first image.
    let mut pic = match read_grid(&ictx) {
        Some(grid) => compose(&mut ictx, &grid)?,
        None => first_picture(&mut ictx)?,
    };
    for _ in 0..turns {
        pic = rotate_cw(&pic);
    }
    Ok(pic)
}

/// Reads the tile grid stream group from the raw format context, if the file has one.
fn read_grid(ictx: &ff::format::context::Input) -> Option<Grid> {
    // SAFETY: `ictx` borrows the format context, so the context and its stream groups stay alive for this function.
    // Each group, tile grid, offsets array and stream list is checked for null before it is read, and the counts come
    // from FFmpeg itself, so each slice is of memory FFmpeg allocated for that many elements. Each stream index taken
    // from the offsets is checked against the group's stream list (`get`) before it is dereferenced.
    unsafe {
        let ctx = &*ictx.as_ptr();
        if ctx.nb_stream_groups == 0 || ctx.stream_groups.is_null() {
            return None;
        }
        let groups = std::slice::from_raw_parts(ctx.stream_groups, ctx.nb_stream_groups as usize);
        for &group in groups {
            if group.is_null() {
                continue;
            }
            let g = &*group;
            if g.type_ != ff::ffi::AVStreamGroupParamsType::AV_STREAM_GROUP_PARAMS_TILE_GRID {
                continue;
            }
            let tg_ptr = g.params.tile_grid;
            if tg_ptr.is_null() {
                return None;
            }
            let tg = &*tg_ptr;
            if tg.offsets.is_null() || tg.nb_tiles == 0 || g.streams.is_null() || g.nb_streams == 0
            {
                return None;
            }
            let offsets = std::slice::from_raw_parts(tg.offsets, tg.nb_tiles as usize);
            let streams = std::slice::from_raw_parts(g.streams, g.nb_streams as usize);
            let mut tiles = Vec::with_capacity(offsets.len());
            for o in offsets {
                let s = *streams.get(o.idx as usize)?;
                if s.is_null() {
                    return None;
                }
                tiles.push((
                    (*s).index as usize,
                    i64::from(o.horizontal),
                    i64::from(o.vertical),
                ));
            }
            let (cw, ch) = (tg.coded_width, tg.coded_height);
            if cw <= 0 || ch <= 0 {
                return None;
            }
            let (x, y) = (
                tg.horizontal_offset.max(0) as u32,
                tg.vertical_offset.max(0) as u32,
            );
            let (w, h) = (tg.width, tg.height);
            let (cw, ch) = (cw as u32, ch as u32);
            let crop = if w > 0 && h > 0 && x < cw && y < ch {
                (x, y, (w as u32).min(cw - x), (h as u32).min(ch - y))
            } else {
                (0, 0, cw, ch)
            };
            return Some(Grid {
                canvas_w: cw,
                canvas_h: ch,
                background: tg.background,
                tiles,
                crop,
            });
        }
    }
    None
}

/// Quarter turns clockwise that make the picture upright, from its display matrix (0 when there is none).
fn quarter_turns(ictx: &ff::format::context::Input) -> u32 {
    for stream in ictx.streams() {
        let params = stream.parameters();
        let degrees = unsafe {
            // SAFETY: `params` owns the codec parameters for the duration of this loop body, so the pointer from
            // `as_ptr` is valid to read. Null and count are checked before the side data array is used, and the side
            // data is read only when its size is at least 36 bytes (a 3x3 matrix of 32-bit values), the size the display
            // matrix has. `av_display_rotation_get` reads only that matrix.
            let p = &*params.as_ptr();
            if p.coded_side_data.is_null() || p.nb_coded_side_data <= 0 {
                continue;
            }
            let sd = ff::ffi::av_packet_side_data_get(
                p.coded_side_data,
                p.nb_coded_side_data,
                ff::ffi::AVPacketSideDataType::AV_PKT_DATA_DISPLAYMATRIX,
            );
            if sd.is_null() || (*sd).data.is_null() || (*sd).size < 36 {
                continue;
            }
            ff::ffi::av_display_rotation_get((*sd).data.cast::<i32>().cast_const())
        };
        // FFmpeg reports the angle counter-clockwise; the picture is turned by its negative to stand upright.
        return ((-degrees / 90.0).round() as i64).rem_euclid(4) as u32;
    }
    0
}

/// Decodes the grid's tiles and places them on the canvas, then crops.
fn compose(ictx: &mut ff::format::context::Input, grid: &Grid) -> Result<Picture, String> {
    let mut wanted: Vec<usize> = grid.tiles.iter().map(|t| t.0).collect();
    wanted.sort_unstable();
    wanted.dedup();
    let packets = first_packets(ictx, &wanted)?;

    let (cw, ch) = (grid.canvas_w, grid.canvas_h);
    let mut canvas = Picture {
        w: cw,
        h: ch,
        rgba: grid.background.repeat(cw as usize * ch as usize),
    };
    let mut decoded: HashMap<usize, Picture> = HashMap::new();
    for &(idx, x, y) in &grid.tiles {
        if !decoded.contains_key(&idx) {
            let packet = packets
                .get(&idx)
                .ok_or("a tile of the picture has no data")?;
            decoded.insert(idx, decode_first(ictx, idx, packet)?);
        }
        if let Some(tile) = decoded.get(&idx) {
            place(&mut canvas, tile, x, y);
        }
    }

    let (x, y, w, h) = grid.crop;
    let w = w.min(cw.saturating_sub(x)).max(1);
    let h = h.min(ch.saturating_sub(y)).max(1);
    Ok(crop_px(&canvas, x, y, w, h))
}

/// The first video stream's first picture (single-image files without a tile grid).
fn first_picture(ictx: &mut ff::format::context::Input) -> Result<Picture, String> {
    let idx = ictx
        .streams()
        .find(|s| s.parameters().medium() == ff::media::Type::Video)
        .map(|s| s.index())
        .ok_or("the file has no picture")?;
    let mut packets = first_packets(ictx, &[idx])?;
    let packet = packets.remove(&idx).ok_or("the picture has no data")?;
    decode_first(ictx, idx, &packet)
}

/// The first packet of each wanted stream, read until all are found or the file ends.
fn first_packets(
    ictx: &mut ff::format::context::Input,
    wanted: &[usize],
) -> Result<HashMap<usize, ff::Packet>, String> {
    let mut found: HashMap<usize, ff::Packet> = HashMap::new();
    for (stream, packet) in ictx.packets() {
        let idx = stream.index();
        if wanted.contains(&idx) && !found.contains_key(&idx) {
            found.insert(idx, packet);
            if found.len() == wanted.len() {
                break;
            }
        }
    }
    Ok(found)
}

/// Decodes one packet of a stream to RGBA (the first picture the decoder gives).
fn decode_first(
    ictx: &ff::format::context::Input,
    idx: usize,
    packet: &ff::Packet,
) -> Result<Picture, String> {
    let stream = ictx
        .stream(idx)
        .ok_or("a stream of the picture is missing")?;
    let mut dec = ff::codec::context::Context::from_parameters(stream.parameters())
        .map_err(err)?
        .decoder()
        .video()
        .map_err(err)?;
    let mut frame = ff::frame::Video::empty();
    dec.send_packet(packet).map_err(err)?;
    if dec.receive_frame(&mut frame).is_err() {
        dec.send_eof().map_err(err)?;
        dec.receive_frame(&mut frame)
            .map_err(|e| format!("the picture could not be decoded: {e}"))?;
    }
    to_rgba(&frame)
}

/// Converts a decoded frame to RGBA with software scaling.
fn to_rgba(frame: &ff::frame::Video) -> Result<Picture, String> {
    let (w, h) = (frame.width(), frame.height());
    let mut scaler = ff::software::scaling::Context::get(
        frame.format(),
        w,
        h,
        ff::format::Pixel::RGBA,
        w,
        h,
        ff::software::scaling::Flags::BILINEAR,
    )
    .map_err(err)?;
    let mut out = ff::frame::Video::new(ff::format::Pixel::RGBA, w, h);
    scaler.run(frame, &mut out).map_err(err)?;
    let stride = out.stride(0);
    let data = out.data(0);
    let row = w as usize * 4;
    let mut rgba = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        rgba.extend_from_slice(&data[y * stride..y * stride + row]);
    }
    Ok(Picture { w, h, rgba })
}

/// Copies a tile onto the canvas at (x, y), clipped to the canvas.
fn place(canvas: &mut Picture, tile: &Picture, x: i64, y: i64) {
    let (cw, ch) = (i64::from(canvas.w), i64::from(canvas.h));
    let (tw, th) = (i64::from(tile.w), i64::from(tile.h));
    for ty in 0..th {
        let cy = y + ty;
        if cy < 0 || cy >= ch {
            continue;
        }
        for tx in 0..tw {
            let cx = x + tx;
            if cx < 0 || cx >= cw {
                continue;
            }
            let s = ((ty * tw + tx) * 4) as usize;
            let d = ((cy * cw + cx) * 4) as usize;
            canvas.rgba[d..d + 4].copy_from_slice(&tile.rgba[s..s + 4]);
        }
    }
}
