//! Noet application model.
//!
//! This crate sits between `noet-core` and GUI frontends. It owns application
//! state that should be testable without Slint: selections, commands,
//! workspaces, panes, surfaces, and workspace presets.

pub mod ai;
pub mod ai_apply;
pub mod ai_housekeeping;
pub mod ai_semantic;
pub mod ai_surface;
pub mod ai_tools;
pub mod ai_workflow;
pub mod command;
pub mod history_workflow;
pub mod model;
pub mod navigation;
pub mod note_workflow;
pub mod selection;
pub mod smart_list_workflow;
pub mod task_workflow;
pub mod workspace;

pub use ai::{
    AiJobId, AiJobStatus, AiProgress, AiSettings, AiState, AiStatus, ProposalId, ProposalStatus,
    QueuedAiJob, QueuedProposal,
};
pub use ai_apply::{apply_ai_proposal, apply_ai_proposal_with_metadata, AiApplyReport};
pub use ai_housekeeping::run_housekeeping_job;
pub use ai_semantic::{
    collect_semantic_contexts, load_semantic_index, refresh_semantic_index, save_semantic_index,
    semantic_index_path, stale_semantic_note_count, SemanticEntry, SemanticIndex,
    SemanticIndexStorage, SemanticMatch, SemanticRefreshPolicy, SemanticRefreshTrigger,
    SemanticStaleSearchBehavior,
};
pub use ai_surface::{ai_surface, AiJobRow, AiProposalRow, AiSurface};
pub use ai_tools::{execute_noet_tool, NoetToolHost};
pub use command::{AppCommand, CommandOutcome};
pub use history_workflow::{
    note_history, note_revision_detail, restore_revision_after, restore_revision_before,
    NoteRevisionDetail, NoteRevisionRow,
};
pub use model::AppModel;
pub use navigation::{FilterToken, NavigationState};
pub use note_workflow::{
    add_tag_to_current_note, add_tag_to_note, archive_note, attach_path_to_current_note,
    attach_path_to_note, create_note, create_note_for_workstream, create_note_from_body,
    create_note_from_template, create_note_from_template_workflow, create_note_in_workstream,
    create_related_note, delete_note, delete_note_and_select_replacement, file_note, restore_note,
    save_note, seed_note_if_vault_empty, select_note, set_note_kind, toggle_note_kind,
    AddTagWorkflowReport, AttachPathWorkflowReport, DeleteNoteWorkflowReport,
    NewNoteWorkflowReport, SelectNoteWorkflowReport, SelectNoteWorkflowRequest,
    TemplateNoteWorkflowReport, TemplateNoteWorkflowRequest, ToggleNoteKindWorkflowReport,
    ToggleNoteKindWorkflowRequest,
};
pub use selection::SelectionState;
pub use smart_list_workflow::{apply_smart_list, delete_smart_list, save_smart_list};
pub use task_workflow::{
    add_task, apply_followup_action, carry_task_to_note, cycle_task, defer_task_to_someday,
    drop_task_on_board, move_task_on_board, promote_task_to_note, reopen_task, resolve_task,
    save_task_form, toggle_task, update_task, CarryTaskReport, FollowupAction,
    FollowupActionReport, PromoteTaskReport, SaveTaskFormReport, TaskFormMode,
};
pub use workspace::{
    Axis, Pane, PaneId, PaneLayout, PanePlacement, PaneRole, PaneSize, Surface, SurfaceId,
    Workspace, WorkspaceId, WorkspacePreset, WorkspaceRegistry,
};
