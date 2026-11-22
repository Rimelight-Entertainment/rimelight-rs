//! Top-level workspaces (planned feature B, `PLAN.md section 6.6` part B) and their persistence. Plain Rust, always
//! compiled, so its tests run without a window.
//!
//! A [`Workspaces`] holds up to [`WORKSPACE_CAP`] workspaces. Each one holds a tile layout ([`TileLayout`], in
//! `tiles.rs`): one or more tile groups, each with its own page tabs ([`PageTabs`] in `tabs.rs`), and also the rail
//! area and the sidebar selection. [`Store`] saves the open workspaces as one versioned text blob. The window loads them at
//! start-up ([`startup`]) and saves through a [`Saver`]: about a second after a change, and on exit (see `app_shell`).
//!
//! Persistence. [`Workspaces::encode`] writes the `v2` text: the active index, then for each workspace a `ws` row and its
//! tile tree rows (`split` and `group` rows, see `tiles.rs`; a `group` row is followed by its page rows). The `v1` text (one
//! page list per workspace, no tile tree) still decodes, as one group. Names and titles are hex-encoded, and the closed
//! stacks are not saved.
//!
//! The app's [`Routes`] give the rail areas (their saved ids too) and the Home route and its title (see [`crate::routes`]).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::history::History;
use crate::routes::Routes;
use crate::tabs::{PageTab, PageTabs};
use crate::tiles::TileLayout;

/// The version of the workspace text that [`Workspaces::encode`] writes.
pub const WORKSPACE_FORMAT_VERSION: &str = "v2";

/// The most top-level workspaces open at once (owner decision P6).
pub const WORKSPACE_CAP: usize = 10;

/// The most closed workspaces kept for Reopen (planned feature B). The oldest is dropped when a close goes past this.
pub const WORKSPACE_CLOSED_CAP: usize = 10;

/// The name shown for a workspace that has no user name and no page open.
pub const EMPTY_WORKSPACE_NAME: &str = "Empty workspace";

/// One top-level workspace: its tile layout (the tile groups, each with its page tabs, pinned state, closed stack and
/// histories), rail area, sidebar selection, and a user name. A `None` name is automatic: the active page's title of the
/// focused group (see [`Workspaces::display_name`]).
#[derive(Clone)]
pub struct WorkspaceTab<R: Routes> {
    tiles: TileLayout<R>,
    area: R::Area,
    sidebar_selected: i32,
    name: Option<String>,
    pinned: bool,
}

impl<R: Routes> WorkspaceTab<R> {
    /// A workspace with no page open, in the Home area, with sidebar row 0 selected, unnamed and unpinned. Its tile layout
    /// is one empty group.
    fn blank() -> Self {
        Self {
            tiles: TileLayout::single(PageTabs::<R>::default()),
            area: R::HOME_AREA,
            sidebar_selected: 0,
            name: None,
            pinned: false,
        }
    }

    /// The rail area of this workspace.
    pub fn area(&self) -> R::Area {
        self.area
    }

    pub fn set_area(&mut self, area: R::Area) {
        self.area = area;
    }

    /// The index of the selected sidebar row.
    pub fn sidebar_selected(&self) -> i32 {
        self.sidebar_selected
    }

    pub fn set_sidebar_selected(&mut self, index: i32) {
        self.sidebar_selected = index;
    }

    /// The user-set name, or `None` when the name is automatic.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// True when the workspace is pinned: it sits in the pinned group at the left, narrow.
    pub fn pinned(&self) -> bool {
        self.pinned
    }

    /// The tile layout of this workspace.
    #[cfg(test)]
    pub fn tiles(&self) -> &TileLayout<R> {
        &self.tiles
    }

    #[cfg(test)]
    pub fn tiles_mut(&mut self) -> &mut TileLayout<R> {
        &mut self.tiles
    }

    /// The route of the active page tab of the focused group: the page this workspace shows (Copy link on its menu). `None`
    /// when the group has no tab.
    pub fn active_route(&self) -> Option<String> {
        self.tiles.focused_tabs().active_route()
    }

    /// The page tabs of the focused group.
    #[cfg(test)]
    pub fn pages(&self) -> &PageTabs<R> {
        self.tiles.focused_tabs()
    }

    #[cfg(test)]
    pub fn pages_mut(&mut self) -> &mut PageTabs<R> {
        self.tiles.focused_tabs_mut()
    }
}

/// A workspace on the closed stack, kept with its full contents (pages and histories) for Reopen.
#[derive(Clone)]
pub struct ClosedWorkspace<R: Routes>(WorkspaceTab<R>);

/// The top-level workspaces (planned feature B1). The active one's page tabs are what the window shows. With no workspace
/// open ([`Workspaces::is_empty`]) the window shows the empty state. Plain Rust, so the rules run without a window.
#[derive(Clone, Default)]
pub struct Workspaces<R: Routes> {
    items: Vec<WorkspaceTab<R>>,
    active: Option<usize>,
    /// The workspaces closed most recently last, at most [`WORKSPACE_CLOSED_CAP`].
    closed: Vec<ClosedWorkspace<R>>,
}

/// True when the items that have `pinned` set come first. The pinned group sits at the left of the strip.
pub fn pinned_first<T>(items: &[T], pinned: impl Fn(&T) -> bool) -> bool {
    items
        .windows(2)
        .all(|pair| pinned(&pair[0]) || !pinned(&pair[1]))
}

/// The title of the Home page, which a new workspace opens on. `"Home"` when the routes give no title.
fn home_title<R: Routes>() -> String {
    R::title(R::HOME_ROUTE).unwrap_or_else(|| "Home".to_owned())
}

impl<R: Routes> Workspaces<R> {
    /// The B1 starting state: one blank workspace, active, with no page open. The app starts this way so that it behaves as
    /// it did before workspaces (B2 adds the strip and opens Home).
    pub fn with_blank_workspace() -> Self {
        Self {
            items: vec![WorkspaceTab::blank()],
            active: Some(0),
            closed: Vec::new(),
        }
    }

    /// The number of open workspaces.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True when no workspace is open: the empty state.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// True when [`WORKSPACE_CAP`] workspaces are open.
    pub fn is_full(&self) -> bool {
        self.items.len() >= WORKSPACE_CAP
    }

    /// The index of the active workspace, or `None` when none is open.
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    /// The workspace at `index`.
    pub fn get(&self, index: usize) -> Option<&WorkspaceTab<R>> {
        self.items.get(index)
    }

    /// Mutable access to the workspace at `index`, for its area, sidebar selection and pinned state.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut WorkspaceTab<R>> {
        self.items.get_mut(index)
    }

    /// The number of closed workspaces that Reopen can bring back.
    pub fn closed_len(&self) -> usize {
        self.closed.len()
    }

    /// The number of pinned workspaces. They are the first ones.
    pub fn pinned_count(&self) -> usize {
        self.items.iter().filter(|tab| tab.pinned).count()
    }

    /// The page tabs of the focused group of the active workspace, or `None` when none is open. The window shows these.
    pub fn active_pages(&self) -> Option<&PageTabs<R>> {
        self.items
            .get(self.active?)
            .map(|tab| tab.tiles.focused_tabs())
    }

    /// The tile layout of the active workspace, or `None` when none is open.
    pub fn active_tiles(&self) -> Option<&TileLayout<R>> {
        self.items.get(self.active?).map(|tab| &tab.tiles)
    }

    /// The tile layout of the active workspace, for changing.
    pub fn active_tiles_mut(&mut self) -> Option<&mut TileLayout<R>> {
        let index = self.active?;
        self.items.get_mut(index).map(|tab| &mut tab.tiles)
    }

    /// The page tabs of the focused group of the active workspace, for changing.
    pub fn active_pages_mut(&mut self) -> Option<&mut PageTabs<R>> {
        let index = self.active?;
        self.items
            .get_mut(index)
            .map(|tab| tab.tiles.focused_tabs_mut())
    }

    /// Closes the empty groups of the active workspace (see [`TileLayout::close_empty_groups`]). Called after a change to
    /// the focused group's tabs, so a group that lost its last tab closes itself.
    pub fn tidy_active(&mut self) {
        if let Some(tab) = self.active.and_then(|index| self.items.get_mut(index)) {
            tab.tiles.close_empty_groups();
        }
    }

    /// The pinned flag of each open workspace, in strip order.
    pub fn pinned_flags(&self) -> Vec<bool> {
        self.items.iter().map(|tab| tab.pinned).collect()
    }

    /// Opens a workspace with no page and makes it active. It goes at the end. Returns its index, or `None` when
    /// [`WORKSPACE_CAP`] workspaces are already open. A page is opened into it by the caller (see [`Workspaces::new_workspace`]).
    pub fn new_blank_workspace(&mut self) -> Option<usize> {
        if self.is_full() {
            return None;
        }
        self.items.push(WorkspaceTab::blank());
        let at = self.items.len() - 1;
        self.active = Some(at);
        Some(at)
    }

    /// Opens a workspace on the Home page and makes it active. It goes at the end, after the pinned group. Returns its
    /// index, or `None` when [`WORKSPACE_CAP`] workspaces are already open.
    pub fn new_workspace(&mut self) -> Option<usize> {
        let at = self.new_blank_workspace()?;
        self.items[at]
            .tiles
            .focused_tabs_mut()
            .open(R::HOME_ROUTE, &home_title::<R>());
        Some(at)
    }

    /// Puts a closed workspace on the closed stack. The oldest entry goes when the stack is over [`WORKSPACE_CLOSED_CAP`].
    fn remember(&mut self, tab: WorkspaceTab<R>) {
        self.closed.push(ClosedWorkspace(tab));
        if self.closed.len() > WORKSPACE_CLOSED_CAP {
            self.closed.remove(0);
        }
    }

    /// Closes workspace `index`, pinned or not, and keeps it on the closed stack. The active one stays active while it
    /// is open; when the active one closes, the workspace at the same index (or the last one) becomes active. Closing
    /// the last workspace leaves the empty state. Returns false when there is no workspace `index`.
    pub fn close(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let tab = self.items.remove(index);
        self.remember(tab);
        self.active = if self.items.is_empty() {
            None
        } else {
            Some(match self.active {
                Some(active) if index < active => active - 1,
                Some(active) if index == active => index.min(self.items.len() - 1),
                Some(active) => active,
                None => 0,
            })
        };
        true
    }

    /// Closes the unpinned workspaces for which `doomed` holds (called with the index), in order. Pinned workspaces are
    /// never closed here. When the active one closes, `anchor` becomes active, or else the first workspace left.
    fn close_unpinned(&mut self, anchor: Option<usize>, doomed: impl Fn(usize) -> bool) {
        let active = self.active;
        let mut kept = Vec::with_capacity(self.items.len());
        // The new index of each old workspace, or None when it closed.
        let mut moved: Vec<Option<usize>> = Vec::with_capacity(self.items.len());
        for (index, tab) in std::mem::take(&mut self.items).into_iter().enumerate() {
            if !tab.pinned && doomed(index) {
                moved.push(None);
                self.remember(tab);
            } else {
                moved.push(Some(kept.len()));
                kept.push(tab);
            }
        }
        self.items = kept;
        self.active = if self.items.is_empty() {
            None
        } else {
            let new_index = active
                .and_then(|a| moved.get(a).copied().flatten())
                .or_else(|| anchor.and_then(|a| moved.get(a).copied().flatten()))
                .unwrap_or(0);
            Some(new_index.min(self.items.len() - 1))
        };
    }

    /// Closes the unpinned workspaces to the left of `index`. Returns false when there is no workspace `index`.
    pub fn close_left(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.close_unpinned(Some(index), |at| at < index);
        true
    }

    /// Closes the unpinned workspaces to the right of `index`. Returns false when there is no workspace `index`.
    pub fn close_right(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.close_unpinned(Some(index), |at| at > index);
        true
    }

    /// Closes every unpinned workspace except `index`. Returns false when there is no workspace `index`.
    pub fn close_others(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.close_unpinned(Some(index), |at| at != index);
        true
    }

    /// Closes every unpinned workspace. The pinned ones stay.
    pub fn close_all(&mut self) {
        self.close_unpinned(None, |_| true);
    }

    /// Copies workspace `index` in full: its page tabs with their histories and closed stacks, area, sidebar selection
    /// and user name. The copy is unpinned, goes right after the original (or after the pinned group when the original is
    /// pinned), and becomes active. Returns false when there is no workspace `index`, or when the cap is reached.
    pub fn duplicate(&mut self, index: usize) -> bool {
        if self.is_full() {
            return false;
        }
        let Some(source) = self.items.get(index) else {
            return false;
        };
        let copy = WorkspaceTab {
            pinned: false,
            ..source.clone()
        };
        let at = if source.pinned {
            self.pinned_count()
        } else {
            index + 1
        };
        self.items.insert(at, copy);
        self.active = Some(at);
        true
    }

    /// Pins or unpins workspace `index`, with the same rules as the page tabs: a pinned workspace moves to the end of the
    /// pinned group at the left, an unpinned one to just after it. The active workspace stays the active one. Returns
    /// false when there is no workspace `index`.
    pub fn set_pinned(&mut self, index: usize, pinned: bool) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let was_active = self.active;
        let mut tab = self.items.remove(index);
        tab.pinned = pinned;
        // After the removal, the pinned workspaces left are the ones before the new position.
        let at = self.pinned_count();
        self.items.insert(at, tab);
        self.active = was_active.map(|active| {
            if active == index {
                at
            } else {
                let shifted = if active > index { active - 1 } else { active };
                if at <= shifted { shifted + 1 } else { shifted }
            }
        });
        true
    }

    /// Moves workspace `from` to index `to` (counted after the workspace has left), for a drag in the strip. The caller picks
    /// `to` with the pinned rules (see `tile_drop::reorder`). The active workspace stays the active one. Returns false when
    /// there is no workspace `from` or `to`.
    pub fn move_workspace(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() {
            return false;
        }
        let was_active = self.active;
        let tab = self.items.remove(from);
        self.items.insert(to, tab);
        self.active = was_active.map(|active| {
            if active == from {
                to
            } else {
                let shifted = if active > from { active - 1 } else { active };
                if to <= shifted { shifted + 1 } else { shifted }
            }
        });
        true
    }

    /// Sets the user name of workspace `index`. Surrounding spaces are trimmed, and an empty name (`None` or only spaces)
    /// clears it, so the name becomes automatic again. Returns false when there is no workspace `index`.
    pub fn rename(&mut self, index: usize, name: Option<String>) -> bool {
        let Some(tab) = self.items.get_mut(index) else {
            return false;
        };
        tab.name = name
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty());
        true
    }

    /// The name shown for workspace `index`: the user name when one is set, otherwise the title of its active page, or
    /// [`EMPTY_WORKSPACE_NAME`] when no page is open. `None` when there is no workspace `index`.
    pub fn display_name(&self, index: usize) -> Option<String> {
        let tab = self.items.get(index)?;
        Some(match &tab.name {
            Some(name) => name.clone(),
            None => tab
                .tiles
                .focused_tabs()
                .active_title()
                .unwrap_or(EMPTY_WORKSPACE_NAME)
                .to_owned(),
        })
    }

    /// Makes workspace `index` active. Returns false (and changes nothing) when there is no such workspace.
    pub fn activate(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.active = Some(index);
            true
        } else {
            false
        }
    }

    /// Reopens the last closed workspace: it comes back unpinned at the end, with its contents, and becomes active. Returns
    /// its index. `None` when the closed stack is empty, or when the cap is reached (the stack is then left as it was).
    pub fn reopen(&mut self) -> Option<usize> {
        if self.is_full() {
            return None;
        }
        let ClosedWorkspace(mut tab) = self.closed.pop()?;
        tab.pinned = false;
        self.items.push(tab);
        let at = self.items.len() - 1;
        self.active = Some(at);
        Some(at)
    }

    /// The text form of all open workspaces (`v2`): the active index, then each workspace's `ws` row and its tile tree rows.
    /// Names and titles are hex-encoded, so no user text can break the layout. The closed stacks are not saved. Fails when a
    /// route holds a reserved character (see [`History::encode`]).
    pub fn encode(&self) -> Result<String, String> {
        let active = self
            .active
            .map_or_else(|| "-".to_owned(), |index| index.to_string());
        let mut lines = vec![
            WORKSPACE_FORMAT_VERSION.to_owned(),
            format!("active\t{active}"),
        ];
        for tab in &self.items {
            let name = tab.name.as_deref().map_or_else(|| "-".to_owned(), hex);
            lines.push(format!(
                "ws\t{}\t{}\t{}\t{}\t{}",
                R::area_id(tab.area),
                tab.sidebar_selected,
                flag(tab.pinned),
                name,
                tab.tiles.encoded_focus()
            ));
            tab.tiles.encode_rows(&mut lines)?;
        }
        Ok(lines.join("\n"))
    }

    /// Reads text written by [`Workspaces::encode`] (`v2`), or the older `v1` text (read as one group). Returns `None` for an
    /// unknown version, any malformed or inconsistent row (an unknown area, a bad flag or hex, an index past its list, a
    /// pinned tab after an unpinned one, a bad tile tree, more than [`WORKSPACE_CAP`] workspaces), or any garbage. Never
    /// panics. The closed stacks come back empty.
    pub fn decode(text: &str) -> Option<Self> {
        let rows: Vec<Vec<&str>> = text
            .split('\n')
            .map(|line| line.split('\t').collect())
            .collect();
        let v2 = match rows.first()?.as_slice() {
            ["v2"] => true,
            ["v1"] => false,
            _ => return None,
        };
        let mut at = 1;
        let active = match rows.get(at)?.as_slice() {
            ["active", "-"] => None,
            ["active", index] => Some(index.parse::<usize>().ok()?),
            _ => return None,
        };
        at += 1;
        let mut items = Vec::new();
        while at < rows.len() {
            let tab = if v2 {
                decode_workspace_v2::<R>(&rows, &mut at)?
            } else {
                decode_workspace_v1::<R>(&rows, &mut at)?
            };
            items.push(tab);
        }
        if items.len() > WORKSPACE_CAP || !pinned_first(&items, |tab| tab.pinned) {
            return None;
        }
        let active_valid = match active {
            None => items.is_empty(),
            Some(index) => index < items.len(),
        };
        if !active_valid {
            return None;
        }
        Some(Self {
            items,
            active,
            closed: Vec::new(),
        })
    }
}

/// The `page` rows of one group's tabs, in order. Fails when a route holds a reserved character.
pub fn encode_pages<R: Routes>(tabs: &PageTabs<R>) -> Result<Vec<String>, String> {
    let mut rows = Vec::with_capacity(tabs.pages.len());
    for page in &tabs.pages {
        rows.push(format!(
            "page\t{}\t{}\t{}",
            flag(page.pinned),
            hex(&page.title),
            page.history.encode()?
        ));
    }
    Ok(rows)
}

/// Reads `count` `page` rows from `rows[*at]` and moves `at` past them. `None` for a bad row or too few rows.
pub fn decode_pages(rows: &[Vec<&str>], at: &mut usize, count: usize) -> Option<Vec<PageTab>> {
    let mut pages = Vec::new();
    for _ in 0..count {
        let ["page", pinned, title, history] = rows.get(*at)?.as_slice() else {
            return None;
        };
        *at += 1;
        pages.push(PageTab {
            history: History::decode(history)?,
            title: unhex(title)?,
            pinned: parse_flag(pinned)?,
        });
    }
    Some(pages)
}

/// Reads one `v1` workspace: its `ws` row (which counts its pages) and those page rows. The pages become one group.
fn decode_workspace_v1<R: Routes>(rows: &[Vec<&str>], at: &mut usize) -> Option<WorkspaceTab<R>> {
    let ["ws", area, selected, pinned, name, page_count, page_active] = rows.get(*at)?.as_slice()
    else {
        return None;
    };
    *at += 1;
    let (area, sidebar_selected, pinned, name) = decode_header::<R>(area, selected, pinned, name)?;
    let page_count: usize = page_count.parse().ok()?;
    let page_active: usize = page_active.parse().ok()?;
    let pages = decode_pages(rows, at, page_count)?;
    let active_valid = if pages.is_empty() {
        page_active == 0
    } else {
        page_active < pages.len()
    };
    if !active_valid || !pinned_first(&pages, |page| page.pinned) {
        return None;
    }
    let tabs = PageTabs::from_pages(pages, page_active);
    Some(WorkspaceTab {
        tiles: TileLayout::single(tabs),
        area,
        sidebar_selected,
        name,
        pinned,
    })
}

/// Reads one `v2` workspace: its `ws` row (which names the focused group) and its tile tree rows.
fn decode_workspace_v2<R: Routes>(rows: &[Vec<&str>], at: &mut usize) -> Option<WorkspaceTab<R>> {
    let ["ws", area, selected, pinned, name, focused] = rows.get(*at)?.as_slice() else {
        return None;
    };
    *at += 1;
    let (area, sidebar_selected, pinned, name) = decode_header::<R>(area, selected, pinned, name)?;
    let focused: u32 = focused.parse().ok()?;
    let tiles = TileLayout::<R>::decode_rows(rows, at, focused)?;
    Some(WorkspaceTab {
        tiles,
        area,
        sidebar_selected,
        name,
        pinned,
    })
}

/// The fields of a `ws` row that both versions share: the area, the sidebar selection, the pinned flag and the name.
fn decode_header<R: Routes>(
    area: &str,
    selected: &str,
    pinned: &str,
    name: &str,
) -> Option<(R::Area, i32, bool, Option<String>)> {
    let area = R::area_from_id(area)?;
    let sidebar_selected: i32 = selected.parse().ok()?;
    let pinned = parse_flag(pinned)?;
    let name = match name {
        "-" => None,
        hex_text => Some(unhex(hex_text).filter(|text| !text.is_empty())?),
    };
    Some((area, sidebar_selected, pinned, name))
}

/// The saved text of the open workspaces (planned feature B, persistence P7). One implementation per platform; the text is
/// [`Workspaces::encode`]'s. The window saves through a [`Saver`] (debounced, and on exit).
pub trait Store {
    /// The saved text, or `None` when nothing is saved or it cannot be read.
    fn load(&self) -> Option<String>;

    /// Replaces the saved text.
    fn save(&self, text: &str) -> Result<(), String>;
}

/// Decodes the saved workspaces. `None` when nothing is saved, or the saved text is garbage or an unknown version.
pub fn load_workspaces<R: Routes>(store: &dyn Store) -> Option<Workspaces<R>> {
    Workspaces::decode(&store.load()?)
}

/// Encodes `workspaces` and saves them.
pub fn save_workspaces<R: Routes>(
    store: &dyn Store,
    workspaces: &Workspaces<R>,
) -> Result<(), String> {
    store.save(&workspaces.encode()?)
}

/// The workspaces the app starts with, and the [`Saver`] that writes them back. The store is read once. A missing, corrupt
/// or unknown-version save gives one blank workspace, and the flag is false: the caller opens its start page in it.
pub fn startup<R: Routes>(store: Rc<dyn Store>) -> (Workspaces<R>, Saver, bool) {
    let saved = store.load();
    let loaded = saved.as_deref().and_then(Workspaces::<R>::decode);
    let is_loaded = loaded.is_some();
    let workspaces = loaded.unwrap_or_else(Workspaces::<R>::with_blank_workspace);
    (workspaces, Saver::new(store, saved), is_loaded)
}

/// The saved text a web tab starts from (planned D11, per-tab store). `text` is the first one present, in this order: the
/// tab's own state (sessionStorage), the last saved state of any tab (localStorage, the seed for new tabs), and the
/// old single localStorage key of B3a. `migrate` is true only when the old key was chosen. Pure, so it runs on the host.
#[derive(Debug, PartialEq, Eq)]
pub struct StartText<'a> {
    /// The text to decode, or `None` when no source holds any.
    pub text: Option<&'a str>,
    /// True when the text came from the old B3a key.
    pub migrate: bool,
}

/// Chooses the start text of a web tab. See [`StartText`].
pub fn choose_start_text<'a>(
    session: Option<&'a str>,
    last: Option<&'a str>,
    legacy: Option<&'a str>,
) -> StartText<'a> {
    match (session, last, legacy) {
        (Some(text), _, _) | (None, Some(text), _) => StartText {
            text: Some(text),
            migrate: false,
        },
        (None, None, Some(text)) => StartText {
            text: Some(text),
            migrate: true,
        },
        (None, None, None) => StartText {
            text: None,
            migrate: false,
        },
    }
}

/// Writes the workspaces to a [`Store`], and keeps the text of the last write, so a save that changes nothing writes
/// nothing. The window calls [`Saver::save`] from its debounce timer and on exit.
pub struct Saver {
    store: Rc<dyn Store>,
    /// The text last read or written, or `None` when the store held nothing.
    saved: RefCell<Option<String>>,
}

impl Saver {
    /// A saver for `store`, whose text at start-up is `saved` (`None` when it held nothing).
    pub fn new(store: Rc<dyn Store>, saved: Option<String>) -> Self {
        Self {
            store,
            saved: RefCell::new(saved),
        }
    }

    /// Writes `workspaces` when their text differs from the last write. Returns `Ok(true)` when it wrote, `Ok(false)` when
    /// the text is the same. An encode or write error is returned, and the last saved text stays as it was.
    pub fn save<R: Routes>(&self, workspaces: &Workspaces<R>) -> Result<bool, String> {
        let text = workspaces.encode()?;
        if self.saved.borrow().as_deref() == Some(text.as_str()) {
            return Ok(false);
        }
        self.store.save(&text)?;
        *self.saved.borrow_mut() = Some(text);
        Ok(true)
    }
}

/// A store in memory: for tests, and for a run with no disk.
#[derive(Default)]
pub struct MemoryStore {
    text: RefCell<Option<String>>,
}

impl Store for MemoryStore {
    fn load(&self) -> Option<String> {
        self.text.borrow().clone()
    }

    fn save(&self, text: &str) -> Result<(), String> {
        *self.text.borrow_mut() = Some(text.to_owned());
        Ok(())
    }
}

/// A store in one file. The file is written through a temporary sibling and renamed, so a crash does not leave half a file.
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    /// A store in the file at `path`.
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// The store in the OS app-data folder: `<app data>/<app_folder>/workspaces.txt`. `None` where this build has no such
    /// folder (see [`app_data_dir`]). `app_folder` names the folder inside it.
    pub fn in_app_data(app_folder: &str) -> Option<Self> {
        let dir = app_data_dir()?;
        Some(Self::at(dir.join(app_folder).join("workspaces.txt")))
    }
}

impl Store for FileStore {
    fn load(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }

    fn save(&self, text: &str) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }
        let temp = self.path.with_extension("txt.tmp");
        std::fs::write(&temp, text).map_err(|error| error.to_string())?;
        std::fs::rename(&temp, &self.path).map_err(|error| error.to_string())
    }
}

/// The OS folder for app data: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, `$XDG_DATA_HOME` (or
/// `~/.local/share`) on other desktop Unix systems.
#[cfg(windows)]
fn app_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn app_data_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
fn app_data_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

// Android has no app-data folder here: `apps/android` passes its internal data path to a `FileStore`. The web build passes a
// localStorage store from `apps/web`. Neither goes through this function.
#[cfg(target_os = "android")]
fn app_data_dir() -> Option<PathBuf> {
    None
}

#[cfg(not(any(windows, unix)))]
fn app_data_dir() -> Option<PathBuf> {
    None
}

fn flag(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn parse_flag(text: &str) -> Option<bool> {
    match text {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

/// Lower-case hex of the UTF-8 bytes of `text`.
fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

/// The text that [`hex`] wrote, or `None` when it is not even-length hex of UTF-8.
fn unhex(text: &str) -> Option<String> {
    if !text.is_ascii() || !text.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::Routes;
    use crate::routes::test_routes::{TestArea as Area, TestRoutes};

    type Workspaces = super::Workspaces<TestRoutes>;

    /// The Home title of the test routes.
    fn home_title() -> String {
        super::home_title::<TestRoutes>()
    }

    /// Adds a workspace with the given name (the new workspace is active).
    fn named(ws: &mut Workspaces, name: &str) -> usize {
        let at = ws.new_workspace().expect("room for another workspace");
        ws.rename(at, Some(name.to_owned()));
        at
    }

    /// Builds workspaces from (name, pinned) pairs, in order. Pinned ones are listed first, as the strip keeps them.
    fn build(spec: &[(&str, bool)]) -> Workspaces {
        let mut ws = Workspaces::default();
        for &(name, pinned) in spec {
            let at = named(&mut ws, name);
            if pinned {
                ws.set_pinned(at, true);
            }
        }
        ws
    }

    /// The display names of all open workspaces, in strip order.
    fn names(ws: &Workspaces) -> Vec<String> {
        (0..ws.len())
            .map(|at| ws.display_name(at).expect("open"))
            .collect()
    }

    #[test]
    fn new_workspace_opens_home_and_stops_at_the_cap() {
        let mut ws = Workspaces::default();
        assert!(ws.is_empty());
        for expected in 0..WORKSPACE_CAP {
            assert_eq!(ws.new_workspace(), Some(expected));
        }
        assert!(ws.is_full());
        assert_eq!(ws.new_workspace(), None, "the cap is 10");
        assert_eq!(ws.len(), WORKSPACE_CAP);
        assert_eq!(ws.active(), Some(WORKSPACE_CAP - 1));
        let first = ws.get(0).expect("open");
        assert_eq!(first.area(), Area::Home);
        assert_eq!(first.pages().pages.len(), 1);
        assert_eq!(
            first.pages().pages[0].history.current(),
            TestRoutes::HOME_ROUTE
        );
        assert_eq!(ws.display_name(0).as_deref(), Some(home_title().as_str()));
    }

    #[test]
    fn closing_the_last_workspace_leaves_the_empty_state() {
        let mut ws = Workspaces::default();
        let at = ws.new_workspace().expect("room");
        assert!(ws.close(at));
        assert!(ws.is_empty());
        assert_eq!(ws.active(), None);
        assert_eq!(ws.closed_len(), 1);
        assert!(!ws.close(0), "nothing to close");
        assert_eq!(ws.display_name(0), None);
    }

    #[test]
    fn close_keeps_the_active_workspace_and_clamps_it() {
        let mut ws = build(&[("a", false), ("b", false), ("c", false)]);
        ws.activate(1);
        assert!(ws.close(0));
        assert_eq!(names(&ws), ["b", "c"]);
        assert_eq!(ws.active(), Some(0), "the active one stays active");
        ws.activate(1);
        assert!(ws.close(1));
        assert_eq!(names(&ws), ["b"]);
        assert_eq!(ws.active(), Some(0), "the last one becomes active");
    }

    #[test]
    fn bulk_closes_skip_pinned_workspaces_and_keep_the_anchor() {
        let scenario = || build(&[("p", true), ("x", false), ("y", false), ("z", false)]);

        let mut ws = scenario();
        assert!(ws.close_left(3));
        assert_eq!(names(&ws), ["p", "z"], "close left keeps the pinned one");
        assert_eq!(ws.active(), Some(1), "the anchor z stays active");

        let mut ws = scenario();
        assert!(ws.close_right(1));
        assert_eq!(names(&ws), ["p", "x"]);

        let mut ws = scenario();
        assert!(ws.close_others(2));
        assert_eq!(names(&ws), ["p", "y"]);
        assert_eq!(ws.active(), Some(1));

        let mut ws = build(&[("p", true), ("q", true), ("x", false)]);
        ws.close_all();
        assert_eq!(names(&ws), ["p", "q"], "close all keeps every pinned one");
        assert_eq!(ws.active(), Some(0), "the first one is active afterwards");

        let mut ws = scenario();
        assert!(!ws.close_left(9) && !ws.close_right(9) && !ws.close_others(9));
        assert_eq!(ws.len(), 4, "a bad index closes nothing");
    }

    #[test]
    fn duplicate_copies_pages_histories_and_name_and_respects_the_cap() {
        let mut ws = build(&[("a", false), ("b", false)]);
        {
            let pages = ws.get_mut(0).expect("open").pages_mut();
            pages.open("/home/todos", "To-dos");
            pages.navigate("/home/time", "Time");
        }
        assert!(ws.duplicate(0));
        assert_eq!(names(&ws), ["a", "a", "b"]);
        assert_eq!(ws.active(), Some(1), "the copy is active");
        {
            let original = &ws.get(0).expect("open").pages().pages;
            let copy = &ws.get(1).expect("open").pages().pages;
            assert_eq!(copy.len(), 2, "every page tab comes too");
            assert!(copy[1].history == original[1].history, "histories copied");
        }

        ws.get_mut(1)
            .expect("open")
            .pages_mut()
            .back()
            .expect("the copy has history to go back through");
        assert_eq!(
            ws.get(1).expect("open").pages().pages[1].history.current(),
            "/home/todos",
            "the copy moved back"
        );
        assert_eq!(
            ws.get(0).expect("open").pages().pages[1].history.current(),
            "/home/time",
            "the original's history is its own"
        );

        let mut ws = build(&[("p", true), ("a", false)]);
        assert!(ws.duplicate(0));
        assert_eq!(names(&ws), ["p", "p", "a"]);
        assert!(!ws.get(1).expect("open").pinned(), "the copy is unpinned");

        let mut full = Workspaces::default();
        for _ in 0..WORKSPACE_CAP {
            full.new_workspace();
        }
        assert!(!full.duplicate(0), "no copy past the cap");
        assert!(!Workspaces::default().duplicate(0), "no such workspace");
    }

    #[test]
    fn set_pinned_moves_to_the_pinned_group_and_keeps_the_active_one() {
        let mut ws = build(&[("a", false), ("b", false), ("c", false)]);
        assert_eq!(ws.active(), Some(2));
        assert!(ws.set_pinned(2, true));
        assert_eq!(names(&ws), ["c", "a", "b"]);
        assert_eq!(ws.active(), Some(0), "c stays the active one");
        assert_eq!(ws.pinned_count(), 1);

        assert!(ws.set_pinned(2, true));
        assert_eq!(names(&ws), ["c", "b", "a"], "pinned after the pinned group");
        assert!(ws.set_pinned(0, false));
        assert_eq!(ws.pinned_count(), 1, "the rest of the group stays");
        assert!(!ws.set_pinned(9, true));
    }

    #[test]
    fn rename_trims_and_an_empty_name_clears_it() {
        let mut ws = build(&[("a", false)]);
        assert!(ws.rename(0, Some("  Work  ".to_owned())));
        assert_eq!(ws.display_name(0).as_deref(), Some("Work"));
        assert!(ws.rename(0, Some("   ".to_owned())));
        assert_eq!(ws.get(0).expect("open").name(), None);
        assert_eq!(
            ws.display_name(0).as_deref(),
            Some(home_title().as_str()),
            "back to the active page's title"
        );
        ws.rename(0, Some("Work".to_owned()));
        ws.rename(0, None);
        assert_eq!(ws.display_name(0).as_deref(), Some(home_title().as_str()));
        assert!(!ws.rename(5, None));
    }

    #[test]
    fn display_name_falls_back_to_the_active_page_then_to_the_empty_name() {
        let mut ws = Workspaces::default();
        let at = ws.new_workspace().expect("room");
        ws.get_mut(at)
            .expect("open")
            .pages_mut()
            .open("/home/todos", "To-dos");
        assert_eq!(ws.display_name(at).as_deref(), Some("To-dos"));
        ws.get_mut(at).expect("open").pages_mut().close(0);
        ws.get_mut(at).expect("open").pages_mut().close(0);
        assert_eq!(
            ws.display_name(at).as_deref(),
            Some(EMPTY_WORKSPACE_NAME),
            "no page open"
        );
        assert_eq!(ws.display_name(9), None);
    }

    #[test]
    fn reopen_brings_back_the_last_closed_first_and_the_stack_is_capped() {
        let mut ws = Workspaces::default();
        for i in 0..(WORKSPACE_CLOSED_CAP + 2) {
            let at = named(&mut ws, &format!("w{i}"));
            assert!(ws.close(at));
        }
        assert_eq!(ws.closed_len(), WORKSPACE_CLOSED_CAP, "the stack is capped");

        let mut reopened = Vec::new();
        while let Some(at) = ws.reopen() {
            reopened.push(ws.display_name(at).expect("open"));
        }
        let expected: Vec<String> = (2..(WORKSPACE_CLOSED_CAP + 2))
            .rev()
            .map(|i| format!("w{i}"))
            .collect();
        assert_eq!(reopened, expected, "newest first, the two oldest dropped");
        assert_eq!(ws.closed_len(), 0);
        assert_eq!(ws.active(), Some(reopened.len() - 1));
    }

    #[test]
    fn reopen_does_nothing_at_the_cap_and_keeps_the_stack() {
        let mut ws = build(&[("a", false)]);
        let at = named(&mut ws, "gone");
        ws.close(at);
        for _ in 0..(WORKSPACE_CAP - 1) {
            ws.new_workspace();
        }
        assert_eq!(ws.reopen(), None, "full: nothing reopens");
        assert_eq!(ws.closed_len(), 1, "the stack is left as it was");
    }

    #[test]
    fn activate_checks_the_bounds() {
        let mut ws = build(&[("a", false), ("b", false)]);
        assert!(ws.activate(0));
        assert_eq!(ws.active(), Some(0));
        assert!(!ws.activate(2));
        assert_eq!(ws.active(), Some(0), "a bad index changes nothing");
    }

    #[test]
    fn encode_decode_round_trip_keeps_pins_names_histories_and_areas() {
        let mut ws = build(&[("first", true), ("second", false)]);
        {
            let first = ws.get_mut(0).expect("open").pages_mut();
            first.open("/home/todos", "To-dos");
            first.navigate("/home/time?view=mine", "Time");
            first.back();
        }
        {
            let second = ws.get_mut(1).expect("open");
            second.set_area(Area::Projects);
            second.set_sidebar_selected(3);
        }
        ws.rename(1, Some("Tab;with|odd=chars\tand\nnewline é".to_owned()));
        ws.activate(1);

        let text = ws.encode().expect("encodes");
        let decoded = Workspaces::decode(&text).expect("decodes");
        assert_eq!(decoded.encode().expect("encodes"), text, "stable");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.active(), Some(1));
        assert_eq!(decoded.closed_len(), 0, "the closed stack is not saved");
        assert_eq!(names(&decoded), names(&ws));
        let first = decoded.get(0).expect("open");
        assert!(first.pinned());
        assert_eq!(first.pages().active, 1);
        assert_eq!(first.pages().pages[1].history.current(), "/home/todos");
        assert!(
            first.pages().pages[1].history == ws.get(0).expect("open").pages().pages[1].history
        );
        let second = decoded.get(1).expect("open");
        assert_eq!(second.area(), Area::Projects);
        assert_eq!(second.sidebar_selected(), 3);
        assert_eq!(second.name(), Some("Tab;with|odd=chars\tand\nnewline é"));
    }

    #[test]
    fn empty_state_round_trips() {
        let ws = Workspaces::default();
        let text = ws.encode().expect("encodes");
        let decoded = Workspaces::decode(&text).expect("decodes");
        assert!(decoded.is_empty());
        assert_eq!(decoded.active(), None);
    }

    #[test]
    fn decode_accepts_a_minimal_valid_text() {
        let ok = Workspaces::decode("v1\nactive\t0\nws\thome\t0\t0\t-\t0\t0").expect("valid");
        assert_eq!(ok.len(), 1);
    }

    #[test]
    fn decode_rejects_garbage_and_bad_versions_without_panicking() {
        for text in [
            "",
            "garbage",
            "v3\nactive\t-",
            "v1",
            "v1\nactive\t0",
            "v1\nactive\t-\nws\thome\t0\t0\t-\t0\t0",
            "v1\nactive\t1\nws\thome\t0\t0\t-\t0\t0",
            "v1\nactive\t-\nextra",
            "v1\nactive\t0\nws\tnowhere\t0\t0\t-\t0\t0",
            "v1\nactive\t0\nws\thome\tx\t0\t-\t0\t0",
            "v1\nactive\t0\nws\thome\t0\t2\t-\t0\t0",
            "v1\nactive\t0\nws\thome\t0\t0\tzz\t0\t0",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t1\t0",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t1\t0\npage\t0\t2f\tgarbage",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t1\t1\npage\t0\t2f\tv1;c=0;e=/home",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t1\t0\npage\t0\t2\tv1;c=0;e=/home",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t1\t0\npage\t0\t2f\tv1;c=0;e=/home\nextra",
            "v1\nactive\t0\nws\thome\t0\t0\t6\t0\t0",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t0\t0\nws\thome\t0\t0\t-\t0\t0\n",
            "v1\nactive\t0\nws\thome\t0\t0\t0\t0\t0\nws\thome\t0\t0\t1\t0\t0",
            "v1\nactive\t0\nws\thome\t0\t0\t-\t0\t0\n\n",
        ] {
            assert_eq!(Workspaces::decode(text).map(|_| ()), None, "{text:?}");
        }
        let good = build(&[("a", false), ("b", true)])
            .encode()
            .expect("encodes");
        for end in 0..good.len() {
            let _ = Workspaces::decode(&good[..end]);
        }
    }

    #[test]
    fn decode_rejects_more_workspaces_than_the_cap() {
        let mut text = String::from("v1\nactive\t0");
        for _ in 0..=WORKSPACE_CAP {
            text.push_str("\nws\thome\t0\t0\t-\t0\t0");
        }
        assert_eq!(Workspaces::decode(&text).map(|_| ()), None);
    }

    #[test]
    fn store_round_trip_and_bad_text_loads_as_none() {
        let store = MemoryStore::default();
        assert!(
            load_workspaces::<TestRoutes>(&store).is_none(),
            "nothing saved yet"
        );
        let ws = build(&[("a", false)]);
        save_workspaces(&store, &ws).expect("saves");
        let loaded = load_workspaces::<TestRoutes>(&store).expect("loads");
        assert_eq!(loaded.encode(), ws.encode());
        store.save("not a workspace file").expect("saves");
        assert!(load_workspaces::<TestRoutes>(&store).is_none());
    }

    #[test]
    fn hex_round_trips_and_rejects_bad_input() {
        assert_eq!(unhex(&hex("é;\n")).as_deref(), Some("é;\n"));
        assert_eq!(unhex("zz"), None);
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("é"), None);
    }

    /// A store that counts its writes, for the save tests.
    #[derive(Default)]
    struct SpyStore {
        text: RefCell<Option<String>>,
        writes: std::cell::Cell<u32>,
    }

    impl Store for SpyStore {
        fn load(&self) -> Option<String> {
            self.text.borrow().clone()
        }

        fn save(&self, text: &str) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            *self.text.borrow_mut() = Some(text.to_owned());
            Ok(())
        }
    }

    /// Several workspaces: a split layout with two groups, pinned and renamed workspaces, histories and a closed one.
    fn sample() -> Workspaces {
        let mut ws = Workspaces::default();
        let first = ws.new_workspace().expect("room");
        ws.get_mut(first)
            .expect("first")
            .pages_mut()
            .navigate("/home/todos", "To-dos");
        ws.rename(first, Some("Sales & ops".to_owned()));
        let second = ws.new_workspace().expect("room");
        {
            let tab = ws.get_mut(second).expect("second");
            tab.pages_mut().open("/home/notes", "Notes");
            tab.pages_mut().open_new_tab("/home/time", "Time");
            let group = tab.tiles().focused_group();
            tab.tiles_mut()
                .split(group, crate::tiles::Edge::Right, Some((group, 1)))
                .expect("a split with two tabs");
        }
        ws.set_pinned(second, true);
        let third = ws.new_workspace().expect("room");
        ws.rename(third, Some("Third".to_owned()));
        ws.close(third);
        ws.activate(0);
        ws
    }

    #[test]
    fn multi_workspace_state_round_trips_through_a_file_store() {
        let dir =
            std::env::temp_dir().join(format!("rimelight-space-workspaces-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = FileStore::at(dir.join("nested").join("workspaces.txt"));
        assert!(
            load_workspaces::<TestRoutes>(&store).is_none(),
            "no file yet"
        );

        let ws = sample();
        save_workspaces(&store, &ws).expect("the file store writes, creating the folder");
        let loaded = load_workspaces::<TestRoutes>(&store).expect("the file loads");
        assert_eq!(loaded.encode(), ws.encode());
        assert_eq!(loaded.len(), 2);
        // The pinned workspace (the split one) moved to the front of the strip.
        assert_eq!(loaded.pinned_flags(), vec![true, false]);
        assert_eq!(loaded.get(0).map(|tab| tab.tiles().group_count()), Some(2));
        assert_eq!(loaded.display_name(1).as_deref(), Some("Sales & ops"));
        assert_eq!(loaded.get(1).map(|tab| tab.tiles().group_count()), Some(1));
        assert_eq!(loaded.closed_len(), 0, "the closed stack is not saved");
        std::fs::remove_dir_all(&dir).expect("cleans up the temp folder");
    }

    #[test]
    fn corrupt_file_starts_as_one_blank_workspace_and_is_not_a_crash() {
        let dir =
            std::env::temp_dir().join(format!("rimelight-space-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        let path = dir.join("workspaces.txt");
        std::fs::write(&path, "v9\nunknown version\n\u{0}\u{1}").expect("writes");
        let (ws, _saver, loaded) = startup::<TestRoutes>(Rc::new(FileStore::at(path)));
        assert!(!loaded, "a corrupt save is not loaded");
        assert_eq!(ws.len(), 1, "one workspace");
        assert_eq!(ws.active(), Some(0));
        assert_eq!(ws.get(0).map(|tab| tab.tiles().group_count()), Some(1));
        std::fs::remove_dir_all(&dir).expect("cleans up the temp folder");
    }

    #[test]
    fn startup_reads_the_store_once_and_keeps_a_good_save() {
        let spy = Rc::new(SpyStore::default());
        let ws = sample();
        *spy.text.borrow_mut() = Some(ws.encode().expect("encodes"));
        let (loaded, saver, is_loaded) = startup::<TestRoutes>(spy.clone());
        assert!(is_loaded);
        assert_eq!(loaded.encode(), ws.encode());
        assert_eq!(
            saver.save(&loaded),
            Ok(false),
            "the same text is not written again"
        );
        assert_eq!(spy.writes.get(), 0);
    }

    #[test]
    fn saver_writes_only_when_the_text_changes() {
        let spy = Rc::new(SpyStore::default());
        let saver = Saver::new(spy.clone(), None);
        let mut ws = Workspaces::with_blank_workspace();
        assert_eq!(saver.save(&ws), Ok(true), "the first save writes");
        assert_eq!(
            saver.save(&ws),
            Ok(false),
            "an unchanged state writes nothing"
        );
        ws.rename(0, Some("Renamed".to_owned()));
        assert_eq!(saver.save(&ws), Ok(true), "a change writes");
        assert_eq!(spy.writes.get(), 2);
        assert_eq!(
            Workspaces::decode(&spy.text.borrow().clone().expect("saved"))
                .expect("decodes")
                .display_name(0)
                .as_deref(),
            Some("Renamed")
        );
    }

    #[test]
    fn web_start_text_prefers_this_tabs_session_state() {
        assert_eq!(
            choose_start_text(Some("tab"), Some("last"), Some("old")),
            StartText {
                text: Some("tab"),
                migrate: false
            }
        );
        assert_eq!(
            choose_start_text(Some("tab"), None, None),
            StartText {
                text: Some("tab"),
                migrate: false
            }
        );
    }

    #[test]
    fn web_start_text_seeds_a_new_tab_from_the_last_save() {
        assert_eq!(
            choose_start_text(None, Some("last"), Some("old")),
            StartText {
                text: Some("last"),
                migrate: false
            }
        );
    }

    #[test]
    fn web_start_text_migrates_the_old_key_only_when_nothing_newer_exists() {
        assert_eq!(
            choose_start_text(None, None, Some("old")),
            StartText {
                text: Some("old"),
                migrate: true
            }
        );
    }

    #[test]
    fn web_start_text_is_none_when_nothing_is_saved() {
        assert_eq!(
            choose_start_text(None, None, None),
            StartText {
                text: None,
                migrate: false
            }
        );
    }

    #[test]
    fn empty_workspaces_are_a_valid_saved_state() {
        let spy = Rc::new(SpyStore::default());
        let saver = Saver::new(spy.clone(), None);
        assert_eq!(saver.save(&Workspaces::default()), Ok(true));
        let (loaded, _, is_loaded) = startup::<TestRoutes>(spy);
        assert!(
            is_loaded,
            "an empty save loads as the empty state, not a fallback"
        );
        assert!(loaded.is_empty());
    }
}
