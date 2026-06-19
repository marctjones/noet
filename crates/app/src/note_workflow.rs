use crate::command::AppCommand;
use noet_core::{backend as core_backend, Backend, Filter, Note};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteNoteWorkflowReport {
    pub deleted_note_id: String,
    pub replacement_note_id: Option<String>,
    pub status_message: String,
    pub open_in_edit_mode: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToggleNoteKindWorkflowRequest {
    pub note_id: String,
    pub current_kind: String,
    pub current_title: String,
    pub current_body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToggleNoteKindWorkflowReport {
    pub note_id: String,
    pub new_kind: String,
    pub rendered_kind: String,
    pub status_message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplateNoteWorkflowRequest {
    pub template: String,
    pub selected_person: String,
    pub filter_person: String,
    pub started_in_workspace: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateNoteWorkflowReport {
    pub note_id: String,
    pub open_command: AppCommand,
    pub followup_commands: Vec<AppCommand>,
    pub view: String,
    pub status_message: String,
    pub open_in_edit_mode: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreNoteWorkflowReport {
    pub filename: String,
    pub status_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveNoteWorkflowReport {
    pub note_id: String,
    pub archived: bool,
    pub status_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileNoteWorkflowReport {
    pub note_id: String,
    pub topic: String,
    pub status_message: String,
    pub refresh_note: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickCaptureWorkflowReport {
    pub note_id: String,
    pub title: String,
    pub status_message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedNoteWorkflowReport {
    pub source_note_id: String,
    pub note_id: String,
    pub view: String,
    pub status_message: String,
    pub open_in_edit_mode: bool,
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

pub fn quick_capture_note(
    backend: &mut Backend,
    text: &str,
) -> Result<Option<QuickCaptureWorkflowReport>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }

    let title: String = text.chars().take(60).collect();
    let note = create_note_from_body(backend, &title, &format!("{text}\n"))?;
    Ok(Some(QuickCaptureWorkflowReport {
        note_id: note.id,
        title,
        status_message: "Captured to inbox".into(),
    }))
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

pub fn create_note_from_template_workflow(
    backend: &mut Backend,
    request: TemplateNoteWorkflowRequest,
) -> Result<TemplateNoteWorkflowReport, String> {
    let template = request.template.trim();
    let selected_person = request.selected_person.trim();
    let filter_person = request.filter_person.trim();
    let person = if selected_person.is_empty() {
        filter_person
    } else {
        selected_person
    };
    let note = create_note_from_template(backend, template, Some(person))?;
    let note_id = note.id;
    let mut followup_commands = Vec::new();
    if template == "oneonone" {
        if !selected_person.is_empty() {
            followup_commands.push(AppCommand::SelectPerson(selected_person.into()));
        }
        followup_commands.push(AppCommand::SwitchWorkspace("one-on-one-focus".into()));
    } else if request.started_in_workspace {
        followup_commands.push(AppCommand::SwitchWorkspace("notes".into()));
    }
    let view = if request.started_in_workspace {
        "workspace"
    } else if template == "oneonone" {
        "oneonone"
    } else {
        "notes"
    };

    Ok(TemplateNoteWorkflowReport {
        open_command: AppCommand::OpenNote(note_id.clone()),
        note_id,
        followup_commands,
        view: view.into(),
        status_message: "New note from template".into(),
        open_in_edit_mode: true,
    })
}

pub fn create_related_note(backend: &mut Backend, source_note_id: &str) -> Result<Note, String> {
    backend
        .new_related_note(source_note_id)
        .map_err(|err| err.to_string())
}

pub fn create_related_note_workflow(
    backend: &mut Backend,
    source_note_id: &str,
) -> Result<Option<RelatedNoteWorkflowReport>, String> {
    let source_note_id = source_note_id.trim();
    if source_note_id.is_empty() {
        return Ok(None);
    }

    let note = create_related_note(backend, source_note_id)?;
    Ok(Some(RelatedNoteWorkflowReport {
        source_note_id: source_note_id.into(),
        note_id: note.id,
        view: "notes".into(),
        status_message: "New related note".into(),
        open_in_edit_mode: true,
    }))
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

pub fn file_current_note(
    backend: &mut Backend,
    note_id: &str,
    topic: &str,
) -> Result<Option<FileNoteWorkflowReport>, String> {
    let note_id = note_id.trim();
    let topic = topic.trim();
    if note_id.is_empty() || topic.is_empty() {
        return Ok(None);
    }

    file_note(backend, note_id, topic)?;
    Ok(Some(FileNoteWorkflowReport {
        note_id: note_id.into(),
        topic: topic.into(),
        status_message: format!("Filed into {topic}"),
        refresh_note: true,
    }))
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

pub fn delete_note_and_select_replacement(
    backend: &mut Backend,
    note_id: &str,
) -> Result<Option<DeleteNoteWorkflowReport>, String> {
    let note_id = note_id.trim();
    if note_id.is_empty() {
        return Ok(None);
    }

    delete_note(backend, note_id)?;
    let replacement_note_id = backend
        .query_notes(&Filter::default())
        .map_err(|err| err.to_string())?
        .into_iter()
        .next()
        .map(|note| note.id);

    Ok(Some(DeleteNoteWorkflowReport {
        deleted_note_id: note_id.into(),
        replacement_note_id,
        status_message: "Moved to trash".into(),
        open_in_edit_mode: false,
    }))
}

pub fn restore_note(backend: &mut Backend, filename: &str) -> Result<(), String> {
    backend
        .restore_note(filename)
        .map_err(|err| err.to_string())
}

pub fn restore_note_workflow(
    backend: &mut Backend,
    filename: &str,
) -> Result<Option<RestoreNoteWorkflowReport>, String> {
    let filename = filename.trim();
    if filename.is_empty() {
        return Ok(None);
    }

    restore_note(backend, filename)?;
    Ok(Some(RestoreNoteWorkflowReport {
        filename: filename.into(),
        status_message: "Restored from trash".into(),
    }))
}

pub fn archive_note(backend: &mut Backend, note_id: &str, archive: bool) -> Result<(), String> {
    backend
        .archive_note(note_id, archive)
        .map_err(|err| err.to_string())
}

pub fn archive_note_workflow(
    backend: &mut Backend,
    note_id: &str,
    archive: bool,
) -> Result<Option<ArchiveNoteWorkflowReport>, String> {
    let note_id = note_id.trim();
    if note_id.is_empty() {
        return Ok(None);
    }

    archive_note(backend, note_id, archive)?;
    Ok(Some(ArchiveNoteWorkflowReport {
        note_id: note_id.into(),
        archived: archive,
        status_message: if archive { "Archived" } else { "Unarchived" }.into(),
    }))
}

pub fn set_note_kind(backend: &mut Backend, note_id: &str, kind: &str) -> Result<(), String> {
    backend
        .set_note_kind(note_id, kind)
        .map_err(|err| err.to_string())
}

pub fn toggle_note_kind(
    backend: &mut Backend,
    request: ToggleNoteKindWorkflowRequest,
) -> Result<Option<ToggleNoteKindWorkflowReport>, String> {
    let note_id = request.note_id.trim();
    if note_id.is_empty() {
        return Ok(None);
    }

    let new_kind = match request.current_kind.trim() {
        "auto" => "markdown",
        "markdown" => "typst",
        _ => "auto",
    };
    save_note(
        backend,
        note_id,
        &request.current_title,
        &request.current_body,
    )?;
    set_note_kind(backend, note_id, new_kind)?;
    let detected = core_backend::detect_kind(&request.current_body);
    let rendered_kind = if new_kind == "auto" {
        detected
    } else {
        new_kind
    };

    Ok(Some(ToggleNoteKindWorkflowReport {
        note_id: note_id.into(),
        new_kind: new_kind.into(),
        rendered_kind: rendered_kind.into(),
        status_message: format!("Mode: {new_kind} (renders as {rendered_kind})"),
    }))
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
    fn quick_capture_workflow_creates_trimmed_inbox_note() {
        let (mut backend, dir) = backend();

        let report = quick_capture_note(&mut backend, "  Capture this thought  ")
            .unwrap()
            .unwrap();

        assert_eq!(report.title, "Capture this thought");
        assert_eq!(report.status_message, "Captured to inbox");
        let note = backend.load_note(&report.note_id).unwrap();
        assert_eq!(note.title, "Capture this thought");
        assert_eq!(note.body, "Capture this thought\n");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quick_capture_workflow_truncates_long_titles() {
        let (mut backend, dir) = backend();
        let text = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 extra";

        let report = quick_capture_note(&mut backend, text).unwrap().unwrap();

        assert_eq!(report.title.chars().count(), 60);
        assert!(backend
            .load_note(&report.note_id)
            .unwrap()
            .body
            .contains(" extra"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn quick_capture_workflow_noops_without_text() {
        let (mut backend, dir) = backend();

        assert!(quick_capture_note(&mut backend, " ").unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn related_note_workflow_reports_new_note_and_preserves_backlink() {
        let (mut backend, dir) = backend();
        let source = create_note_from_body(&mut backend, "Source", "# Source\n\n").unwrap();

        let report = create_related_note_workflow(&mut backend, &source.id)
            .unwrap()
            .unwrap();

        assert_eq!(report.source_note_id, source.id);
        assert_eq!(report.view, "notes");
        assert_eq!(report.status_message, "New related note");
        assert!(report.open_in_edit_mode);
        let related = backend.load_note(&report.note_id).unwrap();
        assert!(related.body.contains("[[Source]]"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn related_note_workflow_noops_without_source_note() {
        let (mut backend, dir) = backend();

        assert!(create_related_note_workflow(&mut backend, " ")
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn template_workflow_builds_one_on_one_commands_and_body() {
        let (mut backend, dir) = backend();

        let report = create_note_from_template_workflow(
            &mut backend,
            TemplateNoteWorkflowRequest {
                template: "oneonone".into(),
                selected_person: "Jane Smith".into(),
                filter_person: "Ignored Person".into(),
                started_in_workspace: false,
            },
        )
        .unwrap();

        assert_eq!(
            report.open_command,
            AppCommand::OpenNote(report.note_id.clone())
        );
        assert_eq!(
            report.followup_commands,
            vec![
                AppCommand::SelectPerson("Jane Smith".into()),
                AppCommand::SwitchWorkspace("one-on-one-focus".into())
            ]
        );
        assert_eq!(report.view, "oneonone");
        assert_eq!(report.status_message, "New note from template");
        assert!(report.open_in_edit_mode);
        let note = backend.load_note(&report.note_id).unwrap();
        assert!(note.body.contains("@[[Jane Smith]]"));
        assert!(!note.body.contains("Ignored Person"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn template_workflow_preserves_workspace_view_for_workspace_launch() {
        let (mut backend, dir) = backend();

        let report = create_note_from_template_workflow(
            &mut backend,
            TemplateNoteWorkflowRequest {
                template: "meeting".into(),
                selected_person: "".into(),
                filter_person: "Jane Smith".into(),
                started_in_workspace: true,
            },
        )
        .unwrap();

        assert_eq!(
            report.open_command,
            AppCommand::OpenNote(report.note_id.clone())
        );
        assert_eq!(
            report.followup_commands,
            vec![AppCommand::SwitchWorkspace("notes".into())]
        );
        assert_eq!(report.view, "workspace");
        assert!(report.open_in_edit_mode);
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
    fn file_note_workflow_trims_topic_and_reports_status() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Note", "# Note\n\n").unwrap();

        let report = file_current_note(&mut backend, &note.id, " clients/acme ")
            .unwrap()
            .unwrap();

        assert_eq!(report.note_id, note.id);
        assert_eq!(report.topic, "clients/acme");
        assert_eq!(report.status_message, "Filed into clients/acme");
        assert!(report.refresh_note);
        assert!(backend
            .load_note(&report.note_id)
            .unwrap()
            .body
            .contains("#workstream/clients/acme"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn file_note_workflow_noops_without_note_or_topic() {
        let (mut backend, dir) = backend();
        assert!(file_current_note(&mut backend, "", "clients/acme")
            .unwrap()
            .is_none());
        assert!(file_current_note(&mut backend, "note-id", " ")
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

    #[test]
    fn delete_note_workflow_returns_replacement_note() {
        let (mut backend, dir) = backend();
        let deleted = create_note_from_body(&mut backend, "Delete", "# Delete\n\n").unwrap();
        let survivor = create_note_from_body(&mut backend, "Keep", "# Keep\n\n").unwrap();

        let report = delete_note_and_select_replacement(&mut backend, &deleted.id)
            .unwrap()
            .unwrap();

        assert_eq!(report.deleted_note_id, deleted.id);
        assert_eq!(report.replacement_note_id, Some(survivor.id.clone()));
        assert_eq!(report.status_message, "Moved to trash");
        assert!(!report.open_in_edit_mode);
        assert!(!backend
            .query_notes(&Filter::default())
            .unwrap()
            .iter()
            .any(|note| note.id == deleted.id));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn delete_note_workflow_clears_when_no_replacement_exists() {
        let (mut backend, dir) = backend();
        let deleted = create_note_from_body(&mut backend, "Delete", "# Delete\n\n").unwrap();

        let report = delete_note_and_select_replacement(&mut backend, &deleted.id)
            .unwrap()
            .unwrap();

        assert_eq!(report.deleted_note_id, deleted.id);
        assert_eq!(report.replacement_note_id, None);
        assert!(!report.open_in_edit_mode);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn delete_note_workflow_noops_without_note_id() {
        let (mut backend, dir) = backend();

        assert!(delete_note_and_select_replacement(&mut backend, " ")
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn restore_note_workflow_reports_restored_file() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Restore", "# Restore\n\n").unwrap();
        delete_note(&mut backend, &note.id).unwrap();
        let filename = backend.list_trash().unwrap()[0].0.clone();

        let report = restore_note_workflow(&mut backend, &filename)
            .unwrap()
            .unwrap();

        assert_eq!(report.filename, filename);
        assert_eq!(report.status_message, "Restored from trash");
        assert!(backend
            .query_notes(&Filter::default())
            .unwrap()
            .iter()
            .any(|candidate| candidate.id == note.id));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn restore_note_workflow_noops_without_filename() {
        let (mut backend, dir) = backend();

        assert!(restore_note_workflow(&mut backend, " ").unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn archive_note_workflow_reports_archive_state() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Archive", "# Archive\n\n").unwrap();

        let archived = archive_note_workflow(&mut backend, &note.id, true)
            .unwrap()
            .unwrap();

        assert_eq!(archived.note_id, note.id);
        assert!(archived.archived);
        assert_eq!(archived.status_message, "Archived");
        assert!(backend
            .query_notes(&Filter {
                show_archived: true,
                ..Default::default()
            })
            .unwrap()
            .iter()
            .any(|candidate| candidate.id == archived.note_id));

        let unarchived = archive_note_workflow(&mut backend, &note.id, false)
            .unwrap()
            .unwrap();
        assert!(!unarchived.archived);
        assert_eq!(unarchived.status_message, "Unarchived");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn archive_note_workflow_noops_without_note_id() {
        let (mut backend, dir) = backend();

        assert!(archive_note_workflow(&mut backend, " ", true)
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn toggle_note_kind_workflow_saves_body_and_reports_render_mode() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Note", "# Note\n\nOld\n").unwrap();

        let report = toggle_note_kind(
            &mut backend,
            ToggleNoteKindWorkflowRequest {
                note_id: note.id.clone(),
                current_kind: "auto".into(),
                current_title: "Note".into(),
                current_body: "# Note\n\nUpdated\n".into(),
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(report.note_id, note.id);
        assert_eq!(report.new_kind, "markdown");
        assert_eq!(report.rendered_kind, "markdown");
        assert_eq!(
            report.status_message,
            "Mode: markdown (renders as markdown)"
        );
        let saved = backend.load_note(&report.note_id).unwrap();
        assert_eq!(saved.kind, "markdown");
        assert!(saved.body.contains("Updated"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn toggle_note_kind_workflow_auto_uses_detected_render_kind() {
        let (mut backend, dir) = backend();
        let note = create_note_from_body(&mut backend, "Note", "#import \"x.typ\"\n").unwrap();

        let report = toggle_note_kind(
            &mut backend,
            ToggleNoteKindWorkflowRequest {
                note_id: note.id.clone(),
                current_kind: "typst".into(),
                current_title: "Note".into(),
                current_body: "#import \"x.typ\"\n".into(),
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(report.new_kind, "auto");
        assert_eq!(report.rendered_kind, "typst");
        assert_eq!(report.status_message, "Mode: auto (renders as typst)");
        assert_eq!(backend.load_note(&note.id).unwrap().kind, "auto");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn toggle_note_kind_workflow_noops_without_note_id() {
        let (mut backend, dir) = backend();

        assert!(toggle_note_kind(
            &mut backend,
            ToggleNoteKindWorkflowRequest {
                note_id: " ".into(),
                current_kind: "auto".into(),
                current_title: "Note".into(),
                current_body: "# Note\n".into(),
            },
        )
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
