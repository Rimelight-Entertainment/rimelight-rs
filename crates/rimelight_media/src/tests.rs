//! Tests against the 2 s fixture clip (160x90, 10 fps, AAC stereo). They need no audio device.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::{Player, PlayerOptions, VideoFrame, poster, probe};

fn clip() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/clip.mp4"
    ))
}

fn silent_player(max: u32) -> Player {
    let options = PlayerOptions {
        max_width: max,
        max_height: max,
        audio: false,
    };
    Player::open(clip(), options).expect("the fixture opens")
}

/// Calls `take_frame` every 10 ms until `done` returns true for a frame or the time runs out.
fn pump_until(
    player: &Player,
    limit: Duration,
    mut done: impl FnMut(&VideoFrame) -> bool,
) -> Option<VideoFrame> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Some(frame) = player.take_frame()
            && done(&frame)
        {
            return Some(frame);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn probe_reads_the_clip() {
    let info = probe(clip()).expect("probe");
    assert_eq!((info.width, info.height), (160, 90));
    assert!(
        (1900..=2100).contains(&info.duration_ms),
        "duration {} ms",
        info.duration_ms
    );
    assert!(info.has_audio);
    assert_eq!(info.mime, "video/mp4");
}

#[test]
fn poster_is_scaled_and_has_a_picture() {
    let picture = poster(clip(), 0.5, 64).expect("poster");
    assert_eq!((picture.width, picture.height), (64, 36));
    let bytes = picture.bytes();
    assert_eq!(bytes.len(), 64 * 36 * 4);
    let first = &bytes[..4];
    assert!(
        bytes.as_chunks::<4>().0.iter().any(|px| px != first),
        "the poster is one flat colour"
    );
}

#[test]
fn player_plays_frames_in_order_without_audio() {
    let player = silent_player(80);
    player.play();
    let mut frames = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        if let Some(frame) = player.take_frame() {
            frames.push(frame);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(frames.len() >= 5, "only {} frames arrived", frames.len());
    assert!(
        frames.windows(2).all(|w| w[0].pts_ms < w[1].pts_ms),
        "frame times do not increase"
    );
    // Aspect 16:9 at 80 px wide gives 45 px high, but sides must be even, so 44.
    assert!(
        frames
            .iter()
            .all(|f| (f.picture.width, f.picture.height) == (80, 44))
    );
    assert!(
        player.position_ms() > 500,
        "position {} ms",
        player.position_ms()
    );
}

#[test]
fn seek_while_playing_jumps_forward() {
    let player = silent_player(80);
    player.play();
    std::thread::sleep(Duration::from_millis(300));
    player.seek(1500);
    let frame =
        pump_until(&player, Duration::from_secs(4), |_| true).expect("a frame after the seek");
    assert!(
        frame.pts_ms >= 1400,
        "first frame after seek was at {} ms",
        frame.pts_ms
    );
}

#[test]
fn playing_to_the_end_sets_ended() {
    let player = silent_player(80);
    player.play();
    let start = Instant::now();
    while !player.ended() {
        assert!(
            start.elapsed() < Duration::from_secs(4),
            "playback did not end"
        );
        let _ = player.take_frame();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!player.is_playing());
}
