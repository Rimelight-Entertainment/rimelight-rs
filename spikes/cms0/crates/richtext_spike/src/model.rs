//! Plain text plus byte-range marks, and the markdown the read-only StyledText shows.
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Mention(String),
}

impl Mark {
    /// Outer-to-inner nesting order when wrapping markdown.
    fn rank(&self) -> u8 {
        match self {
            Mark::Mention(_) => 0,
            Mark::Strike => 1,
            Mark::Bold => 2,
            Mark::Italic => 3,
            Mark::Underline => 4,
            Mark::Code => 5,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Para {
    pub text: String,
    pub marks: Vec<(Range<usize>, Mark)>,
}

impl Para {
    /// Applies a user edit: diff old/new text by common prefix and suffix, then remap marks.
    pub fn edit(&mut self, new: &str) {
        let (p, old_end, new_end) = diff(&self.text, new);
        self.remap(p, old_end, new_end);
        self.text = new.to_owned();
    }

    /// Inserts `s` at byte `at` and returns the byte offset just after it.
    /// Ranges touching `at` are extended, like typing at a range edge.
    pub fn insert(&mut self, at: usize, s: &str, mark: Option<Mark>) -> usize {
        let end = at + s.len();
        self.remap(at, at, end);
        self.text.insert_str(at, s);
        if let Some(m) = mark {
            self.marks.push((at..end, m));
        }
        end
    }

    /// Toggles `mark` on `r`: removes it if the whole range already has it, adds it otherwise.
    pub fn toggle(&mut self, r: Range<usize>, mark: &Mark) {
        if r.is_empty() {
            return;
        }
        if self.covered(&r, mark) {
            let mut out = Vec::new();
            for (x, m) in self.marks.drain(..) {
                if m != *mark || x.end <= r.start || x.start >= r.end {
                    out.push((x, m));
                    continue;
                }
                if x.start < r.start {
                    out.push((x.start..r.start, m.clone()));
                }
                if x.end > r.end {
                    out.push((r.end..x.end, m));
                }
            }
            self.marks = out;
        } else {
            self.marks.push((r, mark.clone()));
        }
    }

    fn covered(&self, r: &Range<usize>, mark: &Mark) -> bool {
        let mut cuts = vec![r.start, r.end];
        for (x, m) in self.marks.iter().filter(|(_, m)| m == mark) {
            cuts.extend([x.start, x.end].into_iter().filter(|c| r.contains(c)));
        }
        cuts.sort_unstable();
        cuts.dedup();
        cuts.windows(2).all(|w| {
            self.marks
                .iter()
                .any(|(x, m)| m == mark && x.start <= w[0] && w[1] <= x.end)
        })
    }

    /// Maps old offsets to new ones for an edit that replaced [p, old_end) with [p, new_end).
    fn remap(&mut self, p: usize, old_end: usize, new_end: usize) {
        let shift = |x: usize, is_end: bool| {
            if x < p {
                x
            } else if x >= old_end {
                new_end + (x - old_end)
            } else if is_end {
                new_end
            } else {
                p
            }
        };
        self.marks = self
            .marks
            .iter()
            .filter_map(|(r, m)| {
                let (s, e) = (shift(r.start, false), shift(r.end, true));
                (s < e).then(|| (s..e, m.clone()))
            })
            .collect();
    }
}

/// Returns (prefix_len, old_end, new_end) of the single changed region.
fn diff(old: &str, new: &str) -> (usize, usize, usize) {
    let p: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let s: usize = old[p..]
        .chars()
        .rev()
        .zip(new[p..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    (p, old.len() - s, new.len() - s)
}

/// Markdown for the StyledText. Plain text is escaped; marks nest outer-to-inner.
pub fn to_markdown(p: &Para) -> String {
    let len = p.text.len();
    let mut cuts = vec![0, len];
    for (r, _) in &p.marks {
        cuts.push(r.start.min(len));
        cuts.push(r.end.min(len));
    }
    cuts.sort_unstable();
    cuts.dedup();

    // Maximal runs of identical mark sets: (marks, start, end).
    let mut runs: Vec<(Vec<Mark>, usize, usize)> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut set: Vec<Mark> = p
            .marks
            .iter()
            .filter(|(r, _)| r.start <= a && b <= r.end)
            .map(|(_, m)| m.clone())
            .collect();
        set.sort_by_key(Mark::rank);
        set.dedup();
        let extend = matches!(runs.last(), Some(l) if l.0 == set && l.2 == a);
        if extend {
            runs.last_mut().unwrap().2 = b;
        } else {
            runs.push((set, a, b));
        }
    }
    runs.iter()
        .map(|(set, a, b)| render_run(&p.text[*a..*b], set))
        .collect()
}

fn render_run(text: &str, marks: &[Mark]) -> String {
    // Keep whitespace outside the delimiters, or emphasis will not parse.
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len();
    if start >= end {
        return escape(text);
    }
    let (lead, core, trail) = (&text[..start], &text[start..end], &text[end..]);
    let mut inner = if marks.contains(&Mark::Code) {
        code_span(core)
    } else {
        escape(core)
    };
    // Innermost first: reverse of the outer-to-inner order. Code was already applied.
    for m in marks.iter().rev().filter(|m| **m != Mark::Code) {
        inner = wrap(m, inner);
    }
    format!("{}{}{}", escape(lead), inner, escape(trail))
}

fn wrap(mark: &Mark, inner: String) -> String {
    match mark {
        Mark::Bold => format!("**{inner}**"),
        Mark::Italic => format!("*{inner}*"),
        Mark::Underline => format!("<u>{inner}</u>"),
        Mark::Strike => format!("~~{inner}~~"),
        Mark::Code => inner,
        Mark::Mention(id) => {
            let id: String = id.chars().filter(|c| c.is_alphanumeric() || *c == '-').collect();
            format!("<font color=\"#2a6fdb\">[{inner}](mention:{id})</font>")
        }
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "*_~[]<>\\`".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Code span with a backtick fence longer than any backtick run inside.
fn code_span(s: &str) -> String {
    let longest = s
        .split(|c: char| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if s.starts_with('`') || s.ends_with('`') { " " } else { "" };
    format!("{fence}{pad}{s}{pad}{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(text: &str, marks: Vec<(Range<usize>, Mark)>) -> Para {
        Para { text: text.into(), marks }
    }

    #[test]
    fn insert_before_range_shifts_it() {
        let mut p = para("abcdef", vec![(2..4, Mark::Bold)]);
        p.edit("XXabcdef");
        assert_eq!(p.marks, vec![(4..6, Mark::Bold)]);
    }

    #[test]
    fn insert_inside_range_grows_it() {
        let mut p = para("abcdef", vec![(2..4, Mark::Bold)]);
        p.edit("abcZdef");
        assert_eq!(p.marks, vec![(2..5, Mark::Bold)]);
    }

    #[test]
    fn insert_after_range_leaves_it() {
        let mut p = para("abcdef", vec![(2..4, Mark::Bold)]);
        p.edit("abcdeXf");
        assert_eq!(p.marks, vec![(2..4, Mark::Bold)]);
    }

    #[test]
    fn typing_at_range_end_continues_mark() {
        let mut p = para("abcdef", vec![(2..4, Mark::Bold)]);
        p.edit("abcdXef");
        assert_eq!(p.marks, vec![(2..5, Mark::Bold)]);
    }

    #[test]
    fn delete_across_range_end_clips_it() {
        let mut p = para("abcdef", vec![(2..5, Mark::Bold)]);
        p.edit("abcd");
        assert_eq!(p.marks, vec![(2..4, Mark::Bold)]);
    }

    #[test]
    fn delete_across_range_start_clips_it() {
        let mut p = para("abcdefgh", vec![(5..8, Mark::Italic)]);
        p.edit("abch");
        assert_eq!(p.marks, vec![(3..4, Mark::Italic)]);
    }

    #[test]
    fn plain_text_is_escaped() {
        let p = para("a*b_c[d]", vec![]);
        assert_eq!(to_markdown(&p), r"a\*b\_c\[d\]");
    }

    #[test]
    fn marks_nest_outer_to_inner() {
        let p = para("xy", vec![(0..2, Mark::Bold), (0..2, Mark::Italic)]);
        assert_eq!(to_markdown(&p), "***xy***");
    }

    #[test]
    fn whitespace_stays_outside_delimiters() {
        let p = para("hi there", vec![(0..3, Mark::Bold)]);
        assert_eq!(to_markdown(&p), "**hi** there");
    }

    #[test]
    fn code_is_not_escaped() {
        let p = para("x*y", vec![(0..3, Mark::Code)]);
        assert_eq!(to_markdown(&p), "`x*y`");
    }

    #[test]
    fn mention_renders_as_coloured_link() {
        let p = para("@Ana", vec![(0..4, Mark::Mention("entry-123".into()))]);
        assert_eq!(
            to_markdown(&p),
            "<font color=\"#2a6fdb\">[@Ana](mention:entry-123)</font>"
        );
    }

    #[test]
    fn toggle_twice_removes_mark() {
        let mut p = para("hello", vec![]);
        p.toggle(0..3, &Mark::Bold);
        assert_eq!(to_markdown(&p), "**hel**lo");
        p.toggle(0..3, &Mark::Bold);
        assert!(p.marks.is_empty());
    }
}
