//! Mutations: every operation that changes the vault. Each writes the markdown
//! file (the source of truth) and then reindexes incrementally so the change is
//! visible immediately without a full rebuild.

use super::parse::{
    advance_date, format_todo_line, parse_links, parse_tags, parse_todo_lines, set_marker_kind,
};
use super::vault::{markdown_title, read_note, write_note};
use super::{
    is_workstream_label, workstream_label, Backend, Filter, NamedFilter, Note, Todo, TodoFields,
    KINDS, STATUSES,
};
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;

impl Backend {
    pub fn new_note(&mut self) -> Result<Note> {
        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let mut note = Note {
            id: id.clone(),
            title: format!("Note {date}"),
            created: now.clone(),
            updated: now,
            kind: "markdown".into(),
            body: format!("# Note {date}\n\n"),
            path: self.vault.join("notes").join(format!("{id}.md")),
        };
        note.title = markdown_title(&note.body);
        self.persist_with_history(&note, "create_note")?;
        Ok(note)
    }

    /// Create a new note that joins the same topics/clusters as `source_id`
    /// (copies its `[[links]]`) and back-links to it. For "new meeting note in
    /// the same thread" without rewriting the old one.
    pub fn new_related_note(&mut self, source_id: &str) -> Result<Note> {
        let src = self.load_note(source_id)?;
        let workstreams = parse_tags(&src.body)
            .into_iter()
            .filter(|tag| is_workstream_label(tag))
            .collect::<Vec<_>>();
        let links = parse_links(&src.body);
        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let date = Utc::now().format("%Y-%m-%d").to_string();
        let mut body = format!("# {} — {date}\n\n", src.title);
        if !src.title.is_empty() {
            body += &format!("(continues [[{}]])\n", src.title);
        }
        if !links.is_empty() {
            body += &links
                .iter()
                .map(|l| format!("[[{l}]]"))
                .collect::<Vec<_>>()
                .join(" ");
            body.push('\n');
        }
        if !workstreams.is_empty() {
            body += &workstreams
                .iter()
                .map(|label| format!("#{label}"))
                .collect::<Vec<_>>()
                .join(" ");
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
        self.persist_with_history(&note, "create_related_note")?;
        Ok(note)
    }

    pub fn save_note(&mut self, id: &str, _title: &str, body: &str) -> Result<()> {
        let mut note = self.load_note(id)?;
        note.body = body.to_string();
        note.title = markdown_title(&note.body);
        note.updated = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        if note.created.is_empty() {
            note.created = note.updated.clone();
        }
        self.persist_with_history(&note, "save_note")?;
        Ok(())
    }

    /// Load the note owning `todo_id`, transform the resolved task line, save +
    /// reindex. Anchored ids (`note:^anchor`) resolve by block anchor first so
    /// write-back survives line insertions above the task.
    fn rewrite_line<F: Fn(&str) -> String>(&mut self, todo_id: &str, transform: F) -> Result<()> {
        let (note_id, _) = todo_id.rsplit_once(':').context("bad todo id")?;
        let mut note = self.load_note(note_id)?;
        let line_no = resolve_todo_line(&note, todo_id)?;
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
        self.persist_with_history(&note, "rewrite_todo_line")?;
        Ok(())
    }

    /// Cycle a todo done <-> not-done (the list checkbox).
    pub fn toggle_todo(&mut self, todo_id: &str) -> Result<()> {
        self.rewrite_line(todo_id, |line| {
            if line.contains("[x]") || line.contains("[X]") {
                set_marker_kind(line, Some(" "), None)
            } else {
                set_marker_kind(line, Some("x"), None)
            }
        })
    }

    pub fn set_todo_status(&mut self, todo_id: &str, status: &str) -> Result<()> {
        let marker = match status {
            "doing" => "/",
            "done" => "x",
            _ => " ",
        };
        self.rewrite_line(todo_id, |line| set_marker_kind(line, Some(marker), None))
    }

    pub fn set_todo_kind(&mut self, todo_id: &str, kind: &str) -> Result<()> {
        let kind = kind.to_string();
        self.rewrite_line(todo_id, move |line| {
            set_marker_kind(line, None, Some(&kind))
        })
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
        self.persist_with_history(&note, "add_todo")?;
        Ok(format!("{note_id}:{line_no}"))
    }

    /// Carry a todo forward into another note, marking the original complete.
    pub fn carry_todo_to_note(&mut self, todo_id: &str, target_note_id: &str) -> Result<String> {
        let todo = self.get_todo(todo_id)?;
        let mut fields = TodoFields::from_todo(&todo);
        fields.status = "todo".into();
        let new_id = self.add_todo(target_note_id, &fields)?;
        self.set_todo_status(todo_id, "done")?;
        Ok(new_id)
    }

    /// Promote an inline todo into a full task note while leaving a source link
    /// at the original line. The promoted note carries the actionable task; the
    /// original meeting/note line remains a readable backlink to where it came
    /// from.
    pub fn promote_todo_to_note(&mut self, todo_id: &str) -> Result<Note> {
        let todo = self.get_todo(todo_id)?;
        let source = self.load_note(&todo.note_id)?;
        let title = todo_title(&todo);
        let anchor = if todo.anchor.is_empty() {
            block_anchor(&title)
        } else {
            todo.anchor.clone()
        };
        let source_ref = format!("[[{}#^{}]]", source.title, anchor);

        let mut fields = TodoFields::from_todo(&todo);
        fields.text = title.clone();
        fields.status = "todo".into();

        let mut body = format!("# {title}\n\n#task\n");
        if !todo.person.is_empty() {
            body.push_str(&format!("@[[{}]]\n", todo.person));
        }
        if !todo.project.is_empty() {
            body.push_str(&format!("#{}\n", todo.project.trim_start_matches('#')));
        }
        if !todo.kind.is_empty() && todo.kind != "do" {
            body.push_str(&format!("#{}\n", todo.kind));
        }
        for (key, value) in [
            ("priority", todo.priority.as_str()),
            ("start", todo.start.as_str()),
            ("due", todo.due.as_str()),
            ("repeat", todo.repeat.as_str()),
        ] {
            if !value.is_empty() {
                body.push_str(&format!("{key}:{value}\n"));
            }
        }
        if !todo.external.is_empty() {
            body.push_str(&format!("{}\n", todo.external));
        }
        body.push_str(&format!("source:{source_ref}\n\n"));
        body.push_str(&format_todo_line(&fields));
        body.push_str("\n\n## Context\n");
        body.push_str(&format!("Promoted from {source_ref}.\n"));

        let id = ulid::Ulid::new().to_string();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let note = Note {
            id: id.clone(),
            title,
            created: now.clone(),
            updated: now,
            kind: "markdown".into(),
            body,
            path: self.vault.join("notes").join(format!("{id}.md")),
        };
        self.persist_with_history(&note, "promote_todo_to_note")?;

        let mut source_fields = TodoFields::from_todo(&todo);
        source_fields.text = format!("[[{}]]", note.title);
        let linked_line = format!("{} ^{}", format_todo_line(&source_fields), anchor);
        self.rewrite_line(todo_id, |_old| linked_line.clone())?;

        Ok(note)
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
        self.persist_with_history(&note, "attach_path")
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
        self.load_smartlists()
            .into_iter()
            .find(|n| n.name == name)
            .map(|n| n.filter)
    }

    pub fn save_smart_list(&self, name: &str, f: &Filter) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(());
        }
        let mut v = self.load_smartlists();
        v.retain(|n| n.name != name);
        v.push(NamedFilter {
            name: name.to_string(),
            filter: f.clone(),
        });
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
                "# Meeting\n\n#meeting\n\n## Attendees\n@\n\n## Notes\n\n## Action items\n- [ ] \n",
            ),
            "oneonone" => (
                format!("1:1 — {date}"),
                "# 1:1\n\n#meeting/one-on-one\n@[[ ]]\n\n## Updates\n\n## To discuss\n- [ ] ask about ... @[[ ]] #followup\n\n## Delegated / awaiting\n- [ ] ... @[[ ]] #delegated\n",
            ),
            "decision" => (
                format!("Decision — {date}"),
                "# Decision\n\n#decision\n\n## Context\n\n## Decision\n\n## Owner & next steps\n- [ ] \n",
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
        self.persist_with_history(&note, "create_note_from_template")?;
        Ok(note)
    }

    /// Soft-delete a note: move it to the vault `.trash` (not indexed).
    pub fn delete_note(&mut self, note_id: &str) -> Result<()> {
        let note = self.load_note(note_id)?;
        let trash = self.vault.join(".trash");
        std::fs::create_dir_all(&trash)?;
        let dest = trash.join(note.path.file_name().unwrap());
        std::fs::rename(&note.path, &dest)?;
        let mut after = note.clone();
        after.path = dest;
        self.record_note_revision(Some(&note), Some(&after), "delete_note")?;
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
            let before = read_note(&src).ok();
            std::fs::rename(&src, &dest)?;
            if let Some(before) = before {
                let mut after = before.clone();
                after.path = dest;
                self.record_note_revision(Some(&before), Some(&after), "restore_note")?;
            }
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
        let mut after = note.clone();
        after.path = dest;
        self.record_note_revision(
            Some(&note),
            Some(&after),
            if archive {
                "archive_note"
            } else {
                "unarchive_note"
            },
        )?;
        self.reindex_all()
    }

    /// Switch a note between markdown and typst rendering.
    pub fn set_note_kind(&mut self, note_id: &str, kind: &str) -> Result<()> {
        let mut note = self.load_note(note_id)?;
        note.kind = kind.to_string();
        note.updated = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        self.persist_with_history(&note, "set_note_kind")
    }

    /// File a note into a workstream by appending a `#workstream/...` label.
    pub fn add_link(&mut self, note_id: &str, topic: &str) -> Result<()> {
        let topic = topic
            .trim()
            .trim_start_matches("[[")
            .trim_end_matches("]]")
            .trim_start_matches('#')
            .trim();
        let topic = workstream_label(topic);
        if topic.is_empty() {
            return Ok(());
        }
        let mut note = self.load_note(note_id)?;
        if parse_tags(&note.body)
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&topic))
        {
            return Ok(());
        }
        if !note.body.is_empty() && !note.body.ends_with('\n') {
            note.body.push('\n');
        }
        note.body.push_str(&format!("#{topic}\n"));
        self.persist_with_history(&note, "file_note_workstream")
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
        self.persist_with_history(&note, "add_tag")
    }

    /// Replace a todo's line wholesale from form fields.
    pub fn update_todo(&mut self, todo_id: &str, fields: &TodoFields) -> Result<()> {
        let existing = self.get_todo(todo_id)?;
        let mut line = format_todo_line(fields);
        if !existing.anchor.is_empty() {
            line.push_str(&format!(" ^{}", existing.anchor));
        }
        self.rewrite_line(todo_id, |_old| line.clone())
    }

    /// Drag-and-drop a card onto a column: set the grouped dimension to the
    /// column's value (status/workflow/workstream/person), rewriting the line.
    pub fn drop_card(&mut self, todo_id: &str, group_by: &str, target_key: &str) -> Result<()> {
        let mut fields = TodoFields::from_todo(&self.get_todo(todo_id)?);
        let val = if target_key == "(none)" {
            ""
        } else {
            target_key
        };
        match group_by {
            "status" => fields.status = val.to_string(),
            "kind" => fields.kind = val.to_string(),
            "project" | "workstream" => fields.project = workstream_label(val),
            "person" => fields.person = val.to_string(),
            _ => {}
        }
        self.update_todo(todo_id, &fields)
    }

    /// Write a note to disk and reindex it in one shot.
    fn persist_with_history(&mut self, note: &Note, operation: &str) -> Result<()> {
        let before = note
            .path
            .exists()
            .then(|| read_note(&note.path).ok())
            .flatten();
        let mut note = note.clone();
        note.title = markdown_title(&note.body);
        write_note(&note)?;
        let fts = self.fts;
        let tx = self.conn.transaction()?;
        Self::index_note(&tx, &note, fts)?;
        tx.commit()?;
        self.record_note_revision(before.as_ref(), Some(&note), operation)?;
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
        let cur = if group_by == "status" {
            &todo.status
        } else {
            &todo.kind
        };
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

fn todo_title(todo: &Todo) -> String {
    let title = todo.text.trim();
    if title.is_empty() {
        "Task".into()
    } else {
        title.chars().take(80).collect()
    }
}

fn block_anchor(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in title.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        format!("task-{}", ulid::Ulid::new().to_string().to_lowercase())
    } else {
        out.chars().take(48).collect()
    }
}

fn resolve_todo_line(note: &Note, todo_id: &str) -> Result<usize> {
    let (note_id, key) = todo_id.rsplit_once(':').context("bad todo id")?;
    if note_id != note.id {
        anyhow::bail!("todo id {todo_id} does not belong to note {}", note.id);
    }
    let parsed = parse_todo_lines(&note.id, &note.body);
    if let Some(anchor) = key.strip_prefix('^') {
        if let Some(task) = parsed
            .iter()
            .find(|task| task.todo.anchor == anchor || task.todo.id == todo_id)
        {
            return Ok(task.todo.line_no);
        }
        anyhow::bail!("task anchor ^{anchor} not found in {}", note.title);
    }
    let line_no: usize = key.parse()?;
    if parsed.iter().any(|task| task.todo.line_no == line_no) {
        Ok(line_no)
    } else {
        anyhow::bail!("task line {line_no} not found in {}", note.title);
    }
}
