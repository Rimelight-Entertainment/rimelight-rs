//! The gallery harness on the Slint interpreter (decision 0044). It compiles `ui/dev.slint` at run time and builds the
//! `GalleryWindow` or the `FoundationsWindow` from options. An app's stories come in through the `@app_stories` library
//! path (see [`compile`]). It runs the window with live reload, or captures one frame to a PNG.
//!
//! Capture uses `Window::take_snapshot`, which renders the frame in software through the Slint renderer. A frame whose
//! sampled colours are fewer than [`MIN_DISTINCT_COLOURS`] is refused: it is blank, and a blank capture is a failure,
//! not evidence.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use rimelight_ui_kit::table;
use rimelight_ui_kit::theme::{Mode, TextScale};
use slint::ComponentHandle as _;
use slint_interpreter::{CompilationResult, Compiler, ComponentInstance, Value};

use crate::nav;
use crate::registry::{self, AfterRender, AppStory, Section, Selection};
use crate::values::{list, number, record};

/// The markup folder of the gallery. The gallery reads it at run time, so a markup edit needs no cargo build.
pub const UI_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ui");

/// The folder `@app_stories` maps to when no app stories are given: an `AppStories` that draws nothing.
const NO_APP_STORIES: &str = "no-app-stories";

/// The library path that maps to an app's story folder: `@app_stories/stories.slint`.
const APP_LIBRARY: &str = "app_stories";

/// A capture with fewer distinct colours than this is refused as blank.
pub const MIN_DISTINCT_COLOURS: usize = 8;

/// The row count of the List and Table gallery stories (P0.17 batch 5, the 10,000-row target).
pub const GALLERY_ROWS: usize = 10_000;

/// The viewport frame: desktop fills the content area; tablet and phone are fixed widths (768 and 390 logical px).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewport {
    Desktop,
    Tablet,
    Phone,
}

impl Viewport {
    /// The index the markup reads: 0 desktop, 1 tablet, 2 phone.
    pub fn index(self) -> i32 {
        match self {
            Viewport::Desktop => 0,
            Viewport::Tablet => 1,
            Viewport::Phone => 2,
        }
    }

    pub fn parse(text: &str) -> Option<Viewport> {
        match text {
            "desktop" => Some(Viewport::Desktop),
            "tablet" => Some(Viewport::Tablet),
            "phone" => Some(Viewport::Phone),
            _ => None,
        }
    }

    fn from_index(index: i32) -> Viewport {
        match index {
            1 => Viewport::Tablet,
            2 => Viewport::Phone,
            _ => Viewport::Desktop,
        }
    }
}

/// Everything the window starts with. Defaults match the gallery's own defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Show the foundations sheet (S1) instead of the gallery. Only the theme, locale, text scale and size apply.
    pub foundations: bool,
    pub selection: Selection,
    pub theme: Mode,
    pub portuguese: bool,
    pub viewport: Viewport,
    pub scale: TextScale,
    pub reduced_motion: bool,
    pub chrome: bool,
    pub size: (u32, u32),
    /// The app's story folder (`--stories`), or None for the built-in stories only.
    pub app_dir: Option<PathBuf>,
    /// The app's stories, read from `app_dir` (see [`registry::load_app_stories`]). They are listed after the built-ins.
    pub apps: Vec<AppStory>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            foundations: false,
            selection: Selection {
                story: 0,
                variant: 0,
            },
            theme: Mode::System,
            portuguese: false,
            viewport: Viewport::Desktop,
            scale: TextScale::P100,
            reduced_motion: false,
            chrome: true,
            size: (1280, 800),
            app_dir: None,
            apps: Vec::new(),
        }
    }
}

impl Options {
    /// The number of selectable stories: the built-in ones, then the app's.
    pub fn story_count(&self) -> usize {
        registry::total(&self.apps)
    }

    /// The same options with a selection that exists (a reload can follow a change of the app's stories).
    fn checked(mut self) -> Options {
        let story = self.selection.story.min(self.story_count() - 1);
        self.selection.story = story;
        if self.selection.variant >= registry::variant_count(story, &self.apps) {
            self.selection.variant = 0;
        }
        self
    }
}

/// The development entry point the interpreter compiles.
pub fn dev_slint() -> PathBuf {
    Path::new(UI_DIR).join("dev.slint")
}

/// Compiles `ui/dev.slint`, with `@app_stories` mapped to the app's story folder, or to the empty stub `ui/no-app-stories`
/// when there is none. On an error, returns every error diagnostic, one per line.
pub fn compile(app_dir: Option<&Path>) -> Result<CompilationResult, String> {
    // `@rimelight_design/tokens.slint` resolves to the design tokens markup (packages/design in the web monorepo).
    // `@rimelight_ui/...` resolves to the shared component library (rimelight-rs, ST2.3), `@rimelight_shell/...` to the shared shell (ST2.5a).
    let mut libraries = HashMap::from([
        (
            "rimelight_design".to_string(),
            PathBuf::from(rimelight_design::UI_DIR),
        ),
        (
            "rimelight_ui".to_string(),
            PathBuf::from(rimelight_ui::UI_DIR),
        ),
        (
            "rimelight_shell".to_string(),
            PathBuf::from(rimelight_shell::UI_DIR),
        ),
    ]);
    let stories = app_dir.map_or_else(|| Path::new(UI_DIR).join(NO_APP_STORIES), Path::to_path_buf);
    libraries.insert(APP_LIBRARY.to_string(), stories);
    let mut compiler = Compiler::default();
    compiler.set_library_paths(libraries);
    let result = spin_on::spin_on(compiler.build_from_path(dev_slint()));
    if result.has_errors() {
        let lines: Vec<String> = result
            .diagnostics()
            .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
            .map(|d| d.to_string())
            .collect();
        return Err(lines.join("\n"));
    }
    Ok(result)
}

fn set(ui: &ComponentInstance, name: &str, value: Value) {
    if let Err(e) = ui.set_property(name, value) {
        panic!("gallery property {name}: {e}");
    }
}

fn get(ui: &ComponentInstance, name: &str) -> Value {
    ui.get_property(name)
        .unwrap_or_else(|e| panic!("gallery property {name}: {e}"))
}

/// An int property of the window.
pub fn get_int(ui: &ComponentInstance, name: &str) -> i32 {
    f64::try_from(get(ui, name)).unwrap_or_default() as i32
}

/// Builds the window with the options applied. Does not show it.
pub fn build(compiled: &CompilationResult, options: &Options) -> Result<ComponentInstance, String> {
    let name = if options.foundations {
        "FoundationsWindow"
    } else {
        "GalleryWindow"
    };
    let definition = compiled
        .component(name)
        .ok_or_else(|| format!("the markup exports no {name}"))?;
    let ui = definition
        .create()
        .map_err(|e| format!("could not create the {name}: {e}"))?;
    set_mode(&ui, options.theme);
    set_text_scale(&ui, options.scale);
    ui.window().set_size(slint::LogicalSize::new(
        options.size.0 as f32,
        options.size.1 as f32,
    ));
    if options.foundations {
        set(&ui, "pt", Value::Bool(options.portuguese));
        return Ok(ui);
    }

    // Every window that shows a Select or a Menu wires their search callbacks (S6, S7), and a ColorPicker its hex parser.
    nav::wire_select(&ui);
    nav::wire_menu(&ui);
    nav::wire_colour(&ui);
    set(&ui, "rows", list(sidebar_rows(&options.apps)));
    set(&ui, "builtin-count", number(registry::STORIES.len()));
    set(&ui, "story-count", number(options.story_count()));
    // The List and Table stories (P0.17 batch 5) show 10,000 generated rows each.
    set(&ui, "list-rows", list(list_rows(GALLERY_ROWS)));
    set(&ui, "table-columns", list(table_columns()));
    set(&ui, "table-rows", list(table_rows(GALLERY_ROWS)));
    // The Shell story's rail, badges and sidebar come from the sample data (`sample`), with the command palette.
    nav::wire_palette(&ui);
    nav::wire_shell(&ui, options.selection.variant);
    set(&ui, "story", number(options.selection.story));
    set(&ui, "variant", number(options.selection.variant));
    set(&ui, "locale-index", number(usize::from(options.portuguese)));
    set(
        &ui,
        "viewport",
        Value::Number(options.viewport.index().into()),
    );
    set(&ui, "scale-index", number(scale_index(options.scale)));
    set(&ui, "chrome", Value::Bool(options.chrome));
    set_reduced_motion(&ui, options.reduced_motion);
    set(&ui, "theme-index", number(theme_index(options.theme)));
    Ok(ui)
}

/// The rows of the List story: `count` generated labels, none selected.
pub fn list_rows(count: usize) -> Vec<Value> {
    table::sample_list_labels(count)
        .into_iter()
        .map(|label| {
            record([
                ("label", Value::String(label.into())),
                ("selected", Value::Bool(false)),
            ])
        })
        .collect()
}

/// The columns of the Table story, from [`table::SAMPLE_COLUMNS`]. All sortable, each 160 px wide.
pub fn table_columns() -> Vec<Value> {
    table::SAMPLE_COLUMNS
        .iter()
        .map(|title| {
            record([
                ("title", Value::String((*title).into())),
                ("width", Value::Number(160.0)),
                ("sortable", Value::Bool(true)),
            ])
        })
        .collect()
}

/// The rows of the Table story: `count` generated rows from [`table::sample_rows`], none selected.
pub fn table_rows(count: usize) -> Vec<Value> {
    table::sample_rows(count)
        .into_iter()
        .map(|cells| {
            let cells = cells
                .into_iter()
                .map(|cell| Value::String(cell.into()))
                .collect();
            record([("cells", list(cells)), ("selected", Value::Bool(false))])
        })
        .collect()
}

/// One sidebar row per heading, and one per story. An empty section gets a note row, not a story. The app's stories
/// follow the built-in sections under one heading, [`registry::APP_LABEL`].
pub fn sidebar_rows(apps: &[AppStory]) -> Vec<Value> {
    let row = |title: &str, header: bool, story: i32| {
        record([
            ("title", Value::String(title.into())),
            ("header", Value::Bool(header)),
            ("story", Value::Number(story.into())),
        ])
    };
    let mut rows = Vec::new();
    for section in Section::ALL {
        rows.push(row(section.label(), true, -1));
        let stories = registry::stories_in(section);
        if stories.is_empty() {
            rows.push(row("Not ported yet", false, -1));
        }
        for index in stories {
            rows.push(row(registry::STORIES[index].title, false, index as i32));
        }
    }
    if !apps.is_empty() {
        rows.push(row(registry::APP_LABEL, true, -1));
        for (offset, app) in apps.iter().enumerate() {
            let story = registry::STORIES.len() + offset;
            rows.push(row(app.title.as_str(), false, story as i32));
        }
    }
    rows
}

fn theme_index(mode: Mode) -> usize {
    match mode {
        Mode::Light => 0,
        Mode::Dark => 1,
        Mode::System => 2,
    }
}

fn scale_index(scale: TextScale) -> usize {
    match scale {
        TextScale::P100 => 0,
        TextScale::P125 => 1,
        TextScale::P150 => 2,
        TextScale::P200 => 3,
    }
}

fn set_tokens(ui: &ComponentInstance, name: &str, value: Value) {
    if let Err(e) = ui.set_global_property("Tokens", name, value) {
        panic!("Tokens.{name}: {e}");
    }
}

/// Sets light, dark or system on the `Tokens` global.
pub fn set_mode(ui: &ComponentInstance, mode: Mode) {
    let choice = match mode {
        Mode::Light => "light",
        Mode::Dark => "dark",
        Mode::System => "system",
    };
    set_tokens(
        ui,
        "theme",
        Value::EnumerationValue("ThemeChoice".into(), choice.into()),
    );
}

/// The text scale (100, 125, 150 or 200 per cent).
pub fn set_text_scale(ui: &ComponentInstance, scale: TextScale) {
    set_tokens(ui, "text-scale-percent", number(scale.percent() as usize));
}

/// The manual reduced-motion flag.
pub fn set_reduced_motion(ui: &ComponentInstance, on: bool) {
    set_tokens(ui, "reduced-motion", Value::Bool(on));
}

/// Whether the tokens currently resolve to the dark set.
pub fn is_dark(ui: &ComponentInstance) -> bool {
    ui.get_global_property("Tokens", "dark")
        .ok()
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}

/// Runs the action of the selected variant, if it has one. Called once the window is shown. App variants have none.
pub fn run_after_render(ui: &ComponentInstance, options: &Options) {
    if options.foundations {
        return;
    }
    let invoke = |name: &str| {
        if let Err(e) = ui.invoke(name, &[]) {
            panic!("gallery callback {name}: {e}");
        }
    };
    match registry::after_render(options.selection) {
        AfterRender::None => {}
        AfterRender::FocusFirstChip => invoke("focus-first"),
        AfterRender::FocusSidebar => invoke("focus-sidebar"),
        AfterRender::OpenPopover => {
            // A first render lays the window out, so the popup is placed from the trigger's real position.
            let _ = ui.window().take_snapshot();
            invoke("open-popover");
        }
    }
}

/// The options a reload keeps: the window's current story, variant and toolbar choices. The app folder is kept from
/// `previous`; the caller reloads its stories.
fn current_options(ui: &ComponentInstance, previous: &Options) -> Options {
    if previous.foundations {
        return previous.clone();
    }
    let theme = match get_int(ui, "theme-index") {
        0 => Mode::Light,
        1 => Mode::Dark,
        _ => Mode::System,
    };
    let scale = match get_int(ui, "scale-index") {
        1 => TextScale::P125,
        2 => TextScale::P150,
        3 => TextScale::P200,
        _ => TextScale::P100,
    };
    let size = ui.window().size().to_logical(ui.window().scale_factor());
    Options {
        foundations: false,
        selection: Selection {
            story: get_int(ui, "story").max(0) as usize,
            variant: get_int(ui, "variant").max(0) as usize,
        },
        theme,
        portuguese: get_int(ui, "locale-index") == 1,
        viewport: Viewport::from_index(get_int(ui, "viewport")),
        scale,
        reduced_motion: ui
            .get_global_property("Tokens", "reduced-motion")
            .ok()
            .and_then(|v| bool::try_from(v).ok())
            .unwrap_or(false),
        chrome: bool::try_from(get(ui, "chrome")).unwrap_or(true),
        size: (size.width as u32, size.height as u32),
        app_dir: previous.app_dir.clone(),
        apps: previous.apps.clone(),
    }
    .checked()
}

/// The newest modification time of any file under `dir`.
fn newest_change(dir: &Path) -> SystemTime {
    let mut newest = SystemTime::UNIX_EPOCH;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return newest;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let time = if path.is_dir() {
            newest_change(&path)
        } else {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        };
        newest = newest.max(time);
    }
    newest
}

/// The newest change under `ui/` and, when there is an app folder, under that folder.
fn newest_change_in(options: &Options) -> SystemTime {
    let mut newest = newest_change(Path::new(UI_DIR));
    if let Some(dir) = &options.app_dir {
        newest = newest.max(newest_change(dir));
    }
    newest
}

/// Opens the window and runs its event loop until it closes. Every half second it checks `ui/` (and the app folder) for a
/// change; on one it recompiles, reloads the app's stories, swaps in a new window with the same story and toolbar choices,
/// and closes the old one. A markup error is printed and the old window stays.
pub fn run(options: &Options) -> Result<(), String> {
    let compiled = compile(options.app_dir.as_deref())?;
    let ui = build(&compiled, options)?;
    ui.show().map_err(|e| format!("show failed: {e}"))?;
    run_after_render(&ui, options);

    let current = Rc::new(RefCell::new(ui));
    let seen = Rc::new(RefCell::new(newest_change_in(options)));
    let base = options.clone();
    let timer = slint::Timer::default();
    let watched = current.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(500),
        move || {
            let now = newest_change_in(&base);
            if now <= *seen.borrow() {
                return;
            }
            *seen.borrow_mut() = now;
            let started = std::time::Instant::now();
            let mut options = current_options(&watched.borrow(), &base);
            if let Some(dir) = options.app_dir.clone() {
                match registry::load_app_stories(&dir) {
                    Ok(apps) => {
                        options.apps = apps;
                        options = options.checked();
                    }
                    Err(message) => {
                        eprintln!(
                            "rimelight_gallery: reload failed, keeping the old window:\n{message}"
                        );
                        return;
                    }
                }
            }
            let fresh =
                compile(options.app_dir.as_deref()).and_then(|compiled| build(&compiled, &options));
            match fresh {
                Ok(next) => {
                    if let Err(e) = next.show() {
                        eprintln!("rimelight_gallery: reload show failed: {e}");
                        return;
                    }
                    run_after_render(&next, &options);
                    let old = std::mem::replace(&mut *watched.borrow_mut(), next);
                    let _ = old.hide();
                    eprintln!("rimelight_gallery: reloaded in {:?}", started.elapsed());
                }
                Err(message) => eprintln!(
                    "rimelight_gallery: reload failed, keeping the old window:\n{message}"
                ),
            }
        },
    );
    slint::run_event_loop().map_err(|e| format!("run failed: {e}"))?;
    drop(timer);
    Ok(())
}

/// Installs the headless platform: Slint's testing backend with the software renderer. No native window is ever
/// created in this process, so a capture shows nothing on the desktop. Call once per thread, before any window is built.
pub fn install_headless() -> Result<(), String> {
    use i_slint_backend_testing::testing_backend::{TestingBackend, TestingBackendOptions};
    let backend = TestingBackend::new(TestingBackendOptions {
        mock_time: true,
        threading: false,
        renderer_name: Some("software".into()),
    });
    slint::platform::set_platform(Box::new(backend))
        .map_err(|e| format!("could not install the headless platform: {e}"))
}

/// Why a capture failed. `Blank` is the blank-image check, which the binary reports with its own exit code.
#[derive(Debug)]
pub enum CaptureError {
    Blank(String),
    Failed(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::Blank(message) | CaptureError::Failed(message) => f.write_str(message),
        }
    }
}

/// Renders the window headlessly at its size and writes the PNG at `out`. Needs [`install_headless`] first.
pub fn capture(options: &Options, out: &Path) -> Result<String, CaptureError> {
    let compiled = compile(options.app_dir.as_deref()).map_err(CaptureError::Failed)?;
    let ui = build(&compiled, options).map_err(CaptureError::Failed)?;
    ui.show()
        .map_err(|e| CaptureError::Failed(format!("show failed: {e}")))?;
    run_after_render(&ui, options);
    snapshot_to_png(&ui, out)
}

/// Takes the snapshot, refuses a blank frame, and writes the PNG. Returns a one-line description.
fn snapshot_to_png(ui: &ComponentInstance, out: &Path) -> Result<String, CaptureError> {
    let failed = CaptureError::Failed;
    let buffer = ui
        .window()
        .take_snapshot()
        .map_err(|e| failed(format!("take_snapshot failed: {e}")))?;
    let bytes = buffer.as_bytes();
    let (pixels, _) = bytes.as_chunks::<4>();
    let distinct = distinct_colours(pixels);
    if distinct < MIN_DISTINCT_COLOURS {
        return Err(CaptureError::Blank(format!(
            "captured image is blank ({distinct} distinct colours, need {MIN_DISTINCT_COLOURS}): {}",
            out.display()
        )));
    }
    let img = image::RgbaImage::from_raw(buffer.width(), buffer.height(), bytes.to_vec())
        .ok_or_else(|| failed("snapshot buffer does not match its size".to_string()))?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| failed(format!("cannot create {}: {e}", parent.display())))?;
    }
    img.save(out)
        .map_err(|e| failed(format!("could not write {}: {e}", out.display())))?;
    Ok(format!(
        "snapshot {}x{} dark={} distinct={distinct} -> {}",
        buffer.width(),
        buffer.height(),
        is_dark(ui),
        out.display()
    ))
}

fn distinct_colours(pixels: &[[u8; 4]]) -> usize {
    pixels
        .iter()
        .copied()
        .collect::<std::collections::HashSet<[u8; 4]>>()
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;
    use slint::platform::{Key, WindowEvent};
    use slint::{Model as _, SharedString};

    #[test]
    fn blank_frames_have_one_colour() {
        let flat = [[1u8, 2, 3, 255]; 100];
        assert_eq!(distinct_colours(&flat), 1);
    }

    #[test]
    fn viewports_parse_and_index() {
        assert_eq!(Viewport::parse("phone"), Some(Viewport::Phone));
        assert_eq!(Viewport::parse("watch"), None);
        assert_eq!(Viewport::Tablet.index(), 1);
        assert_eq!(Viewport::from_index(2), Viewport::Phone);
    }

    #[test]
    fn every_section_has_a_row_and_each_story_one() {
        let rows = sidebar_rows(&[]);
        let field = |row: &Value, name: &str| match row {
            Value::Struct(s) => s.get_field(name).cloned().unwrap_or_default(),
            _ => Value::Void,
        };
        let headers = rows
            .iter()
            .filter(|r| field(r, "header") == Value::Bool(true))
            .count();
        assert_eq!(headers, Section::ALL.len());
        let stories = rows
            .iter()
            .filter(|r| f64::try_from(field(r, "story")).unwrap_or(-1.0) >= 0.0)
            .count();
        assert_eq!(stories, registry::STORIES.len());
    }

    fn press(ui: &ComponentInstance, key: Key) {
        let text: SharedString = key.into();
        ui.window()
            .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        ui.window()
            .dispatch_event(WindowEvent::KeyReleased { text });
    }

    /// S2 gallery interaction test on the interpreter. Real key events: Up and Down move the sidebar selection, Tab moves
    /// between toolbar chips, and Space chooses one. One test function, because the testing backend is installed once
    /// per thread. The app-stories check runs at the end, on the same backend.
    #[test]
    fn gallery_compiles_builds_every_story_and_follows_the_keys() {
        install_headless().expect("install the testing backend");
        let compiled = compile(None).expect("ui/dev.slint compiles");

        // The foundations sheet builds and renders.
        let foundations = Options {
            foundations: true,
            ..Options::default()
        };
        let sheet = build(&compiled, &foundations).expect("build the foundations sheet");
        sheet.show().expect("show the foundations sheet");
        let buffer = sheet.window().take_snapshot().expect("snapshot");
        let (pixels, _) = buffer.as_bytes().as_chunks::<4>();
        assert!(distinct_colours(pixels) >= MIN_DISTINCT_COLOURS);

        // The story list is a scroll area under the sidebar title. At 1280x800 its visible height must stay tall, so every
        // section is reachable. A collapsed list (about one row, as in the P0.17 regression) fails this.
        let ui = build(&compiled, &Options::default()).expect("build the gallery");
        ui.show().expect("show the gallery");
        let height = f64::try_from(get(&ui, "story-list-height")).unwrap_or_default();
        assert!(
            height > 300.0,
            "story list visible height is {height} px at 1280x800"
        );

        // Every registered story builds and shows its own index in the window.
        for line in registry::listing(&[]) {
            let selection = registry::resolve(&line, &[]).expect("listing ids resolve");
            let options = Options {
                selection,
                ..Options::default()
            };
            let ui = build(&compiled, &options).expect("build the gallery");
            ui.show().expect("show the gallery");
            assert_eq!(get_int(&ui, "story"), selection.story as i32, "{line}");
            assert_eq!(get_int(&ui, "variant"), selection.variant as i32, "{line}");
        }

        // The sidebar: Up and Down change the story, and stop at the ends.
        assert_eq!(get_int(&ui, "story-count"), registry::STORIES.len() as i32);
        assert_eq!(
            get_int(&ui, "builtin-count"),
            registry::STORIES.len() as i32
        );
        ui.invoke("focus-sidebar", &[]).expect("focus-sidebar");
        press(&ui, Key::DownArrow);
        assert_eq!(get_int(&ui, "story"), 1, "Down selects the next story");
        press(&ui, Key::DownArrow);
        assert_eq!(get_int(&ui, "story"), 2);
        press(&ui, Key::UpArrow);
        assert_eq!(get_int(&ui, "story"), 1, "Up selects the previous story");
        for _ in 0..20 {
            press(&ui, Key::UpArrow);
        }
        assert_eq!(get_int(&ui, "story"), 0, "Up stops at the first story");
        for _ in 0..registry::STORIES.len() {
            press(&ui, Key::DownArrow);
        }
        assert_eq!(
            get_int(&ui, "story"),
            registry::STORIES.len() as i32 - 1,
            "Down stops at the last story"
        );

        // The rows: one heading per section, and each story once.
        let Value::Model(rows) = get(&ui, "rows") else {
            panic!("rows is a model")
        };
        assert_eq!(rows.row_count(), sidebar_rows(&[]).len());

        // The toolbar: the focus action gives the first chip (Light) focus. Tab moves to Dark, Space chooses it.
        set(&ui, "story", number(0));
        ui.invoke("focus-first", &[]).expect("focus-first");
        press(&ui, Key::Tab);
        press(&ui, Key::Space);
        assert_eq!(
            get_int(&ui, "theme-index"),
            1,
            "Tab then Space chooses the Dark chip"
        );
        assert!(is_dark(&ui));

        // Focus is on Dark (index 1). Chips in tab order: Light, Dark, System, en, pt, Desktop, Tablet, Phone, 100, 125,
        // 150, 200, Motion on, Reduced. Nine Tabs reach 150 percent (index 10); Return chooses it.
        for _ in 0..9 {
            press(&ui, Key::Tab);
        }
        press(&ui, Key::Return);
        assert_eq!(
            get_int(&ui, "scale-index"),
            2,
            "Tab to 150 percent and Return chooses it"
        );

        // The Select and Menu rules answer through the interpreter (a wired global returns more than its default).
        let options = list(vec![
            nav::select_option_value("a", "Alpha", false),
            nav::select_option_value("b", "Beta", true),
            nav::select_option_value("c", "Gamma", false),
        ]);
        let next = ui
            .invoke_global(
                "SelectNav",
                "next-enabled",
                &[options, Value::Number(0.0), Value::Number(1.0)],
            )
            .expect("SelectNav.next-enabled");
        assert_eq!(next, Value::Number(2.0), "the disabled Beta is skipped");

        // The shell story: the rail is filled from the sample areas and a rail pick changes the area.
        let Value::Model(rail) = get(&ui, "shell-rail") else {
            panic!("shell-rail is a model")
        };
        assert_eq!(rail.row_count(), sample::areas().len());
        ui.invoke("shell-area-picked", &[Value::Number(1.0)])
            .expect("shell-area-picked");
        assert_eq!(get_int(&ui, "shell-area"), 1);

        app_stories_build_after_the_built_ins();
    }

    /// The app stories path: a temp folder with `stories.slint` (an `AppStories` that shows its story and variant)
    /// and `stories.txt` with one story. The window builds, counts the app story after the built-ins, and draws it.
    /// Called from the one test that installs the backend, so the folder is removed on every path.
    fn app_stories_build_after_the_built_ins() {
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let dir = std::env::temp_dir().join(format!(
            "rimelight_gallery_app_stories_{}",
            std::process::id()
        ));
        let _cleanup = Cleanup(dir.clone());
        std::fs::create_dir_all(&dir).expect("create the app folder");
        const APP_MARKUP: &str = r#"export component AppStories inherits Rectangle {
    in property <int> story;
    in property <int> variant;
    in property <bool> pt;
    Text { text: "App story " + story + " variant " + variant; }
}
"#;
        std::fs::write(dir.join("stories.slint"), APP_MARKUP).expect("write stories.slint");
        std::fs::write(
            dir.join("stories.txt"),
            "# one app story\nshell/app-note|App note|detail\n",
        )
        .expect("write stories.txt");

        let apps = registry::load_app_stories(&dir).expect("stories.txt parses");
        let story = registry::STORIES.len();
        let options = Options {
            app_dir: Some(dir.clone()),
            apps,
            selection: Selection { story, variant: 1 },
            ..Options::default()
        };
        let compiled = compile(options.app_dir.as_deref()).expect("compile with the app stories");
        let ui = build(&compiled, &options).expect("build GalleryWindow with app stories");
        ui.show().expect("show GalleryWindow with app stories");
        run_after_render(&ui, &options);

        assert_eq!(
            get_int(&ui, "builtin-count"),
            registry::STORIES.len() as i32
        );
        assert_eq!(
            get_int(&ui, "story-count"),
            (registry::STORIES.len() + 1) as i32
        );
        assert_eq!(get_int(&ui, "story"), story as i32);
        assert_eq!(get_int(&ui, "variant"), 1);
        let buffer = ui.window().take_snapshot().expect("snapshot");
        let (pixels, _) = buffer.as_bytes().as_chunks::<4>();
        assert!(distinct_colours(pixels) >= MIN_DISTINCT_COLOURS);

        // The listing and the sidebar know the app story, and it resolves to its index.
        assert!(registry::listing(&options.apps).contains(&"shell/app-note#detail".to_string()));
        assert_eq!(
            registry::resolve("shell/app-note", &options.apps).expect("resolves"),
            Selection { story, variant: 0 }
        );
        assert!(sidebar_rows(&options.apps).len() > sidebar_rows(&[]).len());
    }
}
