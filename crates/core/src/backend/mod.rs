//! Noet backend: plain markdown files are the source of truth; SQLite is a
//! disposable index rebuilt from those files. No network, no JS — just files.
//!
//! The logic is split into focused submodules around the [`Backend`] façade:
//! - [`model`] — the shared data types (notes, typed todos, filters).
//! - [`parse`] — the file-first grammar (markdown blocks, todo lines, entities).
//! - [`vault`] — file IO: frontmatter, kind detection, safe filenames.
//! - [`index`] — the disposable SQLite schema, (re)indexing, open lifecycle.
//! - [`query`] — read-only queries that power every view.
//! - [`mutate`] — every operation that changes the vault.
//! - [`workflow`] — typed read models for workspaces and reviews.
//! - [`render`] — Typst rendering for the read view.
//! - [`export`] — per-note Markdown / PDF export.

mod export;
mod index;
mod model;
mod mutate;
mod parse;
mod query;
mod render;
mod settings;
mod vault;
mod workflow;

// The public surface frontends consume as `noet_core::backend::*`.
pub use index::{background_reindex, reindex_connection};
pub use model::{
    Filter, MdBlock, Note, Project, RelatedNote, Segment, SourceSpan, Todo, TodoFields, KINDS,
    STATUSES,
};
pub use parse::{
    clean_inline, line_segments, markdown_blocks, parse_inline_entities, parse_links,
    parse_markdown, parse_mentions, parse_properties, parse_source_links, parse_tags,
    parse_todo_lines, parse_todos, ContactFact, ContactKind, InlineEntity, InlineEntityKind,
    ParseDiagnostic, ParseSeverity, ParsedMarkdown, ParsedTodoLine, SourceLink,
};
pub use settings::Settings;
pub use vault::{detect_kind, effective_kind, markdown_title, set_markdown_title};
pub use workflow::{
    BoardColumn, BoardModel, LabelContext, LabelReview, LabelSummary, NoteContext, NoteFacts,
    NoteSummary, OneOnOneContext, ParsedNote, PropertyFact, SourceRef, TaskFact, TaskReview,
    TaskSource, TaskStatus, TaskWorkflow, WaitingGroup, WaitingReview,
};

// Crate-internal types that submodules reach for via `super::`.
pub(crate) use model::{entity_key, NamedFilter};

use rusqlite::Connection;
use std::path::PathBuf;

/// The façade over a vault + its disposable SQLite index. Inherent methods are
/// implemented across the [`index`], [`query`], [`mutate`], [`render`], and
/// [`export`] submodules; the struct (and its private `conn`/`fts`) lives here so
/// all of them — as descendants of `backend` — can reach the fields.
pub struct Backend {
    pub vault: PathBuf,
    index_dir: PathBuf, // where the disposable SQLite index + render cache live
    conn: Connection,
    fts: bool, // FTS5 available in this SQLite build
}

#[cfg(test)]
mod tests;
