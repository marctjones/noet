//! Parsing — the file-first grammar. Notes are plain markdown; tasks are
//! GitHub-style task list items plus Noet labels, people, links, and properties.

use super::{MdBlock, Segment, Todo, TodoFields};
use regex::Regex;
use std::sync::OnceLock;

// A task line: - [ ] text @[[Person]] [[Project]] #followup due:2026-06-10
// Marker is [ ] / [/] / [x] -> status.
fn todo_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?P<ws>\s*)(?:[-*+]|\d+[.)])\s+\[(?P<marker>[ xX/])\]\s+(?P<rest>.*)$")
            .unwrap()
    })
}

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[(?P<t>[^\]]+)\]\]").unwrap())
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)#(?P<t>[A-Za-z][A-Za-z0-9_/-]*)").unwrap())
}

fn workflow_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:^|\s)#(?P<t>followup|delegated|mine|someday|waiting|reading|do)\b").unwrap()
    })
}

fn marker_to_status(marker: &str) -> String {
    match marker {
        "/" => "doing",
        "x" | "X" => "done",
        _ => "todo",
    }
    .to_string()
}

fn status_to_marker(status: &str) -> &'static str {
    match status {
        "doing" => "/",
        "done" => "x",
        _ => " ",
    }
}

fn text_lines(body: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut in_code = false;
    body.lines().enumerate().filter(move |(_, line)| {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            return false;
        }
        !in_code
    })
}

/// `#tag` labels anywhere in a note body.
pub fn parse_tags(body: &str) -> Vec<String> {
    let mut v: Vec<String> = text_lines(body)
        .flat_map(|(_, line)| tag_re().captures_iter(line))
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
    let mut v: Vec<String> = text_lines(body)
        .flat_map(|(_, line)| person_re().captures_iter(line))
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
    // `[[X]]`, `@[[X]]` -> X so prose reads cleanly (entities show as
    // clickable chips elsewhere). Bare @name / #tag are already readable.
    let wiki = WIKI.get_or_init(|| Regex::new(r"@?\[\[([^\]]+)\]\]").unwrap());
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
            r"(?P<mdlink>\[[^\]]+\]\([^)]*\))|(?P<url>https?://[^\s)]+)|(?P<proj>\[\[[^\]]+\]\])|(?P<pers>@(?:\[\[[^\]]+\]\]|[A-Za-z][A-Za-z0-9_.\-]*))|(?P<tag>#[A-Za-z][A-Za-z0-9_/-]*)",
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

fn property_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(?P<key>[A-Za-z][A-Za-z0-9_-]*):(?P<val>[^\s]+)").unwrap())
}

pub fn parse_properties(text: &str) -> Vec<(String, String)> {
    property_re()
        .captures_iter(text)
        .map(|m| (m["key"].to_string(), m["val"].to_string()))
        .collect()
}

fn first_wikilink(rest: &str) -> String {
    link_re()
        .captures_iter(rest)
        .find(|c| {
            c.get(0).is_some_and(|m| {
                let before = &rest[..m.start()];
                !before.ends_with('@') && !before.ends_with('+')
            })
        })
        .map(|c| c["t"].trim().to_string())
        .unwrap_or_default()
}

fn task_kind(labels: &[String]) -> String {
    for label in labels {
        match label.as_str() {
            "followup" | "delegated" | "mine" | "someday" | "waiting" | "reading" | "do" => {
                return label.clone()
            }
            _ => {}
        }
    }
    "do".into()
}

/// Extract todos from a note body. line_no is 0-based for stable ids.
pub fn parse_todos(note_id: &str, body: &str) -> Vec<Todo> {
    let mut out = Vec::new();
    for (line_no, line) in text_lines(body) {
        if let Some(c) = todo_re().captures(line) {
            let status = marker_to_status(&c["marker"]);
            let rest = c["rest"].to_string();
            let labels: Vec<String> = tag_re()
                .captures_iter(&rest)
                .map(|m| m["t"].to_string())
                .collect();
            let kind = task_kind(&labels);
            let project = first_wikilink(&rest);
            let person = person_re()
                .captures(&rest)
                .map(|m| person_name(&m))
                .unwrap_or_default();
            let prop = |key: &str| {
                property_re()
                    .captures_iter(&rest)
                    .find(|m| &m["key"] == key)
                    .map(|m| m["val"].to_string())
                    .unwrap_or_default()
            };
            let due = prop("due");
            let start = prop("start");
            let priority = prop("priority");
            let repeat = prop("repeat");
            let external = property_re()
                .captures_iter(&rest)
                .find(|m| matches!(&m["key"], "gh" | "ref"))
                .map(|m| format!("{}:{}", &m["key"], &m["val"]))
                .unwrap_or_default();

            // Clean display text: drop the tokens we lifted into fields.
            let mut text = rest.clone();
            text = person_re().replace_all(&text, " ").to_string();
            text = link_re().replace_all(&text, "").to_string();
            text = tag_re().replace_all(&text, " ").to_string();
            text = property_re().replace_all(&text, " ").to_string();
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
    let mut v: Vec<String> = text_lines(body)
        .flat_map(|(_, line)| {
            link_re().captures_iter(line).filter(move |c| {
                c.get(0).is_some_and(|m| {
                    let before = &line[..m.start()];
                    !before.ends_with('@') && !before.ends_with('+')
                })
            })
        })
        .map(|c| c["t"].trim().to_string())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Render structured fields back into a canonical todo line.
pub(crate) fn format_todo_line(f: &TodoFields) -> String {
    let kind = if f.kind.is_empty() {
        "do"
    } else {
        f.kind.as_str()
    };
    let mut s = format!("- [{}] {}", status_to_marker(&f.status), f.text.trim());
    if !f.person.is_empty() {
        s += &format!(" @[[{}]]", f.person.trim());
    }
    if !f.project.is_empty() {
        s += &format!(" [[{}]]", f.project.trim());
    }
    if !kind.is_empty() && kind != "do" {
        s += &format!(" #{}", kind);
    }
    if !f.priority.is_empty() {
        s += &format!(" priority:{}", f.priority.trim());
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

/// Replace the marker and/or workflow label inside a task line, preserving
/// leading whitespace and the rest of the line.
pub(crate) fn set_marker_kind(line: &str, marker: Option<&str>, kind: Option<&str>) -> String {
    if let Some(c) = todo_re().captures(line) {
        let ws = &c["ws"];
        let m = marker.unwrap_or(&c["marker"]);
        let mut rest = c["rest"].to_string();
        if let Some(k) = kind {
            rest = workflow_tag_re().replace_all(&rest, " ").to_string();
            if k != "do" && !k.is_empty() {
                rest = format!("{} #{}", rest.trim(), k);
            }
        }
        format!("{ws}- [{m}] {}", rest.trim())
    } else {
        line.to_string()
    }
}
