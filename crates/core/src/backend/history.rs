//! Durable per-note revision history. This is separate from the disposable
//! search index because history is user data, not rebuildable cache.

use super::vault::format_note;
use super::{Backend, Note};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use similar::TextDiff;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevisionActor {
    User,
    Ai,
    System,
}

impl RevisionActor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Ai => "ai",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevisionContext {
    pub actor: RevisionActor,
    pub operation: Option<String>,
    pub proposal_id: Option<String>,
    pub model_id: Option<String>,
    pub rationale: Option<String>,
}

impl RevisionContext {
    pub fn user(operation: impl Into<String>) -> Self {
        Self {
            actor: RevisionActor::User,
            operation: Some(operation.into()),
            proposal_id: None,
            model_id: None,
            rationale: None,
        }
    }

    pub fn ai(
        operation: impl Into<String>,
        proposal_id: Option<String>,
        model_id: Option<String>,
        rationale: Option<String>,
    ) -> Self {
        Self {
            actor: RevisionActor::Ai,
            operation: Some(operation.into()),
            proposal_id,
            model_id,
            rationale,
        }
    }
}

impl Default for RevisionContext {
    fn default() -> Self {
        Self {
            actor: RevisionActor::User,
            operation: None,
            proposal_id: None,
            model_id: None,
            rationale: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRevision {
    pub id: String,
    pub note_id: String,
    pub created_at: String,
    pub actor: String,
    pub operation: String,
    pub proposal_id: Option<String>,
    pub model_id: Option<String>,
    pub rationale: Option<String>,
    pub before_path: Option<String>,
    pub after_path: Option<String>,
    pub before_title: Option<String>,
    pub after_title: Option<String>,
    pub before_kind: Option<String>,
    pub after_kind: Option<String>,
    pub before_content: String,
    pub after_content: String,
    pub diff: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteRevisionSummary {
    pub id: String,
    pub note_id: String,
    pub created_at: String,
    pub actor: String,
    pub operation: String,
    pub proposal_id: Option<String>,
    pub model_id: Option<String>,
    pub before_title: Option<String>,
    pub after_title: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevisionSnapshot {
    Before,
    After,
}

impl RevisionSnapshot {
    fn content<'a>(&self, revision: &'a NoteRevision) -> &'a str {
        match self {
            Self::Before => &revision.before_content,
            Self::After => &revision.after_content,
        }
    }

    fn path<'a>(&self, revision: &'a NoteRevision) -> Option<&'a str> {
        match self {
            Self::Before => revision.before_path.as_deref(),
            Self::After => revision.after_path.as_deref(),
        }
    }

    fn operation(&self) -> &'static str {
        match self {
            Self::Before => "restore_revision_before",
            Self::After => "restore_revision_after",
        }
    }
}

impl Backend {
    pub fn history_path(&self) -> PathBuf {
        self.vault.join(".noet").join("history.sqlite")
    }

    pub fn with_revision_context<T>(
        &mut self,
        context: RevisionContext,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous = std::mem::replace(&mut self.revision_context, context);
        let result = f(self);
        self.revision_context = previous;
        result
    }

    pub(crate) fn record_note_revision(
        &self,
        before: Option<&Note>,
        after: Option<&Note>,
        fallback_operation: &str,
    ) -> Result<()> {
        let before_content = before.map(format_note).unwrap_or_default();
        let after_content = after.map(format_note).unwrap_or_default();
        let before_path = before.map(note_path_string);
        let after_path = after.map(note_path_string);
        if before_content == after_content && before_path == after_path {
            return Ok(());
        }

        let note_id = after
            .or(before)
            .map(|note| note.id.clone())
            .context("revision has no note id")?;
        let context = &self.revision_context;
        let operation = context
            .operation
            .clone()
            .unwrap_or_else(|| fallback_operation.to_string());
        let diff = unified_diff(&before_content, &after_content);
        let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let id = ulid::Ulid::new().to_string();
        let conn = open_history_connection(&self.history_path())?;
        conn.execute(
            "INSERT INTO note_revisions(
                id, note_id, created_at, actor, operation,
                proposal_id, model_id, rationale,
                before_path, after_path, before_title, after_title, before_kind, after_kind,
                before_content, after_content, diff
             ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                id,
                note_id,
                created_at,
                context.actor.as_str(),
                operation,
                context.proposal_id.as_deref(),
                context.model_id.as_deref(),
                context.rationale.as_deref(),
                before_path,
                after_path,
                before.map(|note| note.title.clone()),
                after.map(|note| note.title.clone()),
                before.map(|note| note.kind.clone()),
                after.map(|note| note.kind.clone()),
                before_content,
                after_content,
                diff,
            ],
        )?;
        Ok(())
    }

    pub fn note_revisions(&self, note_id: &str) -> Result<Vec<NoteRevisionSummary>> {
        let conn = open_history_connection(&self.history_path())?;
        let mut stmt = conn.prepare(
            "SELECT id, note_id, created_at, actor, operation, proposal_id, model_id,
                    before_title, after_title
             FROM note_revisions
             WHERE note_id=?
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([note_id], |row| {
            Ok(NoteRevisionSummary {
                id: row.get(0)?,
                note_id: row.get(1)?,
                created_at: row.get(2)?,
                actor: row.get(3)?,
                operation: row.get(4)?,
                proposal_id: row.get(5)?,
                model_id: row.get(6)?,
                before_title: row.get(7)?,
                after_title: row.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn note_revision(&self, revision_id: &str) -> Result<Option<NoteRevision>> {
        let conn = open_history_connection(&self.history_path())?;
        conn.query_row(
            "SELECT id, note_id, created_at, actor, operation,
                    proposal_id, model_id, rationale,
                    before_path, after_path, before_title, after_title, before_kind, after_kind,
                    before_content, after_content, diff
             FROM note_revisions
             WHERE id=?",
            [revision_id],
            |row| {
                Ok(NoteRevision {
                    id: row.get(0)?,
                    note_id: row.get(1)?,
                    created_at: row.get(2)?,
                    actor: row.get(3)?,
                    operation: row.get(4)?,
                    proposal_id: row.get(5)?,
                    model_id: row.get(6)?,
                    rationale: row.get(7)?,
                    before_path: row.get(8)?,
                    after_path: row.get(9)?,
                    before_title: row.get(10)?,
                    after_title: row.get(11)?,
                    before_kind: row.get(12)?,
                    after_kind: row.get(13)?,
                    before_content: row.get(14)?,
                    after_content: row.get(15)?,
                    diff: row.get(16)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn restore_note_revision(
        &mut self,
        revision_id: &str,
        snapshot: RevisionSnapshot,
    ) -> Result<Note> {
        let revision = self
            .note_revision(revision_id)?
            .context("note revision not found")?;
        let content = snapshot.content(&revision);
        if content.trim().is_empty() {
            anyhow::bail!("revision snapshot is empty");
        }

        let target_path = self
            .load_note(&revision.note_id)
            .map(|note| note.path)
            .ok()
            .or_else(|| snapshot.path(&revision).map(PathBuf::from))
            .or_else(|| revision.after_path.as_deref().map(PathBuf::from))
            .or_else(|| revision.before_path.as_deref().map(PathBuf::from))
            .context("revision has no restorable note path")?;

        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let before = target_path
            .exists()
            .then(|| super::vault::read_note(&target_path).ok())
            .flatten();
        std::fs::write(&target_path, content)
            .with_context(|| format!("restoring {}", target_path.display()))?;
        let after = super::vault::read_note(&target_path)?;
        let fts = self.fts;
        let tx = self.conn.transaction()?;
        Self::index_note(&tx, &after, fts)?;
        tx.commit()?;

        let previous = std::mem::replace(
            &mut self.revision_context,
            RevisionContext::user(snapshot.operation()),
        );
        let record = self.record_note_revision(before.as_ref(), Some(&after), snapshot.operation());
        self.revision_context = previous;
        record?;
        Ok(after)
    }
}

fn open_history_connection(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;");
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS note_revisions(
            id TEXT PRIMARY KEY,
            note_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            actor TEXT NOT NULL,
            operation TEXT NOT NULL,
            proposal_id TEXT,
            model_id TEXT,
            rationale TEXT,
            before_path TEXT,
            after_path TEXT,
            before_title TEXT,
            after_title TEXT,
            before_kind TEXT,
            after_kind TEXT,
            before_content TEXT NOT NULL,
            after_content TEXT NOT NULL,
            diff TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_note_revisions_note_created
            ON note_revisions(note_id, created_at DESC, id DESC);
        "#,
    )?;
    Ok(conn)
}

fn note_path_string(note: &Note) -> String {
    note.path.to_string_lossy().to_string()
}

fn unified_diff(before: &str, after: &str) -> String {
    TextDiff::from_lines(before, after)
        .unified_diff()
        .header("before", "after")
        .to_string()
}
