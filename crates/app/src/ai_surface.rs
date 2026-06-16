use crate::{ai::proposal_sources, AiJobStatus, AiState, AiStatus, ProposalStatus};
use noet_ai::{ProposalKind, ProposalPayload, ProposalTarget, SourceRef};

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
    pub preview: String,
    pub source: String,
    pub source_rows: Vec<AiProposalSourceRow>,
    pub rationale: String,
    pub confidence: String,
    pub requires_confirmation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiProposalSourceRow {
    pub label: String,
    pub navigable: bool,
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
            .map(|entry| {
                let source_rows = proposal_source_rows(&entry.proposal);
                AiProposalRow {
                    id: entry.id.clone(),
                    status: proposal_status_label(&entry.status).into(),
                    kind: proposal_kind_label(&entry.proposal.kind).into(),
                    target: target_label(&entry.proposal.target),
                    summary: proposal_summary(&entry.proposal.payload),
                    preview: proposal_preview(&entry.proposal.payload),
                    source: proposal_source_summary(&source_rows),
                    source_rows,
                    rationale: entry.proposal.rationale.clone(),
                    confidence: confidence_label(entry.proposal.confidence),
                    requires_confirmation: entry.proposal.requires_confirmation,
                }
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

fn proposal_preview(payload: &ProposalPayload) -> String {
    let lines = match payload {
        ProposalPayload::DraftAgenda(draft) => draft
            .sections
            .iter()
            .flat_map(|section| {
                section.items.iter().map(move |item| {
                    if section.title.trim().is_empty() {
                        item.text.clone()
                    } else {
                        format!("{}: {}", section.title, item.text)
                    }
                })
            })
            .collect::<Vec<_>>(),
        ProposalPayload::ReviewNote(review) => {
            let mut lines = review
                .findings
                .iter()
                .map(|finding| format!("{:?}: {}", finding.kind, finding.text))
                .collect::<Vec<_>>();
            lines.extend(review.label_suggestions.iter().map(|label| {
                format!("#{}: {}", label.label.trim_start_matches('#'), label.reason)
            }));
            lines.extend(
                review
                    .task_extractions
                    .iter()
                    .map(|task| format!("Task: {}", task.text)),
            );
            lines
        }
        ProposalPayload::AddLabels(labels) => labels
            .suggestions
            .iter()
            .map(|label| format!("#{}: {}", label.label.trim_start_matches('#'), label.reason))
            .collect(),
        ProposalPayload::ExtractTasks(tasks) => tasks
            .tasks
            .iter()
            .map(|task| {
                let mut parts = vec![task.text.clone()];
                if let Some(person) = &task.person {
                    if !person.trim().is_empty() {
                        parts.push(format!("@{person}"));
                    }
                }
                if let Some(due) = &task.due {
                    if !due.trim().is_empty() {
                        parts.push(format!("due:{due}"));
                    }
                }
                parts.extend(task.labels.iter().map(|label| {
                    if label.starts_with('#') {
                        label.clone()
                    } else {
                        format!("#{label}")
                    }
                }));
                parts.join(" ")
            })
            .collect(),
        ProposalPayload::PromoteTask(task) => {
            let body = first_content_line(&task.proposed_body).unwrap_or_default();
            vec![format!("{} {}", task.proposed_title, body)
                .trim()
                .to_string()]
        }
        ProposalPayload::PatchNote(patch) => {
            let first = first_content_line(&patch.patch).unwrap_or_else(|| "Patch proposal".into());
            vec![format!("{}: {first}", patch.note_id)]
        }
        ProposalPayload::ChangeTaskState(change) => {
            vec![format!("{:?}: {}", change.proposed_state, change.task_id)]
        }
    };
    compact_join(lines, "No preview available")
}

fn proposal_source_summary(sources: &[AiProposalSourceRow]) -> String {
    compact_join(
        sources.iter().map(|source| source.label.clone()).collect(),
        "No source",
    )
}

fn proposal_source_rows(proposal: &noet_ai::AiProposal) -> Vec<AiProposalSourceRow> {
    proposal_sources(proposal)
        .into_iter()
        .map(|source| AiProposalSourceRow {
            label: source_label(&source),
            navigable: !matches!(source, SourceRef::Synthetic { .. }),
        })
        .collect()
}

fn source_label(source: &SourceRef) -> String {
    match source {
        SourceRef::Note { note_id } => format!("Note {note_id}"),
        SourceRef::Task { task_id } => format!("Task {task_id}"),
        SourceRef::NoteHeading { note_id, heading } => format!("Note {note_id} / {heading}"),
        SourceRef::SourceSpan {
            note_id,
            start,
            end,
        } => format!("Note {note_id} lines {start}-{end}"),
        SourceRef::Synthetic { label } => label.clone(),
    }
}

fn target_label(target: &ProposalTarget) -> String {
    match target {
        ProposalTarget::Note { note_id } => format!("Note {note_id}"),
        ProposalTarget::Task { task_id } => format!("Task {task_id}"),
        ProposalTarget::Person { name } => format!("Person {name}"),
        ProposalTarget::Vault => "Vault".into(),
    }
}

fn confidence_label(confidence: f32) -> String {
    format!("{:.0}%", (confidence.clamp(0.0, 1.0) * 100.0).round())
}

fn compact_join(lines: Vec<String>, fallback: &str) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let joined = lines
        .into_iter()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .filter(|line| seen.insert(line.clone()))
        .take(3)
        .collect::<Vec<_>>()
        .join(" | ");
    if joined.is_empty() {
        fallback.into()
    } else {
        joined.chars().take(240).collect()
    }
}

fn first_content_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(120).collect())
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
    use noet_ai::{
        AgendaDraft, AgendaItem, AgendaSection, AiProposal, HousekeepingJob, LabelSuggestion,
        LabelSuggestions, NotePatchProposal, NoteReview, ProposalKind, ProposalPayload,
        ProposalTarget, ProposedTaskState, ReviewFinding, ReviewFindingKind, SourceRef,
        TaskExtraction, TaskExtractions, TaskPromotionProposal, TaskStateChangeProposal,
    };

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
        assert_eq!(surface.proposals[0].target, "Person Jane");
        assert_eq!(surface.proposals[0].confidence, "100%");
        assert_eq!(surface.jobs[0].status, "Completed");
        assert_eq!(surface.jobs[0].produced_proposals, 1);
    }

    #[test]
    fn ai_surface_previews_all_proposal_payloads() {
        let mut state = AiState::default();
        for proposal in proposal_samples() {
            state.enqueue(proposal);
        }

        let rows = ai_surface(&state).proposals;

        assert_eq!(rows.len(), 7);
        assert!(rows.iter().any(|row| row.kind == "Draft agenda"
            && row.preview.contains("Follow up")
            && row.source.contains("Task task-1")));
        let review_row = rows
            .iter()
            .find(|row| row.kind == "Review note")
            .expect("review note row");
        assert!(review_row.preview.contains("Risk"));
        assert!(review_row.source.contains("Note note-1"));
        assert_eq!(review_row.source_rows.len(), 1);
        assert_eq!(review_row.source_rows[0].label, "Note note-1");
        assert!(review_row.source_rows[0].navigable);
        assert!(rows.iter().any(|row| row.kind == "Add labels"
            && row.preview.contains("#meeting")
            && row.source.contains("Note note-1")));
        assert!(rows.iter().any(|row| row.kind == "Extract tasks"
            && row.preview.contains("Assign owner")
            && row.source.contains("Note note-1")));
        assert!(rows.iter().any(|row| row.kind == "Promote task"
            && row.preview.contains("Launch owner")
            && row.source.contains("Task task-1")));
        assert!(rows.iter().any(|row| row.kind == "Patch note"
            && row.preview.contains("note-1")
            && row.source.contains("Note note-1")));
        assert!(rows.iter().any(|row| row.kind == "Change task state"
            && row.preview.contains("Resolve")
            && row.source.contains("Task task-1")));
    }

    fn proposal_samples() -> Vec<AiProposal> {
        vec![
            proposal(
                ProposalKind::DraftAgenda,
                ProposalTarget::Person {
                    name: "Jane".into(),
                },
                ProposalPayload::DraftAgenda(AgendaDraft {
                    person: "Jane".into(),
                    sections: vec![AgendaSection {
                        title: "Agenda".into(),
                        items: vec![AgendaItem {
                            text: "Follow up on launch".into(),
                            sources: vec![SourceRef::Task {
                                task_id: "task-1".into(),
                            }],
                        }],
                    }],
                }),
            ),
            proposal(
                ProposalKind::ReviewNote,
                ProposalTarget::Note {
                    note_id: "note-1".into(),
                },
                ProposalPayload::ReviewNote(NoteReview {
                    findings: vec![ReviewFinding {
                        kind: ReviewFindingKind::Risk,
                        text: "Model loading pressure".into(),
                        sources: vec![SourceRef::Note {
                            note_id: "note-1".into(),
                        }],
                    }],
                    label_suggestions: Vec::new(),
                    task_extractions: Vec::new(),
                }),
            ),
            proposal(
                ProposalKind::AddLabels,
                ProposalTarget::Note {
                    note_id: "note-1".into(),
                },
                ProposalPayload::AddLabels(LabelSuggestions {
                    suggestions: vec![LabelSuggestion {
                        label: "meeting".into(),
                        reason: "Looks like meeting notes".into(),
                        sources: vec![SourceRef::Note {
                            note_id: "note-1".into(),
                        }],
                    }],
                }),
            ),
            proposal(
                ProposalKind::ExtractTasks,
                ProposalTarget::Note {
                    note_id: "note-1".into(),
                },
                ProposalPayload::ExtractTasks(TaskExtractions {
                    tasks: vec![TaskExtraction {
                        text: "Assign owner".into(),
                        person: Some("Jane".into()),
                        due: Some("2026-06-17".into()),
                        labels: vec!["followup".into()],
                        source: SourceRef::Note {
                            note_id: "note-1".into(),
                        },
                    }],
                }),
            ),
            proposal(
                ProposalKind::PromoteTask,
                ProposalTarget::Task {
                    task_id: "task-1".into(),
                },
                ProposalPayload::PromoteTask(TaskPromotionProposal {
                    source_task_id: "task-1".into(),
                    proposed_title: "Launch owner".into(),
                    proposed_body: "# Launch owner\n\nConfirm owner.".into(),
                    source: SourceRef::Task {
                        task_id: "task-1".into(),
                    },
                }),
            ),
            proposal(
                ProposalKind::PatchNote,
                ProposalTarget::Note {
                    note_id: "note-1".into(),
                },
                ProposalPayload::PatchNote(NotePatchProposal {
                    note_id: "note-1".into(),
                    patch: "+ #meeting".into(),
                    sources: vec![SourceRef::Note {
                        note_id: "note-1".into(),
                    }],
                }),
            ),
            proposal(
                ProposalKind::ChangeTaskState,
                ProposalTarget::Task {
                    task_id: "task-1".into(),
                },
                ProposalPayload::ChangeTaskState(TaskStateChangeProposal {
                    task_id: "task-1".into(),
                    proposed_state: ProposedTaskState::Resolve,
                    source: SourceRef::Task {
                        task_id: "task-1".into(),
                    },
                }),
            ),
        ]
    }

    fn proposal(
        kind: ProposalKind,
        target: ProposalTarget,
        payload: ProposalPayload,
    ) -> AiProposal {
        AiProposal {
            kind,
            target,
            payload,
            rationale: "Because the proposal is source-linked.".into(),
            confidence: 0.82,
            requires_confirmation: true,
        }
    }
}
