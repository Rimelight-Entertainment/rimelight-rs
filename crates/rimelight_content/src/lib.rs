//! Rimelight content model, shared by the native CMS editor and the content service.
//!
//! The contract is `MAPPING.md` next to this crate: block JSON <-> Loro document, the rich text encoding, the normal
//! form and drafts. Both sides implement exactly that and are tested on the same `fixtures/`.
//!
//! The block registry lives in `registry/blocks.json` and is the single source. This crate has no UI dependency.

pub mod doc;
pub mod error;
pub mod normalize;
pub mod registry;
pub mod richtext;
pub mod types;

pub use doc::{catch_up, configure, export_page, fork_draft, import_page, merge_draft};
pub use error::Error;
pub use normalize::{normalize_page, normalize_rich};
pub use registry::{TextKind, allowed_child, text_kind};
