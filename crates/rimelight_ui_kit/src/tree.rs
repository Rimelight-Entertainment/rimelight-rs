//! Plain-Rust rules of the P0.17 batch 3 Tree. The Slint Tree shows the rows it is given. This module decides which
//! nodes of a flat list are visible, and which visible row is the parent of each row, so the Tree can move the focus
//! to the parent with Left.

/// One node of the caller's flat list. Depth 0 is a top-level node. A node's children follow it in the list, each one
/// deeper than its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatNode {
    pub depth: usize,
    pub expanded: bool,
}

/// A visible row: the index of its node in the flat list, its depth and state, whether the node has children (the next
/// node is deeper), and the visible row of its parent (`None` for a top-level row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleRow {
    pub node: usize,
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub parent_row: Option<usize>,
}

/// The visible rows of a flat list, in order. A node is hidden when any ancestor is collapsed.
pub fn visible_rows(nodes: &[FlatNode]) -> Vec<VisibleRow> {
    let mut rows = Vec::new();
    // Depth of the nearest collapsed node. Nodes deeper than it are hidden until a node at or above that depth.
    let mut hide_below: Option<usize> = None;
    // (depth, row) of the visible rows on the path to the current node.
    let mut path: Vec<(usize, usize)> = Vec::new();
    for (node, flat) in nodes.iter().enumerate() {
        if let Some(depth) = hide_below {
            if flat.depth > depth {
                continue;
            }
            hide_below = None;
        }
        while path.last().is_some_and(|&(depth, _)| depth >= flat.depth) {
            path.pop();
        }
        let has_children = nodes
            .get(node + 1)
            .is_some_and(|next| next.depth > flat.depth);
        let row = rows.len();
        rows.push(VisibleRow {
            node,
            depth: flat.depth,
            expanded: flat.expanded,
            has_children,
            parent_row: path.last().map(|&(_, parent)| parent),
        });
        path.push((flat.depth, row));
        if !flat.expanded {
            hide_below = Some(flat.depth);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsed_nodes_hide_their_descendants() {
        let nodes = [
            FlatNode {
                depth: 0,
                expanded: true,
            },
            FlatNode {
                depth: 1,
                expanded: false,
            },
            FlatNode {
                depth: 2,
                expanded: true,
            },
            FlatNode {
                depth: 1,
                expanded: true,
            },
            FlatNode {
                depth: 0,
                expanded: false,
            },
            FlatNode {
                depth: 1,
                expanded: true,
            },
        ];
        let rows: Vec<usize> = visible_rows(&nodes).iter().map(|r| r.node).collect();
        assert_eq!(rows, vec![0, 1, 3, 4]);
    }

    #[test]
    fn parent_rows_and_children_flags() {
        let nodes = [
            FlatNode {
                depth: 0,
                expanded: true,
            },
            FlatNode {
                depth: 1,
                expanded: true,
            },
            FlatNode {
                depth: 0,
                expanded: true,
            },
        ];
        let rows = visible_rows(&nodes);
        assert!(rows[0].has_children && rows[0].parent_row.is_none());
        assert_eq!(rows[1].parent_row, Some(0));
        assert!(!rows[1].has_children);
        assert!(!rows[2].has_children && rows[2].parent_row.is_none());
    }
}
