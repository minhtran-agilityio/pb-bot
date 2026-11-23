//! One line of `~/.claude/artifact-log.tsv` (HANDOFF §11.3). Pure parsing, no IO.
//! Columns are read by position: 1 time, 2 link, 3 title, 4 folder, 5 cwd (optional), 6 file (optional).
//! Old lines have 4 columns; columns after 6 are ignored. A field is data only: nothing here or in
//! the callers passes it to a shell, an opener or a fetch.

use super::paths::valid_id;

/// The longest log line read; longer lines are bad.
pub const MAX_LINE_BYTES: usize = 64 * 1024;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct LogEntry {
    pub published_at: String,
    pub link: String,
    pub id: String,
    pub title: String,
    pub folder: String,
    pub cwd: Option<String>,
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BadLine {
    TooLong,
    TooFewColumns,
    BadTime,
    BadLink,
}

/// `YYYY-MM-DDTHH:MM:SSZ`, with each field in range.
pub fn valid_utc(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
        return false;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let p = &s[r];
        p.bytes().all(|c| c.is_ascii_digit()).then(|| p.parse().ok()).flatten()
    };
    match (num(0..4), num(5..7), num(8..10), num(11..13), num(14..16), num(17..19)) {
        (Some(_), Some(mo), Some(d), Some(h), Some(mi), Some(se)) => {
            (1..=12).contains(&mo) && (1..=31).contains(&d) && h < 24 && mi < 60 && se < 61
        }
        _ => false,
    }
}

/// The artifact id of `https://claude.ai/artifact/<id>` or `https://claude.ai/code/artifact/<id>`.
pub fn link_id(link: &str) -> Option<&str> {
    let rest = link
        .strip_prefix("https://claude.ai/artifact/")
        .or_else(|| link.strip_prefix("https://claude.ai/code/artifact/"))?;
    valid_id(rest).then_some(rest)
}

fn clean_title(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    t.trim().chars().take(MAX_TITLE_CHARS).collect()
}

fn optional(cols: &[&str], i: usize) -> Option<String> {
    cols.get(i).map(|s| s.trim()).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn parse_line(line: &str) -> Result<LogEntry, BadLine> {
    if line.len() > MAX_LINE_BYTES {
        return Err(BadLine::TooLong);
    }
    let line = line.strip_suffix('\n').unwrap_or(line);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let cols: Vec<&str> = line.split('\t').collect();
    if cols.len() < 4 {
        return Err(BadLine::TooFewColumns);
    }
    if !valid_utc(cols[0]) {
        return Err(BadLine::BadTime);
    }
    let id = link_id(cols[1]).ok_or(BadLine::BadLink)?.to_string();
    let title = match clean_title(cols[2]) {
        t if t.is_empty() => id.clone(),
        t => t,
    };
    Ok(LogEntry {
        published_at: cols[0].to_string(),
        link: cols[1].to_string(),
        id,
        title,
        folder: cols[3].to_string(),
        // The cwd is only compared with project folders; a relative one could never match, so drop it.
        cwd: optional(&cols, 4).filter(|c| c.starts_with('/')),
        file: optional(&cols, 5),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIX: &str = "2026-11-22T11:13:40Z\thttps://claude.ai/artifact/XD26T57sEMLh28o2cF6j1J\tHook Test Page\t/Users/m/claude-artifacts/XD26T57sEMLh28o2cF6j1J\t/Users/m/Documents/agilityio/pb-bot\t/Users/m/claude-artifacts/XD26T57sEMLh28o2cF6j1J/20261122T111340Z_hook-test.html";

    #[test]
    fn six_columns() {
        let e = parse_line(SIX).unwrap();
        assert_eq!(e.published_at, "2026-11-22T11:13:40Z");
        assert_eq!(e.id, "XD26T57sEMLh28o2cF6j1J");
        assert_eq!(e.title, "Hook Test Page");
        assert_eq!(e.cwd.as_deref(), Some("/Users/m/Documents/agilityio/pb-bot"));
        assert!(e.file.unwrap().ends_with("_hook-test.html"));
    }

    #[test]
    fn old_four_column_lines_stay_valid() {
        let e = parse_line("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/Old1\tOld page\t/Users/m/claude-artifacts/Old1\n").unwrap();
        assert_eq!(e.id, "Old1");
        assert_eq!(e.cwd, None);
        assert_eq!(e.file, None);
    }

    #[test]
    fn extra_columns_crlf_and_empty_optionals() {
        let e = parse_line(&format!("{SIX}\textra\tmore\r\n")).unwrap();
        assert!(e.file.is_some());
        let e = parse_line("2026-11-20T08:00:00Z\thttps://claude.ai/code/artifact/a-b\t\t/x\t\t\r").unwrap();
        assert_eq!(e.id, "a-b");
        assert_eq!(e.title, "a-b", "an empty title falls back to the id");
        assert_eq!((e.cwd, e.file), (None, None));
        let e = parse_line("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/a\tt\t/x\trelative/dir").unwrap();
        assert_eq!(e.cwd, None, "a relative cwd is dropped");
    }

    #[test]
    fn control_characters_and_long_titles_are_cleaned() {
        let long = "x".repeat(500);
        let e = parse_line(&format!("2026-11-20T08:00:00Z\thttps://claude.ai/artifact/a\t a\u{7}b {long}\t/x")).unwrap();
        assert!(e.title.starts_with("a b "));
        assert_eq!(e.title.chars().count(), 200);
    }

    #[test]
    fn bad_lines() {
        let ok_link = "https://claude.ai/artifact/a";
        assert_eq!(parse_line(""), Err(BadLine::TooFewColumns));
        assert_eq!(parse_line(&format!("2026-11-20T08:00:00Z\t{ok_link}\tt")), Err(BadLine::TooFewColumns));
        for t in ["2026-11-20 08:00:00", "2026-13-20T08:00:00Z", "2026-11-20T25:00:00Z", "yesterday", "2026-11-2aT08:00:00Z"] {
            assert_eq!(parse_line(&format!("{t}\t{ok_link}\tt\t/x")), Err(BadLine::BadTime), "{t}");
        }
        for l in [
            "http://claude.ai/artifact/a",
            "https://evil.example/artifact/a",
            "https://claude.ai.evil.example/artifact/a",
            "https://claude.ai/artifact/../../etc",
            "https://claude.ai/artifact/a/b",
            "https://claude.ai/artifact/",
            "https://claude.ai/artifact/a?x=1",
            "https://claude.ai/artifact/a/",
            "https://claude.ai/artifact/a#f",
            "$(rm -rf ~)",
        ] {
            assert_eq!(parse_line(&format!("2026-11-20T08:00:00Z\t{l}\tt\t/x")), Err(BadLine::BadLink), "{l}");
        }
        let long = format!("2026-11-20T08:00:00Z\t{ok_link}\t{}\t/x", "y".repeat(MAX_LINE_BYTES));
        assert_eq!(parse_line(&long), Err(BadLine::TooLong));
    }
}
