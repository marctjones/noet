use noet_core::{Backend, TodoFields};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromoteTaskReport {
    pub promoted_note_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarryTaskReport {
    pub carried_task_id: String,
}

pub fn resolve_task(backend: &mut Backend, task_id: &str) -> Result<(), String> {
    backend
        .set_todo_status(task_id, "done")
        .map_err(|err| err.to_string())
}

pub fn reopen_task(backend: &mut Backend, task_id: &str) -> Result<(), String> {
    backend
        .set_todo_status(task_id, "todo")
        .map_err(|err| err.to_string())
}

pub fn toggle_task(backend: &mut Backend, task_id: &str) -> Result<(), String> {
    backend.toggle_todo(task_id).map_err(|err| err.to_string())
}

pub fn cycle_task(backend: &mut Backend, task_id: &str) -> Result<(), String> {
    backend.cycle_todo(task_id).map_err(|err| err.to_string())
}

pub fn add_task(
    backend: &mut Backend,
    note_id: &str,
    fields: &TodoFields,
) -> Result<String, String> {
    backend
        .add_todo(note_id, fields)
        .map_err(|err| err.to_string())
}

pub fn update_task(
    backend: &mut Backend,
    task_id: &str,
    fields: &TodoFields,
) -> Result<(), String> {
    backend
        .update_todo(task_id, fields)
        .map_err(|err| err.to_string())
}

pub fn move_task_on_board(
    backend: &mut Backend,
    task_id: &str,
    group_by: &str,
    direction: i32,
) -> Result<(), String> {
    backend
        .board_move(task_id, group_by, direction)
        .map_err(|err| err.to_string())
}

pub fn drop_task_on_board(
    backend: &mut Backend,
    task_id: &str,
    group_by: &str,
    target_key: &str,
) -> Result<(), String> {
    backend
        .drop_card(task_id, group_by, target_key)
        .map_err(|err| err.to_string())
}

pub fn carry_task_to_note(
    backend: &mut Backend,
    task_id: &str,
    target_note_id: &str,
) -> Result<CarryTaskReport, String> {
    let carried_task_id = backend
        .carry_todo_to_note(task_id, target_note_id)
        .map_err(|err| err.to_string())?;
    Ok(CarryTaskReport { carried_task_id })
}

pub fn defer_task_to_someday(backend: &mut Backend, task_id: &str) -> Result<(), String> {
    backend
        .set_todo_kind(task_id, "someday")
        .map_err(|err| err.to_string())
}

pub fn promote_task_to_note(
    backend: &mut Backend,
    task_id: &str,
) -> Result<PromoteTaskReport, String> {
    let note = backend
        .promote_todo_to_note(task_id)
        .map_err(|err| err.to_string())?;
    Ok(PromoteTaskReport {
        promoted_note_id: note.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_core::{Backend, Filter};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn promote_task_routes_through_core_writeback_and_returns_note_id() {
        let (mut backend, dir) =
            backend_with_note("# Note\n\n- [ ] Ask Jane about launch @[[Jane]] #followup\n");
        let task = backend.query_todos(&Filter::default()).unwrap()[0].clone();

        let report = promote_task_to_note(&mut backend, &task.id).unwrap();

        let notes = backend.query_notes(&Filter::default()).unwrap();
        assert!(notes.iter().any(|note| note.id == report.promoted_note_id));
        assert!(notes.iter().any(|note| note.title.contains("Ask Jane")));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn followup_workflows_route_through_core_writeback() {
        let (mut backend, dir) =
            backend_with_note("# Note\n\n- [ ] Ask Jane about launch @[[Jane]] #followup\n");
        let target = backend.new_note().unwrap();
        let task = backend.query_todos(&Filter::default()).unwrap()[0].clone();

        let carried = carry_task_to_note(&mut backend, &task.id, &target.id).unwrap();
        assert!(!carried.carried_task_id.is_empty());
        let carried_task = backend.get_todo(&carried.carried_task_id).unwrap();
        assert_eq!(carried_task.note_id, target.id);
        assert_eq!(backend.get_todo(&task.id).unwrap().status, "done");

        reopen_task(&mut backend, &task.id).unwrap();
        assert_eq!(backend.get_todo(&task.id).unwrap().status, "todo");

        toggle_task(&mut backend, &task.id).unwrap();
        assert_eq!(backend.get_todo(&task.id).unwrap().status, "done");

        defer_task_to_someday(&mut backend, &carried.carried_task_id).unwrap();
        assert_eq!(
            backend.get_todo(&carried.carried_task_id).unwrap().kind,
            "someday"
        );

        resolve_task(&mut backend, &carried.carried_task_id).unwrap();
        assert_eq!(
            backend.get_todo(&carried.carried_task_id).unwrap().status,
            "done"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn task_form_and_board_workflows_route_through_core_writeback() {
        let (mut backend, dir) = backend_with_note("# Note\n\n");
        let note = backend.query_notes(&Filter::default()).unwrap()[0].clone();
        let fields = TodoFields {
            text: "Draft launch note".into(),
            kind: "do".into(),
            status: "todo".into(),
            project: "workstream/clients/acme".into(),
            ..Default::default()
        };

        let task_id = add_task(&mut backend, &note.id, &fields).unwrap();
        assert_eq!(
            backend.get_todo(&task_id).unwrap().text,
            "Draft launch note"
        );

        move_task_on_board(&mut backend, &task_id, "status", 1).unwrap();
        assert_eq!(backend.get_todo(&task_id).unwrap().status, "doing");

        drop_task_on_board(&mut backend, &task_id, "person", "Jane Smith").unwrap();
        assert_eq!(backend.get_todo(&task_id).unwrap().person, "Jane Smith");

        cycle_task(&mut backend, &task_id).unwrap();
        assert_eq!(backend.get_todo(&task_id).unwrap().status, "done");

        let updated = TodoFields {
            text: "Draft final launch note".into(),
            kind: "followup".into(),
            status: "todo".into(),
            person: "Jane Smith".into(),
            due: "2026-06-17".into(),
            ..Default::default()
        };
        update_task(&mut backend, &task_id, &updated).unwrap();
        let task = backend.get_todo(&task_id).unwrap();
        assert_eq!(task.text, "Draft final launch note");
        assert_eq!(task.kind, "followup");
        assert_eq!(task.due, "2026-06-17");
        std::fs::remove_dir_all(dir).ok();
    }

    fn backend_with_note(body: &str) -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-task-workflow-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let notes = dir.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("note.md"), body).unwrap();
        let mut backend = Backend::open(dir.clone()).unwrap();
        backend.reindex_all().unwrap();
        (backend, dir)
    }
}
