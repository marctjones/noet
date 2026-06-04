//! Core data types shared across the backend: notes, typed todos, filters, and
//! the small value objects the query/render layers hand back to a frontend.

use std::path::PathBuf;

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
    pub(crate) fn like(s: &str) -> String {
        format!("%{s}%")
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct NamedFilter {
    pub(crate) name: String,
    pub(crate) filter: Filter,
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

/// One inline piece of a rendered line.
#[derive(Debug, Clone)]
pub struct Segment {
    pub text: String,  // what to show
    pub kind: String,  // "" plain | "url" | "project" | "person" | "tag"
    pub value: String, // url to open, or entity name to filter
}
