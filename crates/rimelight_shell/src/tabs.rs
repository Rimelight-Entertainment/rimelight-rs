//! The page tabs of one tile group (planned features A, A2b and B2; `PLAN.md section 6.6`). Plain Rust, always compiled, so
//! the tab rules run without a window. [`PageTabs`] holds the open page tabs, their pinned state and the closed stack. The
//! strip menu rows ([`StripMenu`], [`TabMenuRow`]) are the rows the window's tab menus show.
//!
//! The title of a route comes from the app's [`Routes`] (see [`crate::routes`]). A title is refreshed only when a route
//! changes by Back, Forward or a browser landing.

use std::marker::PhantomData;

use crate::history::History;
use crate::routes::Routes;

/// The most closed tabs kept for "Reopen closed tab" (planned feature A2b). The oldest is dropped when a close goes past this.
pub const CLOSED_CAP: usize = 10;

/// One open page tab: its back and forward history (the current route is `history.current()`), its title, and whether it
/// is pinned. A pinned tab sits in the pinned group at the left of the strip, narrow, and only plain Close or the middle
/// button closes it (the bulk closes skip it).
#[derive(Clone)]
pub struct PageTab {
    pub history: History,
    pub title: String,
    pub pinned: bool,
}

/// A closed tab on the closed stack: its history (the current route is `history.current()`) and its title. Reopening it
/// gives an unpinned tab with that history.
#[derive(Clone)]
pub struct ClosedTab {
    history: History,
    title: String,
}

/// The open page tabs and the active one (SF.4; IA section 6; planned features A and A2b). Plain Rust, so the rules run
/// without a window; the app adds the tab strip model. A split pane's tab is the active tab, so back and forward act on it.
/// Each tile group holds one of these (`tiles::Group`). A top-level workspace holds its groups in a `tiles::TileLayout`.
#[derive(Clone, Default)]
pub struct PageTabs<R: Routes> {
    pub pages: Vec<PageTab>,
    pub active: usize,
    /// The tabs closed most recently last, at most [`CLOSED_CAP`].
    pub closed: Vec<ClosedTab>,
    _routes: PhantomData<R>,
}

impl<R: Routes> PageTabs<R> {
    /// A tab set holding `pages`, with `active` as the active index and an empty closed stack. The tree and workspace text
    /// readers build their tabs with it.
    pub fn from_pages(pages: Vec<PageTab>, active: usize) -> Self {
        Self {
            pages,
            active,
            closed: Vec::new(),
            _routes: PhantomData,
        }
    }

    /// The title of the active tab, or `None` when no tab is open.
    pub fn active_title(&self) -> Option<&str> {
        self.pages.get(self.active).map(|t| t.title.as_str())
    }

    /// Activates the tab whose current route is `route`, opening it first when it is not open.
    pub fn open(&mut self, route: &str, title: &str) {
        self.active = match self.pages.iter().position(|t| t.history.current() == route) {
            Some(index) => index,
            None => {
                self.push_tab(route, title);
                self.pages.len() - 1
            }
        };
    }

    /// Opens a new tab for `route` and activates it, even when another tab shows the same route.
    pub fn open_new_tab(&mut self, route: &str, title: &str) {
        self.push_tab(route, title);
        self.active = self.pages.len() - 1;
    }

    /// Replaces the active tab's route and pushes it onto that tab's history. Opens a tab first when none is open.
    pub fn navigate(&mut self, route: &str, title: &str) {
        match self.pages.get_mut(self.active) {
            Some(tab) => {
                tab.history.push(route);
                tab.title = title.to_owned();
            }
            None => self.open(route, title),
        }
    }

    /// Lands the active tab on `route` after a browser Back or Forward: the cursor moves to the matching entry, or the route
    /// is added (see [`History::land_on`]). Opens a tab first when none is open.
    pub fn land(&mut self, route: &str, title: &str) {
        match self.pages.get_mut(self.active) {
            Some(tab) => {
                tab.history.land_on(route);
                refresh_title::<R>(tab, route);
            }
            None => self.open(route, title),
        }
    }

    /// Moves the active tab back one history step and returns its route, or `None` when there is nothing to go back to.
    pub fn back(&mut self) -> Option<String> {
        let tab = self.pages.get_mut(self.active)?;
        let route = tab.history.back()?.to_owned();
        refresh_title::<R>(tab, &route);
        Some(route)
    }

    /// Moves the active tab forward one history step and returns its route, or `None` when there is nothing to go to.
    pub fn forward(&mut self) -> Option<String> {
        let tab = self.pages.get_mut(self.active)?;
        let route = tab.history.forward()?.to_owned();
        refresh_title::<R>(tab, &route);
        Some(route)
    }

    /// True when the active tab can go back.
    pub fn can_back(&self) -> bool {
        self.pages
            .get(self.active)
            .is_some_and(|t| t.history.can_back())
    }

    /// True when the active tab can go forward.
    pub fn can_forward(&self) -> bool {
        self.pages
            .get(self.active)
            .is_some_and(|t| t.history.can_forward())
    }

    /// The current route of tab `index`, if it is open.
    pub fn route(&self, index: usize) -> Option<String> {
        self.pages
            .get(index)
            .map(|t| t.history.current().to_owned())
    }

    /// The route of the active tab, or `None` when the strip is empty.
    pub fn active_route(&self) -> Option<String> {
        self.route(self.active)
    }

    /// Makes tab `index` active. Returns false when there is no such tab.
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.pages.len() {
            self.active = index;
            true
        } else {
            false
        }
    }

    /// Closes tab `index`. Any tab can be closed, the last one included and a pinned one too. The tab goes onto the closed
    /// stack. Returns false when there is no tab `index`. Afterwards the active index points at a remaining tab, or is 0
    /// when none is left.
    pub fn close(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        let tab = self.pages.remove(index);
        self.remember(tab);
        let active = self.active;
        self.active = if self.pages.is_empty() {
            0
        } else if index < active {
            active - 1
        } else if index == active {
            index.min(self.pages.len() - 1)
        } else {
            active
        };
        true
    }

    pub fn push_tab(&mut self, route: &str, title: &str) {
        self.pages.push(PageTab {
            history: History::new(route),
            title: title.to_owned(),
            pinned: false,
        });
    }

    /// Puts a closed tab on the closed stack. The oldest entry goes when the stack is over [`CLOSED_CAP`].
    pub fn remember(&mut self, tab: PageTab) {
        self.closed.push(ClosedTab {
            history: tab.history,
            title: tab.title,
        });
        if self.closed.len() > CLOSED_CAP {
            self.closed.remove(0);
        }
    }

    /// The number of pinned tabs. They are the first tabs of the strip.
    pub fn pinned_count(&self) -> usize {
        self.pages.iter().filter(|tab| tab.pinned).count()
    }

    /// Closes every unpinned tab for which `doomed` holds (called with the tab's index), in tab order. Pinned tabs are never
    /// closed here. The active tab stays active while it is open. When it closes, `anchor` (the tab the menu was opened on)
    /// becomes active, or the first tab when there is no anchor.
    pub fn close_unpinned(&mut self, anchor: Option<usize>, doomed: impl Fn(usize) -> bool) {
        let active = self.active;
        let mut kept = Vec::with_capacity(self.pages.len());
        // The new index of each old tab, or None when it closed.
        let mut moved: Vec<Option<usize>> = Vec::with_capacity(self.pages.len());
        for (index, tab) in std::mem::take(&mut self.pages).into_iter().enumerate() {
            if !tab.pinned && doomed(index) {
                moved.push(None);
                self.remember(tab);
            } else {
                moved.push(Some(kept.len()));
                kept.push(tab);
            }
        }
        self.pages = kept;
        self.active = moved
            .get(active)
            .copied()
            .flatten()
            .or_else(|| anchor.and_then(|a| moved.get(a).copied().flatten()))
            .unwrap_or(0);
    }

    /// Closes the unpinned tabs to the left of `index`. Returns false when there is no tab `index`.
    pub fn close_left(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        self.close_unpinned(Some(index), |at| at < index);
        true
    }

    /// Closes the unpinned tabs to the right of `index`. Returns false when there is no tab `index`.
    pub fn close_right(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        self.close_unpinned(Some(index), |at| at > index);
        true
    }

    /// Closes every unpinned tab except `index`. Returns false when there is no tab `index`.
    pub fn close_others(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        self.close_unpinned(Some(index), |at| at != index);
        true
    }

    /// Closes every unpinned tab. The first tab is active afterwards when one is left.
    pub fn close_all(&mut self) {
        self.close_unpinned(None, |_| true);
    }

    /// Opens a copy of tab `index`: a new tab on the same route with the same title and a fresh history (only the route).
    /// It goes right after the tab, or after the pinned group when the tab is pinned, and it becomes the active tab. Returns
    /// false when there is no tab `index`.
    pub fn duplicate(&mut self, index: usize) -> bool {
        let Some(tab) = self.pages.get(index) else {
            return false;
        };
        let route = tab.history.current().to_owned();
        let title = tab.title.clone();
        let at = if tab.pinned {
            self.pinned_count()
        } else {
            index + 1
        };
        self.pages.insert(
            at,
            PageTab {
                history: History::new(&route),
                title,
                pinned: false,
            },
        );
        self.active = at;
        true
    }

    /// Pins or unpins tab `index`. A pinned tab moves to the end of the pinned group at the left; an unpinned one moves to
    /// just after the last pinned tab. The active tab stays the active one. Returns false when there is no tab `index`.
    pub fn set_pinned(&mut self, index: usize, pinned: bool) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        let was_active = self.active;
        let mut tab = self.pages.remove(index);
        tab.pinned = pinned;
        // After the removal, the pinned tabs left are the ones before the new position.
        let at = self.pinned_count();
        self.pages.insert(at, tab);
        self.active = if was_active == index {
            at
        } else {
            let shifted = if was_active > index {
                was_active - 1
            } else {
                was_active
            };
            if at <= shifted { shifted + 1 } else { shifted }
        };
        true
    }

    /// Reopens the last closed tab: it comes back unpinned at the end of the strip, with its history, and it becomes the
    /// active tab. Returns its route, or `None` when the closed stack is empty.
    pub fn reopen(&mut self) -> Option<String> {
        let closed = self.closed.pop()?;
        let route = closed.history.current().to_owned();
        self.pages.push(PageTab {
            history: closed.history,
            title: closed.title,
            pinned: false,
        });
        self.active = self.pages.len() - 1;
        Some(route)
    }

    /// The rows of the page tab menu for tab `index` (see [`StripMenu`]). `split` is the split rows' state, or `None` for no
    /// split rows.
    pub fn tab_menu_rows(&self, index: usize, split: Option<bool>) -> Vec<TabMenuRow> {
        let pinned: Vec<bool> = self.pages.iter().map(|tab| tab.pinned).collect();
        StripMenu {
            noun: "tab",
            pinned: &pinned,
            index,
            reopen_shortcut: reopen_shortcut("T"),
            reopen_enabled: !self.closed.is_empty(),
            rename: false,
            split,
            compact: false,
        }
        .rows()
    }
}

/// The shortcut text of "Reopen closed tab" or "Reopen closed workspace": Ctrl+Alt+Shift+`letter` (Cmd+Option+Shift on macOS).
pub fn reopen_shortcut(letter: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("Cmd+Option+Shift+{letter}")
    } else {
        format!("Ctrl+Alt+Shift+{letter}")
    }
}

/// The rows of a strip tab menu. The page tabs and the workspace strip share it (planned features A2b and B2). The actions
/// are in the owner's order, and an action that would do nothing is disabled. "Copy link" is enabled on every tab (gaps D2).
pub struct StripMenu<'a> {
    /// The name of one tab in the labels: "tab" or "workspace".
    pub noun: &'static str,
    /// The pinned flag of each tab, in strip order.
    pub pinned: &'a [bool],
    /// The tab the menu opened on. An index past the strip gives a menu with every action disabled.
    pub index: usize,
    /// The shortcut text shown on the Reopen row.
    pub reopen_shortcut: String,
    /// True when the closed stack is not empty.
    pub reopen_enabled: bool,
    /// True for the workspace menu, which has a Rename row.
    pub rename: bool,
    /// The split rows (page tabs only): `None` leaves them out (the workspace menu, and the phone, which keeps one group).
    /// `Some(enabled)` lists "Split right" and "Split down", enabled or not.
    pub split: Option<bool>,
    /// True for the phone's workspace sheet menu (D12): only Close, Duplicate, Rename and Pin. The bulk closes, Copy link and
    /// the Reopen row are left out (Reopen is the sheet's footer button).
    pub compact: bool,
}

impl StripMenu<'_> {
    pub fn rows(&self) -> Vec<TabMenuRow> {
        let index = self.index;
        let open = index < self.pinned.len();
        let pinned = open && self.pinned[index];
        let unpinned = |tabs: &[bool]| tabs.iter().any(|&pinned| !pinned);
        let noun = self.noun;
        let mut rows = vec![
            action("close", "Close", open),
            action(
                "close_others",
                "Close others",
                self.pinned
                    .iter()
                    .enumerate()
                    .any(|(at, &pinned)| at != index && !pinned),
            ),
            action(
                "close_left",
                "Close to the left",
                self.pinned.get(..index).is_some_and(unpinned),
            ),
            action(
                "close_right",
                "Close to the right",
                self.pinned
                    .get(index.saturating_add(1)..)
                    .is_some_and(unpinned),
            ),
            action("close_all", "Close all", unpinned(self.pinned)),
            rule("rule-1"),
            action("duplicate", format!("Duplicate {noun}"), open),
        ];
        if let Some(enabled) = self.split {
            rows.push(action("split_right", "Split right", open && enabled));
            rows.push(action("split_down", "Split down", open && enabled));
        }
        if self.rename {
            rows.push(action("rename", "Rename", open));
        }
        let pin_label = if pinned {
            format!("Unpin {noun}")
        } else {
            format!("Pin {noun}")
        };
        rows.push(action("pin", pin_label, open));
        rows.push(action("copy_link", "Copy link", open));
        if self.compact {
            // The phone sheet's row menu (D12, gaps D2): the same Copy link row as the desktop menu (parity).
            rows.retain(|row| {
                matches!(
                    row.id,
                    "close" | "rule-1" | "duplicate" | "rename" | "pin" | "copy_link"
                )
            });
            return rows;
        }
        rows.push(rule("rule-2"));
        rows.push(TabMenuRow {
            id: "reopen",
            label: format!("Reopen closed {noun}"),
            shortcut: self.reopen_shortcut.clone(),
            enabled: self.reopen_enabled,
            separator: false,
        });
        rows
    }
}

/// One row of a strip tab menu. A separator has no action (`separator` is true).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabMenuRow {
    pub id: &'static str,
    pub label: String,
    pub shortcut: String,
    pub enabled: bool,
    pub separator: bool,
}

/// An action row of a strip tab menu, with no shortcut text.
pub fn action(id: &'static str, label: impl Into<String>, enabled: bool) -> TabMenuRow {
    TabMenuRow {
        id,
        label: label.into(),
        shortcut: String::new(),
        enabled,
        separator: false,
    }
}

/// A separator row of a strip tab menu. `id` keeps the Slint rows unique.
pub fn rule(id: &'static str) -> TabMenuRow {
    TabMenuRow {
        id,
        label: String::new(),
        shortcut: String::new(),
        enabled: false,
        separator: true,
    }
}

/// Sets a tab's title to the page title of `route`, when the app's routes know `route` as a page. Otherwise the title stays.
fn refresh_title<R: Routes>(tab: &mut PageTab, route: &str) {
    if let Some(title) = R::title(route) {
        tab.title = title;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::test_routes::TestRoutes;

    type PageTabs = super::PageTabs<TestRoutes>;

    #[test]
    fn page_tabs_navigate_replaces_the_route_and_pushes_history() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/notes", "Notes");
        tabs.navigate("/home/todos", "To-dos");
        assert_eq!(tabs.route(0).as_deref(), Some("/home/todos"));
        assert_eq!(tabs.pages[0].title, "To-dos");
        assert!(tabs.can_back());
        assert!(!tabs.can_forward());
        assert_eq!(tabs.back().as_deref(), Some("/home/notes"));
        assert_eq!(tabs.pages[0].title, "Notes");
        assert!(tabs.can_forward());
        assert_eq!(tabs.forward().as_deref(), Some("/home/todos"));
        assert_eq!(tabs.forward(), None);
    }

    #[test]
    fn page_tabs_back_and_forward_stop_at_the_ends() {
        let mut tabs = PageTabs::default();
        assert_eq!(tabs.back(), None);
        assert!(!tabs.can_back());
        tabs.open("/home/notes", "Notes");
        assert_eq!(tabs.back(), None);
        assert!(!tabs.can_back());
        assert!(!tabs.can_forward());
    }

    #[test]
    fn page_tabs_open_new_tab_and_each_tab_has_its_own_history() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/notes", "Notes");
        tabs.navigate("/home/todos", "To-dos");
        tabs.open_new_tab("/home/time", "Time");
        assert_eq!(tabs.active, 1);
        assert!(!tabs.can_back(), "a new tab starts with no history");
        tabs.navigate("/home/notes", "Notes");
        assert_eq!(tabs.back().as_deref(), Some("/home/time"));
        assert!(tabs.select(0));
        assert!(tabs.can_back(), "the first tab keeps its own history");
        assert_eq!(tabs.back().as_deref(), Some("/home/notes"));
    }

    #[test]
    fn page_tabs_open_activates_an_existing_tab_by_current_route() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/notes", "Notes");
        tabs.open("/home/todos", "To-dos");
        tabs.open("/home/notes", "Notes");
        assert_eq!(tabs.pages.len(), 2);
        assert_eq!(tabs.active, 0);
    }

    #[test]
    fn page_tabs_close_keeps_the_active_tab_and_its_history() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/notes", "Notes");
        tabs.open("/home/time", "Time");
        tabs.navigate("/home/todos", "To-dos");
        assert!(tabs.close(0));
        assert_eq!(tabs.active, 0);
        assert_eq!(tabs.route(0).as_deref(), Some("/home/todos"));
        assert!(tabs.can_back(), "the tab keeps its history after the close");
        assert_eq!(tabs.back().as_deref(), Some("/home/time"));
        assert!(tabs.close(0));
        assert_eq!(tabs.active, 0);
        assert!(tabs.pages.is_empty());
        assert!(!tabs.close(0));
    }

    /// Four tabs, A to D, each on its own route, with A active.
    fn four_tabs() -> PageTabs {
        let mut tabs = PageTabs::default();
        for (route, title) in [
            ("/home/a", "A"),
            ("/home/b", "B"),
            ("/home/c", "C"),
            ("/home/d", "D"),
        ] {
            tabs.open_new_tab(route, title);
        }
        tabs.select(0);
        tabs
    }

    fn titles(tabs: &PageTabs) -> Vec<&str> {
        tabs.pages.iter().map(|tab| tab.title.as_str()).collect()
    }

    fn enabled(rows: &[TabMenuRow], id: &str) -> bool {
        rows.iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("no menu row {id:?}"))
            .enabled
    }

    #[test]
    fn page_tabs_close_others_skips_pinned_tabs() {
        let mut tabs = four_tabs();
        assert!(tabs.set_pinned(0, true));
        assert!(tabs.set_pinned(3, true));
        assert_eq!(titles(&tabs), ["A", "D", "B", "C"]);
        assert!(tabs.select(2));
        assert!(tabs.close_others(2));
        assert_eq!(titles(&tabs), ["A", "D", "B"], "only the unpinned C closes");
        assert_eq!(tabs.active, 2, "the kept tab stays active");
        assert_eq!(tabs.closed.last().map(|c| c.title.as_str()), Some("C"));
        assert!(tabs.close(0), "a pinned tab still closes with plain Close");
        assert_eq!(titles(&tabs), ["D", "B"]);
    }

    #[test]
    fn page_tabs_close_left_and_right_skip_pinned_and_keep_the_anchor() {
        let mut tabs = four_tabs();
        assert!(tabs.set_pinned(0, true));
        assert!(tabs.select(2));
        assert!(tabs.close_left(2));
        assert_eq!(titles(&tabs), ["A", "C", "D"], "A is pinned and stays");
        assert_eq!(tabs.active, 1, "C is still active, now at index 1");
        assert!(tabs.close_right(1));
        assert_eq!(titles(&tabs), ["A", "C"]);

        let mut tabs = four_tabs();
        assert!(tabs.select(0));
        assert!(tabs.close_left(2));
        assert_eq!(
            titles(&tabs),
            ["C", "D"],
            "the active A closed, so the anchor C is active"
        );
        assert_eq!(tabs.active, 0);
        assert_eq!(tabs.route(0).as_deref(), Some("/home/c"));
        assert!(!tabs.close_left(9), "no tab 9");
    }

    #[test]
    fn page_tabs_close_all_keeps_pinned_tabs() {
        let mut tabs = four_tabs();
        assert!(tabs.set_pinned(1, true));
        assert_eq!(titles(&tabs), ["B", "A", "C", "D"]);
        assert!(tabs.select(3));
        tabs.close_all();
        assert_eq!(titles(&tabs), ["B"]);
        assert_eq!(tabs.active, 0, "the pinned tab is active");
        assert_eq!(tabs.closed.len(), 3, "A, C and D are on the closed stack");
    }

    #[test]
    fn page_tabs_menu_disables_actions_that_would_do_nothing() {
        let tabs = four_tabs();
        let first = tabs.tab_menu_rows(0, None);
        assert!(
            !enabled(&first, "close_left"),
            "nothing is left of the first tab"
        );
        assert!(enabled(&first, "close_others"));
        assert!(enabled(&first, "close_right"));
        assert!(enabled(&first, "close_all"));
        assert!(enabled(&first, "duplicate"));
        assert!(enabled(&first, "pin"));
        assert!(
            enabled(&first, "copy_link"),
            "Copy link is enabled (gaps D2)"
        );
        assert!(!enabled(&first, "reopen"), "the closed stack is empty");

        let mut single = PageTabs::default();
        single.open("/home/a", "A");
        let rows = single.tab_menu_rows(0, None);
        assert!(
            !enabled(&rows, "close_others"),
            "a single tab has no others"
        );
        assert!(!enabled(&rows, "close_left"));
        assert!(!enabled(&rows, "close_right"));
        assert!(enabled(&rows, "close_all"));

        let mut tabs = four_tabs();
        assert!(tabs.set_pinned(0, true));
        assert!(tabs.set_pinned(1, true));
        assert!(tabs.set_pinned(2, true));
        assert!(tabs.set_pinned(3, true));
        let rows = tabs.tab_menu_rows(0, None);
        assert!(!enabled(&rows, "close_all"), "only pinned tabs are open");
        assert!(!enabled(&rows, "close_others"));

        assert!(tabs.close(3));
        let rows = tabs.tab_menu_rows(0, None);
        assert!(enabled(&rows, "reopen"), "Reopen is enabled after a close");
        let pin = rows.iter().find(|row| row.id == "pin").expect("a pin row");
        assert_eq!(pin.label, "Unpin tab", "the label toggles on a pinned tab");
    }

    #[test]
    fn page_tabs_duplicate_gets_a_fresh_history() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/a", "A");
        tabs.navigate("/home/b", "B");
        assert!(tabs.can_back());
        assert!(tabs.duplicate(0));
        assert_eq!(titles(&tabs), ["B", "B"], "the copy keeps the title");
        assert_eq!(tabs.active, 1, "the copy is active");
        assert_eq!(tabs.route(1).as_deref(), Some("/home/b"));
        assert!(!tabs.can_back(), "the copy has no history to go back to");
        assert!(!tabs.can_forward());
        assert!(tabs.select(0));
        assert!(tabs.can_back(), "the original keeps its history");
        assert!(!tabs.duplicate(7));
    }

    #[test]
    fn page_tabs_pin_moves_the_tab_into_the_pinned_group_and_back() {
        let mut tabs = four_tabs();
        assert!(tabs.set_pinned(2, true));
        assert_eq!(titles(&tabs), ["C", "A", "B", "D"], "pinned C moves left");
        assert_eq!(tabs.route(tabs.active).as_deref(), Some("/home/a"));
        assert!(tabs.set_pinned(3, true));
        assert_eq!(titles(&tabs), ["C", "D", "A", "B"]);
        assert!(tabs.set_pinned(0, false));
        assert_eq!(
            titles(&tabs),
            ["D", "C", "A", "B"],
            "an unpinned tab moves just after the last pinned tab"
        );
        assert_eq!(tabs.route(tabs.active).as_deref(), Some("/home/a"));
        assert!(!tabs.set_pinned(9, true));
    }

    #[test]
    fn page_tabs_closed_stack_reopens_in_reverse_order_and_is_capped() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/keep", "Keep");
        for i in 0..12 {
            tabs.open_new_tab(&format!("/home/t{i}"), &format!("T{i}"));
        }
        for _ in 0..12 {
            assert!(tabs.close(1));
        }
        assert_eq!(
            tabs.closed.len(),
            CLOSED_CAP,
            "the stack keeps the last ten"
        );
        let mut reopened = Vec::new();
        while let Some(route) = tabs.reopen() {
            reopened.push(route);
        }
        let expected: Vec<String> = (2..12).rev().map(|i| format!("/home/t{i}")).collect();
        assert_eq!(reopened, expected, "the last closed comes back first");
        assert_eq!(tabs.reopen(), None, "an empty stack reopens nothing");
        assert_eq!(
            titles(&tabs),
            [
                "Keep", "T11", "T10", "T9", "T8", "T7", "T6", "T5", "T4", "T3", "T2"
            ],
            "each reopened tab goes to the end of the strip"
        );
        assert_eq!(
            tabs.active,
            tabs.pages.len() - 1,
            "the reopened tab is active"
        );
        assert!(
            tabs.pages.iter().all(|tab| !tab.pinned),
            "a reopened tab is unpinned"
        );
    }
}
