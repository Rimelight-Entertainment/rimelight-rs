//! Plain-Rust rules of the S7 Menu and ContextMenu: the visible rows, the next selectable row, the row of an id, the first
//! child row, the height of the rows and the width a menu fits. No Slint types here. The app expands [`wire_menu!`] to get
//! the `MenuNav` wiring and the Slint conversions, in the crate that generates the Slint types.

/// The kind of a list entry, as the rules see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Item,
    Separator,
    Heading,
    Submenu,
}

/// One list entry: its id, kind, whether it is disabled, and the id of its submenu ("" for a top-level entry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    pub disabled: bool,
    pub parent: String,
}

/// One visible row: the index of its entry, its depth (0 top level, 1 a child of an expanded submenu) and whether it is an
/// expanded submenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub index: i32,
    pub depth: i32,
    pub expanded: bool,
}

/// Whether a row can be highlighted and chosen: an item or a submenu that is not disabled.
pub fn selectable(entry: &Entry) -> bool {
    matches!(entry.kind, Kind::Item | Kind::Submenu) && !entry.disabled
}

/// The visible rows: every top-level entry in order, and the children of the submenu whose id is `expanded` straight after it.
pub fn flatten(items: &[Entry], expanded: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for (index, entry) in items.iter().enumerate() {
        if !entry.parent.is_empty() {
            continue;
        }
        let is_open = entry.kind == Kind::Submenu && !expanded.is_empty() && entry.id == expanded;
        rows.push(Row {
            index: index as i32,
            depth: 0,
            expanded: is_open,
        });
        if is_open {
            for (child, item) in items.iter().enumerate() {
                if item.parent == entry.id {
                    rows.push(Row {
                        index: child as i32,
                        depth: 1,
                        expanded: false,
                    });
                }
            }
        }
    }
    rows
}

/// The next selectable row from `from` in direction `dir` (1 down, -1 up). Non-selectable rows are skipped and the search does
/// not wrap: at either end it returns `from`. `from` is -1 when nothing is highlighted yet.
pub fn next_row(items: &[Entry], rows: &[Row], from: i32, dir: i32) -> i32 {
    let n = rows.len() as i32;
    if n == 0 || dir == 0 {
        return from;
    }
    let mut at = if from < 0 {
        if dir > 0 { -1 } else { n }
    } else {
        from.min(n)
    };
    loop {
        at += dir;
        if at < 0 || at >= n {
            return from;
        }
        if selectable(&items[rows[at as usize].index as usize]) {
            return at;
        }
    }
}

/// The row of the entry with this id, or -1.
pub fn row_of_id(items: &[Entry], rows: &[Row], id: &str) -> i32 {
    rows.iter()
        .position(|row| items[row.index as usize].id == id)
        .map_or(-1, |at| at as i32)
}

/// The first selectable child directly under the submenu row `parent_row`, or -1. Children follow their submenu row.
pub fn first_child_row(items: &[Entry], rows: &[Row], parent_row: i32) -> i32 {
    if parent_row < 0 || parent_row as usize >= rows.len() {
        return -1;
    }
    let parent_id = &items[rows[parent_row as usize].index as usize].id;
    for (at, row) in rows.iter().enumerate().skip(parent_row as usize + 1) {
        let entry = &items[row.index as usize];
        if row.depth == 0 || entry.parent != *parent_id {
            return -1;
        }
        if selectable(entry) {
            return at as i32;
        }
    }
    -1
}

/// The height of the rows: separators take `rule_height`, headings three quarters of `item_height`, everything else `item_height`.
pub fn list_height(items: &[Entry], rows: &[Row], item_height: f32, rule_height: f32) -> f32 {
    rows.iter()
        .map(|row| match items[row.index as usize].kind {
            Kind::Separator => rule_height,
            Kind::Heading => item_height * 0.75,
            Kind::Item | Kind::Submenu => item_height,
        })
        .sum()
}

/// What the width estimate needs from one entry: its kind, the length of its label and of its shortcut (in characters),
/// and whether it is a child row (indented).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidthEntry {
    pub kind: Kind,
    pub label_chars: usize,
    pub child: bool,
    pub shortcut_chars: usize,
}

/// The width the widest row needs, before the owner clamps it: the label (plus the leading slot, the submenu chevron and the
/// indent of a child) and the shortcut text (plus its gap), plus `chrome`. The text widths are estimated from the character
/// count with an average advance per em, which is close enough for a menu that is then clamped to its own maximum.
pub fn fit_width(
    entries: &[WidthEntry],
    leading: bool,
    label_size: f32,
    shortcut_size: f32,
    slot: f32,
    chrome: f32,
    indent: f32,
) -> f32 {
    // Average advance of the body and caption faces, in em.
    const LABEL_EM: f32 = 0.56;
    const SHORTCUT_EM: f32 = 0.62;
    let mut widest = 0.0_f32;
    for entry in entries {
        if matches!(entry.kind, Kind::Separator | Kind::Heading) {
            continue;
        }
        let mut width = entry.label_chars as f32 * label_size * LABEL_EM;
        if leading {
            width += slot;
        }
        if entry.kind == Kind::Submenu {
            width += slot;
        }
        if entry.child {
            width += indent;
        }
        if entry.shortcut_chars > 0 {
            width += entry.shortcut_chars as f32 * shortcut_size * SHORTCUT_EM;
        }
        widest = widest.max(width);
    }
    widest + chrome
}

/// Emits, in the calling module, the Slint side of the Menu rules: `to_slint_row`, `slint_rows`, `entries`, `has_leading`,
/// `fit_width` and `wire_nav`, which answers the `MenuNav` callbacks with the rules above. The arguments are the app's
/// generated types: the item struct (fields `id`, `kind`, `disabled`, `parent`, `label`, `shortcut`, `icon`, `checked`),
/// the row struct (fields `index`, `depth`, `expanded`), the `MenuNav` global, the `MenuKind` enum and the `MenuChecked`
/// enum. Expand it in the app crate, then call `wire_nav` once per window that shows a Menu or ContextMenu.
#[macro_export]
macro_rules! wire_menu {
    ($item:path, $row:path, $nav:path, $kind:path, $checked:path $(,)?) => {
        /// One plain row as the Slint row.
        pub fn to_slint_row(row: $crate::menu::Row) -> $row {
            $row {
                index: row.index,
                depth: row.depth,
                expanded: row.expanded,
            }
        }

        /// The Slint rows as the rules see them.
        pub fn slint_rows(rows: &::slint::ModelRc<$row>) -> ::std::vec::Vec<$crate::menu::Row> {
            use ::slint::Model as _;
            rows.iter()
                .map(|row| $crate::menu::Row {
                    index: row.index,
                    depth: row.depth,
                    expanded: row.expanded,
                })
                .collect()
        }

        /// The Slint kind as the rules see it.
        fn kind_of(kind: $kind) -> $crate::menu::Kind {
            use $kind as __Kind;
            match kind {
                __Kind::Item => $crate::menu::Kind::Item,
                __Kind::Separator => $crate::menu::Kind::Separator,
                __Kind::Heading => $crate::menu::Kind::Heading,
                __Kind::Submenu => $crate::menu::Kind::Submenu,
            }
        }

        /// The Slint items as the rules see them.
        pub fn entries(items: &::slint::ModelRc<$item>) -> ::std::vec::Vec<$crate::menu::Entry> {
            use ::slint::Model as _;
            items
                .iter()
                .map(|item| $crate::menu::Entry {
                    id: item.id.to_string(),
                    kind: kind_of(item.kind),
                    disabled: item.disabled,
                    parent: item.parent.to_string(),
                })
                .collect()
        }

        /// True when some item has an icon or a check mark, so the list keeps one leading column for all its rows.
        pub fn has_leading(items: &::slint::ModelRc<$item>) -> bool {
            use ::slint::Model as _;
            use $checked as __Checked;
            items
                .iter()
                .any(|item| item.icon.size().width > 0 || item.checked != __Checked::None)
        }

        /// The width the widest row needs, from the Slint items (see `$crate::menu::fit_width`).
        pub fn fit_width(
            items: &::slint::ModelRc<$item>,
            leading: bool,
            label_size: f32,
            shortcut_size: f32,
            slot: f32,
            chrome: f32,
            indent: f32,
        ) -> f32 {
            use ::slint::Model as _;
            let widths: ::std::vec::Vec<$crate::menu::WidthEntry> = items
                .iter()
                .map(|item| $crate::menu::WidthEntry {
                    kind: kind_of(item.kind),
                    label_chars: item.label.chars().count(),
                    child: !item.parent.is_empty(),
                    shortcut_chars: item.shortcut.chars().count(),
                })
                .collect();
            $crate::menu::fit_width(
                &widths,
                leading,
                label_size,
                shortcut_size,
                slot,
                chrome,
                indent,
            )
        }

        /// Wires the `MenuNav` callbacks of one window to the rules.
        pub fn wire_nav(nav: &$nav) {
            nav.on_flatten(|items, expanded| {
                let rows = $crate::menu::flatten(&entries(&items), expanded.as_str());
                ::slint::ModelRc::new(::slint::VecModel::from(
                    rows.into_iter()
                        .map(to_slint_row)
                        .collect::<::std::vec::Vec<_>>(),
                ))
            });
            nav.on_next_row(|items, rows, from, dir| {
                $crate::menu::next_row(&entries(&items), &slint_rows(&rows), from, dir)
            });
            nav.on_row_of_id(|items, rows, id| {
                $crate::menu::row_of_id(&entries(&items), &slint_rows(&rows), id.as_str())
            });
            nav.on_first_child_row(|items, rows, parent| {
                $crate::menu::first_child_row(&entries(&items), &slint_rows(&rows), parent)
            });
            nav.on_has_leading(|items| has_leading(&items));
            nav.on_fit_width(
                |items, leading, label_size, shortcut_size, slot, chrome, indent| {
                    fit_width(
                        &items,
                        leading,
                        label_size,
                        shortcut_size,
                        slot,
                        chrome,
                        indent,
                    )
                },
            );
            nav.on_list_height(|items, rows, item_height, rule_height| {
                $crate::menu::list_height(
                    &entries(&items),
                    &slint_rows(&rows),
                    item_height,
                    rule_height,
                )
            });
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, disabled: bool) -> Entry {
        Entry {
            id: id.into(),
            kind: Kind::Item,
            disabled,
            parent: String::new(),
        }
    }

    fn of_kind(id: &str, kind: Kind) -> Entry {
        Entry {
            id: id.into(),
            kind,
            disabled: false,
            parent: String::new(),
        }
    }

    fn child(id: &str, parent: &str, disabled: bool) -> Entry {
        Entry {
            id: id.into(),
            kind: Kind::Item,
            disabled,
            parent: parent.into(),
        }
    }

    /// Undo (disabled), Redo, a rule, a heading, More (submenu) with Copy and Print (disabled), Delete.
    fn sample() -> Vec<Entry> {
        vec![
            item("undo", true),
            item("redo", false),
            of_kind("rule", Kind::Separator),
            of_kind("heading", Kind::Heading),
            of_kind("more", Kind::Submenu),
            child("copy", "more", false),
            child("print", "more", true),
            item("delete", false),
        ]
    }

    #[test]
    fn collapsed_rows_are_the_top_level_entries() {
        let items = sample();
        let rows = flatten(&items, "");
        let indices: Vec<i32> = rows.iter().map(|r| r.index).collect();
        assert_eq!(
            indices,
            vec![0, 1, 2, 3, 4, 7],
            "children hidden while collapsed"
        );
    }

    #[test]
    fn expanded_submenu_shows_its_children_under_it() {
        let items = sample();
        let rows = flatten(&items, "more");
        let indices: Vec<i32> = rows.iter().map(|r| r.index).collect();
        assert_eq!(indices, vec![0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(rows[4].expanded, "the submenu row is marked expanded");
        assert_eq!(rows[5].depth, 1);
    }

    #[test]
    fn first_press_on_open_skips_the_disabled_first_item() {
        let items = sample();
        let rows = flatten(&items, "");
        assert_eq!(
            next_row(&items, &rows, -1, 1),
            1,
            "Undo is disabled: the first highlighted row is Redo"
        );
    }

    #[test]
    fn down_and_up_skip_disabled_rules_and_headings() {
        let items = sample();
        let rows = flatten(&items, "");
        assert_eq!(
            next_row(&items, &rows, 1, 1),
            4,
            "rule and heading are skipped"
        );
        assert_eq!(next_row(&items, &rows, 4, -1), 1, "back over them");
        assert_eq!(next_row(&items, &rows, 1, -1), 1, "no wrap at the top");
        assert_eq!(next_row(&items, &rows, 5, 1), 5, "no wrap at the bottom");
    }

    #[test]
    fn end_and_home_land_on_selectable_rows() {
        let items = sample();
        let rows = flatten(&items, "");
        assert_eq!(
            next_row(&items, &rows, rows.len() as i32, -1),
            5,
            "End: Delete"
        );
        assert_eq!(next_row(&items, &rows, -1, 1), 1, "Home: Redo");
    }

    #[test]
    fn first_child_skips_a_disabled_child() {
        let items = sample();
        let rows = flatten(&items, "more");
        assert_eq!(
            first_child_row(&items, &rows, 4),
            5,
            "Copy is the first child"
        );
        let mut only_disabled = sample();
        only_disabled[5].disabled = true;
        let rows = flatten(&only_disabled, "more");
        assert_eq!(
            first_child_row(&only_disabled, &rows, 4),
            -1,
            "Copy and Print are both disabled: no selectable child"
        );
    }

    #[test]
    fn row_of_id_finds_the_submenu_row() {
        let items = sample();
        let rows = flatten(&items, "more");
        assert_eq!(row_of_id(&items, &rows, "more"), 4);
        assert_eq!(row_of_id(&items, &rows, "nope"), -1);
        let collapsed = flatten(&items, "");
        assert_eq!(
            row_of_id(&items, &collapsed, "copy"),
            -1,
            "hidden children are not rows"
        );
    }

    #[test]
    fn height_counts_rules_and_headings_at_their_own_size() {
        let items = sample();
        let rows = flatten(&items, "");
        // Undo, Redo, More and Delete are rows of 28; the rule is 9; the heading is 0.75 * 28 = 21.
        let height = list_height(&items, &rows, 28.0, 9.0);
        assert!(
            (height - (4.0 * 28.0 + 9.0 + 21.0)).abs() < 0.01,
            "got {height}"
        );
    }
}
