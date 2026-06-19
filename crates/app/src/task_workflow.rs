use noet_core::{Backend, TodoFields};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromoteTaskReport {
    pub promoted_note_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CarryTaskReport {
    pub carried_task_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FollowupAction {
    Resolve,
    CarryToCurrentNote { target_note_id: String },
    DeferToSomeday,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowupActionReport {
    pub status_message: String,
    pub refresh_note_id: Option<String>,
    pub carried_task_id: Option<String>,
}

pub fn apply_followup_action(
    backend: &mut Backend,
    task_id: &str,
    action: FollowupAction,
    current_note_id: Option<&str>,
) -> Result<FollowupActionReport, String> {
    let current_note_id = current_note_id
        .map(str::trim)
        .filter(|note_id| !note_id.is_empty())
        .map(str::to_string);
    match action {
        FollowupAction::Resolve => {
            resolve_task(backend, task_id)?;
            Ok(FollowupActionReport {
                status_message: "Follow-up resolved".into(),
                refresh_note_id: current_note_id,
                carried_task_id: None,
            })
        }
        FollowupAction::CarryToCurrentNote { target_note_id } => {
            let target_note_id = target_note_id.trim();
            if target_note_id.is_empty() {
                return Err("Open a 1:1 note first.".into());
            }
            let carried = carry_task_to_note(backend, task_id, target_note_id)?;
            Ok(FollowupActionReport {
                status_message: "Follow-up carried into the current 1:1".into(),
                refresh_note_id: Some(target_note_id.into()),
                carried_task_id: Some(carried.carried_task_id),
            })
        }
        FollowupAction::DeferToSomeday => {
            defer_task_to_someday(backend, task_id)?;
            Ok(FollowupActionReport {
                status_message: "Follow-up deferred to someday".into(),
                refresh_note_id: current_note_id,
                carried_task_id: None,
            })
        }
    }
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    fn followup_action_workflow_reports_refresh_and_status() {
        let (mut backend, dir) =
            backend_with_note("# Prior\n\n- [ ] review budget @[[Jane]] #followup\n");
        let current = backend.new_note().unwrap();
        let task = backend.query_todos(&Filter::default()).unwrap()[0].clone();

        let carried = apply_followup_action(
            &mut backend,
            &task.id,
            FollowupAction::CarryToCurrentNote {
                target_note_id: current.id.clone(),
            },
            Some(&current.id),
        )
        .unwrap();
        assert_eq!(
            carried.status_message,
            "Follow-up carried into the current 1:1"
        );
        assert_eq!(
            carried.refresh_note_id.as_deref(),
            Some(current.id.as_str())
        );
        let carried_task_id = carried.carried_task_id.unwrap();
        assert_eq!(backend.get_todo(&task.id).unwrap().status, "done");
        assert_eq!(
            backend.get_todo(&carried_task_id).unwrap().note_id,
            current.id
        );

        let resolved = apply_followup_action(
            &mut backend,
            &task.id,
            FollowupAction::Resolve,
            Some(&current.id),
        )
        .unwrap();
        assert_eq!(resolved.status_message, "Follow-up resolved");
        assert_eq!(
            resolved.refresh_note_id.as_deref(),
            Some(current.id.as_str())
        );

        let deferred = apply_followup_action(
            &mut backend,
            &carried_task_id,
            FollowupAction::DeferToSomeday,
            Some(&current.id),
        )
        .unwrap();
        assert_eq!(deferred.status_message, "Follow-up deferred to someday");
        assert_eq!(backend.get_todo(&carried_task_id).unwrap().kind, "someday");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn carry_followup_action_requires_current_note() {
        let (mut backend, dir) =
            backend_with_note("# Prior\n\n- [ ] review budget @[[Jane]] #followup\n");
        let task = backend.query_todos(&Filter::default()).unwrap()[0].clone();

        let err = apply_followup_action(
            &mut backend,
            &task.id,
            FollowupAction::CarryToCurrentNote {
                target_note_id: String::new(),
            },
            None,
        )
        .unwrap_err();

        assert_eq!(err, "Open a 1:1 note first.");
        assert_eq!(backend.get_todo(&task.id).unwrap().status, "todo");
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
            "noet-task-workflow-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default(),
            TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let notes = dir.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("note.md"), body).unwrap();
        let mut backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        backend.reindex_all().unwrap();
        (backend, dir)
    }
}
