//! SigmaOS-apt-listchanges — Debian changelog/news entry parser.
//!
//! Inspired by Debian's `apt-listchanges`: parses `changelog`-style entries —
//! a header `package (version) distribution; urgency=level`, metadata lines
//! (`Author`, `Date`), a blank line, then `*`-bulleted change items — into
//! structured records for display/filtering.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Errors raised by this component.
#[derive(Debug, thiserror::Error)]
pub enum ComponentError {
    /// The first line was not a valid changelog header.
    #[error("malformed changelog header: {0}")]
    BadHeader(String),
}

/// Result alias for component operations.
pub type ComponentResult<T> = Result<T, ComponentError>;

/// Urgency level of a changelog entry (ordered low→emergency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency { Low, Medium, High, Emergency, Unknown }

impl Urgency {
    /// Parse an urgency token (case-insensitive).
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Urgency::Low,
            "medium" | "normal" => Urgency::Medium,
            "high" => Urgency::High,
            "emergency" | "critical" | "urgent" => Urgency::Emergency,
            _ => Urgency::Unknown,
        }
    }
}

/// One parsed changelog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Package name from the header.
    pub package: String,
    /// Version from the header.
    pub version: String,
    /// Distribution/target from the header.
    pub distribution: String,
    /// Urgency level.
    pub urgency: Urgency,
    /// Author/maintainer line if present.
    pub author: Option<String>,
    /// Date line if present.
    pub date: Option<String>,
    /// Change description lines (text after `* ` bullets).
    pub changes: Vec<String>,
}

/// Parse changelog text into entries. A new entry starts at each header line.
pub fn parse(input: &str) -> ComponentResult<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut cur: Option<Entry> = None;
    let mut in_changes = false;
    for raw in input.lines() {
        let line = raw.trim_end();
        if let Some(hdr) = parse_header(line) {
            if let Some(e) = cur.take() { entries.push(e); }
            cur = Some(hdr);
            in_changes = false;
            continue;
        }
        let Some(e) = cur.as_mut() else { continue };
        if let Some(rest) = line.strip_prefix("Author:") {
            e.author = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Date:") {
            e.date = Some(rest.trim().to_string());
        } else if line.trim().is_empty() {
            in_changes = true;
        } else if in_changes {
            let item = line.trim_start();
            let item = item.strip_prefix("* ").unwrap_or(item);
            let item = item.strip_prefix('-').map(str::trim_start).unwrap_or(item);
            if !item.is_empty() { e.changes.push(item.to_string()); }
        }
    }
    if let Some(e) = cur.take() { entries.push(e); }
    if entries.is_empty() && !input.trim().is_empty() {
        return Err(ComponentError::BadHeader(input.lines().next().unwrap_or("").to_string()));
    }
    Ok(entries)
}

/// Try to parse a header line `pkg (version) dist; urgency=x`.
fn parse_header(line: &str) -> Option<Entry> {
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    let pkg = line[..open].trim();
    if pkg.is_empty() || pkg.contains(char::is_whitespace) { return None; }
    let version = line[open + 1..close].trim().to_string();
    let rest = line[close + 1..].trim();
    let (dist, urgency) = match rest.split_once(';') {
        Some((d, u)) => (d.trim().to_string(), extract_urgency(u)),
        None => (rest.trim_end_matches(';').trim().to_string(), Urgency::Unknown),
    };
    Some(Entry { package: pkg.to_string(), version, distribution: dist,
        urgency, author: None, date: None, changes: Vec::new() })
}

fn extract_urgency(s: &str) -> Urgency {
    for tok in s.split(|c: char| c == ',' || c.is_whitespace()) {
        if let Some(v) = tok.strip_prefix("urgency=") { return Urgency::parse(v); }
    }
    Urgency::Unknown
}

/// Entries at/above an urgency.
pub fn by_urgency(entries: &[Entry], min: Urgency) -> Vec<&Entry> {
    entries.iter().filter(|e| e.urgency >= min).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "apt (2.6.1) unstable; urgency=medium\nAuthor: A U Thor <a@b.c>\nDate: Mon, 01 Jan 2024 00:00:00 +0000\n\n* Fix bug in resolver.\n* Improve error message.\n\nbash (5.2.15-2) unstable; urgency=low\n\n* Update completions.\n";

    #[test]
    fn parses_two_entries() {
        let e = parse(LOG).unwrap();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].package, "apt");
        assert_eq!(e[0].version, "2.6.1");
        assert_eq!(e[0].urgency, Urgency::Medium);
        assert_eq!(e[0].author.as_deref(), Some("A U Thor <a@b.c>"));
        assert_eq!(e[1].package, "bash");
    }

    #[test]
    fn collects_change_items() {
        let e = parse(LOG).unwrap();
        assert!(e[0].changes.iter().any(|c| c.contains("resolver")));
        assert_eq!(e[0].changes.len(), 2);
        assert_eq!(e[1].changes.len(), 1);
    }

    #[test]
    fn urgency_filter() {
        let e = parse(LOG).unwrap();
        assert!(by_urgency(&e, Urgency::High).is_empty());
        assert_eq!(by_urgency(&e, Urgency::Medium).len(), 1);
    }

    #[test]
    fn urgency_parse() {
        assert_eq!(Urgency::parse("high"), Urgency::High);
        assert_eq!(Urgency::parse("EMERGENCY"), Urgency::Emergency);
        assert_eq!(Urgency::parse("bogus"), Urgency::Unknown);
    }

    #[test]
    fn empty_input_ok() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse("   \n\n").unwrap().is_empty());
    }
}
