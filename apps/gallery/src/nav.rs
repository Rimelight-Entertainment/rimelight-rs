//! The gallery's Rust side on the interpreter: the `SelectNav`, `MenuNav` and `ColorHex` callbacks, the command palette and
//! the shell story. The rules come from `rimelight_ui_kit` (`select`, `menu`, `color`) and `rimelight_shell` (`palette`,
//! `badge`). This file converts the interpreter's values to the rules' plain types and the results back to values. The
//! sample data the palette and the shell show is in [`crate::sample`].

use rimelight_shell::{badge, palette};
use rimelight_ui_kit::{color, menu, select};
use slint::{ComponentHandle as _, Image};
use slint_interpreter::{ComponentInstance, Value};

use crate::sample;
use crate::values::{
    enumeration, field, flag, image, int, length, list, number, record, rows, text, variant_name,
};

fn on_global(
    ui: &ComponentInstance,
    global: &str,
    name: &str,
    callback: impl Fn(&[Value]) -> Value + 'static,
) {
    if let Err(e) = ui.set_global_callback(global, name, callback) {
        panic!("{global}.{name}: {e}");
    }
}

fn on(ui: &ComponentInstance, name: &str, callback: impl Fn(&[Value]) -> Value + 'static) {
    if let Err(e) = ui.set_callback(name, callback) {
        panic!("gallery callback {name}: {e}");
    }
}

fn set(ui: &ComponentInstance, name: &str, value: Value) {
    if let Err(e) = ui.set_property(name, value) {
        panic!("gallery property {name}: {e}");
    }
}

fn arg(args: &[Value], index: usize) -> Value {
    args.get(index).cloned().unwrap_or_default()
}

// Select (S6).

/// The Slint options (the markup's `SelectOption` values) as the rules see them.
fn select_entries(value: &Value) -> Vec<select::Entry> {
    rows(value)
        .iter()
        .map(|option| select::Entry {
            value: text(field(option, "value")).to_string(),
            label: text(field(option, "label")).to_string(),
            disabled: flag(field(option, "disabled")),
            group: text(field(option, "group")).to_string(),
        })
        .collect()
}

/// One option value rebuilt from its fields, as the markup reads it (the six fields of `SelectOption`).
fn select_option_copy(option: &Value) -> Value {
    record([
        ("value", Value::String(text(field(option, "value")))),
        ("label", Value::String(text(field(option, "label")))),
        ("icon", Value::Image(image(field(option, "icon")))),
        ("badge", Value::String(text(field(option, "badge")))),
        ("disabled", Value::Bool(flag(field(option, "disabled")))),
        ("group", Value::String(text(field(option, "group")))),
    ])
}

/// One option as a value (the gallery test builds a list with it).
#[cfg(test)]
pub fn select_option_value(value: &str, label: &str, disabled: bool) -> Value {
    record([
        ("value", Value::String(value.into())),
        ("label", Value::String(label.into())),
        ("icon", Value::Image(Image::default())),
        ("badge", Value::String("".into())),
        ("disabled", Value::Bool(disabled)),
        ("group", Value::String("".into())),
    ])
}

/// The `SelectNav` callbacks, answered by `rimelight_ui_kit::select`.
pub fn wire_select(ui: &ComponentInstance) {
    const NAV: &str = "SelectNav";
    on_global(ui, NAV, "next-enabled", |args| {
        let options = select_entries(&arg(args, 0));
        Value::Number(select::next_enabled(&options, int(arg(args, 1)), int(arg(args, 2))).into())
    });
    on_global(ui, NAV, "typeahead", |args| {
        let options = select_entries(&arg(args, 0));
        let typed = text(arg(args, 2));
        Value::Number(select::typeahead(&options, int(arg(args, 1)), typed.as_str()).into())
    });
    on_global(ui, NAV, "heading-count", |args| {
        let options = select_entries(&arg(args, 0));
        Value::Number(select::heading_count(&options).into())
    });
    on_global(ui, NAV, "index-of", |args| {
        let options = select_entries(&arg(args, 0));
        let value = text(arg(args, 1));
        Value::Number(select::index_of(&options, value.as_str()).into())
    });
    on_global(ui, NAV, "filter-options", |args| {
        let query = text(arg(args, 1));
        let kept: Vec<Value> = rows(&arg(args, 0))
            .iter()
            .filter(|option| {
                select::matches_query(text(field(option, "label")).as_str(), query.as_str())
            })
            .map(select_option_copy)
            .collect();
        list(kept)
    });
}

// ColorPicker's Hex field (`ColorHex.parse-hex`).

/// The `ColorHex` callback, answered by `rimelight_ui_kit::color`.
pub fn wire_colour(ui: &ComponentInstance) {
    on_global(ui, "ColorHex", "parse-hex", |args| {
        let colour = color::parse_colour(text(arg(args, 0)).as_str());
        Value::Brush(slint::Brush::SolidColor(colour))
    });
}

// Menu and ContextMenu (S7).

/// The kind of a menu item value (the markup's `MenuKind` variant name). Anything else is an item.
fn menu_kind(name: &str) -> menu::Kind {
    match name {
        "separator" => menu::Kind::Separator,
        "heading" => menu::Kind::Heading,
        "submenu" => menu::Kind::Submenu,
        _ => menu::Kind::Item,
    }
}

/// The Slint items (the markup's `MenuItem` values) as the rules see them.
fn menu_entries(value: &Value) -> Vec<menu::Entry> {
    rows(value)
        .iter()
        .map(|item| menu::Entry {
            id: text(field(item, "id")).to_string(),
            kind: menu_kind(&variant_name(field(item, "kind"))),
            disabled: flag(field(item, "disabled")),
            parent: text(field(item, "parent")).to_string(),
        })
        .collect()
}

/// The Slint items as the width estimate sees them.
fn width_entries(value: &Value) -> Vec<menu::WidthEntry> {
    rows(value)
        .iter()
        .map(|item| menu::WidthEntry {
            kind: menu_kind(&variant_name(field(item, "kind"))),
            label_chars: text(field(item, "label")).chars().count(),
            child: !text(field(item, "parent")).is_empty(),
            shortcut_chars: text(field(item, "shortcut")).chars().count(),
        })
        .collect()
}

/// The visible rows the markup passes back (`MenuRow` values) as the rules see them.
fn menu_rows(value: &Value) -> Vec<menu::Row> {
    rows(value)
        .iter()
        .map(|row| menu::Row {
            index: int(field(row, "index")),
            depth: int(field(row, "depth")),
            expanded: flag(field(row, "expanded")),
        })
        .collect()
}

/// A visible row as the markup's `MenuRow` value.
fn menu_row_value(row: menu::Row) -> Value {
    record([
        ("index", Value::Number(row.index.into())),
        ("depth", Value::Number(row.depth.into())),
        ("expanded", Value::Bool(row.expanded)),
    ])
}

/// True when some item has an icon or a check mark, so the list keeps one leading column for all its rows.
fn has_leading(items: &Value) -> bool {
    rows(items).iter().any(|item| {
        image(field(item, "icon")).size().width > 0
            || matches!(variant_name(field(item, "checked")).as_str(), "on" | "off")
    })
}

/// The `MenuNav` callbacks, answered by `rimelight_ui_kit::menu`.
pub fn wire_menu(ui: &ComponentInstance) {
    const NAV: &str = "MenuNav";
    on_global(ui, NAV, "flatten", |args| {
        let items = menu_entries(&arg(args, 0));
        let expanded = text(arg(args, 1));
        list(
            menu::flatten(&items, expanded.as_str())
                .into_iter()
                .map(menu_row_value)
                .collect(),
        )
    });
    on_global(ui, NAV, "next-row", |args| {
        let items = menu_entries(&arg(args, 0));
        let visible = menu_rows(&arg(args, 1));
        let next = menu::next_row(&items, &visible, int(arg(args, 2)), int(arg(args, 3)));
        Value::Number(next.into())
    });
    on_global(ui, NAV, "row-of-id", |args| {
        let items = menu_entries(&arg(args, 0));
        let visible = menu_rows(&arg(args, 1));
        let id = text(arg(args, 2));
        Value::Number(menu::row_of_id(&items, &visible, id.as_str()).into())
    });
    on_global(ui, NAV, "first-child-row", |args| {
        let items = menu_entries(&arg(args, 0));
        let visible = menu_rows(&arg(args, 1));
        Value::Number(menu::first_child_row(&items, &visible, int(arg(args, 2))).into())
    });
    on_global(ui, NAV, "list-height", |args| {
        let items = menu_entries(&arg(args, 0));
        let visible = menu_rows(&arg(args, 1));
        let height =
            menu::list_height(&items, &visible, length(arg(args, 2)), length(arg(args, 3)));
        Value::Number(height.into())
    });
    on_global(ui, NAV, "has-leading", |args| {
        Value::Bool(has_leading(&arg(args, 0)))
    });
    on_global(ui, NAV, "fit-width", |args| {
        let width = menu::fit_width(
            &width_entries(&arg(args, 0)),
            flag(arg(args, 1)),
            length(arg(args, 2)),
            length(arg(args, 3)),
            length(arg(args, 4)),
            length(arg(args, 5)),
            length(arg(args, 6)),
        );
        Value::Number(width.into())
    });
}

// Command palette (S9.3).

fn palette_value(entry: &palette::Entry) -> Value {
    record([
        ("title", Value::String(entry.title.into())),
        ("route", Value::String(entry.route.as_str().into())),
    ])
}

/// The palette rows for a query: the entries it matches, in order.
fn palette_rows(query: &str, all: &[palette::Entry]) -> Vec<Value> {
    palette::filter(query, all)
        .into_iter()
        .map(|index| palette_value(&all[index]))
        .collect()
}

/// Fills the palette with the sample pages and answers its callbacks: a query refilters and resets the highlight to the
/// first row, Up and Down move the highlight, a pick (Enter or a click) and Escape report the result in `palette-result`.
pub fn wire_palette(ui: &ComponentInstance) {
    let all = sample::palette_entries();
    set(ui, "palette-rows", list(palette_rows("", &all)));
    set(ui, "palette-highlight", number(0));

    let weak = ui.as_weak();
    on(ui, "palette-query-changed", move |args| {
        if let Some(ui) = weak.upgrade() {
            let rows = palette_rows(text(arg(args, 0)).as_str(), &all);
            let highlight = if rows.is_empty() { -1 } else { 0 };
            set(&ui, "palette-rows", list(rows));
            set(&ui, "palette-highlight", Value::Number(highlight.into()));
        }
        Value::Void
    });

    let weak = ui.as_weak();
    on(ui, "palette-move", move |args| {
        if let Some(ui) = weak.upgrade() {
            let count = rows(&ui.get_property("palette-rows").unwrap_or_default()).len();
            let highlight = int(ui.get_property("palette-highlight").unwrap_or_default());
            let next = palette::step(highlight, count, int(arg(args, 0)));
            set(&ui, "palette-highlight", Value::Number(next.into()));
        }
        Value::Void
    });

    let weak = ui.as_weak();
    on(ui, "palette-pick", move |args| {
        if let Some(ui) = weak.upgrade() {
            // A pick on no row (an empty result, or a stale index) changes nothing.
            let shown = rows(&ui.get_property("palette-rows").unwrap_or_default());
            if let Some(row) = usize::try_from(int(arg(args, 0)))
                .ok()
                .and_then(|i| shown.get(i))
            {
                let route = text(field(row, "route"));
                set(
                    &ui,
                    "palette-result",
                    Value::String(format!("Opened {route}").into()),
                );
            }
        }
        Value::Void
    });

    let weak = ui.as_weak();
    on(ui, "palette-close", move |_| {
        if let Some(ui) = weak.upgrade() {
            set(&ui, "palette-result", Value::String("Closed".into()));
        }
        Value::Void
    });
}

// Shell story (S9.1).

/// The Lucide icon `name` from the window's own `Icons` global. An unknown name draws nothing.
fn icon(ui: &ComponentInstance, name: &str) -> Image {
    ui.get_global_property("Icons", name)
        .map(image)
        .unwrap_or_default()
}

/// One sample sidebar row as the markup's `SidebarItem` value. A heading has no icon and is expanded.
fn sidebar_row(ui: &ComponentInstance, row: &sample::Row) -> Value {
    let heading = row.icon.is_none();
    let picture = row.icon.map(|name| icon(ui, name)).unwrap_or_default();
    record([
        ("label", Value::String(row.label.into())),
        ("header", Value::Bool(heading)),
        ("icon", Value::Image(picture)),
        (
            "kind",
            enumeration("SidebarKind", if heading { "heading" } else { "page" }),
        ),
        ("expanded", Value::Bool(heading)),
        ("count", Value::Number(row.count.into())),
        ("status", enumeration("SidebarStatus", "none")),
        ("bold", Value::Bool(false)),
        ("leading", enumeration("SidebarLeading", "none")),
        ("image", Value::Image(Image::default())),
        ("initials", Value::String("".into())),
        ("private", Value::Bool(false)),
        ("people", Value::Number(0.0)),
        ("indented", Value::Bool(false)),
    ])
}

/// Fills the rail and its badges from the sample areas, shows the first area (or Projects for the `projects` variant),
/// and wires the rail click to [`show_area`].
pub fn wire_shell(ui: &ComponentInstance, variant: usize) {
    let areas = sample::areas();
    let rail = areas
        .iter()
        .map(|area| {
            record([
                ("label", Value::String(area.title.into())),
                ("icon", Value::Image(icon(ui, area.icon))),
            ])
        })
        .collect();
    set(ui, "shell-rail", list(rail));
    // Mock counts per area; an area without a work queue has no badge (count 0, empty text and noun).
    let badges = areas
        .iter()
        .map(|area| match area.noun {
            Some(noun) => record([
                ("count", number(area.count as usize)),
                ("text", Value::String(badge::badge_text(area.count).into())),
                ("noun", Value::String(noun.into())),
            ]),
            None => record([
                ("count", number(0)),
                ("text", Value::String("".into())),
                ("noun", Value::String("".into())),
            ]),
        })
        .collect();
    set(ui, "shell-badges", list(badges));

    let start = if variant == 1 {
        areas
            .iter()
            .position(|a| a.title == "Projects")
            .unwrap_or(0)
    } else {
        0
    };
    show_area(ui, start);

    let weak = ui.as_weak();
    on(ui, "shell-area-picked", move |args| {
        if let Some(ui) = weak.upgrade() {
            show_area(&ui, int(arg(args, 0)).max(0) as usize);
        }
        Value::Void
    });
}

/// Shows area `index` (a rail index): its title and its sidebar rows. An index past the rail shows the first area.
pub fn show_area(ui: &ComponentInstance, index: usize) {
    let areas = sample::areas();
    let area = areas.get(index).unwrap_or(&areas[0]);
    set(ui, "shell-area", number(index));
    set(ui, "shell-title", Value::String(area.title.into()));
    let shown = area.rows.iter().map(|row| sidebar_row(ui, row)).collect();
    set(ui, "shell-rows", list(shown));
}
