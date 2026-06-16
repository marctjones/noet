use noet_ai::{AiProposal, HousekeepingJob, ProposalPayload, ProposalTarget, SourceRef};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub type ProposalId = String;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AiState {
    proposals: Vec<QueuedProposal>,
    jobs: Vec<QueuedAiJob>,
    next_proposal_number: u64,
    next_job_number: u64,
    pub selected_proposal_id: Option<ProposalId>,
    pub status: AiStatus,
    pub progress: Option<AiProgress>,
    pub settings: AiSettings,
}

impl AiState {
    pub fn enqueue(&mut self, proposal: AiProposal) -> ProposalId {
        self.next_proposal_number += 1;
        let id = format!("ai-proposal-{}", self.next_proposal_number);
        self.proposals.push(QueuedProposal {
            id: id.clone(),
            proposal,
            status: ProposalStatus::Pending,
        });
        self.selected_proposal_id = Some(id.clone());
        id
    }

    pub fn proposals(&self) -> &[QueuedProposal] {
        &self.proposals
    }

    pub fn jobs(&self) -> &[QueuedAiJob] {
        &self.jobs
    }

    pub fn enqueue_job(&mut self, job: HousekeepingJob) -> AiJobId {
        self.next_job_number += 1;
        let id = format!("ai-job-{}", self.next_job_number);
        self.jobs.push(QueuedAiJob {
            id: id.clone(),
            job,
            status: AiJobStatus::Queued,
            produced_proposals: Vec::new(),
            failure: None,
        });
        id
    }

    pub fn start_job(&mut self, job_id: &str) -> bool {
        self.set_job_status(job_id, AiJobStatus::Running, None)
    }

    pub fn complete_job(&mut self, job_id: &str, proposal_ids: Vec<ProposalId>) -> bool {
        if let Some(job) = self.jobs.iter_mut().find(|entry| entry.id == job_id) {
            job.status = AiJobStatus::Completed;
            job.produced_proposals = proposal_ids;
            job.failure = None;
            true
        } else {
            false
        }
    }

    pub fn fail_job(&mut self, job_id: &str, message: impl Into<String>) -> bool {
        self.set_job_status(job_id, AiJobStatus::Failed, Some(message.into()))
    }

    pub fn start_progress(
        &mut self,
        label: impl Into<String>,
        detail: impl Into<String>,
        cancellable: bool,
    ) {
        self.progress = Some(AiProgress {
            label: label.into(),
            detail: detail.into(),
            started_unix_millis: current_unix_millis(),
            cancellable,
            cancel_requested: false,
        });
    }

    pub fn update_progress_detail(&mut self, detail: impl Into<String>) {
        if let Some(progress) = &mut self.progress {
            progress.detail = detail.into();
        }
    }

    pub fn request_cancel(&mut self) -> bool {
        if let Some(progress) = &mut self.progress {
            if progress.cancellable {
                progress.cancel_requested = true;
                return true;
            }
        }
        false
    }

    pub fn clear_progress(&mut self) {
        self.progress = None;
    }

    pub fn pending_count(&self) -> usize {
        self.proposals
            .iter()
            .filter(|entry| entry.status == ProposalStatus::Pending)
            .count()
    }

    pub fn set_status(&mut self, status: AiStatus) {
        self.status = status;
    }

    pub fn set_profile(&mut self, profile_id: impl Into<String>) {
        self.settings.selected_profile_id = profile_id.into();
    }

    pub fn set_embedding_profile(&mut self, profile_id: impl Into<String>) {
        self.settings.selected_embedding_profile_id = profile_id.into();
    }

    pub fn set_min_free_memory_percent(&mut self, percent: u8) {
        self.settings.min_free_memory_percent = percent.clamp(10, 90);
    }

    pub fn set_timeout_seconds(&mut self, seconds: u64) {
        self.settings.timeout_seconds = seconds.clamp(30, 1800);
    }

    pub fn set_runtime_bin(&mut self, path: impl Into<String>) {
        self.settings.runtime_bin = path.into();
    }

    pub fn set_model_root(&mut self, path: impl Into<String>) {
        self.settings.model_root = path.into();
    }

    pub fn proposal(&self, proposal_id: &str) -> Option<&QueuedProposal> {
        self.proposals.iter().find(|entry| entry.id == proposal_id)
    }

    pub fn reject(&mut self, proposal_id: &str) -> bool {
        self.set_proposal_status(proposal_id, ProposalStatus::Rejected)
    }

    pub fn defer(&mut self, proposal_id: &str) -> bool {
        self.set_proposal_status(proposal_id, ProposalStatus::Deferred)
    }

    pub fn mark_accepted(&mut self, proposal_id: &str) -> bool {
        self.set_proposal_status(proposal_id, ProposalStatus::Accepted)
    }

    pub fn clear_resolved(&mut self) -> usize {
        let before = self.proposals.len();
        self.proposals
            .retain(|entry| entry.status == ProposalStatus::Pending);
        let removed = before - self.proposals.len();
        if let Some(selected) = &self.selected_proposal_id {
            if self.proposal(selected).is_none() {
                self.selected_proposal_id = self.proposals.first().map(|entry| entry.id.clone());
            }
        }
        removed
    }

    pub fn first_source_for(&self, proposal_id: &str) -> Option<SourceRef> {
        self.source_for(proposal_id, 0)
    }

    pub fn source_for(&self, proposal_id: &str, source_index: usize) -> Option<SourceRef> {
        let proposal = &self.proposal(proposal_id)?.proposal;
        proposal_sources(proposal).into_iter().nth(source_index)
    }

    fn set_proposal_status(&mut self, proposal_id: &str, status: ProposalStatus) -> bool {
        if let Some(entry) = self
            .proposals
            .iter_mut()
            .find(|entry| entry.id == proposal_id)
        {
            entry.status = status;
            self.selected_proposal_id = Some(proposal_id.to_string());
            true
        } else {
            false
        }
    }

    fn set_job_status(
        &mut self,
        job_id: &str,
        status: AiJobStatus,
        failure: Option<String>,
    ) -> bool {
        if let Some(job) = self.jobs.iter_mut().find(|entry| entry.id == job_id) {
            job.status = status;
            job.failure = failure;
            true
        } else {
            false
        }
    }
}

pub type AiJobId = String;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueuedProposal {
    pub id: ProposalId,
    pub proposal: AiProposal,
    pub status: ProposalStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueuedAiJob {
    pub id: AiJobId,
    pub job: HousekeepingJob,
    pub status: AiJobStatus,
    pub produced_proposals: Vec<ProposalId>,
    pub failure: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProgress {
    pub label: String,
    pub detail: String,
    pub started_unix_millis: u64,
    pub cancellable: bool,
    pub cancel_requested: bool,
}

impl AiProgress {
    pub fn elapsed_seconds(&self) -> u64 {
        current_unix_millis().saturating_sub(self.started_unix_millis) / 1_000
    }
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Accepted,
    Rejected,
    Deferred,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiStatus {
    #[default]
    Disabled,
    Indexing,
    Ready,
    Thinking,
    Proposing,
    Applying,
    Failed {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiSettings {
    pub selected_profile_id: String,
    pub selected_embedding_profile_id: String,
    pub min_free_memory_percent: u8,
    pub timeout_seconds: u64,
    pub max_concurrent_model_jobs: u8,
    pub runtime_bin: String,
    pub model_root: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            selected_profile_id: noet_ai::default_profile().id,
            selected_embedding_profile_id: noet_ai::default_embedding_profile().id,
            min_free_memory_percent: 50,
            timeout_seconds: 300,
            max_concurrent_model_jobs: 1,
            runtime_bin: "mistralrs".into(),
            model_root: default_model_root(),
        }
    }
}

fn default_model_root() -> String {
    std::env::var("HF_HOME")
        .map(|home| format!("{home}/hub"))
        .or_else(|_| std::env::var("HOME").map(|home| format!("{home}/.cache/huggingface/hub")))
        .unwrap_or_default()
}

pub(crate) fn proposal_sources(proposal: &AiProposal) -> Vec<SourceRef> {
    let mut sources = payload_sources(&proposal.payload);
    if sources.is_empty() {
        if let Some(source) = target_source(&proposal.target) {
            push_unique_source(&mut sources, source);
        }
    }
    sources
}

fn payload_sources(payload: &ProposalPayload) -> Vec<SourceRef> {
    let mut sources = Vec::new();
    match payload {
        ProposalPayload::DraftAgenda(draft) => {
            for item in draft
                .sections
                .iter()
                .flat_map(|section| section.items.iter())
            {
                for source in &item.sources {
                    push_unique_source(&mut sources, source.clone());
                }
            }
        }
        ProposalPayload::ReviewNote(review) => {
            for source in review
                .findings
                .iter()
                .flat_map(|finding| finding.sources.iter())
            {
                push_unique_source(&mut sources, source.clone());
            }
            for source in review
                .label_suggestions
                .iter()
                .flat_map(|label| label.sources.iter())
            {
                push_unique_source(&mut sources, source.clone());
            }
            for task in &review.task_extractions {
                push_unique_source(&mut sources, task.source.clone());
            }
        }
        ProposalPayload::AddLabels(labels) => {
            for source in labels
                .suggestions
                .iter()
                .flat_map(|label| label.sources.iter())
            {
                push_unique_source(&mut sources, source.clone());
            }
        }
        ProposalPayload::ExtractTasks(tasks) => {
            for task in &tasks.tasks {
                push_unique_source(&mut sources, task.source.clone());
            }
        }
        ProposalPayload::PromoteTask(task) => {
            push_unique_source(&mut sources, task.source.clone());
        }
        ProposalPayload::PatchNote(patch) => {
            for source in &patch.sources {
                push_unique_source(&mut sources, source.clone());
            }
        }
        ProposalPayload::ChangeTaskState(change) => {
            push_unique_source(&mut sources, change.source.clone());
        }
    }
    sources
}

fn target_source(target: &ProposalTarget) -> Option<SourceRef> {
    match target {
        ProposalTarget::Note { note_id } => Some(SourceRef::Note {
            note_id: note_id.clone(),
        }),
        ProposalTarget::Task { task_id } => Some(SourceRef::Task {
            task_id: task_id.clone(),
        }),
        ProposalTarget::Person { .. } | ProposalTarget::Vault => None,
    }
}

fn push_unique_source(sources: &mut Vec<SourceRef>, source: SourceRef) {
    if !sources.contains(&source) {
        sources.push(source);
    }
}
