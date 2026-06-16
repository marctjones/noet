use serde::{Deserialize, Serialize};

use crate::{
    ai::AiStatus,
    workspace::{PaneId, Surface, WorkspaceId},
};
use noet_ai::{AiProposal, HousekeepingJob};

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
    ResizePane {
        pane_id: PaneId,
        size: f32,
    },
    FocusPane(PaneId),
    SetPrimarySurface(Surface),
    SetPaneSurface {
        pane_id: PaneId,
        surface: Surface,
    },
    SetNavigationSearch(String),
    SetFilter {
        dimension: String,
        value: String,
    },
    ClearFilter(String),
    ClearFilters,
    ResolveTask(String),
    CarryOverFollowup(String),
    PromoteTask(String),
    SetAiStatus(AiStatus),
    SetAiProfile(String),
    SetAiEmbeddingProfile(String),
    SetAiMinFreeMemoryPercent(u8),
    SetAiTimeoutSeconds(u64),
    SetAiRuntimeBin(String),
    SetAiModelRoot(String),
    EnqueueAiJob(HousekeepingJob),
    StartAiJob(String),
    StartAiProgress {
        label: String,
        detail: String,
        cancellable: bool,
    },
    UpdateAiProgressDetail(String),
    RequestAiCancel,
    ClearAiProgress,
    CompleteAiJob {
        job_id: String,
        proposal_ids: Vec<String>,
    },
    FailAiJob {
        job_id: String,
        message: String,
    },
    EnqueueAiProposal(AiProposal),
    RejectAiProposal(String),
    DeferAiProposal(String),
    MarkAiProposalAccepted(String),
    ClearResolvedAiProposals,
    InspectAiProposalSource(String),
    InspectAiProposalSourceAt {
        proposal_id: String,
        source_index: usize,
    },
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

    pub fn accepted_with_message(message: impl Into<String>) -> Self {
        Self {
            accepted: true,
            message: Some(message.into()),
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            accepted: false,
            message: Some(message.into()),
        }
    }
}
