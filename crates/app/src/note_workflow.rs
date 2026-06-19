use crate::command::AppCommand;
use noet_core::{Backend, Note};

#[derive(Clone, Debug, PartialEq)]
pub struct NewNoteWorkflowReport {
    pub note_id: String,
    pub open_command: AppCommand,
    pub status_message: String,
    pub open_in_edit_mode: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectNoteWorkflowRequest {
    pub note_id: String,
    pub current_note_id: String,
    pub current_title: String,
    pub current_body: String,
    pub current_is_editing: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectNoteWorkflowReport {
    pub note_id: String,
    pub open_command: AppCommand,
    pub status_message: String,
    pub open_in_edit_mode: bool,
    pub saved_previous_note: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddTagWorkflowReport {
    pub note_id: String,
    pub normalized_tag: String,
    pub status_message: String,
    pub refresh_note: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachPathWorkflowReport {
    pub note_id: String,
    pub path: String,
    pub status_message: String,
    pub refresh_note: bool,
}

pub fn create_note(backend: &mut Backend) -> Result<Note, String> {
    backend.new_note().map_err(|err| err.to_string())
}

pub fn seed_note_if_vault_empty(
    backend: &mut Backend,
    title: &str,
    body: &str,
) -> Result<Option<Note>, String> {
    if !backend.is_vault_empty() {
        return Ok(None);
    }
    create_note_from_body(backend, title, body).map(Some)
}

pub fn create_note_in_workstream(backend: &mut Backend, workstream: &str) -> Result<Note, String> {
    let note = create_note(backend)?;
    if !workstream.trim().is_empty() {
        file_note(backend, &note.id, workstream)?;
    }
    backend.load_note(&note.id).map_err(|err| err.to_string())
}

pub fn create_note_for_workstream(
    backend: &mut Backend,
    workstream: &str,
) -> Result<NewNoteWorkflowReport, String> {
    let workstream = workstream.trim();
    let note = create_note_in_workstream(backend, workstream)?;
    let note_id = note.id;
    let status_message = if workstream.is_empty() {
        "New note".to_string()
    } else {
        format!("New note in {workstream}")
    };
    Ok(NewNoteWorkflowReport {
        open_command: AppCommand::OpenNote(note_id.clone()),
        note_id,
        status_message,
        open_in_edit_mode: true,
    })
}

pub fn select_note(
    backend: &mut Backend,
    request: SelectNoteWorkflowRequest,
) -> Result<SelectNoteWorkflowReport, String> {
    let note_id = request.note_id.trim().to_string();
    if note_id.is_empty() {
        return Err("Select a note first.".into());
    }

    let current_note_id = request.current_note_id.trim();
    let saved_previous_note =
        request.current_is_editing && !current_note_id.is_empty() && current_note_id != note_id;
    if saved_previous_note {
        save_note(
            backend,
            current_note_id,
            &request.current_title,
            &request.current_body,
        )?;
    }

    Ok(SelectNoteWorkflowReport {
        open_command: AppCommand::OpenNote(note_id.clone()),
        note_id,
        status_message: String::new(),
        open_in_edit_mode: false,
        saved_previous_note,
    })
}

pub fn save_note(
    backend: &mut Backend,
    note_id: &str,
    title: &str,
    body: &str,
) -> Result<(), String> {
    backend
        .save_note(note_id, title, body)
        .map_err(|err| err.to_string())
}

pub fn create_note_from_body(
    backend: &mut Backend,
    title: &str,
    body: &str,
) -> Result<Note, String> {
    let note = create_note(backend)?;
    save_note(backend, &note.id, title, body)?;
    backend.load_note(&note.id).map_err(|err| err.to_string())
}

pub fn create_note_from_template(
    backend: &mut Backend,
    template: &str,
    person: Option<&str>,
) -> Result<Note, String> {
    let note = backend
        .new_from_template(template)
        .map_err(|err| err.to_string())?;
    let person = person.unwrap_or_default().trim();
    if template == "oneonone" && !person.is_empty() {
        let title = format!("1:1 — {person}");
        let body = format!(
            "# {title}\n\n#meeting/one-on-one\n@[[{person}]]\n\n## Updates\n\n## To discuss\n- [ ]  @[[{person}]] #followup\n\n## Delegated / awaiting\n- [ ]  @[[{person}]] #delegated\n"
        );
        save_note(backend, &note.id, &title, &body)?;
    }
    backend.load_note(&note.id).map_err(|err| err.to_string())
}

pub fn create_related_note(backend: &mut Backend, source_note_id: &str) -> Result<Note, String> {
    backend
        .new_related_note(source_note_id)
        .map_err(|err| err.to_string())
}

pub fn add_tag_to_note(backend: &mut Backend, note_id: &str, tag: &str) -> Result<(), String> {
    backend.add_tag(note_id, tag).map_err(|err| err.to_string())
}

pub fn add_tag_to_current_note(
    backend: &mut Backend,
    note_id: &str,
    tag: &str,
) -> Result<Option<AddTagWorkflowReport>, String> {
    let note_id = note_id.trim();
    let normalized_tag = tag.trim().trim_start_matches('#').trim();
    if note_id.is_empty() || normalized_tag.is_empty() {
        return Ok(None);
    }

    add_tag_to_note(backend, note_id, normalized_tag)?;
    Ok(Some(AddTagWorkflowReport {
        note_id: note_id.into(),
        normalized_tag: normalized_tag.into(),
        status_message: format!("Added #{normalized_tag}"),
        refresh_note: true,
    }))
}

pub fn file_note(backend: &mut Backend, note_id: &str, topic: &str) -> Result<(), String> {
    backend
        .add_link(note_id, topic)
        .map_err(|err| err.to_string())
}

pub fn attach_path_to_note(backend: &mut Backend, note_id: &str, path: &str) -> Result<(), String> {
    backend
        .attach_path(note_id, path)
        .map_err(|err| err.to_string())
}

pub fn attach_path_to_current_note(
    backend: &mut Backend,
    note_id: &str,
    path: &str,
) -> Result<Option<AttachPathWorkflowReport>, String> {
    let note_id = note_id.trim();
    let path = path.trim();
    if note_id.is_empty() || path.is_empty() {
        return Ok(None);
    }

    attach_path_to_note(backend, note_id, path)?;
    Ok(Some(AttachPathWorkflowReport {
        note_id: note_id.into(),
        path: path.into(),
        status_message: "Attached".into(),
        refresh_note: true,
    }))
}

pub fn delete_note(backend: &mut Backend, note_id: &str) -> Result<(), String> {
    backend.delete_note(note_id).map_err(|err| err.to_string())
}

pub fn restore_note(backend: &mut Backend, filename: &str) -> Result<(), String> {
    backend
        .restore_note(filename)
        .map_err(|err| err.to_string())
}

pub fn archive_note(backend: &mut Backend, note_id: &str, archive: bool) -> Result<(), String> {
    backend
        .archive_note(note_id, archive)
        .map_err(|err| err.to_string())
}

pub fn set_note_kind(backend: &mut Backend, note_id: &str, kind: &str) -> Result<(), String> {
    backend
        .set_note_kind(note_id, kind)
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_core::{Backend, Filter};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn note_lifecycle_routes_through_app_workflow() {
        let (mut backend, dir) = backend();

        let note = create_note_in_workstream(&mut backend, "clients/acme").unwrap();
        assert!(backend
            .load_note(&note.id)
            .unwrap()
            .body
            .contains("#workstream/clients/acme"));

        save_note(&mut backend, &note.id, "Ignored", "# Saved\n\nBody\n").unwrap();
        add_tag_to_note(&mut backend, &note.id, "followup").unwrap();
        attach_path_to_note(&mut backend, &note.id, "ref:/tmp/example.pdf").unwrap();
        set_note_kind(&mut backend, &note.id, "typst").unwrap();

        let saved = backend.load_note(&note.id).unwrap();
        assert_eq!(saved.kind, "typst");
        assert!(saved.body.contains("#followup"));
        assert!(saved.body.contains("ref:/tmp/example.pdf"));

        archive_note(&mut backend, &note.id, true).unwrap();
        let archived = backend
            .query_notes(&Filter {
                show_archived: true,
                ..Default::default()
            })
            .unwrap();
        assert!(archived.iter().any(|candidate| candidate.id == note.id));

        archive_note(&mut backend, &note.id, false).unwrap();
        delete_note(&mut backend, &note.id).unwrap();
        assert!(!backend
            .query_notes(&Filter::default())
            .unwrap()
            .iter()
            .any(|candidate| candidate.id == note.id));

        let trash = backend.list_trash().unwrap();
        restore_note(&mut backend, &trash[0].0).unwrap();
        assert!(backend
            .query_notes(&Filter::default())
            .unwrap()
            .iter()
            .any(|candidate| candidate.id == note.id));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn note_creation_helpers_preserve_expected_body_shapes() {
        let (mut backend, dir) = backend();

        let welcome = seed_note_if_vault_empty(&mut backend, "Welcome", "# Welcome\n\n").unwrap();
        assert!(welcome.is_some());
        let skipped = seed_note_if_vault_empty(&mut backend, "Welcome", "# Welcome\n\n").unwrap();
        assert!(skipped.is_none());

        let captured =
            create_note_from_body(&mut backend, "Capture", "Capture this thought\n").unwrap();
        assert_eq!(captured.title, "Capture this thought");

        let one_on_one =
            create_note_from_template(&mut backend, "oneonone", Some("Jane Smith")).unwrap();
        assert!(one_on_one.body.contains("@[[Jane Smith]]"));
        assert!(one_on_one.body.contains("#meeting/one-on-one"));

        let related = create_related_note(&mut backend, &one_on_one.id).unwrap();
        assert!(related.body.contains("[[1:1 — Jane Smith]]"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn new_note_workflow_reports_default_note_action() {
        let (mut backend, dir) = backend();

        let report = create_note_for_workstream(&mut backend, "").unwrap();

        assert_eq!(
            report.open_command,
            AppCommand::OpenNote(report.note_id.clone())
        );
        assert_eq!(report.status_message, "New note");
        assert!(report.open_in_edit_mode);
        let saved = backend.load_note(&report.note_id).unwrap();
        assert!(!saved.body.contains("#workstream/"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn new_note_workflow_files_active_workstream() {
        let (mut backend, dir) = backend();

        let report = create_note_for_workstream(&mut backend, " clients/acme ").unwrap();

        assert_eq!(
            report.open_command,
            AppCommand::OpenNote(report.note_id.clone())
        );
        assert_eq!(report.status_message, "New note in clients/acme");
        assert!(report.open_in_edit_mode);
        let saved = backend.load_note(&report.note_id).unwrap();
        assert!(saved.body.contains("#workstream/clients/acme"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn select_note_workflow_saves_current_edit_before_switching() {
        let (mut backend, dir) = backend();
        let current = create_note_from_body(&mut backend, "Current", "# Current\n\nOld\n").unwrap();
        let target = create_note_from_body(&mut backend, "Target", "# Target\n\n").unwrap();

        let report = select_note(
            &mut backend,
            SelectNoteWorkflowRequest {
                note_id: target.id.clone(),
                current_note_id: current.id.clone(),
                current_title: "Current".into(),
                current_body: "# Current\n\nUpdated draft\n".into(),
                current_is_editing: true,
            },
        )
        .unwrap();

        assert_eq!(report.open_command, AppCommand::OpenNote(target.id.clone()));
        assert_eq!(report.note_id, target.id);
        assert_eq!(report.status_message, "");
        assert!(!report.open_in_edit_mode);
        assert!(report.saved_previous_note);
        assert!(backend
            .load_note(&current.id)
            .unwrap()
            .body
            .contains("Updated draft"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn select_note_workflow_skips_save_when_not_editing() {
        let (mut backend, dir) = backend();
        let current = create_note_from_body(&mut backend, "Current", "# Current\n\nOld\n").unwrap();
        let target = create_note_from_body(&mut backend, "Target", "# Target\n\n").unwrap();

        let report = select_note(
            &mut backend,
            SelectNoteWorkflowRequest {
                note_id: target.id.clone(),
                current_note_id: current.id.clone(),
                current_title: "Current".into(),
                current_body: "# Current\n\nShould not save\n".into(),
                current_is_editing: false,
            },
        )
        .unwrap();

        assert!(!report.saved_previous_note);
        assert!(!backend
            .load_note(&current.id)
            .unwrap()
            .body
            .contains("Should not save"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn add_tag_workflow_normalizes_and_reports_tag() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Note", "# Note\n\n").unwrap();

        let report = add_tag_to_current_note(&mut backend, &note.id, " #followup ")
            .unwrap()
            .unwrap();

        assert_eq!(report.note_id, note.id);
        assert_eq!(report.normalized_tag, "followup");
        assert_eq!(report.status_message, "Added #followup");
        assert!(report.refresh_note);
        assert!(backend
            .load_note(&report.note_id)
            .unwrap()
            .body
            .contains("#followup"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn add_tag_workflow_noops_without_note_or_tag() {
        let (mut backend, dir) = backend();
        assert!(add_tag_to_current_note(&mut backend, "", "followup")
            .unwrap()
            .is_none());
        assert!(add_tag_to_current_note(&mut backend, "note-id", " # ")
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn attach_path_workflow_trims_and_reports_attachment() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Note", "# Note\n\n").unwrap();

        let report = attach_path_to_current_note(&mut backend, &note.id, " /tmp/example.pdf ")
            .unwrap()
            .unwrap();

        assert_eq!(report.note_id, note.id);
        assert_eq!(report.path, "/tmp/example.pdf");
        assert_eq!(report.status_message, "Attached");
        assert!(report.refresh_note);
        assert!(backend
            .load_note(&report.note_id)
            .unwrap()
            .body
            .contains("/tmp/example.pdf"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn attach_path_workflow_noops_without_note_or_path() {
        let (mut backend, dir) = backend();
        assert!(
            attach_path_to_current_note(&mut backend, "", "/tmp/example.pdf")
                .unwrap()
                .is_none()
        );
        assert!(attach_path_to_current_note(&mut backend, "note-id", " ")
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    fn backend() -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-note-workflow-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default(),
            TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let mut backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        backend.reindex_all().unwrap();
        (backend, dir)
    }
}
