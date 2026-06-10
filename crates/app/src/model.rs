use crate::{
    command::{AppCommand, CommandOutcome},
    navigation::NavigationState,
    selection::SelectionState,
    workspace::{PaneRole, Surface, WorkspaceRegistry},
};

#[derive(Clone, Debug, Default)]
pub struct AppModel {
    pub selection: SelectionState,
    pub navigation: NavigationState,
    pub workspaces: WorkspaceRegistry,
}

impl AppModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, command: AppCommand) -> CommandOutcome {
        match command {
            AppCommand::SelectPerson(person) => {
                self.selection.select_person(person);
                let person = self.selection.person.clone();
                if let Some(workspace) = self.workspaces.active_mut() {
                    workspace.update_person_surfaces(person);
                    if workspace
                        .panes
                        .values()
                        .any(|pane| matches!(pane.surface, Surface::OneOnOne { .. }))
                    {
                        workspace.close_navigation_panes();
                    }
                }
                CommandOutcome::accepted()
            }
            AppCommand::OpenNote(note_id) => {
                self.selection.select_note(note_id);
                let note_id = self.selection.note_id.clone();
                if let Some(workspace) = self.workspaces.active_mut() {
                    workspace.update_note_surfaces(note_id.clone());
                    if let Some(primary) = workspace.primary_pane_mut() {
                        primary.surface = Surface::NoteEditor { note_id };
                    }
                }
                CommandOutcome::accepted()
            }
            AppCommand::OpenTask(task_id) => {
                self.selection.select_task(task_id);
                CommandOutcome::accepted()
            }
            AppCommand::SelectLabel(label) => {
                self.selection.select_label(label);
                CommandOutcome::accepted()
            }
            AppCommand::SelectWorkstream(workstream) => {
                self.selection.select_workstream(workstream);
                CommandOutcome::accepted()
            }
            AppCommand::SwitchWorkspace(workspace_id) => {
                if self.workspaces.switch(workspace_id.clone()) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown workspace: {workspace_id}"))
                }
            }
            AppCommand::OpenPane(pane_id) => {
                with_active_workspace(&mut self.workspaces, |workspace| {
                    if workspace.open_pane(&pane_id) {
                        CommandOutcome::accepted()
                    } else {
                        CommandOutcome::rejected(format!("Unknown pane: {pane_id}"))
                    }
                })
            }
            AppCommand::ClosePane(pane_id) => {
                with_active_workspace(&mut self.workspaces, |workspace| {
                    if workspace.close_pane(&pane_id) {
                        CommandOutcome::accepted()
                    } else {
                        CommandOutcome::rejected(format!(
                            "Pane is not closable or unknown: {pane_id}"
                        ))
                    }
                })
            }
            AppCommand::ResizePane { pane_id, size } => {
                with_active_workspace(&mut self.workspaces, |workspace| {
                    if workspace.resize_pane(&pane_id, size) {
                        CommandOutcome::accepted()
                    } else {
                        CommandOutcome::rejected(format!(
                            "Pane is not resizable or unknown: {pane_id}"
                        ))
                    }
                })
            }
            AppCommand::FocusPane(pane_id) => {
                with_active_workspace(&mut self.workspaces, |workspace| {
                    if workspace.focus_pane(pane_id.clone()) {
                        CommandOutcome::accepted()
                    } else {
                        CommandOutcome::rejected(format!("Unknown pane: {pane_id}"))
                    }
                })
            }
            AppCommand::SetPrimarySurface(surface) => {
                with_active_workspace(&mut self.workspaces, |workspace| {
                    if let Some(primary) = workspace.primary_pane_mut() {
                        primary.surface = surface;
                        CommandOutcome::accepted()
                    } else {
                        CommandOutcome::rejected("Active workspace has no primary pane")
                    }
                })
            }
            AppCommand::SetNavigationSearch(search) => {
                self.navigation.set_search(search);
                CommandOutcome::accepted()
            }
            AppCommand::SetFilter { dimension, value } => {
                self.navigation.set_filter(dimension, value);
                CommandOutcome::accepted()
            }
            AppCommand::ClearFilter(dimension) => {
                self.navigation.clear_filter(&dimension);
                CommandOutcome::accepted()
            }
            AppCommand::ClearFilters => {
                self.navigation.clear_filters();
                CommandOutcome::accepted()
            }
            AppCommand::ResolveTask(_task_id)
            | AppCommand::CarryOverFollowup(_task_id)
            | AppCommand::PromoteTask(_task_id) => CommandOutcome::accepted(),
        }
    }

    pub fn active_workspace_has_open_work(&self) -> bool {
        self.workspaces
            .active()
            .map(|workspace| {
                workspace
                    .panes
                    .values()
                    .any(|pane| pane.open && pane.role == PaneRole::Primary)
            })
            .unwrap_or(false)
    }
}

fn with_active_workspace(
    registry: &mut WorkspaceRegistry,
    f: impl FnOnce(&mut crate::workspace::Workspace) -> CommandOutcome,
) -> CommandOutcome {
    if let Some(workspace) = registry.active_mut() {
        f(workspace)
    } else {
        CommandOutcome::rejected("No active workspace")
    }
}

#[cfg(test)]
mod tests {
    use crate::{AppCommand, AppModel, Surface};

    #[test]
    fn selection_state_can_change_without_layout_operations() {
        let mut model = AppModel::new();
        let before = model.workspaces.active().unwrap().clone();

        model.selection.select_person("Jane");

        assert_eq!(model.selection.person.as_deref(), Some("Jane"));
        assert_eq!(model.workspaces.active().unwrap(), &before);
    }

    #[test]
    fn select_person_updates_one_on_one_surfaces_and_closes_navigation() {
        let mut model = AppModel::new();
        let outcome = model.apply(AppCommand::SelectPerson("Jane Smith".into()));
        assert!(outcome.accepted);

        let workspace = model.workspaces.active().unwrap();
        assert_eq!(model.selection.person.as_deref(), Some("Jane Smith"));
        assert!(!workspace.pane("people").unwrap().open);
        assert!(workspace.pane("current-1on1").unwrap().open);
        assert!(matches!(
            &workspace.pane("current-1on1").unwrap().surface,
            Surface::OneOnOne { person } if person.as_deref() == Some("Jane Smith")
        ));
        assert!(matches!(
            &workspace.pane("followups").unwrap().surface,
            Surface::FollowupQueue { person } if person.as_deref() == Some("Jane Smith")
        ));
    }

    #[test]
    fn close_navigation_pane_does_not_close_primary_work() {
        let mut model = AppModel::new();
        assert!(model.apply(AppCommand::ClosePane("people".into())).accepted);

        let workspace = model.workspaces.active().unwrap();
        assert!(!workspace.pane("people").unwrap().open);
        assert!(workspace.pane("current-1on1").unwrap().open);
        assert!(model.active_workspace_has_open_work());
    }

    #[test]
    fn pane_resize_is_clamped_through_command_dispatch() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::ResizePane {
                    pane_id: "people".into(),
                    size: 10.0,
                })
                .accepted
        );

        let workspace = model.workspaces.active().unwrap();
        assert_eq!(workspace.pane("people").unwrap().size.current, 180.0);
    }

    #[test]
    fn switching_workspace_changes_active_preset() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("notes".into()))
                .accepted
        );

        let workspace = model.workspaces.active().unwrap();
        assert_eq!(workspace.id, "notes");
        assert!(matches!(
            &workspace.pane("note-editor").unwrap().surface,
            Surface::NoteEditor { note_id: None }
        ));
    }
}
