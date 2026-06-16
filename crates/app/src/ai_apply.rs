use crate::{defer_task_to_someday, promote_task_to_note, reopen_task, resolve_task};
use noet_ai::{AiProposal, LabelSuggestions, ProposalPayload, ProposedTaskState, TaskExtractions};
use noet_core::{Backend, TodoFields};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiApplyReport {
    pub labels_added: usize,
    pub tasks_added: usize,
    pub tasks_promoted: usize,
    pub task_states_changed: usize,
}

pub fn apply_ai_proposal(
    backend: &mut Backend,
    proposal: &AiProposal,
) -> Result<AiApplyReport, String> {
    match &proposal.payload {
        ProposalPayload::DraftAgenda(_) | ProposalPayload::ReviewNote(_) => {
            Ok(AiApplyReport::default())
        }
        ProposalPayload::AddLabels(labels) => apply_label_suggestions(backend, labels),
        ProposalPayload::ExtractTasks(tasks) => apply_task_extractions(backend, proposal, tasks),
        ProposalPayload::PromoteTask(task) => {
            promote_task_to_note(backend, &task.source_task_id)?;
            Ok(AiApplyReport {
                tasks_promoted: 1,
                ..Default::default()
            })
        }
        ProposalPayload::PatchNote(_) => Err("AI note patches require explicit patch UI".into()),
        ProposalPayload::ChangeTaskState(change) => {
            match change.proposed_state {
                ProposedTaskState::Resolve => resolve_task(backend, &change.task_id)?,
                ProposedTaskState::CarryForward | ProposedTaskState::KeepOpen => {
                    reopen_task(backend, &change.task_id)?
                }
                ProposedTaskState::DemoteToSomeday => {
                    defer_task_to_someday(backend, &change.task_id)?
                }
            }
            Ok(AiApplyReport {
                task_states_changed: 1,
                ..Default::default()
            })
        }
    }
}

fn apply_label_suggestions(
    backend: &mut Backend,
    labels: &LabelSuggestions,
) -> Result<AiApplyReport, String> {
    let mut added = 0;
    for suggestion in &labels.suggestions {
        let Some(note_id) = note_id_from_sources(suggestion.sources.iter().chain(std::iter::once(
            &noet_ai::SourceRef::Synthetic {
                label: String::new(),
            },
        ))) else {
            continue;
        };
        backend
            .add_tag(&note_id, &suggestion.label)
            .map_err(|err| err.to_string())?;
        added += 1;
    }
    Ok(AiApplyReport {
        labels_added: added,
        ..Default::default()
    })
}

fn apply_task_extractions(
    backend: &mut Backend,
    proposal: &AiProposal,
    tasks: &TaskExtractions,
) -> Result<AiApplyReport, String> {
    let target_note_id = match &proposal.target {
        noet_ai::ProposalTarget::Note { note_id } => Some(note_id.clone()),
        _ => None,
    };
    let mut added = 0;
    for task in &tasks.tasks {
        let note_id = target_note_id
            .clone()
            .or_else(|| note_id_from_source(&task.source))
            .ok_or_else(|| "Task extraction proposal has no target note".to_string())?;
        let mut fields = TodoFields::default();
        fields.text = task.text.clone();
        fields.status = "todo".into();
        fields.kind = workflow_from_labels(&task.labels);
        fields.person = task.person.clone().unwrap_or_default();
        fields.due = task.due.clone().unwrap_or_default();
        backend
            .add_todo(&note_id, &fields)
            .map_err(|err| err.to_string())?;
        added += 1;
    }
    Ok(AiApplyReport {
        tasks_added: added,
        ..Default::default()
    })
}

fn workflow_from_labels(labels: &[String]) -> String {
    labels
        .iter()
        .find_map(|label| {
            let label = label.trim().trim_start_matches('#');
            matches!(
                label,
                "followup" | "delegated" | "waiting" | "someday" | "mine" | "do"
            )
            .then(|| label.to_string())
        })
        .unwrap_or_else(|| "do".into())
}

fn note_id_from_sources<'a>(
    mut sources: impl Iterator<Item = &'a noet_ai::SourceRef>,
) -> Option<String> {
    sources.find_map(note_id_from_source)
}

fn note_id_from_source(source: &noet_ai::SourceRef) -> Option<String> {
    match source {
        noet_ai::SourceRef::Note { note_id }
        | noet_ai::SourceRef::NoteHeading { note_id, .. }
        | noet_ai::SourceRef::SourceSpan { note_id, .. } => Some(note_id.clone()),
        noet_ai::SourceRef::Task { .. } | noet_ai::SourceRef::Synthetic { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_ai::{
        AiProposal, LabelSuggestion, NotePatchProposal, ProposalKind, ProposalPayload,
        ProposalTarget, SourceRef, TaskExtraction, TaskPromotionProposal, TaskStateChangeProposal,
    };
    use noet_core::{Backend, Filter};
    use std::path::PathBuf;

    #[test]
    fn accepted_label_proposal_routes_through_core_add_tag() {
        let (mut backend, dir) = backend_with_note("# Note\n\nraw body\n");
        let note = backend.query_notes(&Filter::default()).unwrap()[0].clone();
        let proposal = AiProposal {
            kind: ProposalKind::AddLabels,
            target: ProposalTarget::Note {
                note_id: note.id.clone(),
            },
            payload: ProposalPayload::AddLabels(LabelSuggestions {
                suggestions: vec![LabelSuggestion {
                    label: "followup".into(),
                    reason: "contains follow-up language".into(),
                    sources: vec![SourceRef::Note {
                        note_id: note.id.clone(),
                    }],
                }],
            }),
            rationale: "Add missing label.".into(),
            confidence: 0.8,
            requires_confirmation: true,
        };

        let report = apply_ai_proposal(&mut backend, &proposal).unwrap();

        assert_eq!(report.labels_added, 1);
        let body = std::fs::read_to_string(note.path).unwrap();
        assert!(body.contains("#followup"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn accepted_task_extraction_routes_through_core_add_todo() {
        let (mut backend, dir) = backend_with_note("# Note\n\nDiscuss launch risk.\n");
        let note = backend.query_notes(&Filter::default()).unwrap()[0].clone();
        let proposal = AiProposal {
            kind: ProposalKind::ExtractTasks,
            target: ProposalTarget::Note {
                note_id: note.id.clone(),
            },
            payload: ProposalPayload::ExtractTasks(TaskExtractions {
                tasks: vec![TaskExtraction {
                    text: "Assign launch risk owner".into(),
                    person: Some("Jane Smith".into()),
                    due: Some("2026-06-17".into()),
                    labels: vec!["followup".into()],
                    source: SourceRef::Note {
                        note_id: note.id.clone(),
                    },
                }],
            }),
            rationale: "Extract clear follow-up.".into(),
            confidence: 0.9,
            requires_confirmation: true,
        };

        let report = apply_ai_proposal(&mut backend, &proposal).unwrap();

        assert_eq!(report.tasks_added, 1);
        let tasks = backend.query_todos(&Filter::default()).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].kind, "followup");
        assert_eq!(tasks[0].person, "Jane Smith");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn accepted_task_state_proposal_routes_through_core_status_writeback() {
        let (mut backend, dir) = backend_with_note("# Note\n\n- [ ] Follow up #followup\n");
        let task = backend.query_todos(&Filter::default()).unwrap()[0].clone();
        let proposal = AiProposal {
            kind: ProposalKind::ChangeTaskState,
            target: ProposalTarget::Task {
                task_id: task.id.clone(),
            },
            payload: ProposalPayload::ChangeTaskState(TaskStateChangeProposal {
                task_id: task.id.clone(),
                proposed_state: ProposedTaskState::Resolve,
                source: SourceRef::Task {
                    task_id: task.id.clone(),
                },
            }),
            rationale: "User accepted resolution.".into(),
            confidence: 1.0,
            requires_confirmation: true,
        };

        let report = apply_ai_proposal(&mut backend, &proposal).unwrap();

        assert_eq!(report.task_states_changed, 1);
        assert_eq!(backend.get_todo(&task.id).unwrap().status, "done");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejected_proposal_does_not_apply_markdown_patch() {
        let (mut backend, dir) = backend_with_note("# Note\n\noriginal\n");
        let note = backend.query_notes(&Filter::default()).unwrap()[0].clone();
        let proposal = AiProposal {
            kind: ProposalKind::PatchNote,
            target: ProposalTarget::Note {
                note_id: note.id.clone(),
            },
            payload: ProposalPayload::PatchNote(NotePatchProposal {
                note_id: note.id.clone(),
                patch: "replace original".into(),
                sources: vec![SourceRef::Note {
                    note_id: note.id.clone(),
                }],
            }),
            rationale: "Patch proposal.".into(),
            confidence: 0.7,
            requires_confirmation: true,
        };

        let before = std::fs::read_to_string(&note.path).unwrap();
        let err = apply_ai_proposal(&mut backend, &proposal).unwrap_err();
        let after = std::fs::read_to_string(&note.path).unwrap();

        assert!(err.contains("explicit patch UI"));
        assert_eq!(before, after);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn accepted_promotion_routes_through_core_promotion() {
        let (mut backend, dir) =
            backend_with_note("# Note\n\n- [ ] Ask Jane about launch @[[Jane]] #followup\n");
        let task = backend.query_todos(&Filter::default()).unwrap()[0].clone();
        let proposal = AiProposal {
            kind: ProposalKind::PromoteTask,
            target: ProposalTarget::Task {
                task_id: task.id.clone(),
            },
            payload: ProposalPayload::PromoteTask(TaskPromotionProposal {
                source_task_id: task.id.clone(),
                proposed_title: task.text.clone(),
                proposed_body: String::new(),
                source: SourceRef::Task {
                    task_id: task.id.clone(),
                },
            }),
            rationale: "Promote important task.".into(),
            confidence: 0.85,
            requires_confirmation: true,
        };

        let report = apply_ai_proposal(&mut backend, &proposal).unwrap();

        assert_eq!(report.tasks_promoted, 1);
        assert!(backend
            .query_notes(&Filter::default())
            .unwrap()
            .iter()
            .any(|note| note.title.contains("Ask Jane")));
        std::fs::remove_dir_all(dir).ok();
    }

    fn backend_with_note(body: &str) -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!("noet-ai-apply-{}", unique_id()));
        let notes = dir.join("notes");
        std::fs::create_dir_all(&notes).unwrap();
        std::fs::write(notes.join("note.md"), body).unwrap();
        let backend = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();
        (backend, dir)
    }

    fn unique_id() -> String {
        format!(
            "{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }
}
