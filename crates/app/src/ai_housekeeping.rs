use noet_ai::{
    AgendaDraft, AgendaItem, AgendaSection, AiProposal, HousekeepingJob, LabelSuggestion,
    LabelSuggestions, ProposalKind, ProposalPayload, ProposalTarget, ProposedTaskState, SourceRef,
    TaskStateChangeProposal,
};
use noet_core::{Backend, Filter, Note, Todo};

pub fn run_housekeeping_job(
    backend: &Backend,
    job: &HousekeepingJob,
) -> Result<Vec<AiProposal>, String> {
    match job {
        HousekeepingJob::FindUnlabeledMeetings => find_unlabeled_meetings(backend),
        HousekeepingJob::FindFollowupsWithoutPerson => find_followups_without_person(backend),
        HousekeepingJob::ReviewStaleFollowups => review_stale_followups(backend),
        HousekeepingJob::RefreshOneOnOneAgendaDrafts => refresh_one_on_one_agenda_drafts(backend),
        HousekeepingJob::RefreshEmbeddings => Ok(Vec::new()),
    }
}

fn find_unlabeled_meetings(backend: &Backend) -> Result<Vec<AiProposal>, String> {
    let notes = backend
        .query_notes(&Filter::default())
        .map_err(|err| err.to_string())?;
    Ok(notes
        .into_iter()
        .filter(|note| is_probable_meeting(note) && !has_meeting_label(&note.body))
        .map(|note| AiProposal {
            kind: ProposalKind::AddLabels,
            target: ProposalTarget::Note {
                note_id: note.id.clone(),
            },
            payload: ProposalPayload::AddLabels(LabelSuggestions {
                suggestions: vec![LabelSuggestion {
                    label: "meeting".into(),
                    reason: "The title or body looks like a meeting note.".into(),
                    sources: vec![SourceRef::Note {
                        note_id: note.id.clone(),
                    }],
                }],
            }),
            rationale: "Housekeeping found a probable meeting note without a meeting label.".into(),
            confidence: 0.7,
            requires_confirmation: true,
        })
        .collect())
}

fn find_followups_without_person(backend: &Backend) -> Result<Vec<AiProposal>, String> {
    let tasks = backend
        .query_todos(&Filter {
            kind: "followup".into(),
            ..Default::default()
        })
        .map_err(|err| err.to_string())?;
    Ok(tasks
        .into_iter()
        .filter(|task| task.person.trim().is_empty())
        .map(|task| {
            task_state_proposal(
                &task,
                ProposedTaskState::KeepOpen,
                "Follow-up is missing a person; keep it open and review ownership.",
                0.65,
            )
        })
        .collect())
}

fn review_stale_followups(backend: &Backend) -> Result<Vec<AiProposal>, String> {
    let tasks = backend.stale_todos().map_err(|err| err.to_string())?;
    Ok(tasks
        .into_iter()
        .map(|task| {
            task_state_proposal(
                &task,
                ProposedTaskState::KeepOpen,
                "Task appears stale; review whether to resolve, carry forward, or demote.",
                0.6,
            )
        })
        .collect())
}

fn refresh_one_on_one_agenda_drafts(backend: &Backend) -> Result<Vec<AiProposal>, String> {
    let people = backend.list_people().map_err(|err| err.to_string())?;
    let mut proposals = Vec::new();
    for person in people {
        let context = backend
            .one_on_one_context(&person.name)
            .map_err(|err| err.to_string())?;
        if context.current_note.is_none()
            && context.followups.is_empty()
            && context.delegated.is_empty()
            && context.waiting.is_empty()
        {
            continue;
        }
        let mut items = Vec::new();
        for task in context
            .followups
            .iter()
            .chain(context.delegated.iter())
            .chain(context.waiting.iter())
            .take(8)
        {
            items.push(AgendaItem {
                text: task.text.clone(),
                sources: vec![SourceRef::Task {
                    task_id: task.id.clone(),
                }],
            });
        }
        if items.is_empty() {
            if let Some(current) = &context.current_note {
                items.push(AgendaItem {
                    text: format!("Review {}", current.note.title),
                    sources: vec![SourceRef::Note {
                        note_id: current.note.note.id.clone(),
                    }],
                });
            }
        }
        proposals.push(AiProposal {
            kind: ProposalKind::DraftAgenda,
            target: context
                .current_note
                .as_ref()
                .map(|note| ProposalTarget::Note {
                    note_id: note.note.note.id.clone(),
                })
                .unwrap_or_else(|| ProposalTarget::Person {
                    name: person.name.clone(),
                }),
            payload: ProposalPayload::DraftAgenda(AgendaDraft {
                person: person.name.clone(),
                sections: vec![AgendaSection {
                    title: "Next 1:1".into(),
                    items,
                }],
            }),
            rationale: "Housekeeping refreshed the next 1:1 agenda draft.".into(),
            confidence: 0.7,
            requires_confirmation: false,
        });
    }
    Ok(proposals)
}

fn task_state_proposal(
    task: &Todo,
    state: ProposedTaskState,
    rationale: &str,
    confidence: f32,
) -> AiProposal {
    AiProposal {
        kind: ProposalKind::ChangeTaskState,
        target: ProposalTarget::Task {
            task_id: task.id.clone(),
        },
        payload: ProposalPayload::ChangeTaskState(TaskStateChangeProposal {
            task_id: task.id.clone(),
            proposed_state: state,
            source: SourceRef::Task {
                task_id: task.id.clone(),
            },
        }),
        rationale: rationale.into(),
        confidence,
        requires_confirmation: true,
    }
}

fn is_probable_meeting(note: &Note) -> bool {
    let haystack = format!("{} {}", note.title, note.body).to_ascii_lowercase();
    haystack.contains("meeting")
        || haystack.contains("1:1")
        || haystack.contains("attendees")
        || haystack.contains("action items")
}

fn has_meeting_label(body: &str) -> bool {
    body.split_whitespace().any(|token| {
        token
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '#' && c != '/')
            .eq_ignore_ascii_case("#meeting")
            || token.eq_ignore_ascii_case("#meeting/one-on-one")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_ai::ProposalPayload;
    use noet_core::Backend;
    use std::path::PathBuf;

    #[test]
    fn unlabeled_meeting_job_returns_label_proposal() {
        let (backend, dir) = backend_with_notes(&[("meeting.md", "# Team Meeting\n\nAttendees\n")]);

        let proposals =
            run_housekeeping_job(&backend, &HousekeepingJob::FindUnlabeledMeetings).unwrap();

        assert_eq!(proposals.len(), 1);
        assert!(matches!(
            proposals[0].payload,
            ProposalPayload::AddLabels(_)
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn followup_without_person_job_returns_review_proposal() {
        let (backend, dir) =
            backend_with_notes(&[("task.md", "# Task\n\n- [ ] Check launch risk #followup\n")]);

        let proposals =
            run_housekeeping_job(&backend, &HousekeepingJob::FindFollowupsWithoutPerson).unwrap();

        assert_eq!(proposals.len(), 1);
        assert!(matches!(
            proposals[0].payload,
            ProposalPayload::ChangeTaskState(_)
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn refresh_one_on_one_agendas_returns_draft_proposal() {
        let (backend, dir) = backend_with_notes(&[(
            "oneonone.md",
            "# 1:1 Jane\n\n#meeting/one-on-one\n@[[Jane Smith]]\n\n- [ ] Ask about launch @[[Jane Smith]] #followup\n",
        )]);

        let proposals =
            run_housekeeping_job(&backend, &HousekeepingJob::RefreshOneOnOneAgendaDrafts).unwrap();

        assert_eq!(proposals.len(), 1);
        assert!(matches!(
            proposals[0].payload,
            ProposalPayload::DraftAgenda(_)
        ));
        std::fs::remove_dir_all(dir).ok();
    }

    fn backend_with_notes(notes: &[(&str, &str)]) -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-ai-housekeeping-{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let notes_dir = dir.join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        for (name, body) in notes {
            std::fs::write(notes_dir.join(name), body).unwrap();
        }
        (
            Backend::open_at(dir.clone(), dir.join(".index")).unwrap(),
            dir,
        )
    }
}
