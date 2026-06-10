//! Parsing — the file-first grammar. Notes are plain markdown; todos are lines
//! like `TODO(do) text @[[Person]] +[[Project]] due:YYYY-MM-DD #tag`. This module
//! also formats structured fields back into that canonical line syntax.

use super::{MdBlock, Segment, Todo, TodoFields};
use regex::Regex;
use std::sync::OnceLock;

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
            segs.push(Segment {
                text: strip_inline(s),
                kind: String::new(),
                value: String::new(),
            });
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
            let url = s
                .rsplit('(')
                .next()
                .unwrap_or("")
                .trim_end_matches(')')
                .to_string();
            Segment {
                text: txt,
                kind: "url".into(),
                value: url,
            }
        } else if let Some(g) = caps.name("url") {
            Segment {
                text: g.as_str().into(),
                kind: "url".into(),
                value: g.as_str().into(),
            }
        } else if let Some(g) = caps.name("proj") {
            let name = g
                .as_str()
                .trim_start_matches('+')
                .trim_start_matches("[[")
                .trim_end_matches("]]")
                .to_string();
            Segment {
                text: name.clone(),
                kind: "project".into(),
                value: name,
            }
        } else if let Some(g) = caps.name("pers") {
            let inner = g.as_str().trim_start_matches('@');
            let name = inner
                .trim_start_matches("[[")
                .trim_end_matches("]]")
                .to_string();
            Segment {
                text: format!("@{name}"),
                kind: "person".into(),
                value: name,
            }
        } else if let Some(g) = caps.name("tag") {
            let name = g.as_str().trim_start_matches('#').to_string();
            Segment {
                text: format!("#{name}"),
                kind: "tag".into(),
                value: name,
            }
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
                out.push(MdBlock {
                    kind: kind.into(),
                    text: code.trim_end().into(),
                    indent: 0,
                });
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
            out.push(MdBlock {
                kind: "rule".into(),
                text: String::new(),
                indent: 0,
            });
            continue;
        }
        // todo line -> an interactive checkbox block (id/done resolved by the UI layer)
        if let Some(c) = todo_re().captures(t) {
            flush_para(&mut para, &mut out);
            let todos = parse_todos("", t);
            let label = todos
                .first()
                .map(|x| x.text.clone())
                .unwrap_or_else(|| c["rest"].to_string());
            out.push(MdBlock {
                kind: "todo".into(),
                text: label,
                indent,
            });
            continue;
        }
        // headings
        if let Some(rest) = t.strip_prefix("### ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock {
                kind: "h3".into(),
                text: rest.to_string(),
                indent: 0,
            });
            continue;
        }
        if let Some(rest) = t.strip_prefix("## ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock {
                kind: "h2".into(),
                text: rest.to_string(),
                indent: 0,
            });
            continue;
        }
        if let Some(rest) = t.strip_prefix("# ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock {
                kind: "h1".into(),
                text: rest.to_string(),
                indent: 0,
            });
            continue;
        }
        // blockquote
        if let Some(rest) = t.strip_prefix("> ") {
            flush_para(&mut para, &mut out);
            out.push(MdBlock {
                kind: "quote".into(),
                text: rest.to_string(),
                indent,
            });
            continue;
        }
        // bullet
        if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush_para(&mut para, &mut out);
            out.push(MdBlock {
                kind: "bullet".into(),
                text: format!("•  {rest}"),
                indent,
            });
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
        out.push(MdBlock {
            kind: "code".into(),
            text: code.trim_end().into(),
            indent: 0,
        });
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
                re.captures(&rest)
                    .map(|m| m[key].to_string())
                    .unwrap_or_default()
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

/// Render structured fields back into a canonical todo line.
pub(crate) fn format_todo_line(f: &TodoFields) -> String {
    let marker = match f.status.as_str() {
        "doing" => "DOING",
        "done" => "DONE",
        _ => "TODO",
    };
    let kind = if f.kind.is_empty() {
        "do"
    } else {
        f.kind.as_str()
    };
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
pub(crate) fn advance_date(date: &str, repeat: &str) -> String {
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

/// Replace the marker and/or kind inside a `MARKER(kind)` todo line, preserving
/// leading whitespace and the rest of the line verbatim.
pub(crate) fn set_marker_kind(line: &str, marker: Option<&str>, kind: Option<&str>) -> String {
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
