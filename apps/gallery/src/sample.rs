//! Sample data for the stories that show navigation: the command palette's pages, the shell's rail areas with their
//! work-queue badges, and each area's sidebar rows. The gallery is app-independent, so it reads no app's route manifest.
//! Every name here is invented for the gallery. An app that wants its own pages or areas shows them in its own stories.

use rimelight_shell::palette::Entry;

/// One rail area: its title, its Lucide icon name (a property of the window's `Icons` global), its work-queue noun and
/// mock count (`None` for an area with no queue, which shows no badge), and its sidebar rows.
pub struct Area {
    pub title: &'static str,
    pub icon: &'static str,
    pub noun: Option<&'static str>,
    pub count: u32,
    pub rows: &'static [Row],
}

/// One sidebar row: a section heading (`icon` None) or a page with its icon and an optional count.
pub struct Row {
    pub label: &'static str,
    pub icon: Option<&'static str>,
    pub count: u32,
}

const fn heading(label: &'static str) -> Row {
    Row {
        label,
        icon: None,
        count: 0,
    }
}

const fn page(label: &'static str, icon: &'static str) -> Row {
    Row {
        label,
        icon: Some(icon),
        count: 0,
    }
}

const AREAS: &[Area] = &[
    Area {
        title: "Home",
        icon: "house",
        noun: None,
        count: 0,
        rows: &[
            heading("Start"),
            page("Dashboard", "layout-dashboard"),
            page("Favourites", "star"),
            page("Recent", "history"),
        ],
    },
    Area {
        title: "Chat",
        icon: "message-square",
        noun: Some("unread"),
        count: 4,
        rows: &[
            heading("Conversations"),
            page("Direct messages", "messages-square"),
            page("Channels", "hash"),
            page("Mentions", "at-sign"),
        ],
    },
    Area {
        title: "Projects",
        icon: "folder-kanban",
        noun: None,
        count: 0,
        rows: &[
            heading("Work"),
            page("Boards", "kanban"),
            page("Backlog", "list-todo"),
            page("Sprints", "timer"),
            page("Roadmap", "gantt-chart"),
            page("Reviews", "git-pull-request"),
            heading("Library"),
            page("Documents", "file-text"),
            page("Canvases", "pen-tool"),
            page("Assets", "folder"),
            heading("Archive"),
            page("Closed sprints", "history"),
        ],
    },
    Area {
        title: "People",
        icon: "users",
        noun: Some("waiting"),
        count: 3,
        rows: &[
            heading("Directory"),
            page("Employees", "user-round"),
            page("Teams", "users-round"),
            heading("Time"),
            page("Time off", "palm-tree"),
            page("Timesheets", "clock"),
        ],
    },
    Area {
        title: "Finance",
        icon: "landmark",
        noun: Some("due"),
        count: 12,
        rows: &[
            heading("Billing"),
            page("Invoices", "receipt"),
            page("Expenses", "receipt-text"),
            page("Payments", "banknote"),
        ],
    },
    Area {
        title: "Settings",
        icon: "settings",
        noun: None,
        count: 0,
        rows: &[
            heading("Workspace"),
            page("General", "settings"),
            page("Members", "user-cog"),
            page("Security", "shield-check"),
            heading("Account"),
            page("Profile", "user-round"),
        ],
    },
];

/// The six sample rail areas, in rail order.
pub fn areas() -> &'static [Area] {
    AREAS
}

/// The sample pages the command palette lists: a title, a route and search aliases.
pub fn palette_entries() -> Vec<Entry> {
    let page = |title: &'static str, route: &str, aliases: &'static [&'static str]| Entry {
        title,
        route: route.to_owned(),
        aliases,
    };
    vec![
        page("Home", "/home", &["Start"]),
        page("Inbox", "/chat/inbox", &["Mail", "Messages"]),
        page("Calendar", "/calendar", &["Schedule", "Events"]),
        page("Projects", "/projects", &["Boards"]),
        page("Issues", "/projects/issues", &["Bugs", "Tickets"]),
        page("Canvases", "/projects/canvases", &["Whiteboards"]),
        page("Documents", "/documents", &["Files", "Notes"]),
        page("People", "/people", &["Directory", "Staff"]),
        page("Teams", "/people/teams", &["Groups"]),
        page("Invoices", "/finance/invoices", &["Billing", "Bills"]),
        page("Reports", "/analytics/reports", &["Charts", "Dashboards"]),
        page("Settings", "/settings", &["Preferences", "Config"]),
        page("Members", "/settings/members", &["Users", "Access"]),
        page("Help", "/help", &["Support", "Docs"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_area_starts_with_a_heading_and_has_a_title_and_icon() {
        for area in areas() {
            assert!(!area.title.is_empty() && !area.icon.is_empty());
            assert!(
                area.rows.first().is_some_and(|row| row.icon.is_none()),
                "{}",
                area.title
            );
        }
    }

    #[test]
    fn only_areas_with_a_noun_show_a_count() {
        for area in areas() {
            if area.noun.is_none() {
                assert_eq!(area.count, 0, "{} has no queue", area.title);
            }
        }
    }

    #[test]
    fn the_palette_lists_between_ten_and_fifteen_pages() {
        let entries = palette_entries();
        assert!((10..=15).contains(&entries.len()), "{}", entries.len());
        assert!(entries.iter().all(|entry| entry.route.starts_with('/')));
    }
}
