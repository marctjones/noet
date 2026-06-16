use crate::{
    ai::AiState,
    command::{AppCommand, CommandOutcome},
    navigation::NavigationState,
    selection::SelectionState,
    workspace::{PaneRole, Surface, WorkspaceRegistry},
};
use noet_ai::SourceRef;

#[derive(Clone, Debug, Default)]
pub struct AppModel {
    pub selection: SelectionState,
    pub navigation: NavigationState,
    pub workspaces: WorkspaceRegistry,
    pub ai: AiState,
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
                        if !matches!(primary.surface, Surface::OneOnOne { .. }) {
                            primary.surface = Surface::NoteEditor { note_id };
                        }
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
            AppCommand::SetPaneSurface { pane_id, surface } => {
                with_active_workspace(&mut self.workspaces, |workspace| {
                    if let Some(pane) = workspace.pane_mut(&pane_id) {
                        pane.surface = surface;
                        CommandOutcome::accepted()
                    } else {
                        CommandOutcome::rejected(format!("Unknown pane: {pane_id}"))
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
            AppCommand::ResolveTask(task_id)
            | AppCommand::CarryOverFollowup(task_id)
            | AppCommand::PromoteTask(task_id) => {
                self.selection.select_task(task_id);
                CommandOutcome::accepted()
            }
            AppCommand::SetAiStatus(status) => {
                self.ai.set_status(status);
                CommandOutcome::accepted()
            }
            AppCommand::SetAiProfile(profile_id) => {
                self.ai.set_profile(profile_id);
                CommandOutcome::accepted()
            }
            AppCommand::SetAiEmbeddingProfile(profile_id) => {
                self.ai.set_embedding_profile(profile_id);
                CommandOutcome::accepted()
            }
            AppCommand::SetAiMinFreeMemoryPercent(percent) => {
                self.ai.set_min_free_memory_percent(percent);
                CommandOutcome::accepted()
            }
            AppCommand::SetAiTimeoutSeconds(seconds) => {
                self.ai.set_timeout_seconds(seconds);
                CommandOutcome::accepted()
            }
            AppCommand::SetAiModelRoot(path) => {
                self.ai.set_model_root(path);
                CommandOutcome::accepted()
            }
            AppCommand::EnqueueAiJob(job) => {
                let id = self.ai.enqueue_job(job);
                CommandOutcome::accepted_with_message(id)
            }
            AppCommand::StartAiJob(job_id) => {
                if self.ai.start_job(&job_id) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown AI job: {job_id}"))
                }
            }
            AppCommand::StartAiProgress {
                label,
                detail,
                cancellable,
            } => {
                self.ai.start_progress(label, detail, cancellable);
                CommandOutcome::accepted()
            }
            AppCommand::UpdateAiProgressDetail(detail) => {
                self.ai.update_progress_detail(detail);
                CommandOutcome::accepted()
            }
            AppCommand::RequestAiCancel => {
                if self.ai.request_cancel() {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected("No cancellable AI job is running")
                }
            }
            AppCommand::ClearAiProgress => {
                self.ai.clear_progress();
                CommandOutcome::accepted()
            }
            AppCommand::CompleteAiJob {
                job_id,
                proposal_ids,
            } => {
                if self.ai.complete_job(&job_id, proposal_ids) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown AI job: {job_id}"))
                }
            }
            AppCommand::FailAiJob { job_id, message } => {
                if self.ai.fail_job(&job_id, message) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown AI job: {job_id}"))
                }
            }
            AppCommand::EnqueueAiProposal(proposal) => {
                let id = self.ai.enqueue(proposal);
                CommandOutcome::accepted_with_message(id)
            }
            AppCommand::RejectAiProposal(proposal_id) => {
                if self.ai.reject(&proposal_id) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown AI proposal: {proposal_id}"))
                }
            }
            AppCommand::DeferAiProposal(proposal_id) => {
                if self.ai.defer(&proposal_id) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown AI proposal: {proposal_id}"))
                }
            }
            AppCommand::MarkAiProposalAccepted(proposal_id) => {
                if self.ai.mark_accepted(&proposal_id) {
                    CommandOutcome::accepted()
                } else {
                    CommandOutcome::rejected(format!("Unknown AI proposal: {proposal_id}"))
                }
            }
            AppCommand::ClearResolvedAiProposals => {
                let removed = self.ai.clear_resolved();
                CommandOutcome::accepted_with_message(removed.to_string())
            }
            AppCommand::InspectAiProposalSource(proposal_id) => {
                let source = self.ai.first_source_for(&proposal_id);
                inspect_ai_source(&mut self.selection, source, &proposal_id)
            }
            AppCommand::InspectAiProposalSourceAt {
                proposal_id,
                source_index,
            } => {
                let source = self.ai.source_for(&proposal_id, source_index);
                inspect_ai_source(&mut self.selection, source, &proposal_id)
            }
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

fn inspect_ai_source(
    selection: &mut crate::SelectionState,
    source: Option<SourceRef>,
    proposal_id: &str,
) -> CommandOutcome {
    match source {
        Some(SourceRef::Note { note_id })
        | Some(SourceRef::NoteHeading { note_id, .. })
        | Some(SourceRef::SourceSpan { note_id, .. }) => {
            selection.select_note(note_id);
            selection.clear_task();
            CommandOutcome::accepted()
        }
        Some(SourceRef::Task { task_id }) => {
            selection.select_task(task_id);
            selection.clear_note();
            CommandOutcome::accepted()
        }
        Some(SourceRef::Synthetic { .. }) => {
            CommandOutcome::rejected("AI proposal source is not navigable")
        }
        None => CommandOutcome::rejected(format!(
            "AI proposal has no navigable source: {proposal_id}"
        )),
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
    use crate::{AiJobStatus, AiStatus, AppCommand, AppModel, Surface};
    use noet_ai::HousekeepingJob;
    use noet_ai::{
        AgendaDraft, AgendaItem, AgendaSection, AiProposal, ProposalKind, ProposalPayload,
        ProposalTarget, SourceRef,
    };

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
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("one-on-one-focus".into()))
                .accepted
        );
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
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("one-on-one-focus".into()))
                .accepted
        );
        assert!(model.apply(AppCommand::ClosePane("people".into())).accepted);

        let workspace = model.workspaces.active().unwrap();
        assert!(!workspace.pane("people").unwrap().open);
        assert!(workspace.pane("current-1on1").unwrap().open);
        assert!(model.active_workspace_has_open_work());
    }

    #[test]
    fn one_on_one_meeting_can_close_supporting_panes() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("one-on-one-focus".into()))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::SelectPerson("Jane Smith".into()))
                .accepted
        );
        assert!(model.apply(AppCommand::OpenPane("people".into())).accepted);
        assert!(model.apply(AppCommand::OpenPane("history".into())).accepted);
        assert!(
            model
                .apply(AppCommand::OpenPane("followups".into()))
                .accepted
        );

        assert!(model.apply(AppCommand::ClosePane("people".into())).accepted);
        assert!(
            model
                .apply(AppCommand::ClosePane("history".into()))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::ClosePane("followups".into()))
                .accepted
        );

        let workspace = model.workspaces.active().unwrap();
        assert!(!workspace.pane("people").unwrap().open);
        assert!(!workspace.pane("history").unwrap().open);
        assert!(!workspace.pane("followups").unwrap().open);
        assert!(workspace.pane("current-1on1").unwrap().open);
        assert!(matches!(
            &workspace.pane("current-1on1").unwrap().surface,
            Surface::OneOnOne { person } if person.as_deref() == Some("Jane Smith")
        ));
        assert!(model.active_workspace_has_open_work());
    }

    #[test]
    fn notes_writing_can_close_supporting_panes() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("notes".into()))
                .accepted
        );
        assert!(model.apply(AppCommand::OpenNote("n1".into())).accepted);
        assert!(
            model
                .apply(AppCommand::ClosePane("note-browser".into()))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::ClosePane("note-context".into()))
                .accepted
        );

        let workspace = model.workspaces.active().unwrap();
        assert_eq!(model.selection.note_id.as_deref(), Some("n1"));
        assert!(!workspace.pane("note-browser").unwrap().open);
        assert!(!workspace.pane("note-context").unwrap().open);
        assert!(workspace.pane("note-editor").unwrap().open);
        assert!(matches!(
            &workspace.pane("note-editor").unwrap().surface,
            Surface::NoteEditor { note_id } if note_id.as_deref() == Some("n1")
        ));
        assert!(model.active_workspace_has_open_work());
    }

    #[test]
    fn notes_workspace_is_the_focus_first_default() {
        let model = AppModel::new();
        let workspace = model.workspaces.active().unwrap();

        assert_eq!(workspace.id, "notes");
        assert!(
            !workspace.pane("note-browser").unwrap().open,
            "note browser should be opt-in at launch"
        );
        assert!(
            !workspace.pane("note-context").unwrap().open,
            "full context should be opt-in at launch"
        );
        assert!(
            !workspace.pane("ai-proposals").unwrap().open,
            "queues should not compete with note taking by default"
        );
        assert!(workspace.pane("note-editor").unwrap().open);
        assert!(model.active_workspace_has_open_work());
    }

    #[test]
    fn pane_resize_is_clamped_through_command_dispatch() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("one-on-one-focus".into()))
                .accepted
        );
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

    #[test]
    fn notes_workspace_exposes_ai_proposal_queue_surface() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("notes".into()))
                .accepted
        );

        let workspace = model.workspaces.active().unwrap();
        assert_eq!(workspace.layout.bottom.as_deref(), Some("ai-proposals"));
        assert!(matches!(
            workspace.pane("ai-proposals").unwrap().surface,
            Surface::AiProposalQueue
        ));
    }

    #[test]
    fn pane_surface_can_change_without_switching_workspace() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("one-on-one-focus".into()))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::SetPaneSurface {
                    pane_id: "people".into(),
                    surface: Surface::LabelBrowser,
                })
                .accepted
        );

        let workspace = model.workspaces.active().unwrap();
        assert_eq!(workspace.id, "one-on-one-focus");
        assert_eq!(
            workspace.pane("people").unwrap().surface,
            Surface::LabelBrowser
        );
        assert!(workspace.pane("current-1on1").unwrap().open);
    }

    #[test]
    fn opening_note_does_not_replace_one_on_one_primary_surface() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::SwitchWorkspace("one-on-one-focus".into()))
                .accepted
        );
        assert!(model.apply(AppCommand::OpenNote("n1".into())).accepted);

        let workspace = model.workspaces.active().unwrap();
        assert!(matches!(
            &workspace.pane("current-1on1").unwrap().surface,
            Surface::OneOnOne { .. }
        ));
        assert_eq!(model.selection.note_id.as_deref(), Some("n1"));
    }

    #[test]
    fn task_workflow_commands_select_the_task_without_changing_layout() {
        let mut model = AppModel::new();
        let before = model.workspaces.active().unwrap().clone();

        assert!(
            model
                .apply(AppCommand::PromoteTask("note:12".into()))
                .accepted
        );

        assert_eq!(model.selection.task_id.as_deref(), Some("note:12"));
        assert_eq!(model.workspaces.active().unwrap(), &before);
    }

    #[test]
    fn ai_proposal_can_be_enqueued_and_selected() {
        let mut model = AppModel::new();
        let outcome = model.apply(AppCommand::EnqueueAiProposal(agenda_proposal()));

        assert!(outcome.accepted);
        assert_eq!(outcome.message.as_deref(), Some("ai-proposal-1"));
        assert_eq!(model.ai.pending_count(), 1);
        assert_eq!(
            model.ai.selected_proposal_id.as_deref(),
            Some("ai-proposal-1")
        );
    }

    #[test]
    fn ai_status_can_show_runtime_progress_without_loading_model() {
        let mut model = AppModel::new();

        assert_eq!(model.ai.status, AiStatus::Disabled);
        assert!(
            model
                .apply(AppCommand::SetAiStatus(AiStatus::Ready))
                .accepted
        );
        assert_eq!(model.ai.status, AiStatus::Ready);
        assert!(
            model
                .apply(AppCommand::SetAiStatus(AiStatus::Failed {
                    message: "model file missing".into(),
                }))
                .accepted
        );

        assert_eq!(
            model.ai.status,
            AiStatus::Failed {
                message: "model file missing".into(),
            }
        );
    }

    #[test]
    fn ai_settings_can_select_profile_and_clamp_memory_threshold() {
        let mut model = AppModel::new();

        assert_eq!(
            model.ai.settings.min_free_memory_percent, 50,
            "local AI defaults should leave headroom before loading models"
        );
        assert_eq!(
            model.ai.settings.timeout_seconds, 300,
            "local AI calls should be bounded by default"
        );
        assert!(
            model
                .apply(AppCommand::SetAiProfile("mistral-nemo".into()))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::SetAiMinFreeMemoryPercent(95))
                .accepted
        );
        assert!(
            model.apply(AppCommand::SetAiTimeoutSeconds(3)).accepted,
            "too-low timeouts should be accepted but clamped"
        );

        assert_eq!(model.ai.settings.selected_profile_id, "mistral-nemo");
        assert_eq!(model.ai.settings.min_free_memory_percent, 90);
        assert_eq!(model.ai.settings.timeout_seconds, 30);
    }

    #[test]
    fn ai_settings_can_select_embedding_profile() {
        let mut model = AppModel::new();

        assert!(
            model
                .apply(AppCommand::SetAiEmbeddingProfile(
                    "granite-embedding-30m-english".into(),
                ))
                .accepted
        );

        assert_eq!(
            model.ai.settings.selected_embedding_profile_id,
            "granite-embedding-30m-english"
        );
    }

    #[test]
    fn ai_settings_can_store_local_model_root() {
        let mut model = AppModel::new();

        assert!(
            model
                .apply(AppCommand::SetAiModelRoot(
                    "/Users/marc/.cache/huggingface/hub".into(),
                ))
                .accepted
        );

        assert_eq!(
            model.ai.settings.model_root,
            "/Users/marc/.cache/huggingface/hub"
        );
    }

    #[test]
    fn rejected_ai_proposal_can_be_cleared_without_touching_selection() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::EnqueueAiProposal(agenda_proposal()))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::RejectAiProposal("ai-proposal-1".into()))
                .accepted
        );
        let outcome = model.apply(AppCommand::ClearResolvedAiProposals);

        assert!(outcome.accepted);
        assert_eq!(outcome.message.as_deref(), Some("1"));
        assert_eq!(model.ai.proposals().len(), 0);
        assert_eq!(model.selection.person.as_deref(), None);
    }

    #[test]
    fn ai_housekeeping_jobs_are_visible_and_do_not_change_selection() {
        let mut model = AppModel::new();
        let outcome = model.apply(AppCommand::EnqueueAiJob(
            HousekeepingJob::FindUnlabeledMeetings,
        ));

        assert!(outcome.accepted);
        assert_eq!(outcome.message.as_deref(), Some("ai-job-1"));
        assert_eq!(model.ai.jobs().len(), 1);
        assert_eq!(model.ai.jobs()[0].status, AiJobStatus::Queued);
        assert!(
            model
                .apply(AppCommand::StartAiJob("ai-job-1".into()))
                .accepted
        );
        assert_eq!(model.ai.jobs()[0].status, AiJobStatus::Running);
        assert!(
            model
                .apply(AppCommand::CompleteAiJob {
                    job_id: "ai-job-1".into(),
                    proposal_ids: vec!["ai-proposal-1".into()],
                })
                .accepted
        );

        assert_eq!(model.ai.jobs()[0].status, AiJobStatus::Completed);
        assert_eq!(
            model.ai.jobs()[0].produced_proposals,
            vec!["ai-proposal-1".to_string()]
        );
        assert_eq!(model.selection.note_id, None);
        assert_eq!(model.selection.task_id, None);
    }

    #[test]
    fn failed_ai_housekeeping_job_records_failure_without_selection_side_effects() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::EnqueueAiJob(HousekeepingJob::RefreshEmbeddings))
                .accepted
        );
        assert!(
            model
                .apply(AppCommand::FailAiJob {
                    job_id: "ai-job-1".into(),
                    message: "model unavailable".into(),
                })
                .accepted
        );

        assert_eq!(model.ai.jobs()[0].status, AiJobStatus::Failed);
        assert_eq!(
            model.ai.jobs()[0].failure.as_deref(),
            Some("model unavailable")
        );
        assert_eq!(model.selection.note_id, None);
    }

    #[test]
    fn ai_progress_can_be_started_updated_cancelled_and_cleared() {
        let mut model = AppModel::new();

        assert!(
            model
                .apply(AppCommand::StartAiProgress {
                    label: "Review note".into(),
                    detail: "Loading local model".into(),
                    cancellable: true,
                })
                .accepted
        );
        assert_eq!(
            model
                .ai
                .progress
                .as_ref()
                .map(|progress| progress.label.as_str()),
            Some("Review note")
        );
        assert!(
            model
                .ai
                .progress
                .as_ref()
                .map(|progress| progress.elapsed_seconds() < 2)
                .unwrap_or(false),
            "new progress should expose a fresh elapsed timer"
        );
        assert!(
            model
                .apply(AppCommand::UpdateAiProgressDetail(
                    "Generating proposal".into()
                ))
                .accepted
        );
        assert_eq!(
            model
                .ai
                .progress
                .as_ref()
                .map(|progress| progress.detail.as_str()),
            Some("Generating proposal")
        );
        assert!(model.apply(AppCommand::RequestAiCancel).accepted);
        assert_eq!(
            model
                .ai
                .progress
                .as_ref()
                .map(|progress| progress.cancel_requested),
            Some(true)
        );
        assert!(model.apply(AppCommand::ClearAiProgress).accepted);
        assert_eq!(model.ai.progress, None);
    }

    #[test]
    fn inspecting_ai_proposal_source_updates_selection() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::EnqueueAiProposal(agenda_proposal()))
                .accepted
        );
        let outcome = model.apply(AppCommand::InspectAiProposalSource("ai-proposal-1".into()));

        assert!(outcome.accepted);
        assert_eq!(model.selection.person.as_deref(), None);
        assert_eq!(model.selection.note_id.as_deref(), Some("note-1"));
    }

    #[test]
    fn inspecting_indexed_ai_proposal_source_updates_selection() {
        let mut model = AppModel::new();
        assert!(
            model
                .apply(AppCommand::EnqueueAiProposal(multi_source_agenda_proposal()))
                .accepted
        );
        let outcome = model.apply(AppCommand::InspectAiProposalSourceAt {
            proposal_id: "ai-proposal-1".into(),
            source_index: 1,
        });

        assert!(outcome.accepted);
        assert_eq!(model.selection.note_id.as_deref(), Some("note-2"));
    }

    fn agenda_proposal() -> AiProposal {
        AiProposal {
            kind: ProposalKind::DraftAgenda,
            target: ProposalTarget::Note {
                note_id: "note-1".into(),
            },
            payload: ProposalPayload::DraftAgenda(AgendaDraft {
                person: "Jane Smith".into(),
                sections: Vec::new(),
            }),
            rationale: "Prior follow-ups mention Jane.".into(),
            confidence: 0.9,
            requires_confirmation: false,
        }
    }

    fn multi_source_agenda_proposal() -> AiProposal {
        AiProposal {
            kind: ProposalKind::DraftAgenda,
            target: ProposalTarget::Person {
                name: "Jane Smith".into(),
            },
            payload: ProposalPayload::DraftAgenda(AgendaDraft {
                person: "Jane Smith".into(),
                sections: vec![AgendaSection {
                    title: "Follow-ups".into(),
                    items: vec![
                        AgendaItem {
                            text: "Review note one".into(),
                            sources: vec![SourceRef::Note {
                                note_id: "note-1".into(),
                            }],
                        },
                        AgendaItem {
                            text: "Review note two".into(),
                            sources: vec![SourceRef::Note {
                                note_id: "note-2".into(),
                            }],
                        },
                    ],
                }],
            }),
            rationale: "Prior follow-ups mention Jane.".into(),
            confidence: 0.9,
            requires_confirmation: false,
        }
    }
}
