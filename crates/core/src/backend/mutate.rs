//! Mutations: every operation that changes the vault. Each writes the markdown
//! file (the source of truth) and then reindexes incrementally so the change is
//! visible immediately without a full rebuild.

use super::parse::{advance_date, format_todo_line, parse_links, parse_tags, set_marker_kind};
use super::vault::{read_note, write_note};
use super::{Backend, Filter, NamedFilter, Note, TodoFields, KINDS, STATUSES};
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;

impl Backend {
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
