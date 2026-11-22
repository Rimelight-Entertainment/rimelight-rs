//! The component gallery binary: `cargo run -p rimelight_gallery`. The logic is in the library (`lib.rs`), so an app can
//! run the same gallery with its own stories.

use std::process::ExitCode;

fn main() -> ExitCode {
    rimelight_gallery::main_with(None)
}
