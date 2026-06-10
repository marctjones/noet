use serde::{Deserialize, Serialize};

use crate::workspace::{PaneId, Surface, WorkspaceId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AppCommand {
    SelectPerson(String),
    OpenNote(String),
    OpenTask(String),
    SelectLabel(String),
    SelectWorkstream(String),
    SwitchWorkspace(WorkspaceId),
    OpenPane(PaneId),
    ClosePane(PaneId),
    ResizePane { pane_id: PaneId, size: f32 },
    FocusPane(PaneId),
    SetPrimarySurface(Surface),
    SetPaneSurface { pane_id: PaneId, surface: Surface },
    SetNavigationSearch(String),
    SetFilter { dimension: String, value: String },
    ClearFilter(String),
    ClearFilters,
    ResolveTask(String),
    CarryOverFollowup(String),
    PromoteTask(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandOutcome {
    pub accepted: bool,
    pub message: Option<String>,
}

impl CommandOutcome {
    pub fn accepted() -> Self {
        Self {
            accepted: true,
            message: None,
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            accepted: false,
            message: Some(message.into()),
        }
    }
}
