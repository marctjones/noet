use noet_core::Backend;

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
