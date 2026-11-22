//! Tab drag rules (planned feature B, tiles, T2b; `PLAN.md section 6.6` "Tile groups"). Plain Rust, always compiled, so the
//! hit test and the drop plan run without a window. The Slint side only forwards pointer positions (in page area units, px) and
//! draws what [`hit_test`] and [`plan`] say.
//!
//! A group is a page tab strip ([`STRIP_HEIGHT`] tall) over a body. The body has four edge zones, each the outer
//! [`EDGE_FRACTION`] of the group, and a centre. A drop on an edge splits the group there ([`TileLayout::split`]), a drop on
//! the centre moves the tab to the end of that group's strip, and a drop on a strip moves the tab to the slot under the
//! pointer. Slots follow the real tab widths: a pinned tab is [`STRIP_PINNED_WIDTH`] wide, any other [`STRIP_TAB_WIDTH`].
//! Pinned tabs stay in the pinned zone at the left of their strip, and unpinned tabs after it.

use std::cmp::Ordering;

use crate::routes::Routes;
use crate::tiles::{Edge, GroupId, GroupRect, TileLayout};
use rimelight_design::tokens::size;

/// The height of a group's page tab strip, in px (the tab control height).
pub const STRIP_HEIGHT: f32 = size::CONTROL_MD;
/// The width of an unpinned tab in a strip, in px (four control heights; `TabStrip.tab-width` in tabs.slint).
pub const STRIP_TAB_WIDTH: f32 = size::CONTROL_MD * 4.0;
/// The width of a pinned tab in a strip, in px (one control height; `TabStrip.pinned-width` in tabs.slint).
pub const STRIP_PINNED_WIDTH: f32 = size::CONTROL_MD;
/// The outer share of a group that an edge zone takes, on each side.
pub const EDGE_FRACTION: f32 = 0.25;

/// What the pointer is over inside a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// An edge zone: a drop splits the group on that edge.
    Edge(Edge),
    /// The middle of the body: a drop moves the tab to the end of the group's strip.
    Centre,
    /// The strip, at this insertion slot (`0` is before the first tab, the tab count is after the last).
    Strip(usize),
}

/// The group and zone under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub group: GroupId,
    pub zone: Zone,
}

/// A group as the hit test sees it: its rectangle and the pinned flag of each of its tabs, in strip order.
#[derive(Clone, Debug, PartialEq)]
pub struct DropGroup {
    pub rect: GroupRect,
    pub pinned: Vec<bool>,
}

/// What a drop does. [`Plan::Refused`] is the not-allowed state: nothing moves, and the overlay shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Nothing moves: the drop is outside every group, on the source's own centre, or on its own slot.
    Nothing,
    /// The split would break the rules (the cap, or the only tab of the group dropped on its own edge).
    Refused,
    /// Split `group` on `edge`, moving the dragged tab into the new group.
    Split { group: GroupId, edge: Edge },
    /// Move the dragged tab into `group` at `to` (counted as [`TileLayout::move_tab`] counts it). `mark` is the strip slot
    /// the insertion mark shows, or `None` when the order does not change.
    Move {
        group: GroupId,
        to: usize,
        mark: Option<usize>,
    },
}

/// The width of a tab in a strip.
fn width(pinned: bool) -> f32 {
    if pinned {
        STRIP_PINNED_WIDTH
    } else {
        STRIP_TAB_WIDTH
    }
}

/// The insertion slot for strip-local x `x` in a strip of tabs with the given pinned flags. A slot is the boundary between two
/// tabs, and the pointer picks the boundary nearest to it: a pointer left of a tab's middle is before that tab.
pub fn insertion_slot(pinned: &[bool], x: f32) -> usize {
    let mut start = 0.0;
    for (index, pin) in pinned.iter().enumerate() {
        let w = width(*pin);
        if x < start + w / 2.0 {
            return index;
        }
        start += w;
    }
    pinned.len()
}

/// The x of insertion slot `slot` in a strip with the given pinned flags (the sum of the widths before it).
pub fn slot_x(pinned: &[bool], slot: usize) -> f32 {
    pinned.iter().take(slot).map(|pin| width(*pin)).sum()
}

/// Clamps a slot into the zone of a dragged tab. `others` are the pinned flags of the strip without the dragged tab, which
/// has `dragged_pinned`. A pinned tab stays at or before the last pinned tab, and an unpinned tab at or after it.
pub fn zone_slot(others: &[bool], dragged_pinned: bool, slot: usize) -> usize {
    let pinned = others.iter().filter(|pin| **pin).count();
    let slot = slot.min(others.len());
    if dragged_pinned {
        slot.min(pinned)
    } else {
        slot.max(pinned)
    }
}

/// Reorders tab `from` within one strip, dropped at insertion slot `slot` of the full strip. Returns the index to pass to
/// [`TileLayout::move_tab`] (counted after the tab has left) and the mark slot, or `None` when the order does not change.
/// `None` also when `from` is not a tab.
pub fn reorder(pinned: &[bool], from: usize, slot: usize) -> Option<(usize, Option<usize>)> {
    if from >= pinned.len() {
        return None;
    }
    let mut others = pinned.to_vec();
    let dragged = others.remove(from);
    let after = if slot > from { slot - 1 } else { slot };
    let to = zone_slot(&others, dragged, after);
    if to == from {
        return Some((to, None));
    }
    let mark = if to > from { to + 1 } else { to };
    Some((to, Some(mark)))
}

/// The group and zone under the pointer at (`x`, `y`), in page area units. Groups are checked in order. `None` outside every
/// group.
pub fn hit_test(groups: &[DropGroup], x: f32, y: f32) -> Option<Hit> {
    let group = groups.iter().find(|group| {
        let r = group.rect;
        x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
    })?;
    let r = group.rect;
    let local_x = x - r.x;
    let local_y = y - r.y;
    if local_y < STRIP_HEIGHT || r.h <= STRIP_HEIGHT {
        let slot = insertion_slot(&group.pinned, local_x);
        return Some(Hit {
            group: r.id,
            zone: Zone::Strip(slot),
        });
    }
    let body = r.h - STRIP_HEIGHT;
    let fx = local_x / r.w;
    let fy = (local_y - STRIP_HEIGHT) / body;
    let candidates = [
        (fx, Edge::Left),
        (1.0 - fx, Edge::Right),
        (fy, Edge::Top),
        (1.0 - fy, Edge::Bottom),
    ];
    // The nearest edge wins a corner. Only an edge band of EDGE_FRACTION counts.
    let edge = candidates
        .into_iter()
        .filter(|(distance, _)| *distance < EDGE_FRACTION)
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
        .map(|(_, edge)| edge);
    let zone = match edge {
        Some(edge) => Zone::Edge(edge),
        None => Zone::Centre,
    };
    Some(Hit { group: r.id, zone })
}

/// The pinned flags of group `group` of `layout`, in strip order (empty for an unknown group).
pub fn pinned_flags<R: Routes>(layout: &TileLayout<R>, group: GroupId) -> Vec<bool> {
    layout.group(group).map_or_else(Vec::new, |group| {
        group.tabs().pages.iter().map(|tab| tab.pinned).collect()
    })
}

/// The drop groups of a layout laid out in a page area of `width` by `height` with bars `handle` thick.
pub fn drop_groups<R: Routes>(
    layout: &TileLayout<R>,
    width: f32,
    height: f32,
    handle: f32,
) -> Vec<DropGroup> {
    let (rects, _) = layout.layout_rects(width, height, handle);
    rects
        .into_iter()
        .map(|rect| DropGroup {
            pinned: pinned_flags(layout, rect.id),
            rect,
        })
        .collect()
}

/// What dropping tab `index` of group `source` does at `hit` (`None` when the pointer was outside every group). An edge drop
/// is planned on a copy of the layout, so a refusal is exactly the refusal [`TileLayout::split`] gives.
pub fn plan<R: Routes>(
    layout: &TileLayout<R>,
    source: GroupId,
    index: usize,
    hit: Option<Hit>,
) -> Plan {
    let Some(hit) = hit else {
        return Plan::Nothing;
    };
    let Some(group) = layout.group(source) else {
        return Plan::Nothing;
    };
    let Some(page) = group.tabs().pages.get(index) else {
        return Plan::Nothing;
    };
    let dragged_pinned = page.pinned;
    match hit.zone {
        Zone::Edge(edge) => {
            let mut trial = layout.clone();
            match trial.split(hit.group, edge, Some((source, index))) {
                Ok(_) => Plan::Split {
                    group: hit.group,
                    edge,
                },
                Err(_) => Plan::Refused,
            }
        }
        Zone::Centre => {
            if hit.group == source {
                return Plan::Nothing;
            }
            let others = pinned_flags(layout, hit.group);
            let to = zone_slot(&others, dragged_pinned, others.len());
            Plan::Move {
                group: hit.group,
                to,
                mark: None,
            }
        }
        Zone::Strip(slot) => {
            if hit.group == source {
                let flags = pinned_flags(layout, source);
                match reorder(&flags, index, slot) {
                    Some((to, Some(mark))) => Plan::Move {
                        group: source,
                        to,
                        mark: Some(mark),
                    },
                    _ => Plan::Nothing,
                }
            } else {
                let others = pinned_flags(layout, hit.group);
                let to = zone_slot(&others, dragged_pinned, slot);
                Plan::Move {
                    group: hit.group,
                    to,
                    mark: Some(to),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::test_routes::TestRoutes;

    type PageTabs = crate::tabs::PageTabs<TestRoutes>;
    type TileLayout = crate::tiles::TileLayout<TestRoutes>;
    use crate::tiles::GROUP_CAP;

    fn rect(id: u32, x: f32, y: f32, w: f32, h: f32) -> GroupRect {
        GroupRect {
            id: GroupId(id),
            x,
            y,
            w,
            h,
        }
    }

    /// Tabs `a`, `b`, ... in one group (id 0), with the given pinned flags.
    fn layout_with(pins: &[bool]) -> TileLayout {
        let mut tabs = PageTabs::default();
        for (index, pin) in pins.iter().enumerate() {
            let name = ["a", "b", "c", "d", "e"][index];
            tabs.open_new_tab(&format!("/home/{name}"), name);
            tabs.pages[index].pinned = *pin;
        }
        tabs.select(0);
        TileLayout::single(tabs)
    }

    fn one_group(pinned: &[bool]) -> Vec<DropGroup> {
        vec![DropGroup {
            rect: rect(0, 0.0, 0.0, 400.0, 334.0),
            pinned: pinned.to_vec(),
        }]
    }

    #[test]
    fn hit_test_finds_each_edge_zone_and_the_centre() {
        let groups = one_group(&[false]);
        // The body is 300 px tall below the 34 px strip.
        let at = |x, y| hit_test(&groups, x, y).map(|hit| hit.zone);
        assert_eq!(at(10.0, 200.0), Some(Zone::Edge(Edge::Left)));
        assert_eq!(at(390.0, 200.0), Some(Zone::Edge(Edge::Right)));
        assert_eq!(at(200.0, 40.0), Some(Zone::Edge(Edge::Top)));
        assert_eq!(at(200.0, 330.0), Some(Zone::Edge(Edge::Bottom)));
        assert_eq!(at(200.0, 180.0), Some(Zone::Centre));
        // 25% of 400 px is 100 px: the band is below 100 px, so 100 px itself is the centre.
        assert_eq!(at(100.0, 180.0), Some(Zone::Centre));
        assert_eq!(at(99.0, 180.0), Some(Zone::Edge(Edge::Left)));
        // A corner takes the nearer edge.
        assert_eq!(at(5.0, 40.0), Some(Zone::Edge(Edge::Left)));
        assert_eq!(at(200.0, 333.0), Some(Zone::Edge(Edge::Bottom)));
    }

    #[test]
    fn hit_test_strip_insertion_index_follows_pinned_widths() {
        // Pinned tabs are 34 px, the others 136 px: slot boundaries at 17, 51, 136 and 272 for [pinned, pinned, plain, plain].
        let groups = one_group(&[true, true, false, false]);
        let slot = |x| match hit_test(&groups, x, 10.0).map(|hit| hit.zone) {
            Some(Zone::Strip(slot)) => slot,
            other => panic!("expected a strip, got {other:?}"),
        };
        assert_eq!(slot(10.0), 0);
        assert_eq!(slot(30.0), 1);
        assert_eq!(slot(100.0), 2);
        assert_eq!(slot(140.0), 3);
        assert_eq!(slot(390.0), 4);
        // The strip is the top 34 px, even where an edge band of the body would be.
        assert_eq!(
            hit_test(&groups, 2.0, 33.0).map(|hit| hit.zone),
            Some(Zone::Strip(0))
        );
        assert_eq!(slot_x(&[true, true, false], 2), 68.0);
        assert_eq!(slot_x(&[true, true, false], 3), 204.0);
    }

    #[test]
    fn hit_test_outside_every_group_is_none() {
        let groups = vec![
            DropGroup {
                rect: rect(0, 0.0, 0.0, 400.0, 334.0),
                pinned: vec![false],
            },
            DropGroup {
                rect: rect(1, 406.0, 0.0, 400.0, 334.0),
                pinned: vec![false],
            },
        ];
        assert_eq!(hit_test(&groups, -1.0, 100.0), None);
        assert_eq!(
            hit_test(&groups, 402.0, 100.0),
            None,
            "the bar between groups"
        );
        assert_eq!(hit_test(&groups, 100.0, 334.0), None, "below the group");
        assert_eq!(
            hit_test(&groups, 500.0, 40.0).map(|hit| hit.group),
            Some(GroupId(1))
        );
    }

    #[test]
    fn reorder_keeps_pinned_tabs_in_the_pinned_zone() {
        // [pinned P, a, b]: an unpinned tab cannot go before the pinned one.
        let pins = [true, false, false];
        assert_eq!(reorder(&pins, 2, 0), Some((1, Some(1))), "b goes after P");
        // The pinned P cannot leave the pinned zone at the left.
        assert_eq!(reorder(&pins, 0, 3), Some((0, None)), "P stays put");
        // [a, b, c] unpinned: a dropped at the end goes after c.
        let plain = [false, false, false];
        assert_eq!(reorder(&plain, 0, 3), Some((2, Some(3))));
        assert_eq!(reorder(&plain, 2, 0), Some((0, Some(0))));
        assert_eq!(
            reorder(&plain, 0, 1),
            Some((0, None)),
            "the same slot is no move"
        );
        assert_eq!(reorder(&plain, 5, 0), None, "no such tab");
    }

    #[test]
    fn zone_slot_clamps_to_the_zone_of_the_dragged_tab() {
        assert_eq!(zone_slot(&[true, true, false], true, 3), 2);
        assert_eq!(zone_slot(&[true, true, false], false, 0), 2);
        assert_eq!(zone_slot(&[true, false], false, 1), 1);
    }

    #[test]
    fn plan_on_a_strip_of_another_group_moves_at_the_pointer_slot() {
        let mut layout = layout_with(&[false, false]);
        let new = layout
            .split(GroupId(0), Edge::Right, Some((GroupId(0), 1)))
            .expect("a split of two tabs");
        // Group 0 holds `a`; the new group holds `b`.
        let hit = Hit {
            group: new,
            zone: Zone::Strip(0),
        };
        assert_eq!(
            plan(&layout, GroupId(0), 0, Some(hit)),
            Plan::Move {
                group: new,
                to: 0,
                mark: Some(0)
            }
        );
    }

    #[test]
    fn plan_refuses_an_only_tab_on_its_own_edge_and_an_edge_past_the_cap() {
        let layout = layout_with(&[false]);
        let own = Hit {
            group: GroupId(0),
            zone: Zone::Edge(Edge::Left),
        };
        assert_eq!(plan(&layout, GroupId(0), 0, Some(own)), Plan::Refused);

        // Six groups already: an edge drop from another group is refused by the cap.
        let mut layout = layout_with(&[false, false]);
        for _ in 0..(GROUP_CAP - 1) {
            layout
                .split(GroupId(0), Edge::Right, None)
                .expect("a split under the cap");
        }
        assert_eq!(layout.group_count(), GROUP_CAP);
        let edge = Hit {
            group: GroupId(1),
            zone: Zone::Edge(Edge::Top),
        };
        assert_eq!(plan(&layout, GroupId(0), 1, Some(edge)), Plan::Refused);
    }

    #[test]
    fn plan_centre_of_own_group_and_outside_are_nothing() {
        let layout = layout_with(&[false, false]);
        let own = Hit {
            group: GroupId(0),
            zone: Zone::Centre,
        };
        assert_eq!(plan(&layout, GroupId(0), 0, Some(own)), Plan::Nothing);
        assert_eq!(plan(&layout, GroupId(0), 0, None), Plan::Nothing);
    }

    #[test]
    fn plan_edge_drop_splits_with_the_moved_tab() {
        let layout = layout_with(&[false, false]);
        let edge = Hit {
            group: GroupId(0),
            zone: Zone::Edge(Edge::Right),
        };
        assert_eq!(
            plan(&layout, GroupId(0), 1, Some(edge)),
            Plan::Split {
                group: GroupId(0),
                edge: Edge::Right
            }
        );
    }
}
