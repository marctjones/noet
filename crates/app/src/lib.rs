//! Noet application model.
//!
//! This crate sits between `noet-core` and GUI frontends. It owns application
//! state that should be testable without Slint: selections, commands,
//! workspaces, panes, surfaces, and workspace presets.

pub mod command;
pub mod model;
pub mod navigation;
pub mod selection;
pub mod workspace;

pub use command::{AppCommand, CommandOutcome};
pub use model::AppModel;
pub use navigation::{FilterToken, NavigationState};
pub use selection::SelectionState;
pub use workspace::{
    Axis, Pane, PaneId, PaneLayout, PanePlacement, PaneRole, PaneSize, Surface, SurfaceId,
    Workspace, WorkspaceId, WorkspacePreset, WorkspaceRegistry,
};
