//! Noet core — the UI-agnostic heart of the app.
//!
//! Plain markdown files are the source of truth; a disposable SQLite database is
//! a rebuildable index over them. This crate has **no GUI dependencies**, so it
//! can be driven by any frontend: the Slint GUI (`noet-gui`), a future terminal
//! UI (`noet-tui`), a CLI, or tests.
//!
//! The public surface is the [`Backend`] façade plus the parsing/model helpers.

pub mod backend;
pub mod connectors;

// Re-export the common types so frontends can `use noet_core::{Backend, Filter, …}`
// without reaching into module paths.
pub use backend::{Backend, Filter, Note, Project, Segment, Todo, TodoFields};
