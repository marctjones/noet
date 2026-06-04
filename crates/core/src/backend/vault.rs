//! File IO — frontmatter + body. Plain `.md` files with a small YAML header are
//! the source of truth; this module reads and writes them and decides whether a
//! note renders as markdown or Typst.

use super::Note;
use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct FrontMatter {
    id: String,
    title: String,
    #[serde(default)]
    created: String,
    #[serde(default)]
    updated: String,
    #[serde(default = "default_kind")]
    kind: String,
}

fn default_kind() -> String {
    "auto".into()
}

/// Heuristic: a note is Typst only if it shows a *strong* Typst signal
/// (`#set`/`#let`/`#show`/`#import` or a `#func(...)` call). Weak signals like
/// `$…$` are ignored so prose with "$5 … $10" doesn't false-positive. No clear
/// signal → markdown.
pub fn detect_kind(body: &str) -> &'static str {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?m)^\s*#(set|let|show|import)\b|#(figure|image|table|grid|stack|align|block|rect|text|page|heading)\s*\(",
        )
        .unwrap()
    });
    if re.is_match(body) {
        "typst"
    } else {
        "markdown"
    }
}

/// Resolve a note's declared kind to what we actually render: an explicit
/// markdown/typst wins; "auto" (or anything else) is detected from the body.
pub fn effective_kind(declared: &str, body: &str) -> &'static str {
    match declared {
        "typst" => "typst",
        "markdown" => "markdown",
        _ => detect_kind(body),
    }
}

pub(crate) fn read_note(path: &Path) -> Result<Note> {
    let raw = std::fs::read_to_string(path)?;
    let (fm, body) = split_frontmatter(&raw);
    let fm: FrontMatter = if fm.trim().is_empty() {
        FrontMatter::default()
    } else {
        serde_yaml::from_str(&fm).unwrap_or_default()
    };
    let id = if fm.id.is_empty() {
        // derive a stable-ish id from the filename for legacy/hand-made files
        path.file_stem().unwrap().to_string_lossy().to_string()
    } else {
        fm.id
    };
    Ok(Note {
        id,
        title: if fm.title.is_empty() {
            first_line_title(&body)
        } else {
            fm.title
        },
        created: fm.created,
        updated: fm.updated,
        kind: fm.kind,
        body,
        path: path.to_path_buf(),
    })
}

fn split_frontmatter(raw: &str) -> (String, String) {
    if let Some(rest) = raw.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let fm = &rest[..end];
            let body = &rest[end + 5..];
            return (fm.to_string(), body.to_string());
        }
    }
    (String::new(), raw.to_string())
}

fn first_line_title(body: &str) -> String {
    body.lines()
        .map(|l| l.trim_start_matches(['#', '-', '*', ' ']))
        .find(|l| !l.trim().is_empty())
        .map(|l| l.chars().take(60).collect())
        .unwrap_or_else(|| "Untitled".into())
}

pub(crate) fn write_note(note: &Note) -> Result<()> {
    let fm = FrontMatter {
        id: note.id.clone(),
        title: note.title.clone(),
        created: note.created.clone(),
        updated: note.updated.clone(),
        kind: note.kind.clone(),
    };
    let yaml = serde_yaml::to_string(&fm)?;
    let contents = format!("---\n{yaml}---\n{}", note.body);
    std::fs::write(&note.path, contents)
        .with_context(|| format!("writing {}", note.path.display()))?;
    Ok(())
}

/// Filesystem-safe filename from a note title (falls back to its id).
pub(crate) fn safe_filename(title: &str, id: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let s = s.trim();
    if s.is_empty() { id.to_string() } else { s.chars().take(60).collect() }
}
