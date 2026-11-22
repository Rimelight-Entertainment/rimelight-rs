// Compiles ui/main.slint against the two markup libraries: rimelight_ui (the components) and rimelight_design (the
// tokens that the components import as @rimelight_design/tokens.slint).
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=ui");
    println!("cargo::rerun-if-changed={}", rimelight_ui::UI_DIR);
    println!("cargo::rerun-if-changed={}", rimelight_design::UI_DIR);

    let libraries = HashMap::from([
        (
            "rimelight_ui".to_owned(),
            PathBuf::from(rimelight_ui::UI_DIR),
        ),
        (
            "rimelight_design".to_owned(),
            PathBuf::from(rimelight_design::UI_DIR),
        ),
    ]);
    let config = slint_build::CompilerConfiguration::new().with_library_paths(libraries);
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("slint: ui/main.slint failed to compile");
}
