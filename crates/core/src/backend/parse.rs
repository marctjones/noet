//! Parsing — the file-first grammar. Notes are plain markdown; tasks are
//! GitHub-style task list items plus Noet labels, people, links, and properties.

use super::{MdBlock, Segment, SourceSpan, Todo, TodoFields};
use chrono::NaiveDate;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub code: String,
    pub message: String,
    pub severity: ParseSeverity,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactKind {
    Url,
    Email,
    Social,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactFact {
    pub kind: ContactKind,
    pub value: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineEntityKind {
    MarkdownLink,
    Url,
    Email,
    Social,
    Project,
    Person,
    Tag,
}

impl InlineEntityKind {
    pub fn segment_kind(self) -> &'static str {
        match self {
            InlineEntityKind::MarkdownLink | InlineEntityKind::Url => "url",
            InlineEntityKind::Email => "email",
            InlineEntityKind::Social => "social",
            InlineEntityKind::Project => "project",
            InlineEntityKind::Person => "person",
            InlineEntityKind::Tag => "tag",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineEntity {
    pub kind: InlineEntityKind,
    pub text: String,
    pub value: String,
    pub span: SourceSpan,
    pub char_start: usize,
    pub char_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLink {
    pub target: String,
    pub title: String,
    pub anchor: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct ParsedTodoLine {
    pub todo: Todo,
    pub raw_line: String,
    pub labels: Vec<String>,
    pub people: Vec<String>,
    pub workstreams: Vec<String>,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedMarkdown {
    pub labels: Vec<String>,
    pub people: Vec<String>,
    pub workstreams: Vec<String>,
    pub properties: Vec<(String, String)>,
    pub todos: Vec<ParsedTodoLine>,
    pub source_links: Vec<SourceLink>,
    pub contacts: Vec<ContactFact>,
    pub diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Clone, Copy)]
struct TextLine<'a> {
    line_no: usize,
    line: &'a str,
    byte_start: usize,
    byte_end: usize,
}

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

fn text_line_spans(body: &str) -> Vec<TextLine<'_>> {
    let mut out = Vec::new();
    let mut in_code = false;
    let mut byte_start = 0usize;

    for (line_no, raw) in body.split_inclusive('\n').enumerate() {
        let line = raw.trim_end_matches(['\r', '\n']);
        let byte_end = byte_start + line.len();
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            byte_start += raw.len();
            continue;
        }
        if !in_code {
            out.push(TextLine {
                line_no,
                line,
                byte_start,
                byte_end,
            });
        }
        byte_start += raw.len();
    }

    out
}

fn sorted_dedup(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn char_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset].chars().count()
}

fn trim_entity_end(text: &str, mut end: usize) -> usize {
    while end > 0 && text[..end].ends_with(['.', ',']) {
        end -= 1;
    }
    end
}

fn mk_inline_entity(
    source: TextLine<'_>,
    kind: InlineEntityKind,
    text: String,
    value: String,
    byte_start: usize,
    byte_end: usize,
) -> InlineEntity {
    InlineEntity {
        kind,
        text,
        value,
        span: SourceSpan {
            line_no: source.line_no,
            byte_start: source.byte_start + byte_start,
            byte_end: source.byte_start + byte_end,
        },
        char_start: char_offset(source.line, byte_start),
        char_end: char_offset(source.line, byte_end),
    }
}

fn line_inline_entities(source: TextLine<'_>) -> Vec<InlineEntity> {
    let mut entities = Vec::new();

    for caps in inline_entity_re().captures_iter(source.line) {
        let Some(full) = caps.get(0) else {
            continue;
        };

        let entity = if let Some(m) = caps.name("mdlink") {
            let raw = m.as_str();
            let label = raw[1..].split(']').next().unwrap_or("").to_string();
            let url = raw
                .rsplit('(')
                .next()
                .unwrap_or("")
                .trim_end_matches(')')
                .to_string();
            Some(mk_inline_entity(
                source,
                InlineEntityKind::MarkdownLink,
                label,
                url,
                m.start(),
                m.end(),
            ))
        } else if let Some(m) = caps.name("url") {
            let byte_end = trim_entity_end(source.line, m.end());
            let value = source.line[m.start()..byte_end].to_string();
            Some(mk_inline_entity(
                source,
                InlineEntityKind::Url,
                value.clone(),
                value,
                m.start(),
                byte_end,
            ))
        } else if let Some(m) = caps.name("email") {
            let value = m.as_str().to_string();
            Some(mk_inline_entity(
                source,
                InlineEntityKind::Email,
                value.clone(),
                value,
                m.start(),
                m.end(),
            ))
        } else if let Some(m) = caps.name("social") {
            if source.line[m.end()..].starts_with('@') {
                None
            } else {
                let value = m.as_str().to_string();
                Some(mk_inline_entity(
                    source,
                    InlineEntityKind::Social,
                    value.clone(),
                    value,
                    m.start(),
                    m.end(),
                ))
            }
        } else if let Some(m) = caps.name("person") {
            let inner = m.as_str().trim_start_matches('@');
            let name = inner
                .trim_start_matches("[[")
                .trim_end_matches("]]")
                .to_string();
            Some(mk_inline_entity(
                source,
                InlineEntityKind::Person,
                format!("@{name}"),
                name,
                m.start(),
                m.end(),
            ))
        } else if let Some(m) = caps.name("project") {
            let before = &source.line[..m.start()];
            if before.ends_with('+') || before.ends_with("source:") {
                None
            } else {
                let name = m
                    .as_str()
                    .trim_start_matches("[[")
                    .trim_end_matches("]]")
                    .to_string();
                Some(mk_inline_entity(
                    source,
                    InlineEntityKind::Project,
                    name.clone(),
                    name,
                    m.start(),
                    m.end(),
                ))
            }
        } else if let Some(m) = caps.name("tag") {
            let name = m.as_str().trim_start_matches('#').to_string();
            Some(mk_inline_entity(
                source,
                InlineEntityKind::Tag,
                format!("#{name}"),
                name,
                m.start(),
                m.end(),
            ))
        } else {
            None
        };

        if let Some(entity) = entity {
            debug_assert!(entity.span.byte_start >= source.byte_start + full.start());
            entities.push(entity);
        }
    }

    entities
}

pub fn parse_inline_entities(raw: &str) -> Vec<InlineEntity> {
    line_inline_entities(TextLine {
        line_no: 0,
        line: raw,
        byte_start: 0,
        byte_end: raw.len(),
    })
}

fn entity_tags(text: &str) -> Vec<String> {
    parse_inline_entities(text)
        .into_iter()
        .filter(|entity| entity.kind == InlineEntityKind::Tag)
        .map(|entity| entity.value)
        .collect()
}

fn entity_mentions(text: &str) -> Vec<String> {
    parse_inline_entities(text)
        .into_iter()
        .filter(|entity| entity.kind == InlineEntityKind::Person)
        .map(|entity| entity.value)
        .collect()
}

fn entity_links(text: &str) -> Vec<String> {
    parse_inline_entities(text)
        .into_iter()
        .filter(|entity| entity.kind == InlineEntityKind::Project)
        .map(|entity| entity.value)
        .collect()
}

fn entity_properties(text: &str) -> Vec<(String, String)> {
    property_re()
        .captures_iter(text)
        .map(|m| (m["key"].to_string(), m["val"].to_string()))
        .filter(|(key, _)| !ignored_property_key(key))
        .collect()
}

/// `#tag` labels anywhere in a note body.
pub fn parse_tags(body: &str) -> Vec<String> {
    sorted_dedup(
        text_lines(body)
            .flat_map(|(_, line)| entity_tags(line))
            .collect(),
    )
}

// A canonical person mention is bracketed `@[[Jane Smith]]` so it cannot be
// confused with email addresses or social handles. Bare `@name` tokens are
// parsed as social/contact facts and emit ambiguity diagnostics.
fn person_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)@\[\[(?P<pb>[^\]]+)\]\]").unwrap())
}

fn bare_mention_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)(?P<h>@[A-Za-z][A-Za-z0-9_.\-]*)(?:\b|$)").unwrap())
}

fn inline_entity_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?P<mdlink>\[[^\]\n]+\]\([^\)\n]*\))|(?P<url>https?://[^\s<>\]\)]+)|(?P<person>@\[\[[^\]\n]+\]\])|(?P<project>\[\[[^\]\n]+\]\])|(?P<email>\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b)|(?:^|\s)(?P<social>@[A-Za-z][A-Za-z0-9_.\-]*(?:@[A-Za-z0-9.-]+\.[A-Za-z]{2,})?)\b|(?:^|\s)(?P<tag>#[A-Za-z][A-Za-z0-9_/-]*)",
        )
        .unwrap()
    })
}

fn person_name(c: &regex::Captures) -> String {
    c.name("pb")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

/// Every canonical `@[[Person]]` mention anywhere in a note (not just todos).
pub fn parse_mentions(body: &str) -> Vec<String> {
    sorted_dedup(
        text_lines(body)
            .flat_map(|(_, line)| entity_mentions(line))
            .collect(),
    )
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

    for entity in parse_inline_entities(raw) {
        if entity.span.byte_start > last {
            push_plain(&mut segs, &raw[last..entity.span.byte_start]);
        }
        if entity.span.byte_end <= entity.span.byte_start {
            continue;
        }
        segs.push(Segment {
            text: entity.text,
            kind: entity.kind.segment_kind().into(),
            value: entity.value,
        });
        last = entity.span.byte_end;
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

fn old_task_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*(TODO|DOING|DONE)\(").unwrap())
}

fn old_workstream_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)\+\[\[").unwrap())
}

fn block_anchor_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?:^|\s)\^(?P<a>[A-Za-z0-9_-]+)(?:\s|$)").unwrap())
}

pub fn parse_properties(text: &str) -> Vec<(String, String)> {
    entity_properties(text)
}

fn ignored_property_key(key: &str) -> bool {
    matches!(key, "http" | "https" | "mailto" | "source")
}

fn line_anchor(text: &str) -> String {
    block_anchor_re()
        .captures_iter(text)
        .filter_map(|m| m.name("a").map(|a| a.as_str().to_string()))
        .last()
        .unwrap_or_default()
}

fn first_wikilink(rest: &str) -> String {
    link_re()
        .captures_iter(rest)
        .find(|c| {
            c.get(0).is_some_and(|m| {
                let before = &rest[..m.start()];
                !before.ends_with('@') && !before.ends_with('+') && !before.ends_with("source:")
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

fn parse_todo_text_line(note_id: &str, source: TextLine<'_>) -> Option<ParsedTodoLine> {
    let line = source.line;
    if let Some(c) = todo_re().captures(line) {
        let status = marker_to_status(&c["marker"]);
        let rest = c["rest"].to_string();
        let labels = entity_tags(&rest);
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
        let anchor = line_anchor(&rest);
        let span = SourceSpan {
            line_no: source.line_no,
            byte_start: source.byte_start,
            byte_end: source.byte_end,
        };

        // Clean display text: drop the tokens we lifted into fields.
        let mut text = rest.clone();
        text = person_re().replace_all(&text, " ").to_string();
        text = link_re().replace_all(&text, "").to_string();
        text = tag_re().replace_all(&text, " ").to_string();
        text = property_re().replace_all(&text, " ").to_string();
        text = block_anchor_re().replace_all(&text, " ").to_string();
        let mut text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            let mut fallback = rest.clone();
            fallback = person_re().replace_all(&fallback, " ").to_string();
            fallback = tag_re().replace_all(&fallback, " ").to_string();
            fallback = property_re().replace_all(&fallback, " ").to_string();
            fallback = block_anchor_re().replace_all(&fallback, " ").to_string();
            text = clean_inline(&fallback)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
        }

        let id = if anchor.is_empty() {
            format!("{}:{}", note_id, source.line_no)
        } else {
            format!("{note_id}:^{anchor}")
        };
        Some(ParsedTodoLine {
            todo: Todo {
                id,
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
                line_no: source.line_no,
                anchor: anchor.clone(),
                span,
            },
            raw_line: line.to_string(),
            labels,
            people: entity_mentions(&rest),
            workstreams: entity_links(&rest),
            properties: entity_properties(&rest),
        })
    } else {
        None
    }
}

/// Extract todos from a note body. line_no is 0-based for unanchored ids;
/// anchored task lines use the stable id form `<note_id>:^<anchor>`.
pub fn parse_todo_lines(note_id: &str, body: &str) -> Vec<ParsedTodoLine> {
    text_line_spans(body)
        .into_iter()
        .filter_map(|line| parse_todo_text_line(note_id, line))
        .collect()
}

/// Extract todos from a note body. Prefer `parse_todo_lines` when source spans
/// or task-local entities are needed.
pub fn parse_todos(note_id: &str, body: &str) -> Vec<Todo> {
    parse_todo_lines(note_id, body)
        .into_iter()
        .map(|line| line.todo)
        .collect()
}

/// All `[[wikilink]]` targets in a body (projects / workstreams / pages).
pub fn parse_links(body: &str) -> Vec<String> {
    sorted_dedup(
        text_lines(body)
            .flat_map(|(_, line)| entity_links(line))
            .collect(),
    )
}

pub fn parse_source_links(body: &str) -> Vec<SourceLink> {
    let mut links = Vec::new();
    for line in text_line_spans(body) {
        let trimmed = line.line.trim_start();
        let leading = line.line.len() - trimmed.len();
        let Some(rest) = trimmed.strip_prefix("source:[[") else {
            continue;
        };
        let Some(target) = rest.split("]]").next().map(str::trim) else {
            continue;
        };
        if target.is_empty() {
            continue;
        }
        let (title, anchor) = match target.split_once("#^") {
            Some((title, anchor)) => (title.trim().to_string(), anchor.trim().to_string()),
            None => (target.trim().to_string(), String::new()),
        };
        let byte_start = line.byte_start + leading;
        links.push(SourceLink {
            target: target.to_string(),
            title,
            anchor,
            span: SourceSpan {
                line_no: line.line_no,
                byte_start,
                byte_end: line.byte_end,
            },
        });
    }
    links
}

fn span_for_range(line: TextLine<'_>, start: usize, end: usize) -> SourceSpan {
    SourceSpan {
        line_no: line.line_no,
        byte_start: line.byte_start + start,
        byte_end: line.byte_start + end,
    }
}

fn span_for_match(line: TextLine<'_>, m: regex::Match<'_>) -> SourceSpan {
    span_for_range(line, m.start(), m.end())
}

fn warning(code: &str, message: impl Into<String>, span: SourceSpan) -> ParseDiagnostic {
    ParseDiagnostic {
        code: code.to_string(),
        message: message.into(),
        severity: ParseSeverity::Warning,
        span,
    }
}

fn valid_repeat(value: &str) -> bool {
    if value.len() < 2 {
        return false;
    }
    let (number, unit) = value.split_at(value.len() - 1);
    !number.is_empty()
        && number.chars().all(|ch| ch.is_ascii_digit())
        && matches!(unit, "d" | "w" | "m")
}

fn invalid_property_message(key: &str, value: &str) -> Option<String> {
    match key {
        "due" | "start" | "date" => {
            if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok() {
                None
            } else {
                Some(format!("{key} must use YYYY-MM-DD"))
            }
        }
        "priority" => {
            if matches!(value, "A" | "B" | "C") {
                None
            } else {
                Some("priority must be A, B, or C".to_string())
            }
        }
        "repeat" => {
            if valid_repeat(value) {
                None
            } else {
                Some("repeat must use a number followed by d, w, or m".to_string())
            }
        }
        _ => None,
    }
}

fn line_diagnostics(line: TextLine<'_>) -> Vec<ParseDiagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(m) = old_task_re().find(line.line) {
        diagnostics.push(warning(
            "unsupported-old-task-syntax",
            "Use GitHub-style task list syntax such as - [ ] task",
            span_for_match(line, m),
        ));
    }

    if let Some(m) = old_workstream_re().find(line.line) {
        diagnostics.push(warning(
            "unsupported-old-workstream-syntax",
            "Use [[Workstream]] instead of +[[Workstream]]",
            span_for_match(line, m),
        ));
    }

    for caps in bare_mention_re().captures_iter(line.line) {
        let Some(m) = caps.name("h") else {
            continue;
        };
        if line.line[m.end()..].starts_with('@') {
            continue;
        }
        diagnostics.push(warning(
            "ambiguous-person",
            "Bare @name can be a person shorthand or social handle; use @[[Name]] for people",
            span_for_match(line, m),
        ));
    }

    for caps in property_re().captures_iter(line.line) {
        let key = &caps["key"];
        if ignored_property_key(key) {
            continue;
        }
        let value = &caps["val"];
        if let Some(message) = invalid_property_message(key, value) {
            if let Some(m) = caps.get(0) {
                diagnostics.push(warning(
                    "invalid-property",
                    message,
                    span_for_match(line, m),
                ));
            }
        }
    }

    diagnostics
}

fn duplicate_anchor_diagnostics(todos: &[ParsedTodoLine]) -> Vec<ParseDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut first_seen: HashMap<&str, SourceSpan> = HashMap::new();

    for todo in todos {
        if todo.todo.anchor.is_empty() {
            continue;
        }
        if first_seen
            .insert(todo.todo.anchor.as_str(), todo.todo.span)
            .is_some()
        {
            diagnostics.push(warning(
                "duplicate-anchor",
                format!(
                    "Block anchor ^{} is used by more than one task",
                    todo.todo.anchor
                ),
                todo.todo.span,
            ));
        }
    }

    diagnostics
}

pub fn parse_markdown(note_id: &str, body: &str) -> ParsedMarkdown {
    let mut labels = Vec::new();
    let mut people = Vec::new();
    let mut workstreams = Vec::new();
    let mut properties = Vec::new();
    let mut contacts = Vec::new();
    let mut diagnostics = Vec::new();
    for line in text_line_spans(body) {
        for entity in line_inline_entities(line) {
            match entity.kind {
                InlineEntityKind::Tag => labels.push(entity.value),
                InlineEntityKind::Person => people.push(entity.value),
                InlineEntityKind::Project => workstreams.push(entity.value),
                InlineEntityKind::MarkdownLink | InlineEntityKind::Url => {
                    contacts.push(ContactFact {
                        kind: ContactKind::Url,
                        value: entity.value,
                        span: entity.span,
                    });
                }
                InlineEntityKind::Email => contacts.push(ContactFact {
                    kind: ContactKind::Email,
                    value: entity.value,
                    span: entity.span,
                }),
                InlineEntityKind::Social => contacts.push(ContactFact {
                    kind: ContactKind::Social,
                    value: entity.value,
                    span: entity.span,
                }),
            }
        }
        properties.extend(entity_properties(line.line));
        diagnostics.extend(line_diagnostics(line));
    }
    let todos = parse_todo_lines(note_id, body);
    diagnostics.extend(duplicate_anchor_diagnostics(&todos));

    let mut source_links = parse_source_links(body);
    source_links.sort_by(|a, b| {
        a.span
            .line_no
            .cmp(&b.span.line_no)
            .then(a.target.cmp(&b.target))
    });

    ParsedMarkdown {
        labels: sorted_dedup(labels),
        people: sorted_dedup(people),
        workstreams: sorted_dedup(workstreams),
        properties,
        todos,
        source_links,
        contacts,
        diagnostics,
    }
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
