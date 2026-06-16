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
pub mod ai_workflow;
pub mod command;
pub mod model;
pub mod navigation;
pub mod note_workflow;
pub mod selection;
pub mod task_workflow;
pub mod workspace;

pub use ai::{
    AiJobId, AiJobStatus, AiProgress, AiSettings, AiState, AiStatus, ProposalId, ProposalStatus,
    QueuedAiJob, QueuedProposal,
};
pub use ai_apply::{apply_ai_proposal, AiApplyReport};
pub use ai_housekeeping::run_housekeeping_job;
pub use ai_semantic::{
    SemanticEntry, SemanticIndex, SemanticIndexStorage, SemanticMatch, SemanticRefreshPolicy,
    SemanticRefreshTrigger, SemanticStaleSearchBehavior,
};
pub use ai_surface::{ai_surface, AiJobRow, AiProposalRow, AiSurface};
pub use command::{AppCommand, CommandOutcome};
pub use model::AppModel;
pub use navigation::{FilterToken, NavigationState};
pub use note_workflow::{
    add_tag_to_note, archive_note, attach_path_to_note, create_note, create_note_from_body,
    create_note_from_template, create_note_in_workstream, create_related_note, delete_note,
    file_note, restore_note, save_note, set_note_kind,
};
pub use selection::SelectionState;
pub use task_workflow::{
    add_task, carry_task_to_note, cycle_task, defer_task_to_someday, drop_task_on_board,
    move_task_on_board, promote_task_to_note, reopen_task, resolve_task, toggle_task, update_task,
    CarryTaskReport, PromoteTaskReport,
};
pub use workspace::{
    Axis, Pane, PaneId, PaneLayout, PanePlacement, PaneRole, PaneSize, Surface, SurfaceId,
    Workspace, WorkspaceId, WorkspacePreset, WorkspaceRegistry,
};
