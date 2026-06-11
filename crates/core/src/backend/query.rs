//! Read-only queries that power every view (Notes / Tasks / Board / Gantt /
//! Agenda / Inbox / People / Labels). All read from the SQLite index; the one
//! exception is [`Backend::load_note`], which re-reads the file (the truth).

use super::index::fts_query;
use super::parse::{parse_links, parse_mentions, parse_tags};
use super::vault::read_note;
use super::{Backend, Filter, Note, Project, RelatedNote, SourceSpan, Todo, KINDS, STATUSES};
use anyhow::Result;
use chrono::Utc;
use std::path::{Path, PathBuf};

/// Follow-ups/delegated todos go "stale" once their note is untouched this long.
#[allow(dead_code)] // used by the stale view (currently exercised via tests)
const STALE_DAYS: i64 = 14;

impl Backend {
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
            anchor: r.get(14)?,
            span: SourceSpan {
                line_no: r.get::<_, i64>(13)? as usize,
                byte_start: r.get::<_, i64>(15)? as usize,
                byte_end: r.get::<_, i64>(16)? as usize,
            },
        })
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

    /// Canonical todo columns, with the given alias prefix (e.g. "t." or "").
    fn todo_cols(prefix: &str) -> String {
        let f = [
            "id",
            "note_id",
            "kind",
            "status",
            "text",
            "project",
            "person",
            "start",
            "due",
            "external",
            "priority",
            "repeat",
            "done",
            "line_no",
            "anchor",
            "span_start",
            "span_end",
        ];
        f.iter()
            .map(|c| format!("{prefix}{c}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT target, COUNT(*) FROM links GROUP BY target ORDER BY target ASC")?;
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

    /// The one query that powers every view. Joins tags/notes only when needed.
    pub fn query_todos(&self, f: &Filter) -> Result<Vec<Todo>> {
        let mut sql = format!("SELECT {} FROM todos t", Self::todo_cols("t."));
        let mut where_: Vec<String> = Vec::new();
        let mut binds: Vec<String> = Vec::new();
        if !f.tag.is_empty() {
            sql.push_str(
                " JOIN task_tags tg ON tg.task_id = t.id AND (tg.tag = ? OR tg.tag LIKE ?)",
            );
            binds.push(f.tag.clone());
            binds.push(format!("{}/%", f.tag));
        }
        if !f.search.is_empty() {
            where_.push("t.text LIKE ?".into());
            binds.push(Filter::like(&f.search));
        }
        if !f.project.is_empty() {
            sql.push_str(
                " JOIN task_links tl ON tl.task_id = t.id AND (tl.target = ? OR tl.target LIKE ?)",
            );
            binds.push(f.project.clone());
            binds.push(format!("{}/%", f.project));
        }
        if !f.person.is_empty() {
            sql.push_str(" JOIN task_mentions tm ON tm.task_id = t.id AND tm.person = ?");
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
        let mut sql = String::from(
            "SELECT DISTINCT n.id,n.title,n.path,n.created,n.updated,n.kind FROM notes n",
        );
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
        let rows = stmt.query_map(params, Self::note_row)?;
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

    /// "Waiting on" — open delegated todos (things you handed off), clustered by
    /// person and oldest-note-first, so the items most in need of a nudge surface at
    /// the top. Powers the Waiting view.
    pub fn waiting_on(&self) -> Result<Vec<Todo>> {
        let sql = format!(
            "SELECT {} FROM todos t JOIN notes n ON t.note_id = n.id \
             WHERE t.done = 0 AND t.kind = 'delegated' AND n.archived = 0 \
             ORDER BY t.person ASC, n.updated ASC",
            Self::todo_cols("t.")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_todo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
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

    /// The title of a note by id (cheap lookup for the open-notes tab strip).
    pub fn note_title(&self, id: &str) -> Option<String> {
        self.conn
            .query_row("SELECT title FROM notes WHERE id=?", [id], |r| {
                r.get::<_, String>(0)
            })
            .ok()
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

    /// Notes related to `note_id` by shared workstreams (`[[ ]]` links), people
    /// (`@`), or tags (`#`), ranked by how many entities they share, then recency.
    /// Powers "link this meeting note to related prior meetings": the same project
    /// or the same recurring attendees surface the earlier notes in the thread.
    pub fn related_notes(&self, note_id: &str, limit: usize) -> Result<Vec<RelatedNote>> {
        use std::collections::{BTreeSet, HashMap};
        // This note's own entity values, per source table.
        let column_values = |table: &str, col: &str| -> Result<Vec<String>> {
            let mut stmt = self
                .conn
                .prepare(&format!("SELECT {col} FROM {table} WHERE note_id=?"))?;
            let rows = stmt.query_map([note_id], |r| r.get::<_, String>(0))?;
            Ok(rows.filter_map(|r| r.ok()).collect())
        };
        let sources = [
            ("links", "target", column_values("links", "target")?),
            ("mentions", "person", column_values("mentions", "person")?),
            ("tags", "tag", column_values("tags", "tag")?),
        ];
        // note_id -> set of shared entity names.
        let mut hits: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (table, col, values) in &sources {
            for v in values {
                let mut stmt = self.conn.prepare(&format!(
                    "SELECT DISTINCT note_id FROM {table} WHERE {col}=? AND note_id!=?"
                ))?;
                let rows =
                    stmt.query_map(rusqlite::params![v, note_id], |r| r.get::<_, String>(0))?;
                for nid in rows.filter_map(|r| r.ok()) {
                    hits.entry(nid).or_default().insert(v.clone());
                }
            }
        }
        let mut out: Vec<RelatedNote> = Vec::new();
        for (nid, shared) in hits {
            let row = self.conn.query_row(
                "SELECT title, updated, archived FROM notes WHERE id=?",
                [&nid],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            );
            if let Ok((title, updated, archived)) = row {
                if archived == 0 {
                    out.push(RelatedNote {
                        id: nid,
                        title,
                        updated,
                        shared: shared.into_iter().collect(),
                    });
                }
            }
        }
        // Most-shared first, then most-recently-updated.
        out.sort_by(|a, b| {
            b.shared
                .len()
                .cmp(&a.shared.len())
                .then(b.updated.cmp(&a.updated))
        });
        out.truncate(limit);
        Ok(out)
    }

    pub fn get_todo(&self, id: &str) -> Result<Todo> {
        let sql = format!(
            "SELECT {} FROM todos t WHERE t.id = ?",
            Self::todo_cols("t.")
        );
        Ok(self.conn.query_row(&sql, [id], Self::row_to_todo)?)
    }

    /// Todos whose external ref starts with `prefix` (for example `ref:` or
    /// `gh:`). Useful for local reference views and cleanup tools.
    pub fn todos_by_external_prefix(&self, prefix: &str) -> Result<Vec<Todo>> {
        let sql = format!(
            "SELECT {} FROM todos t WHERE t.external LIKE ? ORDER BY t.line_no ASC",
            Self::todo_cols("t.")
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([format!("{prefix}%")], Self::row_to_todo)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Whether a note is currently archived. `false` if it's unknown to the index.
    pub fn note_archived(&self, note_id: &str) -> Result<bool> {
        let archived: i64 = self
            .conn
            .query_row("SELECT archived FROM notes WHERE id=?", [note_id], |r| {
                r.get(0)
            })
            .unwrap_or(0);
        Ok(archived != 0)
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

    pub fn load_note(&self, id: &str) -> Result<Note> {
        let path: String = self
            .conn
            .query_row("SELECT path FROM notes WHERE id=?", [id], |r| r.get(0))?;
        read_note(Path::new(&path))
    }

    /// Entities referenced in a note body: (projects, people, tags).
    pub fn note_entities(body: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
        (parse_links(body), parse_mentions(body), parse_tags(body))
    }
}
