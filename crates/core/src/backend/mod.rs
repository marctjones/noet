//! Noet backend: plain markdown files are the source of truth; SQLite is a
//! disposable index rebuilt from those files. No network, no JS — just files.

use anyhow::{Context, Result};
use chrono::Utc;
use regex::Regex;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use walkdir::WalkDir;

/// Follow-ups/delegated todos go "stale" once their note is untouched this long.
#[allow(dead_code)] // used by the stale view (currently exercised via tests)
const STALE_DAYS: i64 = 14;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub created: String,
    pub updated: String,
    pub kind: String, // "markdown" | "typst"
    pub body: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Todo {
    pub id: String, // "<note_id>:<line_no>"
    pub note_id: String,
    pub kind: String,   // do/followup/delegated/todelegate/someday/reading
    pub status: String, // todo / doing / done
    pub text: String,
    pub project: String,
    pub person: String,
    pub start: String, // optional start:YYYY-MM-DD (for Gantt bars)
    pub due: String,
    pub external: String, // forward-compat: "JIRA:PROJ-12" / "outlook" / ...
    pub priority: String, // "A" / "B" / "C" / "" (org-style [#A])
    pub repeat: String,   // e.g. "1w" / "3d" / "1m" — recurring interval
    pub done: bool,
    pub line_no: usize,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub name: String,
    pub count: i64,
}

/// A rendered markdown block for the read view (native, no webview).
#[derive(Debug, Clone)]
pub struct MdBlock {
    pub kind: String, // h1/h2/h3/para/bullet/numbered/code/quote/todo/rule
    pub text: String,
    pub indent: i32,
}

/// The six todo kinds, in board-column order.
pub const KINDS: [&str; 6] = ["do", "followup", "delegated", "todelegate", "someday", "reading"];
/// The three statuses, in board-column order.
pub const STATUSES: [&str; 3] = ["todo", "doing", "done"];

/// A unified filter applied across every view (Notes / Board / Gantt).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Filter {
    pub search: String,
    pub project: String,
    pub person: String,
    pub tag: String,
    pub kind: String,
    pub status: String, // "" = any, "open" = not done, else a specific status
    pub priority: String, // "" / A / B / C
    pub due_bucket: String, // "" / overdue / week / hasdate / nodate
    pub show_archived: bool,
}

impl Filter {
    fn like(s: &str) -> String {
        format!("%{s}%")
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NamedFilter {
    name: String,
    filter: Filter,
}

/// Structured fields for creating/editing a todo via the GUI form (so users
/// never hand-type the line syntax).
#[derive(Debug, Clone, Default)]
pub struct TodoFields {
    pub kind: String,
    pub status: String,
    pub text: String,
    pub person: String,
    pub project: String,
    pub start: String,
    pub due: String,
    pub external: String,
    pub priority: String,
    pub repeat: String,
}

impl TodoFields {
    pub fn from_todo(t: &Todo) -> Self {
        TodoFields {
            kind: t.kind.clone(),
            status: t.status.clone(),
            text: t.text.clone(),
            person: t.person.clone(),
            project: t.project.clone(),
            start: t.start.clone(),
            due: t.due.clone(),
            external: t.external.clone(),
            priority: t.priority.clone(),
            repeat: t.repeat.clone(),
        }
    }
}

/// Render structured fields back into a canonical todo line.
fn format_todo_line(f: &TodoFields) -> String {
    let marker = match f.status.as_str() {
        "doing" => "DOING",
        "done" => "DONE",
        _ => "TODO",
    };
    let kind = if f.kind.is_empty() { "do" } else { f.kind.as_str() };
    let mut s = format!("{marker}({kind}) ");
    if !f.priority.is_empty() {
        s += &format!("[#{}] ", f.priority.trim());
    }
    s += f.text.trim();
    if !f.person.is_empty() {
        s += &format!(" @[[{}]]", f.person.trim());
    }
    if !f.project.is_empty() {
        s += &format!(" +[[{}]]", f.project.trim());
    }
    if !f.start.is_empty() {
        s += &format!(" start:{}", f.start.trim());
    }
    if !f.due.is_empty() {
        s += &format!(" due:{}", f.due.trim());
    }
    if !f.repeat.is_empty() {
        s += &format!(" repeat:{}", f.repeat.trim());
    }
    if !f.external.is_empty() {
        s += &format!(" {}", f.external.trim());
    }
    s
}

/// Advance a YYYY-MM-DD date by a repeat interval like "3d" / "1w" / "2m".
fn advance_date(date: &str, repeat: &str) -> String {
    use chrono::{Duration, Months, NaiveDate};
    let Ok(d) = NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
        return date.to_string();
    };
    if repeat.len() < 2 {
        return date.to_string();
    }
    let n: i64 = repeat[..repeat.len() - 1].parse().unwrap_or(0);
    let unit = repeat.chars().last().unwrap_or('d');
    let nd = match unit {
        'w' => d.checked_add_signed(Duration::days(7 * n)),
        'm' => d.checked_add_months(Months::new(n as u32)),
        _ => d.checked_add_signed(Duration::days(n)),
    };
    nd.map(|x| x.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| date.to_string())
}

// ---------------------------------------------------------------------------
// Parsing — the file-first grammar
// ---------------------------------------------------------------------------

// A todo line:  TODO(kind) some text @[[Person]] +[[Project]] start:2026-06-01 due:2026-06-10 jira:PROJ-12 #urgent
// Marker is TODO / DOING / DONE -> status. Tokens are stripped from the displayed text.
fn todo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?m)^\s*(?P<marker>TODO|DOING|DONE)\((?P<kind>[a-zA-Z]+)\)\s+(?P<rest>.*)$")
            .unwrap()
    })
}

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[(?P<t>[^\]]+)\]\]").unwrap())
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)#(?P<t>[A-Za-z][A-Za-z0-9_-]*)").unwrap())
}

fn marker_to_status(marker: &str) -> String {
    match marker {
        "DOING" => "doing",
        "DONE" => "done",
        _ => "todo",
    }
    .to_string()
}

/// `#tag` labels anywhere in a note body.
pub fn parse_tags(body: &str) -> Vec<String> {
    let mut v: Vec<String> = tag_re()
        .captures_iter(body)
        .map(|c| c["t"].to_string())
        .collect();
    v.sort();
    v.dedup();
    v
}

// A person mention is either bracketed `@[[Jane Smith]]` (allows spaces) or a
// bare `@jane` token. The `(?:^|\s)` guard keeps emails like a@b.com from matching.
fn person_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:^|\s)@(?:\[\[(?P<pb>[^\]]+)\]\]|(?P<ps>[A-Za-z][A-Za-z0-9_.\-]*))").unwrap()
    })
}

fn person_name(c: &regex::Captures) -> String {
    c.name("pb")
        .or_else(|| c.name("ps"))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Every `@person` / `@[[Person]]` mention anywhere in a note (not just todos).
pub fn parse_mentions(body: &str) -> Vec<String> {
    let mut v: Vec<String> = person_re()
        .captures_iter(body)
        .map(|c| person_name(&c))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Strip inline markdown markers to readable text (Slint Text is single-style,
/// so we render structure + clean text rather than mixed inline runs).
fn strip_inline(s: &str) -> String {
    static LINK: OnceLock<Regex> = OnceLock::new();
    static WIKI: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| Regex::new(r"\[([^\]]+)\]\(([^)]*)\)").unwrap());
    // `[[X]]`, `+[[X]]`, `@[[X]]` -> X so prose reads cleanly (entities show as
    // clickable chips elsewhere). Bare @name / #tag are already readable.
    let wiki = WIKI.get_or_init(|| Regex::new(r"[+@]?\[\[([^\]]+)\]\]").unwrap());
    let s = link.replace_all(s, "$1 ($2)").to_string();
    let s = wiki.replace_all(&s, "$1").to_string();
    s.replace("**", "").replace('`', "")
}

/// Clean markdown emphasis/wikilink noise to readable text (no inline links).
pub fn clean_inline(s: &str) -> String {
    strip_inline(s)
}

/// One inline piece of a rendered line.
#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,  // what to show
    pub kind: String,  // "" plain | "url" | "project" | "person" | "tag"
    pub value: String, // url to open, or entity name to filter
}

/// Split a raw line into plain text + clickable link/url/entity segments.
pub fn line_segments(raw: &str) -> Vec<Segment> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?P<mdlink>\[[^\]]+\]\([^)]*\))|(?P<url>https?://[^\s)]+)|(?P<proj>\+?\[\[[^\]]+\]\])|(?P<pers>@(?:\[\[[^\]]+\]\]|[A-Za-z][A-Za-z0-9_.\-]*))|(?P<tag>#[A-Za-z][A-Za-z0-9_\-]*)",
        )
        .unwrap()
    });
    let mut segs = Vec::new();
    let mut last = 0usize;
    let push_plain = |segs: &mut Vec<Segment>, s: &str| {
        if !s.is_empty() {
            segs.push(Segment { text: strip_inline(s), kind: String::new(), value: String::new() });
        }
    };
    for caps in re.captures_iter(raw) {
        let m = caps.get(0).unwrap();
        if m.start() > last {
            push_plain(&mut segs, &raw[last..m.start()]);
        }
        let seg = if let Some(g) = caps.name("mdlink") {
            let s = g.as_str();
            let txt = s[1..].split(']').next().unwrap_or("").to_string();
            let url = s.rsplit('(').next().unwrap_or("").trim_end_matches(')').to_string();
            Segment { text: txt, kind: "url".into(), value: url }
        } else if let Some(g) = caps.name("url") {
            Segment { text: g.as_str().into(), kind: "url".into(), value: g.as_str().into() }
        } else if let Some(g) = caps.name("proj") {
            let name = g.as_str().trim_start_matches('+').trim_start_matches("[[").trim_end_matches("]]").to_string();
            Segment { text: name.clone(), kind: "project".into(), value: name }
        } else if let Some(g) = caps.name("pers") {
            let inner = g.as_str().trim_start_matches('@');
            let name = inner.trim_start_matches("[[").trim_end_matches("]]").to_string();
            Segment { text: format!("@{name}"), kind: "person".into(), value: name }
        } else if let Some(g) = caps.name("tag") {
            let name = g.as_str().trim_start_matches('#').to_string();
            Segment { text: format!("#{name}"), kind: "tag".into(), value: name }
        } else {
            continue;
        };
        segs.push(seg);
        last = m.end();
    }
    if last < raw.len() {
        push_plain(&mut segs, &raw[last..]);
    }
    segs
}

/// Parse a note body into renderable blocks for the read view.
pub fn markdown_blocks(body: &str) -> Vec<MdBlock> {
    let mut out: Vec<MdBlock> = Vec::new();
    let mut in_code = false;
    let mut code_is_typst = false;
    let mut code = String::new();
    let mut para = String::new();

    fn flush_para(para: &mut String, out: &mut Vec<MdBlock>) {
        if !para.trim().is_empty() {
            out.push(MdBlock {
                kind: "para".into(),
                text: para.trim().to_string(),
                indent: 0,
            });
        }
        para.clear();
    }

    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                let kind = if code_is_typst { "typst" } else { "code" };
                out.push(MdBlock { kind: kind.into(), text: code.trim_end().into(), indent: 0 });
                code.clear();
                in_code = false;
                code_is_typst = false;
            } else {
                flush_para(&mut para, &mut out);
                let info = line.trim_start().trim_start_matches('`').trim();
                code_is_typst = info.eq_ignore_ascii_case("typst");
                in_code = true;
            }
            continue;
        }
        if in_code {
            code.push_str(line);
            code.push('\n');
            continue;
        }

        let t = line.trim_start();
        let indent = ((line.len() - t.len()) / 2) as i32;

        if t.is_empty() {
            flush_para(&mut para, &mut out);
            continue;
        }
        if t == "---" || t == "***" || t == "___" {
            flush_para(&mut para, &mut out);
            out.push(MdBlock { kind: "rule".into(), text: String::new(), indent: 0 });
            continue;
        }
        // todo line -> an interactive checkbox block (id/done resolved by the UI layer)
        if let Some(c) = todo_re().captures(t) {
            flush_para(&mut para, &mut out);
            let todos = parse_todos("", t);
            let label = todos.first().map(|x| x.text.clone()).unwrap_or_else(|| c["rest"].to_string());
            out.push(MdBlock { kind: "todo".into(), text: label, indent });
            continue;
        }
        // headings
        if let Some(rest) = t.strip_prefix("### ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock { kind: "h3".into(), text: rest.to_string(), indent: 0 });
            continue;
        }
        if let Some(rest) = t.strip_prefix("## ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock { kind: "h2".into(), text: rest.to_string(), indent: 0 });
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock { kind: "h1".into(), text: rest.to_string(), indent: 0 });
            continue;
        }
        // blockquote
        if let Some(rest) = t.strip_prefix("> ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock { kind: "quote".into(), text: rest.to_string(), indent });
            continue;
        }
        // bullet
        if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush_para(&mut para, &mut out);
            out.push(MdBlock { kind: "bullet".into(), text: format!("•  {rest}"), indent });
            continue;
        }
        // numbered: "1. ", "2. " …
        if let Some(dot) = t.find(". ") {
            if dot <= 3 && t[..dot].chars().all(|ch| ch.is_ascii_digit()) {
                flush_para(&mut para, &mut out);
                out.push(MdBlock {
                    kind: "numbered".into(),
                    text: format!("{}.  {}", &t[..dot], &t[dot + 2..]),
                    indent,
                });
                continue;
            }
        }
        // plain text -> accumulate into a paragraph
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(t);
    }
    if in_code && !code.trim().is_empty() {
        out.push(MdBlock { kind: "code".into(), text: code.trim_end().into(), indent: 0 });
    }
    flush_para(&mut para, &mut out);
    out
}

/// Extract todos from a note body. line_no is 0-based for stable ids.
pub fn parse_todos(note_id: &str, body: &str) -> Vec<Todo> {
    // Compile each field regex once for the process, not once per note — this is
    // the hot path during indexing (called for every note on every reindex).
    fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
        cell.get_or_init(|| Regex::new(pat).unwrap())
    }
    static PROJ: OnceLock<Regex> = OnceLock::new();
    static DUE: OnceLock<Regex> = OnceLock::new();
    static START: OnceLock<Regex> = OnceLock::new();
    static PRIO: OnceLock<Regex> = OnceLock::new();
    static REPEAT: OnceLock<Regex> = OnceLock::new();
    static EXT: OnceLock<Regex> = OnceLock::new();
    let proj_re = re(&PROJ, r"\+\[\[(?P<p>[^\]]+)\]\]");
    let due_re = re(&DUE, r"\bdue:(?P<d>\d{4}-\d{2}-\d{2})\b");
    let start_re = re(&START, r"\bstart:(?P<d>\d{4}-\d{2}-\d{2})\b");
    let prio_re = re(&PRIO, r"\[#(?P<p>[ABC])\]");
    let repeat_re = re(&REPEAT, r"\brepeat:(?P<r>\d+[dwm])\b");
    // External-ref hook: jira:KEY-123, gh:owner/repo#1, src:outlook:<id>, ref:<anything>
    let ext_re = re(&EXT, r"\b(?P<ext>(?:jira|gh|src|ref):[A-Za-z0-9_./:#-]+)\b");

    let mut out = Vec::new();
    for (line_no, line) in body.lines().enumerate() {
        if let Some(c) = todo_re().captures(line) {
            let status = marker_to_status(&c["marker"]);
            let kind = c["kind"].to_lowercase();
            let rest = c["rest"].to_string();

            let grab = |re: &Regex, key: &str| {
                re.captures(&rest).map(|m| m[key].to_string()).unwrap_or_default()
            };
            let project = grab(proj_re, "p");
            let person = person_re()
                .captures(&rest)
                .map(|m| person_name(&m))
                .unwrap_or_default();
            let due = grab(due_re, "d");
            let start = grab(start_re, "d");
            let priority = grab(prio_re, "p");
            let repeat = grab(repeat_re, "r");
            let external = grab(ext_re, "ext");

            // Clean display text: drop the tokens we lifted into fields.
            let mut text = rest.clone();
            for re in [proj_re, due_re, start_re, prio_re, repeat_re, ext_re] {
                text = re.replace_all(&text, "").to_string();
            }
            text = person_re().replace_all(&text, " ").to_string();
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

            out.push(Todo {
                id: format!("{note_id}:{line_no}"),
                note_id: note_id.to_string(),
                done: status == "done",
                status,
                kind,
                text,
                project,
                person,
                start,
                due,
                external,
                priority,
                repeat,
                line_no,
            });
        }
    }
    out
}

/// All `[[wikilink]]` targets in a body (projects / workstreams / pages).
pub fn parse_links(body: &str) -> Vec<String> {
    let mut v: Vec<String> = link_re()
        .captures_iter(body)
        .map(|c| c["t"].to_string())
        .collect();
    v.sort();
    v.dedup();
    v
}

// ---------------------------------------------------------------------------
// File IO — frontmatter + body
// ---------------------------------------------------------------------------

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

fn read_note(path: &Path) -> Result<Note> {
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

fn write_note(note: &Note) -> Result<()> {
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

// ---------------------------------------------------------------------------
// Backend: vault + index
// ---------------------------------------------------------------------------

pub struct Backend {
    pub vault: PathBuf,
    conn: Connection,
    fts: bool, // FTS5 available in this SQLite build
}

/// Turn a user query into an FTS5 prefix-match expression (sanitized).
fn fts_query(s: &str) -> String {
    s.split_whitespace()
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
        .filter(|w| !w.is_empty())
        .map(|w| format!("{w}*"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Wipe and rebuild every index table from the markdown files on disk, using the
/// given connection. Shared by the synchronous `Backend::reindex_all` (startup)
/// and the background reindex (live/manual). Files remain the source of truth.
pub fn reindex_connection(conn: &mut Connection, vault: &Path, fts: bool) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute_batch("DELETE FROM notes; DELETE FROM links; DELETE FROM tags; DELETE FROM mentions; DELETE FROM todos;")?;
    if fts {
        let _ = tx.execute_batch("DELETE FROM notes_fts;");
    }
    let notes_dir = vault.join("notes");
    for entry in WalkDir::new(&notes_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(note) = read_note(p) {
                Backend::index_note(&tx, &note, fts)?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// Run a full reindex on its own SQLite connection to the same on-disk index.
/// Intended to be called from a background thread; with WAL journaling the UI
/// connection keeps serving reads while this writes. Blocks the *calling*
/// (worker) thread, not the UI event loop.
pub fn background_reindex(vault: &Path, fts: bool) -> Result<()> {
    let mut conn = Connection::open(vault.join(".index").join("index.db"))?;
    let _ = conn.execute_batch("PRAGMA busy_timeout=5000;");
    reindex_connection(&mut conn, vault, fts)
}

/// Filesystem-safe filename from a note title (falls back to its id).
fn safe_filename(title: &str, id: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let s = s.trim();
    if s.is_empty() { id.to_string() } else { s.chars().take(60).collect() }
}

/// Escape the characters Typst treats as markup so arbitrary note text compiles
/// verbatim (we trade markdown emphasis fidelity for guaranteed compilation).
fn typst_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if matches!(c, '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '~' | '=' | '[' | ']') {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// Lightweight markdown→Typst for PDF export: converts `#`/`##`/`###` headings
/// and `- ` bullets, escapes everything else, and forces per-line breaks so the
/// note's literal layout is preserved. Not a full markdown renderer.
fn markdown_to_typst(title: &str, body: &str) -> String {
    let mut out = String::from("#set page(margin: 2cm)\n#set text(size: 11pt)\n#set par(justify: true)\n\n");
    out.push_str(&format!("= {}\n\n", typst_escape(title)));
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(r) = t.strip_prefix("### ") {
            out.push_str(&format!("=== {}\n", typst_escape(r)));
        } else if let Some(r) = t.strip_prefix("## ") {
            out.push_str(&format!("== {}\n", typst_escape(r)));
        } else if let Some(r) = t.strip_prefix("# ") {
            out.push_str(&format!("= {}\n", typst_escape(r)));
        } else if let Some(r) = t.strip_prefix("- ") {
            out.push_str(&format!("- {}\n", typst_escape(r)));
        } else if t.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&typst_escape(line));
            out.push_str(" \\\n");
        }
    }
    out
}

impl Backend {
    /// Open the vault and build the schema, but DO NOT index — the index starts
    /// empty. Callers that need data immediately (tests, CLI) use [`open`];
    /// the app uses this and kicks off a background reindex so the window never
    /// waits on indexing. Queries return nothing until the first reindex lands.
    pub fn open_lazy(vault: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(vault.join("notes"))?;
        let index_dir = vault.join(".index");
        std::fs::create_dir_all(&index_dir)?;
        let conn = Connection::open(index_dir.join("index.db"))?;
        // WAL lets a background reindex (separate connection) write while the UI
        // connection keeps reading without blocking — keeps the event loop snappy.
        let _ = conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
        );
        // The index is disposable, so we (re)create a fresh schema each open —
        // no migrations needed when the schema evolves.
        conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS notes;
            DROP TABLE IF EXISTS links;
            DROP TABLE IF EXISTS tags;
            DROP TABLE IF EXISTS mentions;
            DROP TABLE IF EXISTS todos;
            CREATE TABLE notes(
                id TEXT PRIMARY KEY, title TEXT, path TEXT,
                created TEXT, updated TEXT, kind TEXT, body TEXT, archived INTEGER);
            CREATE TABLE links(note_id TEXT, target TEXT);
            CREATE TABLE tags(note_id TEXT, tag TEXT);
            CREATE TABLE mentions(note_id TEXT, person TEXT);
            CREATE TABLE todos(
                id TEXT PRIMARY KEY, note_id TEXT, kind TEXT, status TEXT, text TEXT,
                project TEXT, person TEXT, start TEXT, due TEXT, external TEXT,
                priority TEXT, repeat TEXT, done INTEGER, line_no INTEGER);
            CREATE INDEX idx_todos_note ON todos(note_id);
            CREATE INDEX idx_links_note ON links(note_id);
            CREATE INDEX idx_tags_note ON tags(note_id);
            CREATE INDEX idx_mentions_note ON mentions(note_id);
            "#,
        )?;
        // FTS5 full-text index for note search; gracefully skip if unavailable.
        let fts = conn
            .execute_batch(
                "DROP TABLE IF EXISTS notes_fts; CREATE VIRTUAL TABLE notes_fts USING fts5(note_id, title, body);",
            )
            .is_ok();
        Ok(Backend { vault, conn, fts })
    }

    /// Open and fully index synchronously (data ready on return).
    pub fn open(vault: PathBuf) -> Result<Self> {
        let mut b = Self::open_lazy(vault)?;
        b.reindex_all()?;
        Ok(b)
    }

    /// Cheap filesystem check (no index needed): does the vault hold any note?
    /// Used at startup to decide whether to seed the welcome note before the
    /// background index has run.
    pub fn is_vault_empty(&self) -> bool {
        let notes_dir = self.vault.join("notes");
        !WalkDir::new(&notes_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().map(|x| x == "md").unwrap_or(false))
    }

    /// Wipe and rebuild the index from the markdown files. Files stay truth.
    /// Runs on the caller's thread — used at startup. For live/manual reindex
    /// while the app is running, use a background connection (see
    /// [`background_reindex`]) so the UI event loop never blocks.
    pub fn reindex_all(&mut self) -> Result<()> {
        reindex_connection(&mut self.conn, &self.vault, self.fts)
    }

    /// Parameters a background-thread reindex needs (vault path + FTS availability).
    pub fn reindex_params(&self) -> (PathBuf, bool) {
        (self.vault.clone(), self.fts)
    }

    fn index_note(tx: &rusqlite::Transaction, note: &Note, fts: bool) -> Result<()> {
        if fts {
            let _ = tx.execute("DELETE FROM notes_fts WHERE note_id=?", [&note.id]);
            let _ = tx.execute(
                "INSERT INTO notes_fts(note_id,title,body) VALUES(?,?,?)",
                rusqlite::params![note.id, note.title, note.body],
            );
        }
        let archived = note.path.to_string_lossy().contains("/archive/") as i64;
        tx.execute(
            "INSERT OR REPLACE INTO notes(id,title,path,created,updated,kind,body,archived) VALUES(?,?,?,?,?,?,?,?)",
            rusqlite::params![
                note.id,
                note.title,
                note.path.to_string_lossy(),
                note.created,
                note.updated,
                note.kind,
                note.body,
                archived
            ],
        )?;
        tx.execute("DELETE FROM links WHERE note_id=?", [&note.id])?;
        for target in parse_links(&note.body) {
            tx.execute(
                "INSERT INTO links(note_id,target) VALUES(?,?)",
                rusqlite::params![note.id, target],
            )?;
        }
        tx.execute("DELETE FROM tags WHERE note_id=?", [&note.id])?;
        for tag in parse_tags(&note.body) {
            tx.execute(
                "INSERT INTO tags(note_id,tag) VALUES(?,?)",
                rusqlite::params![note.id, tag],
            )?;
        }
        tx.execute("DELETE FROM mentions WHERE note_id=?", [&note.id])?;
        for person in parse_mentions(&note.body) {
            tx.execute(
                "INSERT INTO mentions(note_id,person) VALUES(?,?)",
                rusqlite::params![note.id, person],
            )?;
        }
        tx.execute("DELETE FROM todos WHERE note_id=?", [&note.id])?;
        for t in parse_todos(&note.id, &note.body) {
            tx.execute(
                "INSERT OR REPLACE INTO todos(id,note_id,kind,status,text,project,person,start,due,external,priority,repeat,done,line_no)
                 VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    t.id, t.note_id, t.kind, t.status, t.text, t.project, t.person,
                    t.start, t.due, t.external, t.priority, t.repeat, t.done as i64, t.line_no as i64
                ],
            )?;
        }
        Ok(())
    }

    /// Row -> Todo, for the new column order (`t.` prefixed selects).
    fn row_to_todo(r: &rusqlite::Row) -> rusqlite::Result<Todo> {
        Ok(Todo {
            id: r.get(0)?,
            note_id: r.get(1)?,
            kind: r.get(2)?,
            status: r.get(3)?,
            text: r.get(4)?,
            project: r.get(5)?,
            person: r.get(6)?,
            start: r.get(7)?,
            due: r.get(8)?,
            external: r.get(9)?,
            priority: r.get(10)?,
            repeat: r.get(11)?,
            done: r.get::<_, i64>(12)? != 0,
            line_no: r.get::<_, i64>(13)? as usize,
        })
    }

    // ---- Queries used by the UI ----

    #[allow(dead_code)] // superseded by query_notes; kept for tests/back-compat
    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,title,path,created,updated,kind FROM notes ORDER BY updated DESC, title ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Note {
                id: r.get(0)?,
                title: r.get(1)?,
                path: PathBuf::from(r.get::<_, String>(2)?),
                created: r.get(3)?,
                updated: r.get(4)?,
                kind: r.get(5)?,
                body: String::new(),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT target, COUNT(*) FROM links GROUP BY target ORDER BY target ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// People mentioned anywhere (`@[[Name]]`), with how many notes mention them.
    /// Mentioning someone is how you "create" a person.
    pub fn list_people(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT person, COUNT(DISTINCT note_id) FROM mentions \
             GROUP BY person ORDER BY person ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_tags(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag, COUNT(*) FROM tags GROUP BY tag ORDER BY tag ASC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Project {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Canonical todo columns, with the given alias prefix (e.g. "t." or "").
    fn todo_cols(prefix: &str) -> String {
        let f = [
            "id", "note_id", "kind", "status", "text", "project", "person", "start", "due",
            "external", "priority", "repeat", "done", "line_no",
        ];
        f.iter()
            .map(|c| format!("{prefix}{c}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The one query that powers every view. Joins tags/notes only when needed.
    pub fn query_todos(&self, f: &Filter) -> Result<Vec<Todo>> {
        let mut sql = format!("SELECT {} FROM todos t", Self::todo_cols("t."));
        let mut where_: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        if !f.tag.is_empty() {
            sql.push_str(" JOIN tags tg ON tg.note_id = t.note_id AND (tg.tag = ? OR tg.tag LIKE ?)");
            binds.push(f.tag.clone());
            binds.push(format!("{}/%", f.tag));
        }
        if !f.search.is_empty() {
            where_.push("t.text LIKE ?".into());
            binds.push(Filter::like(&f.search));
        }
        if !f.project.is_empty() {
            where_.push("(t.project = ? OR t.project LIKE ?)".into());
            binds.push(f.project.clone());
            binds.push(format!("{}/%", f.project));
        }
        if !f.person.is_empty() {
            where_.push("t.person = ?".into());
            binds.push(f.person.clone());
        }
        if !f.kind.is_empty() {
            where_.push("t.kind = ?".into());
            binds.push(f.kind.clone());
        }
        if !f.priority.is_empty() {
            where_.push("t.priority = ?".into());
            binds.push(f.priority.clone());
        }
        if !f.due_bucket.is_empty() {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let week = (chrono::Local::now() + chrono::Duration::days(7))
                .format("%Y-%m-%d")
                .to_string();
            match f.due_bucket.as_str() {
                "overdue" => {
                    where_.push("(t.due != '' AND t.due < ? AND t.done = 0)".into());
                    binds.push(today);
                }
                "week" => {
                    where_.push("(t.due >= ? AND t.due <= ?)".into());
                    binds.push(today);
                    binds.push(week);
                }
                "hasdate" => where_.push("t.due != ''".into()),
                "nodate" => where_.push("t.due = ''".into()),
                _ => {}
            }
        }
        if f.status == "open" {
            where_.push("t.status != 'done'".into());
        } else if !f.status.is_empty() {
            where_.push("t.status = ?".into());
            binds.push(f.status.clone());
        }
        if !where_.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_.join(" AND "));
        }
        sql.push_str(" ORDER BY t.done ASC, t.due ASC, t.line_no ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(binds.iter());
        let rows = stmt.query_map(params, Self::row_to_todo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Notes view query: search title/body, filter by project/tag/person.
    pub fn query_notes(&self, f: &Filter) -> Result<Vec<Note>> {
        let mut sql = String::from("SELECT DISTINCT n.id,n.title,n.path,n.created,n.updated,n.kind FROM notes n");
        let mut where_: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        if !f.project.is_empty() {
            sql.push_str(" JOIN links l ON l.note_id = n.id AND (l.target = ? OR l.target LIKE ?)");
            binds.push(f.project.clone());
            binds.push(format!("{}/%", f.project));
        }
        if !f.tag.is_empty() {
            sql.push_str(" JOIN tags tg ON tg.note_id = n.id AND (tg.tag = ? OR tg.tag LIKE ?)");
            binds.push(f.tag.clone());
            binds.push(format!("{}/%", f.tag));
        }
        if !f.person.is_empty() {
            sql.push_str(" JOIN mentions mp ON mp.note_id = n.id AND mp.person = ?");
            binds.push(f.person.clone());
        }
        if !f.search.is_empty() {
            let q = fts_query(&f.search);
            if self.fts && !q.is_empty() {
                sql.push_str(" JOIN notes_fts ON notes_fts.note_id = n.id");
                where_.push("notes_fts MATCH ?".into());
                binds.push(q);
            } else {
                where_.push("(n.title LIKE ? OR n.body LIKE ?)".into());
                binds.push(Filter::like(&f.search));
                binds.push(Filter::like(&f.search));
            }
        }
        if !f.show_archived {
            where_.push("n.archived = 0".into());
        }
        if !where_.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_.join(" AND "));
        }
        sql.push_str(" ORDER BY n.updated DESC, n.title ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(binds.iter());
        let rows = stmt.query_map(params, |r| {
            Ok(Note {
                id: r.get(0)?,
                title: r.get(1)?,
                path: PathBuf::from(r.get::<_, String>(2)?),
                created: r.get(3)?,
                updated: r.get(4)?,
                kind: r.get(5)?,
                body: String::new(),
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Bucket filtered todos into Kanban columns by a grouping dimension.
    /// Returns (column-label, column-key, todos) in display order.
    pub fn board(&self, group_by: &str, f: &Filter) -> Result<Vec<(String, String, Vec<Todo>)>> {
        let todos = self.query_todos(f)?;
        let key_of = |t: &Todo| -> String {
            match group_by {
                "status" => t.status.clone(),
                "project" | "workstream" => t.project.clone(),
                "person" => t.person.clone(),
                _ => t.kind.clone(),
            }
        };
        // Fixed column order for status/kind; data-driven (sorted) for project/person.
        let mut order: Vec<String> = match group_by {
            "status" => STATUSES.iter().map(|s| s.to_string()).collect(),
            "kind" => KINDS.iter().map(|s| s.to_string()).collect(),
            _ => {
                let mut keys: Vec<String> = todos.iter().map(&key_of).collect();
                keys.sort();
                keys.dedup();
                keys
            }
        };
        // Ensure any present-but-unlisted key still gets a column.
        for t in &todos {
            let k = key_of(t);
            if !order.contains(&k) {
                order.push(k);
            }
        }
        let label = |key: &str| -> String {
            if key.is_empty() {
                "(none)".into()
            } else if group_by == "status" {
                match key {
                    "todo" => "To Do",
                    "doing" => "Doing",
                    "done" => "Done",
                    other => other,
                }
                .into()
            } else {
                key.into()
            }
        };
        Ok(order
            .into_iter()
            .map(|key| {
                let items: Vec<Todo> = todos.iter().filter(|t| key_of(t) == key).cloned().collect();
                (label(&key), key, items)
            })
            .collect())
    }

    /// Open, due-dated todos for the Gantt view, sorted by start/due.
    pub fn gantt_items(&self, f: &Filter) -> Result<Vec<Todo>> {
        let mut items: Vec<Todo> = self
            .query_todos(f)?
            .into_iter()
            .filter(|t| !t.due.is_empty() && !t.done)
            .collect();
        items.sort_by(|a, b| {
            let ak = if a.start.is_empty() { &a.due } else { &a.start };
            let bk = if b.start.is_empty() { &b.due } else { &b.start };
            ak.cmp(bk)
        });
        Ok(items)
    }

    /// Back-compat string-filter helper used by tests.
    #[cfg(test)]
    pub fn list_todos(&self, filter: &str) -> Result<Vec<Todo>> {
        if filter == "stale" {
            let cutoff = (Utc::now() - chrono::Duration::days(STALE_DAYS))
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string();
            let sql = format!(
                "SELECT {} FROM todos t JOIN notes n ON t.note_id = n.id \
                 WHERE t.done = 0 AND t.kind IN ('followup','delegated') AND n.updated < ? \
                 ORDER BY n.updated ASC",
                Self::todo_cols("t.")
            );
            let mut stmt = self.conn.prepare(&sql)?;
            let rows = stmt.query_map([cutoff], Self::row_to_todo)?;
            return Ok(rows.filter_map(|r| r.ok()).collect());
        }
        let f = if filter == "all" {
            Filter::default()
        } else if let Some(p) = filter.strip_prefix("person:") {
            Filter { person: p.into(), status: "open".into(), ..Default::default() }
        } else if let Some(p) = filter.strip_prefix("project:") {
            Filter { project: p.into(), ..Default::default() }
        } else {
            Filter { kind: filter.into(), ..Default::default() }
        };
        self.query_todos(&f)
    }

    // ---- Mutations ----

    pub fn new_note(&mut self) -> Result<Note> {
        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let note = Note {
            id: id.clone(),
            title: format!("Note {date}"),
            created: now.clone(),
            updated: now,
            kind: "markdown".into(),
            body: String::new(),
            path: self.vault.join("notes").join(format!("{id}.md")),
        };
        write_note(&note)?;
        let tx = self.conn.transaction()?;
        Self::index_note(&tx, &note, self.fts)?;
        tx.commit()?;
        Ok(note)
    }

    /// Create a new note that joins the same topics/clusters as `source_id`
    /// (copies its `[[links]]`) and back-links to it. For "new meeting note in
    /// the same thread" without rewriting the old one.
    pub fn new_related_note(&mut self, source_id: &str) -> Result<Note> {
        let src = self.load_note(source_id)?;
        let links = parse_links(&src.body);
        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let mut body = String::new();
        if !src.title.is_empty() {
            body += &format!("(continues [[{}]])\n", src.title);
        }
        if !links.is_empty() {
            body += &links.iter().map(|l| format!("[[{l}]]")).collect::<Vec<_>>().join(" ");
            body.push('\n');
        }
        body.push('\n');
        let note = Note {
            id: id.clone(),
            title: format!("{} — {date}", src.title),
            created: now.clone(),
            updated: now,
            kind: "auto".into(),
            body,
            path: self.vault.join("notes").join(format!("{id}.md")),
        };
        self.persist(&note)?;
        Ok(note)
    }

    pub fn load_note(&self, id: &str) -> Result<Note> {
        let path: String = self
            .conn
            .query_row("SELECT path FROM notes WHERE id=?", [id], |r| r.get(0))?;
        read_note(Path::new(&path))
    }

    /// Compile a `kind: typst` note to a PNG via the `typst` CLI; return its path.
    pub fn render_typst(&self, note: &Note) -> Option<PathBuf> {
        self.compile_typst(&note.id, &note.body, false)
    }

    /// Compile an inline ```typst block to an auto-sized PNG, cached by a content
    /// hash so unchanged blocks never recompile (keeps live preview snappy).
    pub fn render_typst_src(&self, source: &str) -> Option<PathBuf> {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut h);
        let key = format!("block-{:016x}", h.finish());
        self.compile_typst(&key, source, true)
    }

    fn compile_typst(&self, key: &str, source: &str, auto_size: bool) -> Option<PathBuf> {
        let dir = self.vault.join(".index").join("render");
        std::fs::create_dir_all(&dir).ok()?;
        let png = dir.join(format!("{key}.png"));
        // content-addressed blocks are cacheable; note-level ones always re-render
        if auto_size && png.exists() {
            return Some(png);
        }
        let typ = dir.join(format!("{key}.typ"));
        let body = if auto_size {
            format!("#set page(width: auto, height: auto, margin: 6pt)\n{source}")
        } else {
            source.to_string()
        };
        std::fs::write(&typ, body).ok()?;
        let out = std::process::Command::new("typst")
            .args(["compile", "--format", "png", "--ppi", "144"])
            .arg(&typ)
            .arg(&png)
            .output()
            .ok()?;
        out.status.success().then_some(png)
    }

    /// Export a note to `<vault>/exports/`. `format`: "md" copies the markdown
    /// file as-is; "pdf" compiles via the Typst CLI (typst notes natively;
    /// markdown notes through a lightweight converter). Returns the written path.
    pub fn export_note(&self, id: &str, format: &str) -> Result<PathBuf> {
        let note = self.load_note(id)?;
        let dir = self.vault.join("exports");
        std::fs::create_dir_all(&dir)?;
        let stem = safe_filename(&note.title, &note.id);
        match format {
            "md" => {
                let dest = dir.join(format!("{stem}.md"));
                std::fs::copy(&note.path, &dest)?;
                Ok(dest)
            }
            "pdf" => {
                let dest = dir.join(format!("{stem}.pdf"));
                let src = if effective_kind(&note.kind, &note.body) == "typst" {
                    note.body.clone()
                } else {
                    markdown_to_typst(&note.title, &note.body)
                };
                let typ = dir.join(format!(".{stem}.typ"));
                std::fs::write(&typ, src)?;
                let out = std::process::Command::new("typst")
                    .args(["compile", "--format", "pdf"])
                    .arg(&typ)
                    .arg(&dest)
                    .output();
                let _ = std::fs::remove_file(&typ);
                match out {
                    Ok(o) if o.status.success() => Ok(dest),
                    Ok(o) => anyhow::bail!(
                        "typst failed: {}",
                        String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("compile error")
                    ),
                    Err(_) => anyhow::bail!("typst CLI not found — install typst to export PDF"),
                }
            }
            _ => anyhow::bail!("unknown export format: {format}"),
        }
    }

    /// Entities referenced in a note body: (projects, people, tags).
    pub fn note_entities(body: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
        (parse_links(body), parse_mentions(body), parse_tags(body))
    }

    pub fn save_note(&mut self, id: &str, title: &str, body: &str) -> Result<()> {
        let mut note = self.load_note(id)?;
        note.title = title.to_string();
        note.body = body.to_string();
        note.updated = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        if note.created.is_empty() {
            note.created = note.updated.clone();
        }
        write_note(&note)?;
        let tx = self.conn.transaction()?;
        Self::index_note(&tx, &note, self.fts)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_todo(&self, id: &str) -> Result<Todo> {
        let sql = format!("SELECT {} FROM todos t WHERE t.id = ?", Self::todo_cols("t."));
        Ok(self.conn.query_row(&sql, [id], Self::row_to_todo)?)
    }

    /// Load the note owning `todo_id`, transform the todo's line, save + reindex.
    fn rewrite_line<F: Fn(&str) -> String>(&mut self, todo_id: &str, transform: F) -> Result<()> {
        let (note_id, line_no) = todo_id.rsplit_once(':').context("bad todo id")?;
        let line_no: usize = line_no.parse()?;
        let mut note = self.load_note(note_id)?;
        let mut lines: Vec<String> = note.body.lines().map(|s| s.to_string()).collect();
        if let Some(line) = lines.get_mut(line_no) {
            *line = transform(line);
        }
        note.body = lines.join("\n");
        if note.path.exists() {
            let orig = std::fs::read_to_string(&note.path).unwrap_or_default();
            if orig.ends_with('\n') && !note.body.ends_with('\n') {
                note.body.push('\n');
            }
        }
        write_note(&note)?;
        let tx = self.conn.transaction()?;
        Self::index_note(&tx, &note, self.fts)?;
        tx.commit()?;
        Ok(())
    }

    /// Cycle a todo done <-> not-done (the list checkbox).
    pub fn toggle_todo(&mut self, todo_id: &str) -> Result<()> {
        self.rewrite_line(todo_id, |line| {
            if line.contains("DONE(") {
                set_marker_kind(line, Some("TODO"), None)
            } else {
                set_marker_kind(line, Some("DONE"), None)
            }
        })
    }

    pub fn set_todo_status(&mut self, todo_id: &str, status: &str) -> Result<()> {
        let marker = match status {
            "doing" => "DOING",
            "done" => "DONE",
            _ => "TODO",
        };
        self.rewrite_line(todo_id, |line| set_marker_kind(line, Some(marker), None))
    }

    pub fn set_todo_kind(&mut self, todo_id: &str, kind: &str) -> Result<()> {
        let kind = kind.to_string();
        self.rewrite_line(todo_id, move |line| set_marker_kind(line, None, Some(&kind)))
    }

    /// Append a new todo (built from form fields) to a note. Returns its id.
    pub fn add_todo(&mut self, note_id: &str, fields: &TodoFields) -> Result<String> {
        let mut note = self.load_note(note_id)?;
        if !note.body.is_empty() && !note.body.ends_with('\n') {
            note.body.push('\n');
        }
        let line_no = note.body.lines().count();
        note.body.push_str(&format_todo_line(fields));
        note.body.push('\n');
        self.persist(&note)?;
        Ok(format!("{note_id}:{line_no}"))
    }

    /// Cycle a todo's state TODO → DOING → DONE → TODO. A recurring todo
    /// (`repeat:`) that would complete instead advances its dates and stays TODO.
    pub fn cycle_todo(&mut self, todo_id: &str) -> Result<()> {
        let t = self.get_todo(todo_id)?;
        let next = match t.status.as_str() {
            "todo" => "doing",
            "doing" => "done",
            _ => "todo",
        };
        if next == "done" && !t.repeat.is_empty() && !t.due.is_empty() {
            let mut f = TodoFields::from_todo(&t);
            f.status = "todo".into();
            f.due = advance_date(&t.due, &t.repeat);
            if !t.start.is_empty() {
                f.start = advance_date(&t.start, &t.repeat);
            }
            return self.update_todo(todo_id, &f);
        }
        self.set_todo_status(todo_id, next)
    }

    /// Open, due-dated todos sorted by due date then priority — the Agenda feed.
    pub fn agenda(&self, f: &Filter) -> Result<Vec<Todo>> {
        let mut v: Vec<Todo> = self
            .query_todos(f)?
            .into_iter()
            .filter(|t| !t.done && !t.due.is_empty())
            .collect();
        v.sort_by(|a, b| a.due.cmp(&b.due).then(a.priority.cmp(&b.priority)));
        Ok(v)
    }

    /// Inbox: un-filed notes (no project/workstream link yet), newest first.
    pub fn inbox(&self) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT n.id,n.title,n.path,n.created,n.updated,n.kind FROM notes n \
             WHERE n.archived = 0 AND NOT EXISTS (SELECT 1 FROM links l WHERE l.note_id = n.id) \
             ORDER BY n.updated DESC",
        )?;
        let rows = stmt.query_map([], Self::note_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Backlinks: notes that link to `target` (a project / note title).
    pub fn backlinks(&self, target: &str) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT n.id,n.title,n.path,n.created,n.updated,n.kind FROM notes n \
             JOIN links l ON l.note_id = n.id WHERE l.target = ? AND n.archived = 0 \
             ORDER BY n.updated DESC",
        )?;
        let rows = stmt.query_map([target], Self::note_row)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    fn note_row(r: &rusqlite::Row) -> rusqlite::Result<Note> {
        Ok(Note {
            id: r.get(0)?,
            title: r.get(1)?,
            path: PathBuf::from(r.get::<_, String>(2)?),
            created: r.get(3)?,
            updated: r.get(4)?,
            kind: r.get(5)?,
            body: String::new(),
        })
    }

    /// Attach a filesystem file/folder to a note as a clickable link.
    pub fn attach_path(&mut self, note_id: &str, path: &str) -> Result<()> {
        let path = path.trim();
        if path.is_empty() {
            return Ok(());
        }
        let name = std::path::Path::new(path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        let mut note = self.load_note(note_id)?;
        if !note.body.is_empty() && !note.body.ends_with('\n') {
            note.body.push('\n');
        }
        note.body.push_str(&format!("[📎 {name}]({path})\n"));
        self.persist(&note)
    }

    // ---- Saved smart lists (named filters) ----

    fn smartlists_path(&self) -> PathBuf {
        self.vault.join("smartlists.json")
    }

    fn load_smartlists(&self) -> Vec<NamedFilter> {
        std::fs::read_to_string(self.smartlists_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn list_smart_lists(&self) -> Vec<String> {
        self.load_smartlists().into_iter().map(|n| n.name).collect()
    }

    pub fn get_smart_list(&self, name: &str) -> Option<Filter> {
        self.load_smartlists().into_iter().find(|n| n.name == name).map(|n| n.filter)
    }

    pub fn save_smart_list(&self, name: &str, f: &Filter) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(());
        }
        let mut v = self.load_smartlists();
        v.retain(|n| n.name != name);
        v.push(NamedFilter { name: name.to_string(), filter: f.clone() });
        v.sort_by(|a, b| a.name.cmp(&b.name));
        std::fs::write(self.smartlists_path(), serde_json::to_string_pretty(&v)?)?;
        Ok(())
    }

    pub fn delete_smart_list(&self, name: &str) -> Result<()> {
        let mut v = self.load_smartlists();
        v.retain(|n| n.name != name);
        std::fs::write(self.smartlists_path(), serde_json::to_string_pretty(&v)?)?;
        Ok(())
    }

    /// Create a note from a built-in template (meeting / oneonone / decision).
    pub fn new_from_template(&mut self, template: &str) -> Result<Note> {
        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let (title, body): (String, &str) = match template {
            "meeting" => (
                format!("Meeting — {date}"),
                "## Attendees\n@\n\n## Notes\n- \n\n## Action items\nTODO(do) \n",
            ),
            "oneonone" => (
                format!("1:1 — {date}"),
                "## Updates\n\n## To discuss\nTODO(followup) ask about ... @[[ ]]\n\n## Delegated / awaiting\nTODO(delegated) ... @[[ ]]\n",
            ),
            "decision" => (
                format!("Decision — {date}"),
                "## Context\n\n## Decision\n\n## Owner & next steps\nTODO(do) \n",
            ),
            _ => (format!("Note {date}"), ""),
        };
        let note = Note {
            id: id.clone(),
            title,
            created: now.clone(),
            updated: now,
            kind: "auto".into(),
            body: body.to_string(),
            path: self.vault.join("notes").join(format!("{id}.md")),
        };
        self.persist(&note)?;
        Ok(note)
    }

    /// Open, stale follow-ups/delegated todos (note untouched > STALE_DAYS).
    pub fn stale_todos(&self) -> Result<Vec<Todo>> {
        let cutoff = (Utc::now() - chrono::Duration::days(STALE_DAYS))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string();
        let sql = format!(
            "SELECT {} FROM todos t JOIN notes n ON t.note_id = n.id \
             WHERE t.done = 0 AND t.kind IN ('followup','delegated') AND n.updated < ? AND n.archived = 0 \
             ORDER BY n.updated ASC",
            Self::todo_cols("t.")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([cutoff], Self::row_to_todo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Soft-delete a note: move it to the vault `.trash` (not indexed).
    pub fn delete_note(&mut self, note_id: &str) -> Result<()> {
        let note = self.load_note(note_id)?;
        let trash = self.vault.join(".trash");
        std::fs::create_dir_all(&trash)?;
        let dest = trash.join(note.path.file_name().unwrap());
        std::fs::rename(&note.path, &dest)?;
        self.reindex_all()
    }

    /// Trashed notes as (filename, title), newest first.
    pub fn list_trash(&self) -> Result<Vec<(String, String)>> {
        let trash = self.vault.join(".trash");
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&trash) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "md").unwrap_or(false) {
                    let title = read_note(&p).map(|n| n.title).unwrap_or_default();
                    let file = p.file_name().unwrap().to_string_lossy().to_string();
                    out.push((file, title));
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Restore a trashed note (by filename) back into the vault.
    pub fn restore_note(&mut self, filename: &str) -> Result<()> {
        let src = self.vault.join(".trash").join(filename);
        let dest = self.vault.join("notes").join(filename);
        if src.exists() {
            std::fs::rename(&src, &dest)?;
            self.reindex_all()?;
        }
        Ok(())
    }

    /// Move a note into the archive subfolder (hidden from default views).
    pub fn archive_note(&mut self, note_id: &str, archive: bool) -> Result<()> {
        let note = self.load_note(note_id)?;
        let file = note.path.file_name().unwrap().to_os_string();
        let dest_dir = if archive {
            self.vault.join("notes").join("archive")
        } else {
            self.vault.join("notes")
        };
        std::fs::create_dir_all(&dest_dir)?;
        let dest = dest_dir.join(file);
        if dest != note.path {
            std::fs::rename(&note.path, &dest)?;
        }
        self.reindex_all()
    }

    /// Switch a note between markdown and typst rendering.
    pub fn set_note_kind(&mut self, note_id: &str, kind: &str) -> Result<()> {
        let mut note = self.load_note(note_id)?;
        note.kind = kind.to_string();
        note.updated = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.persist(&note)
    }

    /// File a note into a topic/project by appending a `[[Topic]]` link.
    pub fn add_link(&mut self, note_id: &str, topic: &str) -> Result<()> {
        let topic = topic.trim().trim_start_matches("[[").trim_end_matches("]]").trim();
        if topic.is_empty() {
            return Ok(());
        }
        let mut note = self.load_note(note_id)?;
        if parse_links(&note.body).iter().any(|t| t.eq_ignore_ascii_case(topic)) {
            return Ok(());
        }
        if !note.body.is_empty() && !note.body.ends_with('\n') {
            note.body.push('\n');
        }
        note.body.push_str(&format!("[[{topic}]]\n"));
        self.persist(&note)
    }

    /// Append a `#tag` label to a note (no-op if already present).
    pub fn add_tag(&mut self, note_id: &str, tag: &str) -> Result<()> {
        let tag = tag.trim().trim_start_matches('#').trim();
        if tag.is_empty() {
            return Ok(());
        }
        let mut note = self.load_note(note_id)?;
        if parse_tags(&note.body)
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag))
        {
            return Ok(());
        }
        if !note.body.is_empty() && !note.body.ends_with('\n') {
            note.body.push('\n');
        }
        note.body.push_str(&format!("#{tag}\n"));
        self.persist(&note)
    }

    /// Replace a todo's line wholesale from form fields.
    pub fn update_todo(&mut self, todo_id: &str, fields: &TodoFields) -> Result<()> {
        let line = format_todo_line(fields);
        self.rewrite_line(todo_id, |_old| line.clone())
    }

    /// Drag-and-drop a card onto a column: set the grouped dimension to the
    /// column's value (status/kind/project/person), rewriting the line.
    pub fn drop_card(&mut self, todo_id: &str, group_by: &str, target_key: &str) -> Result<()> {
        let mut fields = TodoFields::from_todo(&self.get_todo(todo_id)?);
        let val = if target_key == "(none)" { "" } else { target_key };
        match group_by {
            "status" => fields.status = val.to_string(),
            "kind" => fields.kind = val.to_string(),
            "project" | "workstream" => fields.project = val.to_string(),
            "person" => fields.person = val.to_string(),
            _ => {}
        }
        self.update_todo(todo_id, &fields)
    }

    /// Write a note to disk and reindex it in one shot.
    fn persist(&mut self, note: &Note) -> Result<()> {
        write_note(note)?;
        let fts = self.fts;
        let tx = self.conn.transaction()?;
        Self::index_note(&tx, note, fts)?;
        tx.commit()?;
        Ok(())
    }

    /// Move a card one column left/right on the board (status or kind boards).
    pub fn board_move(&mut self, todo_id: &str, group_by: &str, dir: i32) -> Result<()> {
        let order: Vec<&str> = match group_by {
            "status" => STATUSES.to_vec(),
            "kind" => KINDS.to_vec(),
            _ => return Ok(()), // project/person columns aren't ordinal — no move
        };
        let todo = self.get_todo(todo_id)?;
        let cur = if group_by == "status" { &todo.status } else { &todo.kind };
        let idx = order.iter().position(|x| x == cur).unwrap_or(0) as i32;
        let ni = (idx + dir).clamp(0, order.len() as i32 - 1) as usize;
        let target = order[ni];
        if group_by == "status" {
            self.set_todo_status(todo_id, target)
        } else {
            self.set_todo_kind(todo_id, target)
        }
    }
}

/// Replace the marker and/or kind inside a `MARKER(kind)` todo line, preserving
/// leading whitespace and the rest of the line verbatim.
fn set_marker_kind(line: &str, marker: Option<&str>, kind: Option<&str>) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(?P<ws>\s*)(?P<m>TODO|DOING|DONE)\((?P<k>[a-zA-Z]+)\)(?P<rest>.*)$").unwrap()
    });
    if let Some(c) = re.captures(line) {
        let m = marker.unwrap_or(&c["m"]);
        let k = kind.unwrap_or(&c["k"]);
        format!("{}{}({}){}", &c["ws"], m, k, &c["rest"])
    } else {
        line.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests — verify the file-first core without a display
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typed_todos_and_tokens() {
        let body = "\
- meeting notes\n\
TODO(do) draft agenda +[[Acme]] due:2026-06-10 jira:PROJ-12\n\
TODO(followup) check pricing @[[Jane]]\n\
DONE(reading) skim the rust book\n";
        let todos = parse_todos("N1", body);
        assert_eq!(todos.len(), 3);

        let do_t = &todos[0];
        assert_eq!(do_t.kind, "do");
        assert_eq!(do_t.project, "Acme");
        assert_eq!(do_t.due, "2026-06-10");
        assert_eq!(do_t.external, "jira:PROJ-12");
        assert_eq!(do_t.text, "draft agenda"); // tokens stripped
        assert!(!do_t.done);
        assert_eq!(do_t.id, "N1:1");

        assert_eq!(todos[1].kind, "followup");
        assert_eq!(todos[1].person, "Jane");

        assert!(todos[2].done);
        assert_eq!(todos[2].kind, "reading");
    }

    #[test]
    fn kind_detection() {
        // strong typst signals
        assert_eq!(detect_kind("#set page(width: 10cm)\n= Hi"), "typst");
        assert_eq!(detect_kind("#figure(image(\"a.png\"))"), "typst");
        // prose with dollars must NOT be mistaken for typst math
        assert_eq!(detect_kind("Budget is $5 to $10 for #urgent items"), "markdown");
        // plain markdown
        assert_eq!(detect_kind("# Heading\n- a bullet"), "markdown");
        // explicit declared kind wins over detection
        assert_eq!(effective_kind("markdown", "#set page()"), "markdown");
        assert_eq!(effective_kind("typst", "plain prose"), "typst");
        assert_eq!(effective_kind("auto", "#import \"x\""), "typst");
        assert_eq!(effective_kind("auto", "just notes"), "markdown");
    }

    #[test]
    fn markdown_blocks_structure() {
        let md = "# Title\n\nA para line\nsecond line.\n\n- one\n- two\n\n```\ncode here\n```\n> a quote\nTODO(do) ship it +[[X]]\n---\n";
        let b = markdown_blocks(md);
        let kinds: Vec<&str> = b.iter().map(|x| x.kind.as_str()).collect();
        assert_eq!(b[0].kind, "h1");
        assert_eq!(b[0].text, "Title");
        // the two text lines collapse into one paragraph
        assert!(b.iter().any(|x| x.kind == "para" && x.text.contains("second line")));
        assert_eq!(kinds.iter().filter(|k| **k == "bullet").count(), 2);
        assert!(b.iter().any(|x| x.kind == "code" && x.text == "code here"));
        assert!(b.iter().any(|x| x.kind == "quote"));
        let todo = b.iter().find(|x| x.kind == "todo").unwrap();
        assert!(todo.text.contains("ship it")); // glyph now drawn by the UI; tokens stripped
        assert!(b.iter().any(|x| x.kind == "rule"));
    }

    #[test]
    fn typst_fence_and_wikilink_cleanup() {
        let md = "See [[Acme Onboarding]] and +[[Roadmap]] with @[[Jane Doe]] at https://x.io\n\n```typst\n#set page()\n= Hi\n```\n";
        let b = markdown_blocks(md);
        // backend stores raw; cleaning + segmenting happen at render time
        let para = b.iter().find(|x| x.kind == "para").unwrap();
        assert!(clean_inline(&para.text).contains("Acme Onboarding"));
        assert!(!clean_inline(&para.text).contains("[["));
        let segs = line_segments(&para.text);
        assert!(segs.iter().any(|s| s.kind == "project" && s.value == "Acme Onboarding"));
        assert!(segs.iter().any(|s| s.kind == "person" && s.value == "Jane Doe"));
        assert!(segs.iter().any(|s| s.kind == "url" && s.value == "https://x.io"));
        // a ```typst fence becomes a typst block carrying its source
        let t = b.iter().find(|x| x.kind == "typst").unwrap();
        assert!(t.text.contains("#set page()"));
        assert!(t.text.contains("= Hi"));
    }

    #[test]
    fn extracts_wikilinks() {
        let links = parse_links("see [[Acme Onboarding]] and +[[Acme Onboarding]] and [[Roadmap]]");
        assert_eq!(links, vec!["Acme Onboarding", "Roadmap"]); // deduped + sorted
    }

    #[test]
    fn backend_roundtrip_and_toggle() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();

        // start clean (Backend::open doesn't seed)
        assert!(b.list_notes().unwrap().is_empty());

        let note = b.new_note().unwrap();
        b.save_note(
            &note.id,
            "Kickoff",
            "TODO(do) ship it +[[Acme]] due:2026-07-01\nlinked [[Roadmap]]\n",
        )
        .unwrap();

        assert_eq!(b.list_notes().unwrap().len(), 1);

        let projects = b.list_projects().unwrap();
        let names: Vec<_> = projects.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Acme"));
        assert!(names.contains(&"Roadmap"));

        let todos = b.list_todos("do").unwrap();
        assert_eq!(todos.len(), 1);
        assert!(!todos[0].done);
        let todo_id = todos[0].id.clone();

        // toggle marks done and rewrites the file
        b.toggle_todo(&todo_id).unwrap();
        let todos = b.list_todos("all").unwrap();
        assert!(todos[0].done);
        let on_disk = std::fs::read_to_string(&note.path).unwrap();
        assert!(on_disk.contains("DONE(do)"));
        assert!(!on_disk.contains("TODO(do)"));

        // reindex from files reproduces the same state (index is disposable)
        b.reindex_all().unwrap();
        assert!(b.list_todos("all").unwrap()[0].done);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn people_stale_and_project_filters() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let notes_dir = dir.join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();

        // A note last touched long ago, with a follow-up tied to Jane.
        std::fs::write(
            notes_dir.join("old.md"),
            "---\nid: OLD\ntitle: Old\nupdated: 2000-01-01T00:00:00\nkind: markdown\n---\n\
             TODO(followup) chase Jane on contract @[[Jane]] +[[Acme]]\n",
        )
        .unwrap();
        // A fresh note with a do-item tied to the same project.
        std::fs::write(
            notes_dir.join("new.md"),
            format!(
                "---\nid: NEW\ntitle: New\nupdated: {}\nkind: markdown\n---\nTODO(do) ship +[[Acme]]\n",
                Utc::now().format("%Y-%m-%dT%H:%M:%S")
            ),
        )
        .unwrap();

        let b = Backend::open(dir.clone()).unwrap();

        // person view: Jane has one open todo
        let people = b.list_people().unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].name, "Jane");

        let janes = b.list_todos("person:Jane").unwrap();
        assert_eq!(janes.len(), 1);
        assert!(janes[0].text.contains("chase Jane"));

        // stale view: only the old follow-up qualifies (fresh do-item excluded)
        let stale = b.list_todos("stale").unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].note_id, "OLD");

        // project view: Acme has both todos
        assert_eq!(b.list_todos("project:Acme").unwrap().len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn status_tags_board_and_moves() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let note = b.new_note().unwrap();
        b.save_note(
            &note.id,
            "Sprint",
            "Sprint planning #urgent #q3\n\
             TODO(do) build api +[[Platform]] start:2026-06-01 due:2026-06-10\n\
             DOING(do) write tests +[[Platform]]\n\
             TODO(followup) ask Sam @[[Sam]] #urgent\n",
        )
        .unwrap();

        // tags indexed
        let tags: Vec<_> = b.list_tags().unwrap().into_iter().map(|t| t.name).collect();
        assert!(tags.contains(&"urgent".to_string()));
        assert!(tags.contains(&"q3".to_string()));

        // status parsed (one DOING)
        let doing = b
            .query_todos(&Filter { status: "doing".into(), ..Default::default() })
            .unwrap();
        assert_eq!(doing.len(), 1);
        assert!(doing[0].text.contains("write tests"));

        // start date captured for the Gantt
        let g = b.gantt_items(&Filter::default()).unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].start, "2026-06-01");
        assert_eq!(g[0].due, "2026-06-10");

        // filter by tag intersects todos
        let urgent = b
            .query_todos(&Filter { tag: "urgent".into(), ..Default::default() })
            .unwrap();
        assert_eq!(urgent.len(), 3); // all todos live in the #urgent note

        // board grouped by status has 3 columns; build-api in "todo"
        let cols = b.board("status", &Filter::default()).unwrap();
        assert_eq!(cols.len(), 3);
        let todo_col = cols.iter().find(|(_, k, _)| k == "todo").unwrap();
        assert!(todo_col.2.iter().any(|t| t.text.contains("build api")));

        // move "build api" status right: todo -> doing
        let build = b
            .query_todos(&Filter { search: "build api".into(), ..Default::default() })
            .unwrap();
        let id = build[0].id.clone();
        b.board_move(&id, "status", 1).unwrap();
        assert_eq!(b.get_todo(&id).unwrap().status, "doing");
        let on_disk = std::fs::read_to_string(&note.path).unwrap();
        assert!(on_disk.contains("DOING(do) build api"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn add_update_and_drop_via_form() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let note = b.new_note().unwrap();

        // add a todo from structured fields (no hand-typed syntax)
        let id = b
            .add_todo(
                &note.id,
                &TodoFields {
                    kind: "followup".into(),
                    status: "todo".into(),
                    text: "ping vendor".into(),
                    person: "Dana".into(),
                    project: "Q3".into(),
                    due: "2026-07-01".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let t = b.get_todo(&id).unwrap();
        assert_eq!(t.kind, "followup");
        assert_eq!(t.person, "Dana");
        assert_eq!(t.project, "Q3");
        assert_eq!(t.due, "2026-07-01");
        let disk = std::fs::read_to_string(&note.path).unwrap();
        assert!(disk.contains("TODO(followup) ping vendor @[[Dana]] +[[Q3]] due:2026-07-01"));

        // edit it: change kind + status + add a start date
        let mut f = TodoFields::from_todo(&b.get_todo(&id).unwrap());
        f.kind = "do".into();
        f.status = "doing".into();
        f.start = "2026-06-25".into();
        b.update_todo(&id, &f).unwrap();
        let t = b.get_todo(&id).unwrap();
        assert_eq!(t.kind, "do");
        assert_eq!(t.status, "doing");
        assert_eq!(t.start, "2026-06-25");

        // drag onto a different project column (group_by = project)
        b.drop_card(&id, "project", "Platform").unwrap();
        assert_eq!(b.get_todo(&id).unwrap().project, "Platform");

        // drag onto a status column
        b.drop_card(&id, "status", "done").unwrap();
        assert!(b.get_todo(&id).unwrap().done);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn priority_repeat_cycle_recurrence() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let note = b.new_note().unwrap();
        b.save_note(
            &note.id,
            "x",
            "TODO(do) [#A] water plants +[[Home]] due:2026-06-10 repeat:1w\n",
        )
        .unwrap();
        let t = b.query_todos(&Filter::default()).unwrap()[0].clone();
        assert_eq!(t.priority, "A");
        assert_eq!(t.repeat, "1w");
        assert_eq!(t.text, "water plants"); // tokens stripped
        let id = t.id.clone();

        // todo -> doing
        b.cycle_todo(&id).unwrap();
        assert_eq!(b.get_todo(&id).unwrap().status, "doing");

        // doing -> (would be done, but recurs) advances due by 1w, stays todo
        b.cycle_todo(&id).unwrap();
        let t2 = b.get_todo(&id).unwrap();
        assert_eq!(t2.status, "todo");
        assert_eq!(t2.due, "2026-06-17");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn full_text_search() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let a = b.new_note().unwrap();
        b.save_note(&a.id, "Quarterly review", "We discussed the budget and pipeline.\n").unwrap();

        let hit = b.query_notes(&Filter { search: "budget".into(), ..Default::default() }).unwrap();
        assert_eq!(hit.len(), 1);
        let prefix = b.query_notes(&Filter { search: "pipe".into(), ..Default::default() }).unwrap();
        assert_eq!(prefix.len(), 1);
        let miss = b.query_notes(&Filter { search: "zzznope".into(), ..Default::default() }).unwrap();
        assert_eq!(miss.len(), 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hierarchical_subtree_filter() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let a = b.new_note().unwrap();
        b.save_note(&a.id, "x", "[[Clients/Acme]]\n").unwrap();
        let c = b.new_note().unwrap();
        b.save_note(&c.id, "y", "[[Clients/Beta]]\n").unwrap();

        // parent shows the whole subtree
        let parent = b
            .query_notes(&Filter { project: "Clients".into(), ..Default::default() })
            .unwrap();
        assert_eq!(parent.len(), 2);
        // a leaf shows only itself
        let leaf = b
            .query_notes(&Filter { project: "Clients/Acme".into(), ..Default::default() })
            .unwrap();
        assert_eq!(leaf.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn related_notes_and_filing() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let a = b.new_note().unwrap();
        b.save_note(&a.id, "Acme kickoff", "minutes [[Client Acme]]\n").unwrap();

        // a related note inherits the topic and back-links to the source
        let r = b.new_related_note(&a.id).unwrap();
        let rn = b.load_note(&r.id).unwrap();
        assert!(rn.body.contains("[[Client Acme]]"));
        assert!(rn.body.contains("[[Acme kickoff]]"));
        // both notes are now in the cluster
        let cluster = b
            .query_notes(&Filter { project: "Client Acme".into(), ..Default::default() })
            .unwrap();
        assert_eq!(cluster.len(), 2);

        // filing an unfiled note adds it to the cluster
        let c = b.new_note().unwrap();
        b.save_note(&c.id, "loose", "idea\n").unwrap();
        b.add_link(&c.id, "Client Acme").unwrap();
        let cluster2 = b
            .query_notes(&Filter { project: "Client Acme".into(), ..Default::default() })
            .unwrap();
        assert_eq!(cluster2.len(), 3);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn inbox_backlinks_and_archive() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        // an unfiled note (no links) -> inbox; a filed one -> not inbox
        let a = b.new_note().unwrap();
        b.save_note(&a.id, "Loose thought", "just an idea\n").unwrap();
        let c = b.new_note().unwrap();
        b.save_note(&c.id, "Filed", "work on [[Project X]]\n").unwrap();

        let inbox: Vec<_> = b.inbox().unwrap().into_iter().map(|n| n.id).collect();
        assert!(inbox.contains(&a.id));
        assert!(!inbox.contains(&c.id));

        // backlinks: who links to "Project X"
        let backs = b.backlinks("Project X").unwrap();
        assert_eq!(backs.len(), 1);
        assert_eq!(backs[0].id, c.id);

        // archive removes the note from default queries
        b.archive_note(&a.id, true).unwrap();
        let visible: Vec<_> = b
            .query_notes(&Filter::default())
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        assert!(!visible.contains(&a.id));
        // and shows again with show_archived
        let with_arch: Vec<_> = b
            .query_notes(&Filter { show_archived: true, ..Default::default() })
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        assert!(with_arch.contains(&a.id));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mentions_make_people_and_add_tag() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let note = b.new_note().unwrap();
        // Mentions in plain prose (not a todo) still create people — both the
        // bare `@bob` form and the bracketed `@[[Two Words]]` form.
        b.save_note(&note.id, "1:1", "Spoke with @bob and @[[Priya Patel]] about the plan.\n")
            .unwrap();

        let people: Vec<_> = b.list_people().unwrap().into_iter().map(|p| p.name).collect();
        assert!(people.contains(&"bob".to_string()));
        assert!(people.contains(&"Priya Patel".to_string())); // spaces survive

        // Filtering notes by either person finds this note.
        for who in ["bob", "Priya Patel"] {
            let notes = b
                .query_notes(&Filter { person: who.into(), ..Default::default() })
                .unwrap();
            assert_eq!(notes.len(), 1, "person {who}");
            assert_eq!(notes[0].id, note.id);
        }

        // add_tag appends a label and is idempotent.
        b.add_tag(&note.id, "#followup-soon").unwrap();
        b.add_tag(&note.id, "followup-soon").unwrap(); // dup ignored
        let tags: Vec<_> = b.list_tags().unwrap().into_iter().map(|t| t.name).collect();
        assert_eq!(tags.iter().filter(|t| *t == "followup-soon").count(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn safe_filename_sanitizes_and_falls_back() {
        assert_eq!(safe_filename("Hello World", "id1"), "Hello World");
        assert_eq!(safe_filename("a/b:c*d?", "id1"), "a_b_c_d_");
        // empty/whitespace-only title falls back to the id
        assert_eq!(safe_filename("", "id1"), "id1");
        assert_eq!(safe_filename("   ", "id1"), "id1");
        // long titles are truncated to 60 chars
        let long = "x".repeat(100);
        assert_eq!(safe_filename(&long, "id1").chars().count(), 60);
    }

    #[test]
    fn typst_escape_covers_markup_chars() {
        // each special char gets a leading backslash
        assert_eq!(typst_escape("a#b"), r"a\#b");
        assert_eq!(typst_escape("[#A]"), r"\[\#A\]");
        assert_eq!(typst_escape("@x +[[P]]"), r"\@x +\[\[P\]\]");
        assert_eq!(typst_escape("3 < 4 > 2 = 1"), r"3 \< 4 \> 2 \= 1");
        assert_eq!(typst_escape("*b* _i_ `c` $x$ ~ \\"), r"\*b\* \_i\_ \`c\` \$x\$ \~ \\");
        // ordinary text is untouched
        assert_eq!(typst_escape("plain text 123"), "plain text 123");
    }

    #[test]
    fn markdown_to_typst_converts_headings_and_escapes() {
        let md = "# Title\n## Sub\n### Deep\n- bullet item\nplain @[[Jane]] [#A] line\n";
        let typ = markdown_to_typst("Doc", md);
        assert!(typ.contains("#set page"));
        assert!(typ.contains("= Doc")); // injected document title
        assert!(typ.contains("= Title")); // h1 -> =
        assert!(typ.contains("== Sub")); // h2 -> ==
        assert!(typ.contains("=== Deep")); // h3 -> ===
        assert!(typ.contains("- bullet item")); // bullets preserved
        // tokens are escaped so they render literally and compile
        assert!(typ.contains(r"\@\[\[Jane\]\] \[\#A\]"));
    }

    #[test]
    fn export_note_markdown_copies_file() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        let note = b.new_note().unwrap();
        b.save_note(&note.id, "My Note", "body line one\n").unwrap();

        let out = b.export_note(&note.id, "md").unwrap();
        assert!(out.exists());
        assert_eq!(out.extension().unwrap(), "md");
        assert!(out.starts_with(dir.join("exports")));
        let exported = std::fs::read_to_string(&out).unwrap();
        assert!(exported.contains("body line one"));

        // unknown format is rejected
        assert!(b.export_note(&note.id, "docx").is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn open_lazy_skips_indexing_until_reindex() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        // seed a note file directly on disk
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        std::fs::write(
            dir.join("notes").join("n1.md"),
            "---\nid: n1\ntitle: Seed\n---\n# Seed\nbody\n",
        )
        .unwrap();

        let mut b = Backend::open_lazy(dir.clone()).unwrap();
        // lazy open does NOT index, but the file IS on disk
        assert!(b.list_notes().unwrap().is_empty());
        assert!(!b.is_vault_empty());

        // an explicit reindex picks the file up
        b.reindex_all().unwrap();
        assert_eq!(b.list_notes().unwrap().len(), 1);

        // background_reindex (separate connection) also reflects on a fresh open
        let (vault, fts) = b.reindex_params();
        background_reindex(&vault, fts).unwrap();

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn is_vault_empty_reflects_disk() {
        let dir = std::env::temp_dir().join(format!("noet-test-{}", ulid::Ulid::new()));
        let mut b = Backend::open(dir.clone()).unwrap();
        assert!(b.is_vault_empty());
        let note = b.new_note().unwrap();
        b.save_note(&note.id, "x", "y").unwrap();
        assert!(!b.is_vault_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
