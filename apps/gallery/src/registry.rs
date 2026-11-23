//! The story and variant registry for the component gallery (S2). Plain Rust, no Slint types, so its tests run without a
//! window.
//!
//! The built-in stories are [`STORIES`]. Each is one Slint component in `ui/stories.slint` or `ui/component-stories.slint`,
//! shown by `ui/gallery.slint` when `GalleryWindow.story` equals its index in [`STORIES`]. Keep the two orders equal: the
//! gallery test in `harness.rs` checks it.
//!
//! An app adds its own stories from a folder (`--stories <dir>`): [`load_app_stories`] reads `<dir>/stories.txt`. They
//! follow the built-in stories in the selection order, so app story N is at index `STORIES.len() + N`. The app's Slint
//! side is described in the crate docs (`lib.rs`).
//!
//! Story ids are `section/name`, and a variant is `section/name#variant`, for example `foundations/typography#weights`.

use std::path::Path;

/// The four sidebar groups, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Foundations,
    Components,
    Templates,
    Shell,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::Foundations,
        Section::Components,
        Section::Templates,
        Section::Shell,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Foundations => "Foundations",
            Section::Components => "Components",
            Section::Templates => "Templates",
            Section::Shell => "Shell",
        }
    }

    /// The id prefix of the section (`foundations`, ...).
    pub fn slug(self) -> &'static str {
        match self {
            Section::Foundations => "foundations",
            Section::Components => "components",
            Section::Templates => "templates",
            Section::Shell => "shell",
        }
    }
}

/// An action the gallery runs after it shows a variant, so a capture can show a state (for example a focus ring).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterRender {
    None,
    /// Gives keyboard focus to the first toolbar chip, so its focus ring is drawn.
    FocusFirstChip,
    /// Gives keyboard focus to the sidebar, so Up and Down are live.
    FocusSidebar,
    /// Opens the story popover after its first layout (the Popover panel is placed from its trigger when it is shown).
    OpenPopover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: &'static str,
    pub after_render: AfterRender,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Story {
    /// `section/name`, unique.
    pub id: &'static str,
    pub title: &'static str,
    pub section: Section,
    /// The first variant is the default. Never empty.
    pub variants: &'static [Variant],
}

const DEFAULT: Variant = Variant {
    name: "default",
    after_render: AfterRender::None,
};

/// Every story, in the order of the Slint `if root.story == N` chain in `ui/gallery.slint`.
pub const STORIES: &[Story] = &[
    Story {
        id: "foundations/tokens",
        title: "Tokens",
        section: Section::Foundations,
        variants: &[
            DEFAULT,
            Variant {
                name: "focus",
                after_render: AfterRender::FocusFirstChip,
            },
        ],
    },
    Story {
        id: "foundations/typography",
        title: "Typography",
        section: Section::Foundations,
        variants: &[
            DEFAULT,
            Variant {
                name: "weights",
                after_render: AfterRender::None,
            },
            Variant {
                name: "mono",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "foundations/icons",
        title: "Icons",
        section: Section::Foundations,
        variants: &[DEFAULT],
    },
    Story {
        id: "foundations/spacing-and-radius",
        title: "Spacing and radius",
        section: Section::Foundations,
        variants: &[DEFAULT],
    },
    Story {
        id: "foundations/motion",
        title: "Motion",
        section: Section::Foundations,
        variants: &[DEFAULT],
    },
    Story {
        id: "foundations/about",
        title: "About",
        section: Section::Foundations,
        variants: &[
            DEFAULT,
            Variant {
                name: "sidebar-focus",
                after_render: AfterRender::FocusSidebar,
            },
        ],
    },
    Story {
        id: "components/button",
        title: "Button",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/icon-button",
        title: "Icon button",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/button-group",
        title: "Button group",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/link",
        title: "Link",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/badge",
        title: "Badge",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/tag",
        title: "Tag",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/kbd",
        title: "Kbd",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/avatar",
        title: "Avatar",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/avatar-stack",
        title: "Avatar stack",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/divider",
        title: "Divider",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/skeleton",
        title: "Skeleton",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/spinner",
        title: "Spinner",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/progress",
        title: "Progress",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/empty-state",
        title: "Empty state",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/error-state",
        title: "Error state",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/input",
        title: "Input",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/search-field",
        title: "SearchField",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/textarea",
        title: "Textarea",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/number-input",
        title: "NumberInput",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/field",
        title: "Field and Form",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/checkbox",
        title: "Checkbox",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/radio-group",
        title: "RadioGroup",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/switch",
        title: "Switch",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/slider",
        title: "Slider",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/select",
        title: "Select",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "open",
                after_render: AfterRender::None,
            },
            Variant {
                name: "flip",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "components/combobox",
        title: "Combobox",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "open",
                after_render: AfterRender::None,
            },
            Variant {
                name: "filtered",
                after_render: AfterRender::None,
            },
            Variant {
                name: "no-results",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "components/tooltip",
        title: "Tooltip",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "focus",
                after_render: AfterRender::None,
            },
            Variant {
                name: "bottom",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "components/popover",
        title: "Popover",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "open",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "flip",
                after_render: AfterRender::OpenPopover,
            },
        ],
    },
    Story {
        id: "components/menu",
        title: "Menu",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "open",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "submenu",
                after_render: AfterRender::OpenPopover,
            },
        ],
    },
    Story {
        id: "components/context-menu",
        title: "ContextMenu",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "open",
                after_render: AfterRender::OpenPopover,
            },
        ],
    },
    Story {
        id: "components/dialog",
        title: "Dialog",
        section: Section::Components,
        variants: &[
            Variant {
                name: "open",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "confirm-danger",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "not-dismissible",
                after_render: AfterRender::OpenPopover,
            },
        ],
    },
    Story {
        id: "components/sheet",
        title: "Sheet",
        section: Section::Components,
        variants: &[
            Variant {
                name: "right",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "left",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "bottom",
                after_render: AfterRender::OpenPopover,
            },
        ],
    },
    Story {
        id: "components/image-viewer",
        title: "ImageViewer",
        section: Section::Components,
        variants: &[
            Variant {
                name: "open",
                after_render: AfterRender::OpenPopover,
            },
            Variant {
                name: "single",
                after_render: AfterRender::OpenPopover,
            },
        ],
    },
    Story {
        id: "components/attachment-grid",
        title: "AttachmentGrid",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/toast",
        title: "Toast",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "stack",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "components/banner",
        title: "Banner",
        section: Section::Components,
        variants: &[
            DEFAULT,
            Variant {
                name: "options",
                after_render: AfterRender::None,
            },
            Variant {
                name: "compact",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "components/card",
        title: "Card",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/section",
        title: "Section",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/status-dot",
        title: "Status dot",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/presence-dot",
        title: "Presence dot",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/breadcrumbs",
        title: "Breadcrumbs",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/tabs",
        title: "Tabs",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/accordion",
        title: "Accordion",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/stepper",
        title: "Stepper",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/pagination",
        title: "Pagination",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/toolbar",
        title: "Toolbar",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/filter-bar",
        title: "Filter bar",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/tree",
        title: "Tree",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/scroll-area",
        title: "Scroll area",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/resizable-panel",
        title: "Resizable panel",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/description-list",
        title: "Description list",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/stat",
        title: "Stat",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/timeline",
        title: "Timeline",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/activity-feed",
        title: "Activity feed",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/list",
        title: "List",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/table",
        title: "Table",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "shell/rail-sidebar",
        title: "Rail and sidebar",
        section: Section::Shell,
        // `projects` starts on the Projects area, the longest sidebar, so the list overflows and scrolls.
        variants: &[
            DEFAULT,
            Variant {
                name: "projects",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "shell/tabs-split",
        title: "Tabs and split panes",
        section: Section::Shell,
        variants: &[DEFAULT],
    },
    Story {
        id: "shell/command-palette",
        title: "Command palette",
        section: Section::Shell,
        variants: &[DEFAULT],
    },
    Story {
        id: "shell/status-bar",
        title: "Status bar",
        section: Section::Shell,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/color-picker",
        title: "ColorPicker",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/toolbar-row",
        title: "ToolbarRow",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/action-row",
        title: "ActionRow",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/tile",
        title: "Tile",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "components/recent-list",
        title: "RecentList",
        section: Section::Components,
        variants: &[DEFAULT],
    },
    Story {
        id: "templates/list-page",
        title: "List page",
        section: Section::Templates,
        // The variants pick the page's state (loaded, loading, empty, error) and a selection of two rows.
        variants: &[
            DEFAULT,
            Variant {
                name: "loading",
                after_render: AfterRender::None,
            },
            Variant {
                name: "empty",
                after_render: AfterRender::None,
            },
            Variant {
                name: "error",
                after_render: AfterRender::None,
            },
            Variant {
                name: "selected",
                after_render: AfterRender::None,
            },
            Variant {
                name: "row-actions",
                after_render: AfterRender::None,
            },
            Variant {
                name: "no-title",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "templates/board-page",
        title: "Board page",
        section: Section::Templates,
        variants: &[
            DEFAULT,
            Variant {
                name: "empty",
                after_render: AfterRender::None,
            },
            Variant {
                name: "error",
                after_render: AfterRender::None,
            },
            Variant {
                name: "move-refused",
                after_render: AfterRender::None,
            },
            Variant {
                name: "rich-cards",
                after_render: AfterRender::None,
            },
            Variant {
                name: "drop-target",
                after_render: AfterRender::None,
            },
        ],
    },
    Story {
        id: "templates/detail-page",
        title: "Detail page",
        section: Section::Templates,
        variants: &[
            DEFAULT,
            Variant {
                name: "loading",
                after_render: AfterRender::None,
            },
            Variant {
                name: "empty",
                after_render: AfterRender::None,
            },
            Variant {
                name: "error",
                after_render: AfterRender::None,
            },
        ],
    },
];

/// The sidebar heading of the app stories.
pub const APP_LABEL: &str = "App stories";

/// A story an app adds from its `stories.txt`. Its variants are owned, and the first is always `default`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppStory {
    /// `section/name`, unique across the built-in and the app stories.
    pub id: String,
    pub title: String,
    /// The variant names, `default` first.
    pub variants: Vec<String>,
}

/// A resolved story and variant index, from [`resolve`]. `story` indexes the built-in stories, then the app stories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub story: usize,
    pub variant: usize,
}

/// Reads `<dir>/stories.txt` and parses it with [`parse_app_stories`].
pub fn load_app_stories(dir: &Path) -> Result<Vec<AppStory>, String> {
    let path = dir.join("stories.txt");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse_app_stories(&text)
}

/// Parses an app's story list: one story per line as `section/name|Title|variant1,variant2`, the variants optional. Blank
/// lines and lines starting with `#` are skipped. The section is one of the built-in section names in lower case
/// (`foundations`, `components`, `templates`, `shell`). The sidebar lists every app story under [`APP_LABEL`]. An error
/// names the line.
pub fn parse_app_stories(text: &str) -> Result<Vec<AppStory>, String> {
    let mut stories: Vec<AppStory> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fail = |message: String| format!("stories.txt line {}: {message}", index + 1);
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(fail(format!(
                "expected section/name|Title|variants, got {line:?}"
            )));
        }
        let id = parts[0];
        let Some((section, name)) = id.split_once('/') else {
            return Err(fail(format!("{id:?} is not section/name")));
        };
        if !Section::ALL.iter().any(|known| known.slug() == section) {
            return Err(fail(format!(
                "unknown section {section:?}: use foundations, components, templates or shell"
            )));
        }
        if name.is_empty() {
            return Err(fail(format!("{id:?} has no name after the section")));
        }
        if STORIES.iter().any(|s| s.id == id) || stories.iter().any(|s| s.id == id) {
            return Err(fail(format!("duplicate story id {id:?}")));
        }
        let title = parts[1];
        if title.is_empty() {
            return Err(fail(format!("story {id:?} has no title")));
        }
        let mut variants = vec!["default".to_string()];
        let listed = parts.get(2).into_iter().flat_map(|list| list.split(','));
        for variant in listed.map(str::trim).filter(|v| !v.is_empty()) {
            if variants.iter().any(|known| known == variant) {
                return Err(fail(format!("story {id:?} repeats variant {variant:?}")));
            }
            variants.push(variant.to_string());
        }
        stories.push(AppStory {
            id: id.to_string(),
            title: title.to_string(),
            variants,
        });
    }
    Ok(stories)
}

/// The number of stories with these app stories: every index below it is a story.
pub fn total(apps: &[AppStory]) -> usize {
    STORIES.len() + apps.len()
}

/// How many variants story `story` has, built-in or app. Zero past the last story.
pub fn variant_count(story: usize, apps: &[AppStory]) -> usize {
    match STORIES.get(story) {
        Some(built_in) => built_in.variants.len(),
        None => apps
            .get(story - STORIES.len())
            .map_or(0, |app| app.variants.len()),
    }
}

/// The action of a selection. Built-in variants have their own; an app variant has none.
pub fn after_render(selection: Selection) -> AfterRender {
    STORIES
        .get(selection.story)
        .and_then(|story| story.variants.get(selection.variant))
        .map_or(AfterRender::None, |variant| variant.after_render)
}

/// Resolves `section/name` or `section/name#variant` against the built-in stories, then the app stories. No variant means
/// the default. Errors name the bad part.
pub fn resolve(text: &str, apps: &[AppStory]) -> Result<Selection, String> {
    let (id, variant_name) = match text.split_once('#') {
        Some((id, variant)) => (id, Some(variant)),
        None => (text, None),
    };
    let story = match STORIES.iter().position(|s| s.id == id) {
        Some(built_in) => built_in,
        None => match apps.iter().position(|app| app.id == id) {
            Some(app) => STORIES.len() + app,
            None => return Err(format!("unknown story {id:?}; see --list-stories")),
        },
    };
    let variant = match variant_name {
        None => 0,
        Some(name) => {
            let found = if story < STORIES.len() {
                STORIES[story].variants.iter().position(|v| v.name == name)
            } else {
                apps[story - STORIES.len()]
                    .variants
                    .iter()
                    .position(|v| v == name)
            };
            found.ok_or_else(|| format!("story {id:?} has no variant {name:?}"))?
        }
    };
    Ok(Selection { story, variant })
}

/// Every selectable id, one line each: `section/name` and `section/name#variant` for each variant after the default. The
/// built-in stories come first, then the app stories.
pub fn listing(apps: &[AppStory]) -> Vec<String> {
    let mut lines = Vec::new();
    for story in STORIES {
        lines.push(story.id.to_string());
        for variant in &story.variants[1..] {
            lines.push(format!("{}#{}", story.id, variant.name));
        }
    }
    for app in apps {
        lines.push(app.id.clone());
        for variant in &app.variants[1..] {
            lines.push(format!("{}#{variant}", app.id));
        }
    }
    lines
}

/// Story indexes of one section, in registry order. Built-in stories only.
pub fn stories_in(section: Section) -> Vec<usize> {
    STORIES
        .iter()
        .enumerate()
        .filter(|(_, s)| s.section == section)
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_apps() -> Vec<AppStory> {
        parse_app_stories("shell/hello|Hello|one,two\ncomponents/extra|Extra\n").unwrap()
    }

    #[test]
    fn ids_are_unique_and_prefixed_by_their_section() {
        let mut seen = std::collections::BTreeSet::new();
        for story in STORIES {
            assert!(seen.insert(story.id), "duplicate id {}", story.id);
            assert!(
                story.id.starts_with(&format!("{}/", story.section.slug())),
                "{} must start with its section",
                story.id
            );
            assert!(!story.variants.is_empty(), "{} has no variants", story.id);
        }
    }

    #[test]
    fn foundations_has_the_six_stories_of_s2() {
        let ids: Vec<&str> = STORIES
            .iter()
            .filter(|s| s.section == Section::Foundations)
            .map(|s| s.id)
            .collect();
        assert_eq!(
            ids,
            [
                "foundations/tokens",
                "foundations/typography",
                "foundations/icons",
                "foundations/spacing-and-radius",
                "foundations/motion",
                "foundations/about",
            ]
        );
    }

    #[test]
    fn resolve_takes_a_default_and_named_variants() {
        assert_eq!(resolve("foundations/icons", &[]).unwrap().variant, 0);
        let weights = resolve("foundations/typography#weights", &[]).unwrap();
        assert_eq!(STORIES[weights.story].id, "foundations/typography");
        assert_eq!(
            STORIES[weights.story].variants[weights.variant].name,
            "weights"
        );
        assert!(resolve("foundations/nope", &[]).is_err());
        assert!(resolve("foundations/icons#nope", &[]).is_err());
    }

    #[test]
    fn listing_names_every_variant_and_resolves_back() {
        let apps = sample_apps();
        for line in listing(&apps) {
            resolve(&line, &apps).unwrap_or_else(|e| panic!("{line}: {e}"));
        }
        assert!(listing(&[]).contains(&"foundations/tokens#focus".to_string()));
    }

    #[test]
    fn app_stories_follow_the_built_ins_in_listing_and_resolve() {
        let apps = sample_apps();
        let lines = listing(&apps);
        assert_eq!(lines.last().map(String::as_str), Some("components/extra"));
        let hello = resolve("shell/hello#two", &apps).unwrap();
        assert_eq!(
            hello,
            Selection {
                story: STORIES.len(),
                variant: 2
            }
        );
        assert_eq!(variant_count(STORIES.len(), &apps), 3);
        assert_eq!(total(&apps), STORIES.len() + 2);
        assert_eq!(after_render(hello), AfterRender::None);
        assert!(resolve("shell/hello#nope", &apps).is_err());
    }

    #[test]
    fn app_story_lines_parse_with_comments_and_optional_variants() {
        let text = "# the app's stories\n\n  components/hello | Hello there | big, small \nshell/plain|Plain\n";
        let stories = parse_app_stories(text).unwrap();
        assert_eq!(stories.len(), 2);
        assert_eq!(stories[0].id, "components/hello");
        assert_eq!(stories[0].title, "Hello there");
        assert_eq!(stories[0].variants, ["default", "big", "small"]);
        assert_eq!(stories[1].variants, ["default"]);
    }

    #[test]
    fn bad_app_story_lines_name_the_problem() {
        let cases = [
            ("nope/x|X", "unknown section"),
            ("components/x", "expected section"),
            ("components/x|", "has no title"),
            ("components|X", "is not section/name"),
            ("components/|X", "has no name"),
            ("components/button|Button", "duplicate story id"),
            ("components/x|X|a,a", "repeats variant"),
            ("components/x|X|default", "repeats variant"),
            ("components/x|X|a|extra", "expected section"),
        ];
        for (text, want) in cases {
            let err = parse_app_stories(text).unwrap_err();
            assert!(err.contains(want), "{text:?}: {err}");
        }
    }

    #[test]
    fn a_missing_stories_file_is_an_error() {
        let dir = std::env::temp_dir().join("rimelight_gallery_no_such_folder_for_tests");
        let err = load_app_stories(&dir).unwrap_err();
        assert!(err.contains("cannot read"), "{err}");
    }

    #[test]
    fn the_registry_order_matches_the_markup_chain() {
        // ui/gallery.slint shows story N for `root.story == N`. Count its branches so a new story cannot skip the
        // markup.
        let markup = std::fs::read_to_string(
            std::path::Path::new(crate::harness::UI_DIR).join("gallery.slint"),
        )
        .expect("read ui/gallery.slint");
        let branches = markup.matches("if root.story == ").count();
        assert_eq!(
            branches,
            STORIES.len(),
            "ui/gallery.slint must show every built-in story"
        );
        let builtin_count = markup.contains("in-out property <int> builtin-count: 74;");
        assert!(
            builtin_count,
            "gallery.slint builtin-count must equal STORIES.len()"
        );
    }
}
