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
    let id = path.file_stem().unwrap().to_string_lossy().to_string();
    Ok(Note {
        id,
        title: markdown_title(&body),
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

pub fn markdown_title(body: &str) -> String {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            let title = title.trim();
            if !title.is_empty() {
                return title.chars().take(80).collect();
            }
        }
    }
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|l| {
            l.trim_start_matches(['#', '-', '*', ' '])
                .chars()
                .take(80)
                .collect()
        })
        .unwrap_or_else(|| "Untitled".into())
}

pub fn set_markdown_title(body: &str, title: &str) -> String {
    let title = title.trim();
    let mut out = Vec::new();
    let mut replaced = false;
    for line in body.lines() {
        if !replaced && line.trim_start().starts_with("# ") {
            if !title.is_empty() {
                out.push(format!("# {title}"));
            }
            replaced = true;
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced && !title.is_empty() {
        let mut s = format!("# {title}\n\n");
        s.push_str(body.trim_start_matches('\n'));
        return s;
    }
    let mut s = out.join("\n");
    if body.ends_with('\n') && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

pub(crate) fn format_note(note: &Note) -> String {
    let fm = FrontMatter {
        created: note.created.clone(),
        updated: note.updated.clone(),
        kind: note.kind.clone(),
    };
    let yaml = serde_yaml::to_string(&fm).unwrap_or_default();
    format!("---\n{yaml}---\n{}", note.body)
}

pub(crate) fn write_note(note: &Note) -> Result<()> {
    let contents = format_note(note);
    std::fs::write(&note.path, contents)
        .with_context(|| format!("writing {}", note.path.display()))?;
    Ok(())
}

/// Filesystem-safe filename from a note title (falls back to its id).
pub(crate) fn safe_filename(title: &str, id: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim();
    if s.is_empty() {
        id.to_string()
    } else {
        s.chars().take(60).collect()
    }
}
