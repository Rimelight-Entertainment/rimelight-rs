//! Compile check for `rimelight_ui`. The crate exists so that every component in the library is type-checked by the
//! Slint compiler. It has no runtime behaviour and is not shipped (`publish = false`).
//!
//! The generated `CheckWindow` (ui/main.slint) instantiates every exported component once. The headless test in
//! `tests/compile.rs` creates it on the testing backend.

// Slint generated code contains unsafe vtable statics. Only this module may allow it.
#[allow(unsafe_code)]
mod generated {
    slint::include_modules!();
}

pub use generated::*;
