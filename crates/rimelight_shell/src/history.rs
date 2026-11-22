//! Back and forward history of one page tab (planned feature A, `PLAN.md section 6.6`). Plain Rust, always compiled, so
//! its tests run without a window.
//!
//! A [`History`] is a list of routes and a cursor on the current one. [`History::push`] adds a route after the cursor and
//! drops the forward entries; the list is capped at [`CAP`] entries, and the oldest one goes first.
//!
//! Persistence. [`History::encode`] and [`History::decode`] write and read one line of text: a leading `v1` (the version of
//! this line format), then `c=<cursor>` and `e=<route>|<route>|...`. A route may not be empty or hold `|`, `;` or a control
//! character. [`History::decode`] never panics: any text it cannot read gives `None`. The workspace text that holds these
//! lines is in `workspace.rs`.

/// The version of the history line format.
const FORMAT_VERSION: &str = "v1";

/// The most entries a history keeps. The oldest entry is dropped when a push goes past this.
pub const CAP: usize = 50;

/// The routes of one page tab and the cursor on the current one. Never empty: it always holds the current route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    entries: Vec<String>,
    cursor: usize,
}

impl History {
    /// A history holding only `route`, which is the current one.
    pub fn new(route: &str) -> Self {
        Self {
            entries: vec![route.to_owned()],
            cursor: 0,
        }
    }

    /// The current route.
    pub fn current(&self) -> &str {
        &self.entries[self.cursor]
    }

    /// Makes `route` the current one. Does nothing when it already is. Otherwise the forward entries are dropped, the route
    /// is added at the end, and the oldest entry is dropped when the list is over [`CAP`].
    pub fn push(&mut self, route: &str) {
        if self.current() == route {
            return;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(route.to_owned());
        if self.entries.len() > CAP {
            self.entries.remove(0);
        }
        self.cursor = self.entries.len() - 1;
    }

    /// Moves the cursor to the entry equal to `route`: the nearest one behind the cursor, else the nearest one ahead. When no
    /// entry is equal, `route` is pushed as a new step (see [`History::push`]). This is how a browser Back or Forward lands
    /// on a location (planned feature A3).
    pub fn land_on(&mut self, route: &str) {
        let behind = (0..self.cursor).rev().find(|&i| self.entries[i] == route);
        let ahead = || (self.cursor + 1..self.entries.len()).find(|&i| self.entries[i] == route);
        match behind.or_else(ahead) {
            Some(index) => self.cursor = index,
            None => self.push(route),
        }
    }

    /// Moves the cursor back one entry and returns that route, or `None` when already at the oldest entry.
    pub fn back(&mut self) -> Option<&str> {
        if !self.can_back() {
            return None;
        }
        self.cursor -= 1;
        Some(self.current())
    }

    /// Moves the cursor forward one entry and returns that route, or `None` when already at the newest entry.
    pub fn forward(&mut self) -> Option<&str> {
        if !self.can_forward() {
            return None;
        }
        self.cursor += 1;
        Some(self.current())
    }

    /// True when [`History::back`] would move.
    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    /// True when [`History::forward`] would move.
    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// The text form of this history. Fails when a route is empty or holds a reserved character.
    pub fn encode(&self) -> Result<String, String> {
        for route in &self.entries {
            if route.is_empty() || route.chars().any(reserved) {
                return Err(format!(
                    "route {route:?} is empty or holds a reserved character"
                ));
            }
        }
        Ok(format!(
            "{FORMAT_VERSION};c={};e={}",
            self.cursor,
            self.entries.join("|")
        ))
    }

    /// Reads a history written by [`History::encode`]. Returns `None` for an unknown version, a missing or repeated field,
    /// an empty route, more than [`CAP`] entries, or a cursor past the entries.
    pub fn decode(text: &str) -> Option<Self> {
        let mut parts = text.split(';');
        if parts.next()? != FORMAT_VERSION {
            return None;
        }
        let mut cursor = None;
        let mut entries = None;
        for part in parts {
            let (key, value) = part.split_once('=')?;
            match key {
                "c" if cursor.is_none() => cursor = Some(value.parse::<usize>().ok()?),
                "e" if entries.is_none() => {
                    entries = Some(value.split('|').map(str::to_owned).collect::<Vec<_>>());
                }
                _ => return None,
            }
        }
        let entries = entries?;
        let cursor = cursor?;
        let valid = !entries.is_empty()
            && entries.len() <= CAP
            && cursor < entries.len()
            && entries.iter().all(|route| !route.is_empty());
        valid.then_some(Self { entries, cursor })
    }
}

fn reserved(c: char) -> bool {
    matches!(c, ';' | '|') || c.is_control()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_truncates_forward_entries_after_going_back() {
        let mut h = History::new("/a");
        h.push("/b");
        h.push("/c");
        assert_eq!(h.back(), Some("/b"));
        h.push("/d");
        assert_eq!(h.current(), "/d");
        assert!(!h.can_forward());
        assert_eq!(h.back(), Some("/b"));
        assert_eq!(h.back(), Some("/a"));
        assert_eq!(h.back(), None);
    }

    #[test]
    fn equal_push_is_a_no_op() {
        let mut h = History::new("/a");
        h.push("/a");
        assert!(!h.can_back());
        h.push("/b");
        h.push("/b");
        assert_eq!(h.back(), Some("/a"));
        assert_eq!(h.forward(), Some("/b"));
        assert!(!h.can_forward());
    }

    #[test]
    fn back_and_forward_stop_at_the_ends() {
        let mut h = History::new("/a");
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), None);
        h.push("/b");
        assert_eq!(h.forward(), None);
        assert_eq!(h.back(), Some("/a"));
        assert_eq!(h.back(), None);
        assert_eq!(h.current(), "/a");
        assert_eq!(h.forward(), Some("/b"));
    }

    #[test]
    fn cap_drops_the_oldest_and_keeps_the_cursor_on_the_newest() {
        let mut h = History::new("/0");
        for i in 1..=(CAP + 9) {
            h.push(&format!("/{i}"));
        }
        assert_eq!(h.current(), format!("/{}", CAP + 9));
        let mut steps = 0;
        while h.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, CAP - 1);
        assert_eq!(h.current(), "/10");
    }

    #[test]
    fn round_trip_keeps_entries_and_cursor() {
        let mut h = History::new("/home");
        h.push("/mail");
        h.push("/home?view=x");
        h.back();
        let text = h.encode().expect("encodes");
        assert_eq!(History::decode(&text), Some(h));
    }

    #[test]
    fn encode_rejects_reserved_characters() {
        assert!(History::new("/a|b").encode().is_err());
        assert!(History::new("").encode().is_err());
    }

    #[test]
    fn decode_rejects_garbage_without_panicking() {
        for text in [
            "",
            "garbage",
            "v2;c=0;e=/a",
            "v1",
            "v1;c=0",
            "v1;e=/a",
            "v1;c=x;e=/a",
            "v1;c=1;e=/a",
            "v1;c=0;e=",
            "v1;c=0;e=/a||/b",
            "v1;c=0;c=0;e=/a",
            "v1;c=0;e=/a;extra",
            "v1;c=-1;e=/a",
        ] {
            assert_eq!(History::decode(text), None, "{text:?}");
        }
    }

    #[test]
    fn land_on_moves_the_cursor_to_a_matching_entry_without_adding_one() {
        let mut h = History::new("/home");
        h.push("/home/todos");
        h.push("/home/time");
        h.land_on("/home");
        assert_eq!(h.current(), "/home");
        assert!(h.can_forward(), "the forward entries stay");
        h.land_on("/home/time");
        assert_eq!(h.current(), "/home/time");
        h.land_on("/home/time");
        assert_eq!(
            h.forward(),
            None,
            "landing on the current route adds nothing"
        );
    }

    #[test]
    fn land_on_pushes_a_route_that_has_no_entry() {
        let mut h = History::new("/home");
        h.push("/home/todos");
        h.land_on("/home/time");
        assert_eq!(h.current(), "/home/time");
        assert_eq!(h.back(), Some("/home/todos"));
        assert_eq!(h.forward(), Some("/home/time"));
    }

    #[test]
    fn decode_rejects_more_than_the_cap() {
        let routes: Vec<String> = (0..=CAP).map(|i| format!("/{i}")).collect();
        let text = format!("v1;c=0;e={}", routes.join("|"));
        assert_eq!(History::decode(&text), None);
    }
}
