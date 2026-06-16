use crate::{AiJobStatus, AiState, AiStatus, ProposalStatus};
use noet_ai::{ProposalKind, ProposalPayload};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiSurface {
    pub status: String,
    pub progress_active: bool,
    pub progress_label: String,
    pub progress_detail: String,
    pub progress_elapsed: String,
    pub progress_cancellable: bool,
    pub pending_proposals: usize,
    pub proposals: Vec<AiProposalRow>,
    pub jobs: Vec<AiJobRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiProposalRow {
    pub id: String,
    pub status: String,
    pub kind: String,
    pub target: String,
    pub summary: String,
    pub requires_confirmation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiJobRow {
    pub id: String,
    pub status: String,
    pub kind: String,
    pub produced_proposals: usize,
    pub failure: Option<String>,
}

pub fn ai_surface(state: &AiState) -> AiSurface {
    let progress = state.progress.as_ref();
    AiSurface {
        status: status_label(&state.status).into(),
        progress_active: progress.is_some(),
        progress_label: progress
            .map(|progress| progress.label.clone())
            .unwrap_or_default(),
        progress_detail: progress
            .map(|progress| {
                if progress.cancel_requested {
                    "Cancel requested".to_string()
                } else {
                    progress.detail.clone()
                }
            })
            .unwrap_or_default(),
        progress_elapsed: progress
            .map(|progress| elapsed_label(progress.elapsed_seconds()))
            .unwrap_or_default(),
        progress_cancellable: progress
            .map(|progress| progress.cancellable && !progress.cancel_requested)
            .unwrap_or(false),
        pending_proposals: state.pending_count(),
        proposals: state
            .proposals()
            .iter()
            .map(|entry| AiProposalRow {
                id: entry.id.clone(),
                status: proposal_status_label(&entry.status).into(),
                kind: proposal_kind_label(&entry.proposal.kind).into(),
                target: format!("{:?}", entry.proposal.target),
                summary: proposal_summary(&entry.proposal.payload),
                requires_confirmation: entry.proposal.requires_confirmation,
            })
            .collect(),
        jobs: state
            .jobs()
            .iter()
            .map(|entry| AiJobRow {
                id: entry.id.clone(),
                status: job_status_label(&entry.status).into(),
                kind: format!("{:?}", entry.job),
                produced_proposals: entry.produced_proposals.len(),
                failure: entry.failure.clone(),
            })
            .collect(),
    }
}

fn elapsed_label(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

fn proposal_summary(payload: &ProposalPayload) -> String {
    match payload {
        ProposalPayload::DraftAgenda(draft) => {
            format!("{} agenda sections", draft.sections.len())
        }
        ProposalPayload::ReviewNote(review) => {
            format!("{} findings", review.findings.len())
        }
        ProposalPayload::AddLabels(labels) => {
            format!("{} labels", labels.suggestions.len())
        }
        ProposalPayload::ExtractTasks(tasks) => format!("{} tasks", tasks.tasks.len()),
        ProposalPayload::PromoteTask(task) => task.proposed_title.clone(),
        ProposalPayload::PatchNote(patch) => format!("patch {}", patch.note_id),
        ProposalPayload::ChangeTaskState(change) => format!("{:?}", change.proposed_state),
    }
}

fn proposal_kind_label(kind: &ProposalKind) -> &'static str {
    match kind {
        ProposalKind::DraftAgenda => "Draft agenda",
        ProposalKind::ReviewNote => "Review note",
        ProposalKind::AddLabels => "Add labels",
        ProposalKind::ExtractTasks => "Extract tasks",
        ProposalKind::PromoteTask => "Promote task",
        ProposalKind::PatchNote => "Patch note",
        ProposalKind::ChangeTaskState => "Change task state",
    }
}

fn proposal_status_label(status: &ProposalStatus) -> &'static str {
    match status {
        ProposalStatus::Pending => "Pending",
        ProposalStatus::Accepted => "Accepted",
        ProposalStatus::Rejected => "Rejected",
        ProposalStatus::Deferred => "Deferred",
    }
}

fn job_status_label(status: &AiJobStatus) -> &'static str {
    match status {
        AiJobStatus::Queued => "Queued",
        AiJobStatus::Running => "Running",
        AiJobStatus::Completed => "Completed",
        AiJobStatus::Failed => "Failed",
    }
}

fn status_label(status: &AiStatus) -> &'static str {
    match status {
        AiStatus::Disabled => "Disabled",
        AiStatus::Indexing => "Indexing",
        AiStatus::Ready => "Ready",
        AiStatus::Thinking => "Thinking",
        AiStatus::Proposing => "Proposing",
        AiStatus::Applying => "Applying",
        AiStatus::Failed { .. } => "Failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AiState;
    use noet_ai::{AgendaDraft, AiProposal, HousekeepingJob, ProposalKind, ProposalTarget};

    #[test]
    fn ai_surface_summarizes_proposals_and_jobs() {
        let mut state = AiState::default();
        state.set_status(AiStatus::Ready);
        let proposal_id = state.enqueue(AiProposal {
            kind: ProposalKind::DraftAgenda,
            target: ProposalTarget::Person {
                name: "Jane".into(),
            },
            payload: ProposalPayload::DraftAgenda(AgendaDraft {
                person: "Jane".into(),
                sections: Vec::new(),
            }),
            rationale: "test".into(),
            confidence: 1.0,
            requires_confirmation: false,
        });
        let job_id = state.enqueue_job(HousekeepingJob::FindUnlabeledMeetings);
        assert!(state.complete_job(&job_id, vec![proposal_id]));
        state.start_progress("Review note", "Loading local model", true);

        let surface = ai_surface(&state);

        assert_eq!(surface.status, "Ready");
        assert!(surface.progress_active);
        assert_eq!(surface.progress_label, "Review note");
        assert_eq!(surface.progress_detail, "Loading local model");
        assert!(!surface.progress_elapsed.is_empty());
        assert!(surface.progress_cancellable);
        assert_eq!(surface.pending_proposals, 1);
        assert_eq!(surface.proposals[0].kind, "Draft agenda");
        assert_eq!(surface.jobs[0].status, "Completed");
        assert_eq!(surface.jobs[0].produced_proposals, 1);
    }
}
