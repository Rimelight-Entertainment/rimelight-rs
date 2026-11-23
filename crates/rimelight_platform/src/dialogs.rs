//! Native file, folder and save dialogs on Windows, through the `rfd` crate. Safe code only: the module has no unsafe
//! block, so it is not in the audit list. Each call blocks until the person answers, so call it from the UI thread
//! only when a modal wait is acceptable (as the Windows dialogs always are).
//!
//! The crate-root functions in `lib.rs` pick these up on Windows and answer empty everywhere else.

use std::path::PathBuf;

use crate::FileFilter;

/// Adds each `(name, extensions)` pair of `filter` to the dialog as one file-type choice.
fn with_filters(mut dialog: rfd::FileDialog, filter: FileFilter<'_>) -> rfd::FileDialog {
    for (name, extensions) in filter {
        let exts: Vec<String> = extensions.iter().map(|ext| (*ext).to_string()).collect();
        dialog = dialog.add_filter(*name, exts.as_slice());
    }
    dialog
}

/// The open-file dialog. `multiple` lets the person choose several files. An empty list when they cancel.
pub fn pick_files(filter: FileFilter<'_>, multiple: bool) -> Vec<PathBuf> {
    let dialog = with_filters(rfd::FileDialog::new(), filter);
    if multiple {
        dialog.pick_files().unwrap_or_default()
    } else {
        dialog.pick_file().into_iter().collect()
    }
}

/// The folder dialog. None when the person cancels.
pub fn pick_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

/// The save dialog, starting with `name` as the file name. None when the person cancels.
pub fn save_file_dialog(name: &str, filter: FileFilter<'_>) -> Option<PathBuf> {
    with_filters(rfd::FileDialog::new(), filter)
        .set_file_name(name)
        .save_file()
}
