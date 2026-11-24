//! Plain-Rust view rules: the rows the Slint side shows ([`RowView`]), the markdown a text field is drawn from
//! ([`to_markdown`]), the font size of a block, and the escaping that keeps user text from turning into markup.
//! No Slint types here. The app's macro converts a [`RowView`] to the `EditorRow` struct of `types.slint`.

use rimelight_content::editor::{BlockRow, MarkKind, Mention, Span, TextValue};

/// One block row as the editor shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct RowView {
    pub id: String,
    pub block_type: String,
    pub depth: usize,
    /// How far the block is drawn in: its depth less the section and callout parents (their children line up with them).
    pub indent: usize,
    /// Position among the siblings (the row's `index`).
    pub index: usize,
    /// Plain text of the inline field, what the `TextInput` edits. Empty when the block has no inline field.
    pub text: String,
    /// Markdown the `StyledText` draws, built by [`to_markdown`] from the rich text.
    pub markdown: String,
    /// True when the block has an inline text field (the `TextInput` takes focus).
    pub inline: bool,
    pub font_size: f32,
    pub heading_level: Option<u8>,
    /// Locked by the template: the text cannot be edited here.
    pub locked: bool,
    /// Visibility or roles restrict this block (shown with a striped edge).
    pub restricted: bool,
    pub web_only: bool,
    /// The embed URL of a web-only block, empty otherwise. Built by [`crate::webview::embed_url`].
    pub web_url: String,
    pub placeholder: String,
    /// True when the block is part of the block-mode selection.
    pub selected: bool,
    /// Colour and name of a co-editor whose caret is in this block (hex colour, for example "#3b82f6").
    pub presence: Option<(String, String)>,
    /// The text is the `en` text shown in place of a locale that has none: greyed and read-only until typed in.
    pub fallback: bool,
    /// Open (unresolved, top-level) comments on this block, for the badge.
    pub comment_count: usize,
    /// The list marker drawn before a list item: "•" in a bulleted list, "1." (its number) in a numbered one.
    pub marker: String,
    /// The box the row is drawn in: "callout-<variant>" for a callout and the blocks inside it, "quote" for a quote,
    /// empty otherwise.
    pub accent: String,
    /// A code block's language, shown on its box.
    pub language: String,
}

/// Font size of a block's text, in logical pixels. Sections take the heading sizes (h2 to h6 by level), code takes
/// the monospace size, everything else the body size.
pub fn font_size(block_type: &str, heading_level: Option<u8>) -> f32 {
    if block_type == "SectionBlock" {
        return match heading_level.unwrap_or(2) {
            0..=2 => 28.0,
            3 => 24.0,
            4 => 20.0,
            5 => 18.0,
            _ => 16.0,
        };
    }
    if block_type == "CodeBlock" {
        14.0
    } else {
        16.0
    }
}

/// Builds the view row of one block. `text` is the inline field's rich text (ignored when the block has no inline
/// field). `web_url` is the embed URL of a web-only block (see `EditorController::set_web_base`), empty otherwise.
pub fn row_view(
    row: &BlockRow,
    text: &TextValue,
    selected: bool,
    presence: Option<(String, String)>,
    web_url: String,
) -> RowView {
    let inline = row.inline_field.is_some();
    let placeholder = if inline && text.plain.is_empty() && row.block_type == "ParagraphBlock" {
        "Type / for blocks".to_string()
    } else {
        String::new()
    };
    RowView {
        id: row.id.clone(),
        block_type: row.block_type.clone(),
        depth: row.depth,
        indent: row.depth,
        index: row.index,
        text: if inline {
            text.plain.clone()
        } else {
            String::new()
        },
        markdown: if inline && row.block_type == "SectionBlock" && !text.plain.is_empty() {
            format!("**{}**", to_markdown(text))
        } else if inline {
            to_markdown(text)
        } else {
            String::new()
        },
        inline,
        font_size: font_size(&row.block_type, row.heading_level),
        heading_level: row.heading_level,
        locked: row.templated,
        restricted: row.access.visibility.is_some() || !row.access.allowed_roles.is_empty(),
        web_only: row.web_only,
        web_url,
        placeholder,
        selected,
        presence,
        fallback: false,
        marker: String::new(),
        accent: String::new(),
        language: row
            .props
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        comment_count: 0,
    }
}

/// Markdown for a rich text value, as `StyledText` reads it. Each run of text with the same marks, link and mention
/// is one delimiter pair, so adjacent runs never produce `****`. User text is escaped: nothing typed in a field can
/// become a heading, list, link or raw HTML. A mention is a link `[@Title](mention:<id>)`, drawn in the link colour.
pub fn to_markdown(value: &TextValue) -> String {
    let mut out = String::new();
    for run in runs(&value.spans) {
        let text = value.plain.get(run.range.clone()).unwrap_or("");
        if !text.is_empty() {
            out.push_str(&render_run(text, &run));
        }
    }
    protect_block_start(&mut out);
    out
}

/// One merged run: the same marks, link and mention across a byte range.
struct Run<'a> {
    range: std::ops::Range<usize>,
    marks: Vec<MarkKind>,
    link: Option<&'a str>,
    mention: Option<&'a Mention>,
}

fn runs(spans: &[Span]) -> Vec<Run<'_>> {
    let mut out: Vec<Run<'_>> = Vec::new();
    for span in spans {
        let mut marks = span.marks.clone();
        marks.sort_by_key(mark_rank);
        marks.dedup();
        let link = span.link.as_deref();
        let mention = span.mention.as_ref();
        if let Some(last) = out.last_mut() {
            let same_mention =
                last.mention.map(|m| m.entry_id.as_str()) == mention.map(|m| m.entry_id.as_str());
            if last.marks == marks
                && last.link == link
                && same_mention
                && last.range.end == span.range.start
            {
                last.range.end = span.range.end;
                continue;
            }
        }
        out.push(Run {
            range: span.range.clone(),
            marks,
            link,
            mention,
        });
    }
    out
}

/// Outer-to-inner order when wrapping: strike, bold, italic, underline, code (a link or mention is outermost).
fn mark_rank(mark: &MarkKind) -> u8 {
    match mark {
        MarkKind::Strikethrough => 1,
        MarkKind::Bold => 2,
        MarkKind::Italic => 3,
        MarkKind::Underline => 4,
        MarkKind::Code => 5,
    }
}

fn render_run(text: &str, run: &Run<'_>) -> String {
    // Whitespace stays outside the delimiters, or emphasis does not parse.
    let start = text.len() - text.trim_start().len();
    let end = text.trim_end().len();
    if start >= end {
        return escape(text);
    }
    let (lead, core, trail) = (&text[..start], &text[start..end], &text[end..]);
    let mut inner = if run.marks.contains(&MarkKind::Code) {
        code_span(core)
    } else {
        escape(core)
    };
    // Innermost first: the reverse of the outer-to-inner order. Code was applied above.
    for mark in run.marks.iter().rev() {
        inner = match mark {
            MarkKind::Bold => format!("**{inner}**"),
            MarkKind::Italic => format!("*{inner}*"),
            MarkKind::Underline => format!("<u>{inner}</u>"),
            MarkKind::Strikethrough => format!("~~{inner}~~"),
            MarkKind::Code => inner,
        };
    }
    if let Some(mention) = run.mention {
        inner = format!("[{inner}](mention:{})", mention_id(&mention.entry_id));
    } else if let Some(url) = run.link {
        inner = format!("[{inner}]({})", link_target(url));
    }
    format!("{}{}{}", escape(lead), inner, escape(trail))
}

/// Characters that carry markup somewhere in a line. Escaped everywhere, which is always valid.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\*_~[]<>`&#!|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A code span with a backtick fence longer than any backtick run inside it.
fn code_span(text: &str) -> String {
    let longest = text
        .split(|c: char| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{fence}{pad}{text}{pad}{fence}")
}

/// The id of a mention target as it goes into the link: letters, digits and dashes only.
fn mention_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// A link target with the characters that would end the link early percent-encoded.
fn link_target(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            _ => out.push(c),
        }
    }
    out
}

/// Block-level syntax that inline escaping does not cover: a leading `-` or `+` makes a list item, and `1.` or `1)`
/// starts an ordered list. The first character is escaped in those cases.
fn protect_block_start(out: &mut String) {
    match out.chars().next() {
        Some('-') | Some('+') => out.insert(0, '\\'),
        Some(c) if c.is_ascii_digit() => {
            let digits = out.chars().take_while(|c| c.is_ascii_digit()).count();
            let marker = out
                .char_indices()
                .nth(digits)
                .filter(|(_, next)| *next == '.' || *next == ')')
                .map(|(at, _)| at);
            if let Some(at) = marker {
                out.insert(at, '\\');
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_sizes_step_down_by_level() {
        assert_eq!(font_size("SectionBlock", Some(2)), 28.0);
        assert_eq!(font_size("SectionBlock", Some(6)), 16.0);
        assert_eq!(font_size("ParagraphBlock", None), 16.0);
        assert_eq!(font_size("CodeBlock", None), 14.0);
    }

    #[test]
    fn plain_text_is_escaped() {
        assert_eq!(escape("a *b* [c]"), "a \\*b\\* \\[c\\]");
    }

    #[test]
    fn a_leading_list_marker_is_escaped() {
        let mut out = "- item".to_string();
        protect_block_start(&mut out);
        assert_eq!(out, "\\- item");
        let mut ordered = "1. item".to_string();
        protect_block_start(&mut ordered);
        assert_eq!(ordered, "1\\. item");
    }
}
