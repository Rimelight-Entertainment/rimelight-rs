// Compiles ui/main.slint. The library `@spike_ui` is added by name, so markup can write `import ... from "@spike_ui/toast.slint";`.
// Only this crate compiles Slint; spike_ui and spike_kit ship markup and plain Rust.
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=ui");
    println!("cargo::rerun-if-changed={}", spike_ui::UI_DIR);

    let libraries = HashMap::from([("spike_ui".to_owned(), PathBuf::from(spike_ui::UI_DIR))]);
    let config = slint_build::CompilerConfiguration::new().with_library_paths(libraries);
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("slint: ui/main.slint failed to compile");
}
