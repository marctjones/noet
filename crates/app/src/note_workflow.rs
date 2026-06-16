use noet_core::{Backend, Note};

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
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn backend() -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-note-workflow-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let mut backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        backend.reindex_all().unwrap();
        (backend, dir)
    }
}
