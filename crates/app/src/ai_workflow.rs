use noet_ai::{
    AgendaDraft, AiContextBlock, AiProposal, AiResult, NoteReview, ProposalKind, ProposalPayload,
    ProposalTarget, SourceRef as AiSourceRef, StructuredRequest, StructuredResponse,
    StructuredRuntime, StructuredTask,
};
use noet_core::{NoteContext, OneOnOneContext, TaskFact};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgendaDraftOptions {
    pub profile_id: String,
    pub max_context_blocks: usize,
    pub max_output_tokens: Option<u32>,
}

impl AgendaDraftOptions {
    pub fn new(profile_id: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            max_context_blocks: 24,
            max_output_tokens: Some(768),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteReviewOptions {
    pub profile_id: String,
    pub max_context_blocks: usize,
    pub max_output_tokens: Option<u32>,
}

impl NoteReviewOptions {
    pub fn new(profile_id: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            max_context_blocks: 16,
            max_output_tokens: Some(768),
        }
    }
}

pub fn assemble_one_on_one_agenda_request(
    context: &OneOnOneContext,
    options: &AgendaDraftOptions,
) -> StructuredRequest {
    let mut blocks = Vec::new();

    if let Some(current) = &context.current_note {
        blocks.push(AiContextBlock {
            source: AiSourceRef::Note {
                note_id: current.note.note.id.clone(),
            },
            title: Some(format!("Current note: {}", current.note.title)),
            text: current.note.note.body.clone(),
            token_estimate: token_estimate(&current.note.note.body),
        });

        for related in &current.related {
            blocks.push(AiContextBlock {
                source: AiSourceRef::Note {
                    note_id: related.id.clone(),
                },
                title: Some(format!("Related note: {}", related.title)),
                text: format!("updated: {}", related.updated),
                token_estimate: None,
            });
        }
    }

    for previous in &context.previous_notes {
        blocks.push(AiContextBlock {
            source: AiSourceRef::Note {
                note_id: previous.id.clone(),
            },
            title: Some(format!("Previous 1:1: {}", previous.title)),
            text: format!("updated: {}", previous.updated),
            token_estimate: None,
        });
    }

    push_task_blocks(&mut blocks, "Open follow-up", &context.followups);
    push_task_blocks(&mut blocks, "Delegated item", &context.delegated);
    push_task_blocks(&mut blocks, "Waiting item", &context.waiting);

    blocks.truncate(options.max_context_blocks);

    StructuredRequest {
        profile_id: options.profile_id.clone(),
        task: StructuredTask::DraftOneOnOneAgenda,
        instructions: format!(
            "Draft a concise, source-linked 1:1 agenda for {}. Prefer unresolved \
             follow-ups, delegated work, waiting items, and decisions or risks from \
             prior notes. Do not invent facts.",
            context.person
        ),
        context: blocks,
        max_output_tokens: options.max_output_tokens,
    }
}

pub fn draft_one_on_one_agenda<R>(
    runtime: &R,
    context: &OneOnOneContext,
    options: &AgendaDraftOptions,
) -> AiResult<AiProposal>
where
    R: StructuredRuntime,
{
    let request = assemble_one_on_one_agenda_request(context, options);
    let StructuredResponse { value, .. } = runtime.complete_structured::<AgendaDraft>(request)?;

    Ok(AiProposal {
        kind: ProposalKind::DraftAgenda,
        target: context
            .current_note
            .as_ref()
            .map(|note| ProposalTarget::Note {
                note_id: note.note.note.id.clone(),
            })
            .unwrap_or_else(|| ProposalTarget::Person {
                name: context.person.clone(),
            }),
        payload: ProposalPayload::DraftAgenda(value),
        rationale: "Drafted from the selected 1:1 context.".into(),
        confidence: 1.0,
        requires_confirmation: false,
    })
}

pub fn assemble_note_review_request(
    context: &NoteContext,
    options: &NoteReviewOptions,
) -> StructuredRequest {
    let note_id = context.note.note.id.clone();
    let mut blocks = vec![AiContextBlock {
        source: AiSourceRef::Note {
            note_id: note_id.clone(),
        },
        title: Some(format!("Current note: {}", context.note.title)),
        text: context.note.note.body.clone(),
        token_estimate: token_estimate(&context.note.note.body),
    }];

    for task in &context.note.facts.tasks {
        blocks.push(AiContextBlock {
            source: AiSourceRef::Task {
                task_id: task.id.clone(),
            },
            title: Some(format!("Task in current note: {}", task.source.note_title)),
            text: task_summary(task),
            token_estimate: token_estimate(&task.text),
        });
    }

    for source in &context.sources {
        blocks.push(AiContextBlock {
            source: AiSourceRef::NoteHeading {
                note_id: source.id.clone(),
                heading: source.anchor.clone(),
            },
            title: Some(format!("Explicit source: {}", source.title)),
            text: source.anchor.clone(),
            token_estimate: token_estimate(&source.anchor),
        });
    }

    for related in context.backlinks.iter().chain(context.related.iter()) {
        blocks.push(AiContextBlock {
            source: AiSourceRef::Note {
                note_id: related.id.clone(),
            },
            title: Some(format!("Related note: {}", related.title)),
            text: format!(
                "updated: {}; labels: {}; people: {}; workstreams: {}",
                related.updated,
                related.labels.join(", "),
                related.people.join(", "),
                related.workstreams.join(", ")
            ),
            token_estimate: None,
        });
    }

    blocks.truncate(options.max_context_blocks);

    StructuredRequest {
        profile_id: options.profile_id.clone(),
        task: StructuredTask::ReviewNote,
        instructions: format!(
            "Review note {} for decisions, risks, open questions, commitments, \
             missing labels, people, due dates, and candidate tasks. Preserve \
             the user's original content. Return source-linked findings and \
             proposals only; do not sanitize, redact, or rewrite the note.",
            note_id
        ),
        context: blocks,
        max_output_tokens: options.max_output_tokens,
    }
}

pub fn review_current_note<R>(
    runtime: &R,
    context: &NoteContext,
    options: &NoteReviewOptions,
) -> AiResult<AiProposal>
where
    R: StructuredRuntime,
{
    let request = assemble_note_review_request(context, options);
    let StructuredResponse { value, .. } = runtime.complete_structured::<NoteReview>(request)?;

    Ok(AiProposal {
        kind: ProposalKind::ReviewNote,
        target: ProposalTarget::Note {
            note_id: context.note.note.id.clone(),
        },
        payload: ProposalPayload::ReviewNote(value),
        rationale: "Reviewed the selected note for source-linked follow-up opportunities.".into(),
        confidence: 1.0,
        requires_confirmation: false,
    })
}

fn push_task_blocks(blocks: &mut Vec<AiContextBlock>, label: &str, tasks: &[TaskFact]) {
    for task in tasks {
        blocks.push(AiContextBlock {
            source: AiSourceRef::Task {
                task_id: task.id.clone(),
            },
            title: Some(format!("{label}: {}", task.source.note_title)),
            text: task_summary(task),
            token_estimate: token_estimate(&task.text),
        });
    }
}

fn task_summary(task: &TaskFact) -> String {
    let mut text = task.text.clone();
    if !task.due.is_empty() {
        text.push_str(&format!(" due:{}", task.due));
    }
    if !task.priority.is_empty() {
        text.push_str(&format!(" priority:{}", task.priority));
    }
    text
}

fn token_estimate(text: &str) -> Option<u32> {
    Some((text.split_whitespace().count() as u32).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_ai::{
        AgendaItem, AgendaSection, AiRuntimeError, AiUsage, LabelSuggestion, ReviewFinding,
        ReviewFindingKind, StructuredResponse, TaskExtraction,
    };
    use noet_core::{
        Note, NoteFacts, NoteSummary, ParsedNote, PropertyFact, SourceRef as CoreSourceRef,
        SourceSpan, TaskSource, TaskStatus, TaskWorkflow,
    };
    use serde::{de::DeserializeOwned, Serialize};
    use std::path::PathBuf;

    struct FakeAgendaRuntime;

    impl StructuredRuntime for FakeAgendaRuntime {
        fn complete_structured<T>(
            &self,
            request: StructuredRequest,
        ) -> AiResult<StructuredResponse<T>>
        where
            T: DeserializeOwned + Serialize,
        {
            assert_eq!(request.task, StructuredTask::DraftOneOnOneAgenda);
            assert!(request
                .context
                .iter()
                .any(|block| matches!(block.source, AiSourceRef::Task { .. })));

            let draft = AgendaDraft {
                person: "Jane Smith".into(),
                sections: vec![AgendaSection {
                    title: "Follow up".into(),
                    items: vec![AgendaItem {
                        text: "Ask about launch risks.".into(),
                        sources: vec![AiSourceRef::Task {
                            task_id: "task-1".into(),
                        }],
                    }],
                }],
            };

            let value = serde_json::to_value(draft)
                .and_then(serde_json::from_value)
                .map_err(|err| AiRuntimeError::StructuredOutputFailed {
                    message: err.to_string(),
                })?;

            Ok(StructuredResponse {
                value,
                usage: Some(AiUsage {
                    input_tokens: Some(64),
                    output_tokens: Some(32),
                }),
            })
        }
    }

    struct FakeNoteReviewRuntime;

    impl StructuredRuntime for FakeNoteReviewRuntime {
        fn complete_structured<T>(
            &self,
            request: StructuredRequest,
        ) -> AiResult<StructuredResponse<T>>
        where
            T: DeserializeOwned + Serialize,
        {
            assert_eq!(request.task, StructuredTask::ReviewNote);
            assert!(request.instructions.contains("Preserve"));
            assert!(request.instructions.contains("do not sanitize"));
            assert!(request
                .context
                .iter()
                .any(|block| matches!(block.source, AiSourceRef::Note { .. })));

            let review = NoteReview {
                findings: vec![ReviewFinding {
                    kind: ReviewFindingKind::Risk,
                    text: "Launch risk needs owner.".into(),
                    sources: vec![AiSourceRef::Note {
                        note_id: "note-1".into(),
                    }],
                }],
                label_suggestions: vec![LabelSuggestion {
                    label: "risk".into(),
                    reason: "The note discusses launch risk.".into(),
                    sources: vec![AiSourceRef::Note {
                        note_id: "note-1".into(),
                    }],
                }],
                task_extractions: vec![TaskExtraction {
                    text: "Assign launch risk owner.".into(),
                    person: Some("Jane Smith".into()),
                    due: None,
                    labels: vec!["followup".into()],
                    source: AiSourceRef::Note {
                        note_id: "note-1".into(),
                    },
                }],
            };

            let value = serde_json::to_value(review)
                .and_then(serde_json::from_value)
                .map_err(|err| AiRuntimeError::StructuredOutputFailed {
                    message: err.to_string(),
                })?;

            Ok(StructuredResponse { value, usage: None })
        }
    }

    #[test]
    fn one_on_one_agenda_request_uses_bounded_source_linked_context() {
        let context = sample_context();
        let request =
            assemble_one_on_one_agenda_request(&context, &AgendaDraftOptions::new("profile"));

        assert_eq!(request.profile_id, "profile");
        assert_eq!(request.task, StructuredTask::DraftOneOnOneAgenda);
        assert!(request.context.len() <= 24);
        assert!(request
            .context
            .iter()
            .any(|block| matches!(block.source, AiSourceRef::Task { .. })));
    }

    #[test]
    fn agenda_workflow_returns_read_only_proposal() {
        let context = sample_context();
        let proposal = draft_one_on_one_agenda(
            &FakeAgendaRuntime,
            &context,
            &AgendaDraftOptions::new("profile"),
        )
        .expect("fake runtime should draft agenda");

        assert_eq!(proposal.kind, ProposalKind::DraftAgenda);
        assert!(!proposal.requires_confirmation);
        assert!(matches!(
            proposal.payload,
            ProposalPayload::DraftAgenda(AgendaDraft { .. })
        ));
    }

    #[test]
    fn note_review_request_uses_source_linked_context_without_sanitization() {
        let context = sample_note_context();
        let request = assemble_note_review_request(&context, &NoteReviewOptions::new("profile"));

        assert_eq!(request.profile_id, "profile");
        assert_eq!(request.task, StructuredTask::ReviewNote);
        assert!(request.context.len() <= 16);
        assert!(request.instructions.contains("do not sanitize"));
        assert!(request
            .context
            .iter()
            .any(|block| block.text.contains("raw user wording")));
    }

    #[test]
    fn note_review_workflow_returns_read_only_review_proposal() {
        let context = sample_note_context();
        let proposal = review_current_note(
            &FakeNoteReviewRuntime,
            &context,
            &NoteReviewOptions::new("profile"),
        )
        .expect("fake runtime should review note");

        assert_eq!(proposal.kind, ProposalKind::ReviewNote);
        assert!(!proposal.requires_confirmation);
        assert!(matches!(
            proposal.payload,
            ProposalPayload::ReviewNote(NoteReview { .. })
        ));
    }

    fn sample_context() -> OneOnOneContext {
        OneOnOneContext {
            person: "Jane Smith".into(),
            current_note: None,
            history: Vec::new(),
            previous_notes: Vec::new(),
            open_items: vec![sample_task(TaskWorkflow::Followup)],
            followups: vec![sample_task(TaskWorkflow::Followup)],
            delegated: vec![sample_task(TaskWorkflow::Delegated)],
            waiting: vec![sample_task(TaskWorkflow::Waiting)],
        }
    }

    fn sample_note_context() -> NoteContext {
        let task = sample_task(TaskWorkflow::Followup);
        NoteContext {
            note: ParsedNote {
                note: Note {
                    id: "note-1".into(),
                    title: "Launch Review".into(),
                    created: "2026-06-15T08:00:00".into(),
                    updated: "2026-06-15T09:00:00".into(),
                    kind: "markdown".into(),
                    body: "# Launch Review\n\nraw user wording about launch risk\n".into(),
                    path: PathBuf::from("Launch Review.md"),
                },
                title: "Launch Review".into(),
                facts: NoteFacts {
                    labels: vec!["meeting".into()],
                    people: vec!["Jane Smith".into()],
                    workstreams: vec!["launch".into()],
                    properties: Vec::new(),
                    tasks: vec![task],
                    primary_task: None,
                },
            },
            backlinks: vec![NoteSummary {
                id: "backlink-1".into(),
                title: "Prior Launch Note".into(),
                updated: "2026-06-14T09:00:00".into(),
                labels: vec!["meeting".into()],
                people: vec!["Jane Smith".into()],
                workstreams: vec!["launch".into()],
            }],
            related: Vec::new(),
            sources: vec![CoreSourceRef {
                id: "source-1".into(),
                title: "Source Note".into(),
                anchor: "risk-heading".into(),
            }],
        }
    }

    fn sample_task(workflow: TaskWorkflow) -> TaskFact {
        TaskFact {
            id: "task-1".into(),
            source: TaskSource {
                note_id: "note-1".into(),
                note_title: "Jane 1:1".into(),
                note_updated: "2026-06-15T09:00:00".into(),
                line_no: 4,
                anchor: "task-1".into(),
                span: SourceSpan {
                    line_no: 4,
                    byte_start: 10,
                    byte_end: 50,
                },
            },
            text: "Ask about launch risks".into(),
            status: TaskStatus::Todo,
            workflow,
            people: vec!["Jane Smith".into()],
            workstreams: Vec::new(),
            labels: vec!["followup".into()],
            properties: vec![PropertyFact {
                key: "due".into(),
                value: "2026-06-17".into(),
            }],
            start: String::new(),
            due: "2026-06-17".into(),
            priority: "A".into(),
            external: String::new(),
            repeat: String::new(),
        }
    }
}
