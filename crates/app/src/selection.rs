use serde::{Deserialize, Serialize};

/// Domain selection independent of workspace layout.
///
/// Selecting a person, note, task, label, or workstream must not imply that any
/// specific pane is open. Layout is owned by workspaces and panes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionState {
    pub note_id: Option<String>,
    pub person: Option<String>,
    pub task_id: Option<String>,
    pub label: Option<String>,
    pub workstream: Option<String>,
}

impl SelectionState {
    pub fn select_person(&mut self, person: impl Into<String>) {
        self.person = non_empty(person.into());
    }

    pub fn select_note(&mut self, note_id: impl Into<String>) {
        self.note_id = non_empty(note_id.into());
    }

    pub fn select_task(&mut self, task_id: impl Into<String>) {
        self.task_id = non_empty(task_id.into());
    }

    pub fn select_label(&mut self, label: impl Into<String>) {
        self.label = non_empty(label.into());
    }

    pub fn select_workstream(&mut self, workstream: impl Into<String>) {
        self.workstream = non_empty(workstream.into());
    }

    pub fn clear_note(&mut self) {
        self.note_id = None;
    }

    pub fn clear_person(&mut self) {
        self.person = None;
    }

    pub fn clear_task(&mut self) {
        self.task_id = None;
    }
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::SelectionState;

    #[test]
    fn trims_and_clears_selection_values() {
        let mut state = SelectionState::default();
        state.select_person("  Jane Smith  ");
        state.select_note(" note-1 ");
        state.select_task("");

        assert_eq!(state.person.as_deref(), Some("Jane Smith"));
        assert_eq!(state.note_id.as_deref(), Some("note-1"));
        assert_eq!(state.task_id, None);

        state.clear_person();
        assert_eq!(state.person, None);
    }
}
