//! Tile groups (planned feature B, tiles: `PLAN.md section 6.6` part B, owner decision 2026-11-13). Plain Rust, always
//! compiled, so its rules run without a window.
//!
//! A [`TileLayout`] holds the page tabs of one workspace in a tree of groups. A [`Group`] is a leaf with its own
//! [`PageTabs`] (pinned tabs, closed stack and active tab). Each tab keeps its own history, so back and forward act per
//! tab. A split is an inner node with two children, side by side or stacked, and a ratio. A layout holds at most
//! [`GROUP_CAP`] groups and always has one focused group: the shared controls act on it.
//!
//! The window will draw the flat lists from [`TileLayout::layout_rects`], since Slint cannot recurse into components
//! (the window part is not built yet, see `PLAN.md section 11.2`).
//!
//! Persistence. [`TileLayout::encode_rows`] and [`TileLayout::decode_rows`] write the tree as text rows in pre-order: a
//! `split` row, then its first child and its second child, or a `group` row, then its page rows (`workspace.rs` writes
//! the page rows).
//!
//! The app's [`Routes`] give the Home route and its title (see [`crate::routes`]).

use crate::routes::Routes;
use crate::tabs::{PageTab, PageTabs};
use crate::workspace::{decode_pages, encode_pages, pinned_first};

/// The most groups one workspace holds.
pub const GROUP_CAP: usize = 6;

/// The smallest and the largest ratio of a split: the share of the space that its first child takes.
pub const RATIO_MIN: f32 = 0.1;
pub const RATIO_MAX: f32 = 0.9;

/// The ratio of a new split.
const RATIO_EVEN: f32 = 0.5;

/// The id of a group. Ids are unique within one layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(pub u32);

/// The edge of a group where a dragged tab is dropped. The group splits on that side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// How the two children of a split are arranged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Side by side: the split runs along x, and the first child is on the left.
    Horizontal,
    /// Stacked: the split runs along y, and the first child is on top.
    Vertical,
}

/// One child of a split. A path of sides leads from the root to a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    First,
    Second,
}

/// Why [`TileLayout::split`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitError {
    /// [`GROUP_CAP`] groups already exist.
    AtCap,
    /// The group to split, or the group the moved tab comes from, does not exist.
    NoSuchGroup,
    /// The moved tab index is not a tab of its group.
    NoSuchTab,
    /// The moved tab is the only tab of the group being split, so that group would be left empty.
    OnlyTab,
}

/// Why [`TileLayout::move_tab`] refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    NoSuchGroup,
    NoSuchTab,
}

/// A leaf of the tree: one group of page tabs.
#[derive(Clone)]
pub struct Group<R: Routes> {
    id: GroupId,
    tabs: PageTabs<R>,
}

impl<R: Routes> Group<R> {
    /// The id of this group.
    pub fn id(&self) -> GroupId {
        self.id
    }

    /// The page tabs of this group.
    pub fn tabs(&self) -> &PageTabs<R> {
        &self.tabs
    }

    /// The page tabs of this group, for changing.
    #[cfg(test)]
    pub fn tabs_mut(&mut self) -> &mut PageTabs<R> {
        &mut self.tabs
    }
}

/// A node of the tree: a group, or a split of two nodes.
#[derive(Clone)]
enum Node<R: Routes> {
    Group(Group<R>),
    Split {
        orientation: Orientation,
        ratio: f32,
        first: Box<Node<R>>,
        second: Box<Node<R>>,
    },
}

/// The tile groups of one workspace: a tree of groups and the focused one.
#[derive(Clone)]
pub struct TileLayout<R: Routes> {
    root: Node<R>,
    focused: GroupId,
    next_id: u32,
}

/// A group's rectangle, in the units given to [`TileLayout::layout_rects`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupRect {
    pub id: GroupId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// The bar between the two children of a split. `path` leads to that split from the root, and `orientation` is the
/// orientation of the split (a horizontal split has a vertical bar).
#[derive(Clone, Debug, PartialEq)]
pub struct HandleRect {
    pub path: Vec<Side>,
    pub orientation: Orientation,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl<R: Routes> TileLayout<R> {
    /// A layout of one group (id 0, focused) that holds `tabs`.
    pub fn single(tabs: PageTabs<R>) -> Self {
        Self {
            root: Node::Group(Group {
                id: GroupId(0),
                tabs,
            }),
            focused: GroupId(0),
            next_id: 1,
        }
    }

    /// The number of groups.
    pub fn group_count(&self) -> usize {
        self.groups().len()
    }

    /// The ids of the groups in layout order: depth first, the first child before the second.
    pub fn groups(&self) -> Vec<GroupId> {
        let mut ids = Vec::new();
        collect_ids(&self.root, &mut ids);
        ids
    }

    /// The focused group. The shared controls act on it.
    pub fn focused_group(&self) -> GroupId {
        self.focused
    }

    /// Focuses group `id`. Returns false (and changes nothing) when there is no such group.
    pub fn focus(&mut self, id: GroupId) -> bool {
        let exists = self.group(id).is_some();
        if exists {
            self.focused = id;
        }
        exists
    }

    /// The group with id `id`.
    pub fn group(&self, id: GroupId) -> Option<&Group<R>> {
        find(&self.root, id)
    }

    /// The group with id `id`, for changing it.
    pub fn group_mut(&mut self, id: GroupId) -> Option<&mut Group<R>> {
        find_mut(&mut self.root, id)
    }

    /// The page tabs of the focused group.
    pub fn focused_tabs(&self) -> &PageTabs<R> {
        &self
            .group(self.focused)
            .unwrap_or_else(|| first_group(&self.root))
            .tabs
    }

    /// The page tabs of the focused group, for changing. A focused id that is not in the layout moves the focus to the
    /// first group first.
    pub fn focused_tabs_mut(&mut self) -> &mut PageTabs<R> {
        if self.group(self.focused).is_none() {
            self.focused = first_group(&self.root).id;
        }
        let focused = self.focused;
        match find_mut(&mut self.root, focused) {
            Some(group) => &mut group.tabs,
            None => unreachable!("the focused group is in the layout"),
        }
    }

    /// Splits group `group` on `edge`. The new group goes on that side and becomes focused. Its tab is the tab `moved`
    /// (group and index in that group), which leaves its own group, or, with no tab moved, a new page on Home. A group
    /// that the moved tab leaves empty closes, and its neighbour takes the space.
    ///
    /// Refused with [`SplitError::AtCap`] at [`GROUP_CAP`] groups, with [`SplitError::NoSuchGroup`] or
    /// [`SplitError::NoSuchTab`] for an unknown group or tab, and with [`SplitError::OnlyTab`] when the moved tab is the
    /// only tab of `group` itself (the group would be left empty).
    pub fn split(
        &mut self,
        group: GroupId,
        edge: Edge,
        moved: Option<(GroupId, usize)>,
    ) -> Result<GroupId, SplitError> {
        if self.group_count() >= GROUP_CAP {
            return Err(SplitError::AtCap);
        }
        if self.group(group).is_none() {
            return Err(SplitError::NoSuchGroup);
        }
        let mut tabs = PageTabs::<R>::default();
        match moved {
            None => tabs.open(R::HOME_ROUTE, &home_title::<R>()),
            Some((from, index)) => {
                let source = self.group(from).ok_or(SplitError::NoSuchGroup)?;
                if index >= source.tabs.pages.len() {
                    return Err(SplitError::NoSuchTab);
                }
                if from == group && source.tabs.pages.len() < 2 {
                    return Err(SplitError::OnlyTab);
                }
                let tab = self.take_tab(from, index).ok_or(SplitError::NoSuchTab)?;
                tabs.pages.push(tab);
            }
        }
        let id = GroupId(self.next_id);
        self.next_id += 1;
        let (orientation, new_first) = match edge {
            Edge::Left => (Orientation::Horizontal, true),
            Edge::Right => (Orientation::Horizontal, false),
            Edge::Top => (Orientation::Vertical, true),
            Edge::Bottom => (Orientation::Vertical, false),
        };
        let mut added = Some(Node::Group(Group { id, tabs }));
        split_leaf(&mut self.root, group, orientation, new_first, &mut added);
        self.focused = id;
        Ok(id)
    }

    /// Moves tab `index` of group `from` into group `to`, at index `to_index` counted after the tab has left `from`, and
    /// focuses `to`. A pinned tab stays in the pinned zone of its new group, and an unpinned tab stays after it. A group
    /// that the move leaves empty closes, and its parent split collapses onto its neighbour. A move within one group
    /// reorders its tabs.
    pub fn move_tab(
        &mut self,
        from: GroupId,
        index: usize,
        to: GroupId,
        to_index: usize,
    ) -> Result<(), MoveError> {
        let source = self.group(from).ok_or(MoveError::NoSuchGroup)?;
        if index >= source.tabs.pages.len() {
            return Err(MoveError::NoSuchTab);
        }
        if self.group(to).is_none() {
            return Err(MoveError::NoSuchGroup);
        }
        if from == to {
            let group = find_mut(&mut self.root, to).ok_or(MoveError::NoSuchGroup)?;
            let tab = take_raw(&mut group.tabs, index);
            insert_tab(&mut group.tabs, to_index, tab);
        } else {
            let tab = self.take_tab(from, index).ok_or(MoveError::NoSuchTab)?;
            let group = find_mut(&mut self.root, to).ok_or(MoveError::NoSuchGroup)?;
            insert_tab(&mut group.tabs, to_index, tab);
        }
        self.focused = to;
        Ok(())
    }

    /// Closes every group that has no tabs, while more than one group is left, and collapses their parent splits. The
    /// only group stays even when it is empty. Returns how many groups closed.
    pub fn close_empty_groups(&mut self) -> usize {
        let mut closed = 0;
        while self.group_count() > 1 {
            let empty = self.groups().into_iter().find(|id| {
                self.group(*id)
                    .is_some_and(|group| group.tabs.pages.is_empty())
            });
            let Some(id) = empty else {
                break;
            };
            self.remove_group(id);
            closed += 1;
        }
        closed
    }

    /// Sets the ratio of the split at `path` (see [`HandleRect::path`]), clamped to [`RATIO_MIN`] to [`RATIO_MAX`] (a NaN
    /// gives an even split). Returns false when the path does not lead to a split.
    pub fn set_ratio(&mut self, path: &[Side], ratio: f32) -> bool {
        let mut node = &mut self.root;
        for side in path {
            node = match node {
                Node::Group(_) => return false,
                Node::Split { first, second, .. } => match side {
                    Side::First => first.as_mut(),
                    Side::Second => second.as_mut(),
                },
            };
        }
        match node {
            Node::Group(_) => false,
            Node::Split { ratio: stored, .. } => {
                *stored = clamp_ratio(ratio);
                true
            }
        }
    }

    /// The origin and length of the split at `path` along its orientation, for a window of `width` by `height` with bars
    /// `handle` thick (the same geometry as [`TileLayout::layout_rects`]). A drag turns a pointer position into a ratio with
    /// it. `None` when the path does not lead to a split.
    pub fn split_extent(
        &self,
        path: &[Side],
        width: f32,
        height: f32,
        handle: f32,
    ) -> Option<(Orientation, f32, f32)> {
        let mut frame = Frame {
            x: 0.0,
            y: 0.0,
            w: width.max(0.0),
            h: height.max(0.0),
        };
        let handle = handle.max(0.0);
        let mut node = &self.root;
        for side in path {
            let Node::Split {
                orientation,
                ratio,
                first,
                second,
            } = node
            else {
                return None;
            };
            let (first_frame, _bar, second_frame) = divide(frame, *orientation, *ratio, handle);
            (frame, node) = match side {
                Side::First => (first_frame, first.as_ref()),
                Side::Second => (second_frame, second.as_ref()),
            };
        }
        match node {
            Node::Group(_) => None,
            Node::Split { orientation, .. } => match orientation {
                Orientation::Horizontal => Some((*orientation, frame.x, frame.w)),
                Orientation::Vertical => Some((*orientation, frame.y, frame.h)),
            },
        }
    }

    /// The rectangles of the groups and the bars between them, for a window of `width` by `height` with bars `handle`
    /// thick. Together they cover the whole area without overlap. Both lists are flat and in layout order.
    pub fn layout_rects(
        &self,
        width: f32,
        height: f32,
        handle: f32,
    ) -> (Vec<GroupRect>, Vec<HandleRect>) {
        let mut groups = Vec::new();
        let mut handles = Vec::new();
        let mut path = Vec::new();
        let frame = Frame {
            x: 0.0,
            y: 0.0,
            w: width.max(0.0),
            h: height.max(0.0),
        };
        place(
            &self.root,
            frame,
            handle.max(0.0),
            &mut path,
            &mut groups,
            &mut handles,
        );
        (groups, handles)
    }

    /// Takes tab `index` out of group `from` and returns it. The closed stack is not touched. A group left empty closes
    /// when another group is left. `None` when there is no such group or tab.
    fn take_tab(&mut self, from: GroupId, index: usize) -> Option<PageTab> {
        let group = find_mut(&mut self.root, from)?;
        if index >= group.tabs.pages.len() {
            return None;
        }
        let tab = take_raw(&mut group.tabs, index);
        let empty = group.tabs.pages.is_empty();
        if empty && self.group_count() > 1 {
            self.remove_group(from);
        }
        Some(tab)
    }

    /// Removes group `id` from the tree; its parent split collapses onto its sibling. Returns the group that took its
    /// place, or `None` when `id` is the only group or does not exist. The focus moves there when it was on `id`.
    fn remove_group(&mut self, id: GroupId) -> Option<GroupId> {
        if self.group_count() < 2 {
            return None;
        }
        let survivor = remove_leaf(&mut self.root, id)?;
        if self.focused == id {
            self.focused = survivor;
        }
        Some(survivor)
    }

    /// The tree rows of this layout, appended to `rows` (see the module note). The focused group is not in them: the
    /// workspace row holds it. Fails when a route holds a reserved character.
    pub fn encode_rows(&self, rows: &mut Vec<String>) -> Result<(), String> {
        encode_node(&self.root, rows)
    }

    /// The focused group id, for the workspace row.
    pub fn encoded_focus(&self) -> u32 {
        self.focused.0
    }

    /// Reads the tree rows written by [`TileLayout::encode_rows`] from `rows[*at]` and moves `at` past them. `focused` is
    /// the focused group id from the workspace row. `None` for any inconsistency: a repeated group id, more than
    /// [`GROUP_CAP`] groups (or more nodes than a layout of that size has), a ratio outside the clamp, a focused id that
    /// is not in the tree, or a page list that does not match its counts.
    pub fn decode_rows(rows: &[Vec<&str>], at: &mut usize, focused: u32) -> Option<Self> {
        let mut budget = 2 * GROUP_CAP - 1;
        let mut ids = Vec::new();
        let root = decode_node(rows, at, &mut budget, &mut ids)?;
        if ids.len() > GROUP_CAP || !ids.contains(&focused) {
            return None;
        }
        let next_id = ids.iter().copied().max()?.checked_add(1)?;
        Some(Self {
            root,
            focused: GroupId(focused),
            next_id,
        })
    }
}

/// The ratio clamped to [`RATIO_MIN`] to [`RATIO_MAX`]. A NaN or infinite ratio gives an even split.
pub fn clamp_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(RATIO_MIN, RATIO_MAX)
    } else {
        RATIO_EVEN
    }
}

/// The title of the Home page, which a split opens when no tab is moved. `"Home"` when the routes give no title.
fn home_title<R: Routes>() -> String {
    R::title(R::HOME_ROUTE).unwrap_or_else(|| "Home".to_owned())
}

fn collect_ids<R: Routes>(node: &Node<R>, ids: &mut Vec<GroupId>) {
    match node {
        Node::Group(group) => ids.push(group.id),
        Node::Split { first, second, .. } => {
            collect_ids(first, ids);
            collect_ids(second, ids);
        }
    }
}

fn find<R: Routes>(node: &Node<R>, id: GroupId) -> Option<&Group<R>> {
    match node {
        Node::Group(group) => (group.id == id).then_some(group),
        Node::Split { first, second, .. } => find(first, id).or_else(|| find(second, id)),
    }
}

fn find_mut<R: Routes>(node: &mut Node<R>, id: GroupId) -> Option<&mut Group<R>> {
    match node {
        Node::Group(group) => (group.id == id).then_some(group),
        Node::Split { first, second, .. } => find_mut(first, id).or_else(|| find_mut(second, id)),
    }
}

/// The first group of the tree in layout order. Every tree has one.
fn first_group<R: Routes>(node: &Node<R>) -> &Group<R> {
    match node {
        Node::Group(group) => group,
        Node::Split { first, .. } => first_group(first),
    }
}

fn is_group<R: Routes>(node: &Node<R>, id: GroupId) -> bool {
    matches!(node, Node::Group(group) if group.id == id)
}

/// Takes a node out of `node`, leaving a placeholder that the caller overwrites.
fn take_node<R: Routes>(node: &mut Node<R>) -> Node<R> {
    std::mem::replace(
        node,
        Node::Group(Group {
            id: GroupId(u32::MAX),
            tabs: PageTabs::default(),
        }),
    )
}

/// Removes tab `index` from `tabs` and returns it. The active index stays on the same tab when that tab survives, and
/// otherwise moves to the tab that takes its place (or the last tab). Unlike [`PageTabs::close`], the tab does not go on
/// the closed stack.
fn take_raw<R: Routes>(tabs: &mut PageTabs<R>, index: usize) -> PageTab {
    let tab = tabs.pages.remove(index);
    let active = tabs.active;
    tabs.active = if tabs.pages.is_empty() {
        0
    } else if index < active {
        active - 1
    } else if index == active {
        index.min(tabs.pages.len() - 1)
    } else {
        active
    };
    tab
}

/// Puts `tab` at index `at` of `tabs` and makes it the active tab. A pinned tab stays in the pinned zone (at most the
/// pinned count), and an unpinned tab stays after it.
fn insert_tab<R: Routes>(tabs: &mut PageTabs<R>, at: usize, tab: PageTab) {
    let pinned_count = tabs.pinned_count();
    let (low, high) = if tab.pinned {
        (0, pinned_count)
    } else {
        (pinned_count, tabs.pages.len())
    };
    let at = at.clamp(low, high);
    tabs.pages.insert(at, tab);
    tabs.active = at;
}

/// Puts the new group `added` beside group `target`, in a split of `orientation`. The new group is the first child when
/// `new_first` holds. Returns false when `target` is not under `node`.
fn split_leaf<R: Routes>(
    node: &mut Node<R>,
    target: GroupId,
    orientation: Orientation,
    new_first: bool,
    added: &mut Option<Node<R>>,
) -> bool {
    if is_group(node, target) {
        let Some(new_node) = added.take() else {
            return false;
        };
        let old = take_node(node);
        let (first, second) = if new_first {
            (new_node, old)
        } else {
            (old, new_node)
        };
        *node = Node::Split {
            orientation,
            ratio: RATIO_EVEN,
            first: Box::new(first),
            second: Box::new(second),
        };
        return true;
    }
    match node {
        Node::Group(_) => false,
        Node::Split { first, second, .. } => {
            split_leaf(first, target, orientation, new_first, added)
                || split_leaf(second, target, orientation, new_first, added)
        }
    }
}

/// Removes the group `id` from under `node`. A split that loses a child is replaced by its other child. Returns the first
/// group of the child that took the place, or `None` when `id` is not under `node`.
fn remove_leaf<R: Routes>(node: &mut Node<R>, id: GroupId) -> Option<GroupId> {
    let (first_is, second_is) = match node {
        Node::Group(_) => return None,
        Node::Split { first, second, .. } => (is_group(first, id), is_group(second, id)),
    };
    if first_is || second_is {
        let Node::Split { first, second, .. } = take_node(node) else {
            return None;
        };
        let survivor = if first_is { *second } else { *first };
        let leader = first_group(&survivor).id;
        *node = survivor;
        return Some(leader);
    }
    match node {
        Node::Group(_) => None,
        Node::Split { first, second, .. } => {
            remove_leaf(first, id).or_else(|| remove_leaf(second, id))
        }
    }
}

/// A rectangle while the layout is placed.
#[derive(Clone, Copy)]
struct Frame {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// Places `node` in `frame`, appending its groups and the bars under it.
fn place<R: Routes>(
    node: &Node<R>,
    frame: Frame,
    handle: f32,
    path: &mut Vec<Side>,
    groups: &mut Vec<GroupRect>,
    handles: &mut Vec<HandleRect>,
) {
    let (orientation, ratio, first, second) = match node {
        Node::Group(group) => {
            groups.push(GroupRect {
                id: group.id,
                x: frame.x,
                y: frame.y,
                w: frame.w,
                h: frame.h,
            });
            return;
        }
        Node::Split {
            orientation,
            ratio,
            first,
            second,
        } => (*orientation, *ratio, first, second),
    };
    let (first_frame, bar, second_frame) = divide(frame, orientation, ratio, handle);
    handles.push(HandleRect {
        path: path.clone(),
        orientation,
        x: bar.x,
        y: bar.y,
        w: bar.w,
        h: bar.h,
    });
    path.push(Side::First);
    place(first, first_frame, handle, path, groups, handles);
    path.pop();
    path.push(Side::Second);
    place(second, second_frame, handle, path, groups, handles);
    path.pop();
}

/// Divides `frame` into the first child, the bar and the second child. The bar is `handle` thick, or the frame's own
/// thickness when that is less, and the children share the rest in the ratio `ratio`.
fn divide(
    frame: Frame,
    orientation: Orientation,
    ratio: f32,
    handle: f32,
) -> (Frame, Frame, Frame) {
    match orientation {
        Orientation::Horizontal => {
            let bar = handle.min(frame.w);
            let first = (frame.w - bar) * ratio;
            let second = frame.w - bar - first;
            (
                Frame {
                    x: frame.x,
                    y: frame.y,
                    w: first,
                    h: frame.h,
                },
                Frame {
                    x: frame.x + first,
                    y: frame.y,
                    w: bar,
                    h: frame.h,
                },
                Frame {
                    x: frame.x + first + bar,
                    y: frame.y,
                    w: second,
                    h: frame.h,
                },
            )
        }
        Orientation::Vertical => {
            let bar = handle.min(frame.h);
            let first = (frame.h - bar) * ratio;
            let second = frame.h - bar - first;
            (
                Frame {
                    x: frame.x,
                    y: frame.y,
                    w: frame.w,
                    h: first,
                },
                Frame {
                    x: frame.x,
                    y: frame.y + first,
                    w: frame.w,
                    h: bar,
                },
                Frame {
                    x: frame.x,
                    y: frame.y + first + bar,
                    w: frame.w,
                    h: second,
                },
            )
        }
    }
}

/// Writes `node` and its children as rows: a `split` row, then the first child, then the second; or a `group` row, then
/// the group's page rows.
fn encode_node<R: Routes>(node: &Node<R>, rows: &mut Vec<String>) -> Result<(), String> {
    match node {
        Node::Group(group) => {
            rows.push(format!(
                "group\t{}\t{}\t{}",
                group.id.0,
                group.tabs.pages.len(),
                group.tabs.active
            ));
            rows.extend(encode_pages(&group.tabs)?);
        }
        Node::Split {
            orientation,
            ratio,
            first,
            second,
        } => {
            let orientation = match orientation {
                Orientation::Horizontal => "h",
                Orientation::Vertical => "v",
            };
            rows.push(format!("split\t{orientation}\t{ratio}"));
            encode_node(first, rows)?;
            encode_node(second, rows)?;
        }
    }
    Ok(())
}

/// Reads one node from `rows[*at]` and its children. `budget` is the number of nodes still allowed, and `ids` collects the
/// group ids read, so a repeated id fails.
fn decode_node<R: Routes>(
    rows: &[Vec<&str>],
    at: &mut usize,
    budget: &mut usize,
    ids: &mut Vec<u32>,
) -> Option<Node<R>> {
    *budget = budget.checked_sub(1)?;
    let row = rows.get(*at)?.as_slice();
    *at += 1;
    match row {
        ["split", orientation, ratio] => {
            let orientation = match *orientation {
                "h" => Orientation::Horizontal,
                "v" => Orientation::Vertical,
                _ => return None,
            };
            let ratio: f32 = ratio.parse().ok()?;
            if !(RATIO_MIN..=RATIO_MAX).contains(&ratio) {
                return None;
            }
            let first = decode_node(rows, at, budget, ids)?;
            let second = decode_node(rows, at, budget, ids)?;
            Some(Node::Split {
                orientation,
                ratio,
                first: Box::new(first),
                second: Box::new(second),
            })
        }
        ["group", id, count, active] => {
            let id: u32 = id.parse().ok()?;
            if ids.contains(&id) {
                return None;
            }
            ids.push(id);
            let count: usize = count.parse().ok()?;
            let active: usize = active.parse().ok()?;
            let pages = decode_pages(rows, at, count)?;
            let active_valid = if pages.is_empty() {
                active == 0
            } else {
                active < pages.len()
            };
            if !active_valid || !pinned_first(&pages, |page| page.pinned) {
                return None;
            }
            Some(Node::Group(Group {
                id: GroupId(id),
                tabs: PageTabs::from_pages(pages, active),
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::Routes;
    use crate::routes::test_routes::TestRoutes;

    type PageTabs = crate::tabs::PageTabs<TestRoutes>;
    type TileLayout = super::TileLayout<TestRoutes>;
    type Workspaces = crate::workspace::Workspaces<TestRoutes>;

    /// Two tabs, `/home/a` and `/home/b`, with the first one active.
    fn two_tabs() -> TileLayout {
        let mut tabs = PageTabs::default();
        tabs.open_new_tab("/home/a", "a");
        tabs.open_new_tab("/home/b", "b");
        tabs.select(0);
        TileLayout::single(tabs)
    }

    /// Three tabs, `a`, `b` and `c`, with the first one active.
    fn three_tabs() -> TileLayout {
        let mut tabs = PageTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open_new_tab(&format!("/home/{name}"), name);
        }
        tabs.select(0);
        TileLayout::single(tabs)
    }

    /// The titles of group `id`'s tabs, in order.
    fn titles(layout: &TileLayout, id: GroupId) -> Vec<String> {
        layout
            .group(id)
            .expect("the group exists")
            .tabs()
            .pages
            .iter()
            .map(|page| page.title.clone())
            .collect()
    }

    fn rect(rects: &[GroupRect], id: GroupId) -> GroupRect {
        *rects
            .iter()
            .find(|rect| rect.id == id)
            .expect("the group has a rectangle")
    }

    const EDGES: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

    #[test]
    fn split_puts_the_moved_tab_in_a_new_group_on_each_edge() {
        for edge in EDGES {
            let mut layout = two_tabs();
            let root = GroupId(0);
            let new = layout
                .split(root, edge, Some((root, 1)))
                .expect("the split is allowed");
            assert_eq!(layout.group_count(), 2, "{edge:?}");
            assert_eq!(titles(&layout, new), ["b"], "{edge:?}");
            assert_eq!(titles(&layout, root), ["a"], "{edge:?}");
            assert_eq!(layout.focused_group(), new, "focus follows the split");

            let (groups, _) = layout.layout_rects(200.0, 100.0, 4.0);
            let (new_rect, old_rect) = (rect(&groups, new), rect(&groups, root));
            match edge {
                Edge::Left => assert!(new_rect.x < old_rect.x && new_rect.y == old_rect.y),
                Edge::Right => assert!(new_rect.x > old_rect.x && new_rect.y == old_rect.y),
                Edge::Top => assert!(new_rect.y < old_rect.y && new_rect.x == old_rect.x),
                Edge::Bottom => assert!(new_rect.y > old_rect.y && new_rect.x == old_rect.x),
            }
        }
    }

    #[test]
    fn split_without_a_moved_tab_opens_a_new_home_group() {
        let mut layout = two_tabs();
        let new = layout
            .split(GroupId(0), Edge::Right, None)
            .expect("the split is allowed");
        let tabs = layout.group(new).expect("the group exists").tabs();
        assert_eq!(tabs.pages.len(), 1);
        assert_eq!(tabs.pages[0].history.current(), TestRoutes::HOME_ROUTE);
        assert_eq!(
            titles(&layout, GroupId(0)),
            ["a", "b"],
            "the source is unchanged"
        );
    }

    #[test]
    fn split_refuses_bad_input_and_the_only_tab_of_its_own_group() {
        let mut tabs = PageTabs::default();
        tabs.open("/home/a", "a");
        let mut layout = TileLayout::single(tabs);
        assert_eq!(
            layout.split(GroupId(0), Edge::Right, Some((GroupId(0), 0))),
            Err(SplitError::OnlyTab)
        );
        assert_eq!(layout.group_count(), 1, "nothing changed");
        assert_eq!(
            layout.split(GroupId(0), Edge::Right, Some((GroupId(0), 3))),
            Err(SplitError::NoSuchTab)
        );
        assert_eq!(
            layout.split(GroupId(9), Edge::Right, None),
            Err(SplitError::NoSuchGroup)
        );
        assert_eq!(
            layout.split(GroupId(0), Edge::Right, Some((GroupId(7), 0))),
            Err(SplitError::NoSuchGroup)
        );
    }

    #[test]
    fn nested_splits_go_to_any_depth_and_the_cap_refuses_a_seventh_group() {
        let mut layout = two_tabs();
        let mut last = GroupId(0);
        for step in 0..(GROUP_CAP - 1) {
            let edge = if step.is_multiple_of(2) {
                Edge::Right
            } else {
                Edge::Bottom
            };
            last = layout.split(last, edge, None).expect("below the cap");
        }
        assert_eq!(layout.group_count(), GROUP_CAP);
        assert_eq!(layout.split(last, Edge::Left, None), Err(SplitError::AtCap));
        assert_eq!(layout.group_count(), GROUP_CAP, "a refusal changes nothing");
        let (groups, handles) = layout.layout_rects(600.0, 400.0, 4.0);
        assert_eq!(groups.len(), GROUP_CAP);
        assert_eq!(handles.len(), GROUP_CAP - 1);
    }

    #[test]
    fn move_tab_between_groups_keeps_pinned_tabs_in_the_pinned_zone() {
        let mut layout = three_tabs();
        let root = GroupId(0);
        let other = layout
            .split(root, Edge::Right, Some((root, 2)))
            .expect("the split is allowed");
        assert_eq!(titles(&layout, root), ["a", "b"]);
        assert_eq!(titles(&layout, other), ["c"]);

        layout
            .group_mut(root)
            .expect("the group exists")
            .tabs_mut()
            .set_pinned(1, true);
        assert_eq!(titles(&layout, root), ["b", "a"]);

        // A pinned tab asked for the end of the strip still lands in the pinned zone (the front).
        assert_eq!(layout.move_tab(root, 0, other, 9), Ok(()));
        assert_eq!(titles(&layout, other), ["b", "c"]);

        // An unpinned tab asked for the front lands after the pinned zone.
        assert_eq!(layout.move_tab(root, 0, other, 0), Ok(()));
        assert_eq!(titles(&layout, other), ["b", "a", "c"]);
        assert_eq!(layout.group_count(), 1, "the group left empty closed");
        assert_eq!(layout.focused_group(), other, "the target is focused");
    }

    #[test]
    fn move_tab_within_a_group_reorders_and_refuses_bad_indexes() {
        let mut layout = three_tabs();
        let root = GroupId(0);
        assert_eq!(layout.move_tab(root, 0, root, 2), Ok(()));
        assert_eq!(titles(&layout, root), ["b", "c", "a"]);
        assert_eq!(layout.move_tab(root, 9, root, 0), Err(MoveError::NoSuchTab));
        assert_eq!(
            layout.move_tab(GroupId(7), 0, root, 0),
            Err(MoveError::NoSuchGroup)
        );
        assert_eq!(
            layout.move_tab(root, 0, GroupId(7), 0),
            Err(MoveError::NoSuchGroup)
        );
    }

    #[test]
    fn moving_the_last_tab_closes_its_group_and_collapses_the_split() {
        let mut layout = three_tabs();
        let root = GroupId(0);
        let right = layout
            .split(root, Edge::Right, Some((root, 2)))
            .expect("the split is allowed");
        // Right column: `right` (c) above `below` (b, moved from the left group).
        let below = layout
            .split(right, Edge::Bottom, Some((root, 1)))
            .expect("the split is allowed");
        assert_eq!(layout.groups(), [root, right, below]);

        // Moving the only tab of `right` into `root` closes `right`, and `below` takes its place.
        assert_eq!(layout.move_tab(right, 0, root, 9), Ok(()));
        assert_eq!(layout.groups(), [root, below]);
        let (groups, _) = layout.layout_rects(300.0, 100.0, 4.0);
        let below_rect = rect(&groups, below);
        assert_eq!(below_rect.y, 0.0);
        assert_eq!(below_rect.h, 100.0, "the neighbour takes the full height");
    }

    #[test]
    fn a_split_that_takes_the_only_tab_of_another_group_closes_that_group() {
        let mut layout = two_tabs();
        let root = GroupId(0);
        let right = layout
            .split(root, Edge::Right, Some((root, 1)))
            .expect("the split is allowed");
        let below = layout
            .split(root, Edge::Bottom, Some((right, 0)))
            .expect("the split is allowed");
        assert_eq!(layout.group_count(), 2, "the emptied group closed");
        assert!(layout.group(right).is_none());
        assert_eq!(titles(&layout, below), ["b"]);
        assert_eq!(layout.focused_group(), below);
    }

    #[test]
    fn focus_follows_splits_moves_and_closed_groups() {
        let mut layout = two_tabs();
        let root = GroupId(0);
        let new = layout
            .split(root, Edge::Right, Some((root, 1)))
            .expect("the split is allowed");
        assert_eq!(layout.focused_group(), new);
        assert!(layout.focus(root));
        assert!(!layout.focus(GroupId(42)), "no such group");
        assert_eq!(layout.focused_group(), root);

        // Close the only tab of the focused group: the sweep closes that group and the focus goes to its neighbour.
        assert!(layout.focus(new));
        assert!(layout.focused_tabs_mut().close(0));
        assert_eq!(layout.close_empty_groups(), 1);
        assert_eq!(layout.groups(), [root]);
        assert_eq!(layout.focused_group(), root);

        // The only group stays, even when it is empty.
        let mut lone = two_tabs();
        assert!(lone.focused_tabs_mut().close(0));
        assert!(lone.focused_tabs_mut().close(0));
        assert_eq!(lone.close_empty_groups(), 0);
        assert_eq!(lone.group_count(), 1);
    }

    #[test]
    fn layout_rects_cover_the_area_without_overlap_and_bars_sit_between_siblings() {
        let mut layout = two_tabs();
        let root = GroupId(0);
        let right = layout
            .split(root, Edge::Right, None)
            .expect("the split is allowed");
        let bottom = layout
            .split(right, Edge::Bottom, None)
            .expect("the split is allowed");
        let (width, height, bar) = (300.0_f32, 200.0_f32, 6.0_f32);
        let (groups, handles) = layout.layout_rects(width, height, bar);
        assert_eq!(groups.len(), 3);
        assert_eq!(handles.len(), 2);

        let boxes: Vec<(f32, f32, f32, f32)> = groups
            .iter()
            .map(|r| (r.x, r.y, r.w, r.h))
            .chain(handles.iter().map(|h| (h.x, h.y, h.w, h.h)))
            .collect();
        let area: f32 = boxes.iter().map(|b| b.2 * b.3).sum();
        assert!(
            (area - width * height).abs() < 0.01,
            "the areas add up: {area}"
        );
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let overlap_x = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
                let overlap_y = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
                assert!(
                    overlap_x <= 0.001 || overlap_y <= 0.001,
                    "{a:?} overlaps {b:?}"
                );
            }
        }

        let (root_rect, right_rect, bottom_rect) = (
            rect(&groups, root),
            rect(&groups, right),
            rect(&groups, bottom),
        );
        let outer = handles
            .iter()
            .find(|h| h.path.is_empty())
            .expect("the root bar");
        assert_eq!(outer.orientation, Orientation::Horizontal);
        assert_eq!(
            outer.x,
            root_rect.x + root_rect.w,
            "the bar starts at the left group's end"
        );
        assert_eq!(
            outer.x + outer.w,
            right_rect.x,
            "the bar ends where the right column starts"
        );
        let inner = handles
            .iter()
            .find(|h| h.path == [Side::Second])
            .expect("the bar of the right column");
        assert_eq!(inner.orientation, Orientation::Vertical);
        assert_eq!(inner.y, right_rect.y + right_rect.h);
        assert_eq!(inner.y + inner.h, bottom_rect.y);
    }

    #[test]
    fn set_ratio_is_clamped_and_needs_a_split_at_its_path() {
        let mut layout = two_tabs();
        assert!(!layout.set_ratio(&[], 0.3), "a single group has no split");
        layout
            .split(GroupId(0), Edge::Right, Some((GroupId(0), 1)))
            .expect("the split is allowed");
        let first_width = |layout: &TileLayout| {
            let (groups, _) = layout.layout_rects(110.0, 10.0, 10.0);
            rect(&groups, GroupId(0)).w
        };

        assert!(layout.set_ratio(&[], 0.0));
        assert!(
            (first_width(&layout) - 10.0).abs() < 0.001,
            "stops at 0.1 of 100"
        );
        assert!(layout.set_ratio(&[], 5.0));
        assert!(
            (first_width(&layout) - 90.0).abs() < 0.001,
            "stops at 0.9 of 100"
        );
        assert!(layout.set_ratio(&[], f32::NAN));
        assert!(
            (first_width(&layout) - 50.0).abs() < 0.001,
            "NaN is an even split"
        );
        assert!(
            !layout.set_ratio(&[Side::First], 0.5),
            "the first child is a group"
        );
        assert_eq!(clamp_ratio(0.25), 0.25);
    }

    #[test]
    fn a_copied_workspace_has_its_own_tile_layout() {
        let mut ws = Workspaces::default();
        let at = ws.new_workspace().expect("room for a workspace");
        ws.get_mut(at)
            .expect("the workspace is open")
            .tiles_mut()
            .split(GroupId(0), Edge::Right, None)
            .expect("the split is allowed");
        assert!(ws.duplicate(at));
        let copy = ws.active().expect("the copy is active");
        ws.get_mut(copy)
            .expect("the copy is open")
            .tiles_mut()
            .split(GroupId(0), Edge::Bottom, None)
            .expect("the split is allowed");
        assert_eq!(
            ws.get(at).expect("open").tiles().group_count(),
            2,
            "the original is unchanged"
        );
        assert_eq!(ws.get(copy).expect("open").tiles().group_count(), 3);
    }

    #[test]
    fn a_workspace_is_named_after_the_active_tab_of_its_focused_group() {
        let mut ws = Workspaces::default();
        let at = ws.new_workspace().expect("room for a workspace");
        let other = {
            let tiles = ws.get_mut(at).expect("the workspace is open").tiles_mut();
            tiles
                .split(GroupId(0), Edge::Right, None)
                .expect("the split is allowed")
        };
        {
            let tiles = ws.get_mut(at).expect("the workspace is open").tiles_mut();
            assert!(tiles.focus(GroupId(0)));
            tiles.focused_tabs_mut().open("/home/todos", "Tasks");
        }
        assert_eq!(ws.display_name(at).as_deref(), Some("Tasks"));
        assert!(ws.get_mut(at).expect("open").tiles_mut().focus(other));
        assert_ne!(ws.display_name(at).as_deref(), Some("Tasks"));
    }

    #[test]
    fn workspace_v2_round_trip_keeps_groups_tabs_ratio_and_focus() {
        let mut ws = Workspaces::default();
        let at = ws.new_workspace().expect("room for a workspace");
        let root = GroupId(0);
        {
            let tiles = ws.get_mut(at).expect("the workspace is open").tiles_mut();
            tiles.focused_tabs_mut().open("/home/todos", "Tasks");
            let other = tiles
                .split(root, Edge::Bottom, Some((root, 1)))
                .expect("the split is allowed");
            assert_ne!(other, root);
            assert!(tiles.set_ratio(&[], 0.3));
            assert!(tiles.focus(root));
        }

        let text = ws.encode().expect("encodes");
        assert!(text.starts_with("v2\n"), "written as v2");
        let decoded = Workspaces::decode(&text).expect("decodes");
        assert_eq!(decoded.encode().expect("encodes"), text, "stable");
        let before = ws.get(at).expect("open").tiles();
        let after = decoded.get(at).expect("open").tiles();
        assert_eq!(after.group_count(), 2);
        assert_eq!(after.focused_group(), root);
        assert_eq!(
            after.layout_rects(300.0, 200.0, 6.0),
            before.layout_rects(300.0, 200.0, 6.0)
        );
    }

    #[test]
    fn a_v1_workspace_text_decodes_as_one_group() {
        let text = "v1\nactive\t0\nws\thome\t0\t0\t-\t1\t0\npage\t0\t486f6d65\tv1;c=0;e=/home";
        let ws = Workspaces::decode(text).expect("v1 still decodes");
        let tiles = ws.get(0).expect("open").tiles();
        assert_eq!(tiles.group_count(), 1);
        assert_eq!(tiles.focused_group(), GroupId(0));
        assert_eq!(titles(tiles, GroupId(0)), ["Home"]);
        assert!(
            ws.encode().expect("encodes").starts_with("v2\n"),
            "saved again as v2"
        );
    }

    #[test]
    fn v2_garbage_and_inconsistent_trees_decode_to_none_without_panicking() {
        let head = "v2\nactive\t0\nws\thome\t0\t0\t-\t0";
        let mut too_many_groups = String::from(head);
        for _ in 0..6 {
            too_many_groups.push_str("\nsplit\th\t0.5");
        }
        for id in 0..7 {
            too_many_groups.push_str(&format!("\ngroup\t{id}\t0\t0"));
        }
        let deep = format!("{head}{}", "\nsplit\th\t0.5".repeat(1000));
        let cases = [
            String::new(),
            "v3\nactive\t-".to_owned(),
            "v2\nactive\t-\nws\thome\t0\t0\t-\t0".to_owned(),
            format!("{head}\nsplit\th\t0.5\ngroup\t0\t0\t0"),
            format!("{head}\nsplit\tx\t0.5\ngroup\t0\t0\t0\ngroup\t1\t0\t0"),
            format!("{head}\nsplit\th\t0.95\ngroup\t0\t0\t0\ngroup\t1\t0\t0"),
            format!("{head}\nsplit\th\tnan\ngroup\t0\t0\t0\ngroup\t1\t0\t0"),
            format!("{head}\nsplit\th\t0.5\ngroup\t0\t0\t0\ngroup\t0\t0\t0"),
            format!("{head}\ngroup\t0\t0\t0\nextra"),
            "v2\nactive\t0\nws\thome\t0\t0\t-\t5\ngroup\t0\t0\t0".to_owned(),
            "v2\nactive\t0\nws\thome\t0\t0\t-\t0\ngroup\t0\t1\t0".to_owned(),
            "v2\nactive\t0\nws\thome\t0\t0\t-\t0\ngroup\t0\t0\t1".to_owned(),
            too_many_groups,
            deep,
        ];
        for text in &cases {
            assert!(
                Workspaces::decode(text).is_none(),
                "should reject {:?}",
                text.chars().take(80).collect::<String>()
            );
        }

        let mut ws = Workspaces::default();
        let at = ws.new_workspace().expect("room for a workspace");
        ws.get_mut(at)
            .expect("open")
            .tiles_mut()
            .split(GroupId(0), Edge::Left, None)
            .expect("the split is allowed");
        let good = ws.encode().expect("encodes");
        for end in 0..good.len() {
            let _ = Workspaces::decode(&good[..end]);
        }
    }
}
