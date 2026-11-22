//! Workspace task runner. Commands are added later; for now it prints usage.

fn main() {
    println!("usage: cargo xtask <command>");
    println!();
    println!(
        "No commands yet. Planned: ffmpeg (fetch the pinned FFmpeg build), screenshot, gallery."
    );
}
