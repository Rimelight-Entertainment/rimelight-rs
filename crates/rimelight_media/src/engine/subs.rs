//! Subtitle cues: from SRT/VTT/ASS files next to a video, and the text clean-up for cues FFmpeg decodes.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    /// A picture subtitle (Blu-ray, DVD) instead of text.
    pub picture: Option<std::sync::Arc<super::bitmap_sub::SubPicture>>,
    /// Shown until the next picture subtitle replaces it.
    pub open: bool,
}

/// The cue text shown at `ms` (several overlapping cues are joined).
pub fn text_at(cues: &[Cue], ms: u64) -> String {
    cues.iter()
        .filter(|c| c.start_ms <= ms && ms < c.end_ms && !c.text.is_empty())
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The picture subtitle shown at `ms` (the latest one that started).
pub fn picture_at(cues: &[Cue], ms: u64) -> Option<std::sync::Arc<super::bitmap_sub::SubPicture>> {
    cues.iter()
        .filter(|c| c.picture.is_some() && c.start_ms <= ms && ms < c.end_ms)
        .max_by_key(|c| c.start_ms)
        .and_then(|c| c.picture.clone())
}

/// "00:01:02,500" or "01:02.500" -> ms.
fn parse_time(s: &str) -> Option<u64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (h.parse::<u64>().ok()?, m.parse::<u64>().ok()?, *s),
        [m, s] => (0, m.parse::<u64>().ok()?, *s),
        _ => return None,
    };
    let secs: f64 = sec.parse().ok()?;
    Some((h * 3600 + m * 60) * 1000 + (secs * 1000.0).round() as u64)
}

/// SRT and WebVTT.
pub fn parse_srt(text: &str) -> Vec<Cue> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let mut lines = block.lines().skip_while(|l| !l.contains("-->"));
        let Some(timing) = lines.next() else { continue };
        let Some((a, b)) = timing.split_once("-->") else {
            continue;
        };
        let b = b.split_whitespace().next().unwrap_or_default();
        let (Some(start_ms), Some(end_ms)) = (parse_time(a), parse_time(b)) else {
            continue;
        };
        let body: Vec<&str> = lines.collect();
        let text = strip_tags(&body.join("\n"));
        if !text.trim().is_empty() {
            cues.push(Cue {
                start_ms,
                end_ms,
                text,
                picture: None,
                open: false,
            });
        }
    }
    cues
}

/// ASS/SSA files: the Dialogue lines.
pub fn parse_ass(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("Dialogue:") else {
            continue;
        };
        let fields: Vec<&str> = rest.splitn(10, ',').collect();
        if fields.len() < 10 {
            continue;
        }
        let (Some(start_ms), Some(end_ms)) = (parse_time(fields[1]), parse_time(fields[2])) else {
            continue;
        };
        let text = clean_ass(fields[9]);
        if !text.trim().is_empty() {
            cues.push(Cue {
                start_ms,
                end_ms,
                text,
                picture: None,
                open: false,
            });
        }
    }
    cues
}

/// The text of one ASS event as FFmpeg decodes it ("ReadOrder,Layer,Style,Name,MarginL,MarginR,MarginV,Effect,Text",
/// or the older "Dialogue: ..." form).
pub fn ass_event_text(event: &str) -> String {
    let event = event.trim();
    if let Some(rest) = event.strip_prefix("Dialogue:") {
        return clean_ass(rest.splitn(10, ',').nth(9).unwrap_or_default());
    }
    clean_ass(event.splitn(9, ',').nth(8).unwrap_or(event))
}

/// Removes {\override} blocks and turns \N into line breaks.
pub fn clean_ass(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.replace("\\N", "\n")
        .replace("\\n", "\n")
        .replace("\\h", " ")
        .trim()
        .to_string()
}

/// Removes <i>, <b>, <font ...> and similar tags.
pub fn strip_tags(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

pub fn load_file(path: &Path) -> Result<Vec<Cue>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes.clone())
        .unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
    let cues = match super::files::ext_of(path).as_str() {
        "ass" | "ssa" => parse_ass(&text),
        _ => parse_srt(&text),
    };
    if cues.is_empty() {
        Err("no subtitles found in the file".into())
    } else {
        Ok(cues)
    }
}

/// Subtitle files next to a video that share its name ("Movie.srt", "Movie.en.srt").
pub fn files_beside(video: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(stem)) = (video.parent(), video.file_stem().and_then(|s| s.to_str()))
    else {
        return Vec::new();
    };
    let stem = stem.to_lowercase();
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    super::files::SUBTITLE_EXTS.contains(&super::files::ext_of(p).as_str())
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.to_lowercase().starts_with(&stem))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_and_ass() {
        let srt = "1\n00:00:01,000 --> 00:00:02,500\n<i>Hello</i>\nthere\n\n2\n00:00:03,000 --> 00:00:04,000\nBye\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "Hello\nthere");
        assert_eq!(text_at(&cues, 2000), "Hello\nthere");
        assert_eq!(text_at(&cues, 2600), "");
        assert_eq!(
            ass_event_text("0,0,Default,,0,0,0,,{\\i1}Hi\\Nyou"),
            "Hi\nyou"
        );
        let ass = "Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Line, with comma";
        assert_eq!(parse_ass(ass)[0].text, "Line, with comma");
    }
}
