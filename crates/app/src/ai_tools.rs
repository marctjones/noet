use noet_ai::{
    AiProposal, AiResult, AiRuntimeError, LabelSuggestion, LabelSuggestions, NoetTool,
    NoetToolCall, NoetToolResult, NotePatchProposal, ProposalKind, ProposalPayload, ProposalTarget,
    ProposedTaskState, SourceRef, TaskExtraction, TaskExtractions, TaskPromotionProposal,
    TaskStateChangeProposal, ToolRuntime,
};
use noet_core::{Backend, Filter};
use serde_json::json;

pub struct NoetToolHost<'a> {
    backend: &'a Backend,
}

impl<'a> NoetToolHost<'a> {
    pub fn new(backend: &'a Backend) -> Self {
        Self { backend }
    }
}

impl ToolRuntime for NoetToolHost<'_> {
    fn call_tool(&self, call: NoetToolCall) -> AiResult<NoetToolResult> {
        execute_noet_tool(self.backend, call)
    }
}

pub fn execute_noet_tool(backend: &Backend, call: NoetToolCall) -> AiResult<NoetToolResult> {
    match call.tool.clone() {
        NoetTool::SearchNotes => search_notes(backend, &call),
        NoetTool::LoadNoteContext => load_note_context(backend, &call),
        NoetTool::ListTasks => list_tasks(backend, &call),
        NoetTool::FindRelatedNotes => find_related_notes(backend, &call),
        NoetTool::ListNoteRevisions => list_note_revisions(backend, &call),
        NoetTool::LoadNoteRevision => load_note_revision(backend, &call),
        NoetTool::DraftOneOnOneAgenda => load_one_on_one_agenda_context(backend, &call),
        NoetTool::SuggestLabels => suggest_labels(&call),
        NoetTool::SuggestTaskExtraction => suggest_task_extraction(&call),
        NoetTool::ProposeTaskPromotion => propose_task_promotion(backend, &call),
        NoetTool::ProposeNotePatch => propose_note_patch(&call),
        NoetTool::ProposeTaskStateChange => propose_task_state_change(&call),
    }
}

fn search_notes(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let query = optional_arg(call, "query").unwrap_or_default();
    let limit = limit_arg(call, 8);
    let notes = backend
        .query_notes(&Filter {
            search: query,
            ..Default::default()
        })
        .map_err(|err| tool_error(call, err))?
        .into_iter()
        .take(limit)
        .map(|note| {
            json!({
                "note_id": note.id,
                "title": note.title,
                "updated": note.updated,
                "excerpt": excerpt(&note.body),
            })
        })
        .collect::<Vec<_>>();
    result(
        call.tool.clone(),
        json!({ "notes": notes }),
        Vec::new(),
        None,
    )
}

fn load_note_context(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let note_id = required_arg(call, "note_id")?;
    let context = backend
        .note_context(&note_id)
        .map_err(|err| tool_error(call, err))?;
    let sources = std::iter::once(SourceRef::Note {
        note_id: context.note.note.id.clone(),
    })
    .chain(context.note.facts.tasks.iter().map(|task| SourceRef::Task {
        task_id: task.id.clone(),
    }))
    .collect::<Vec<_>>();
    result(
        call.tool.clone(),
        json!({
            "note": {
                "note_id": context.note.note.id,
                "title": context.note.title,
                "body": context.note.note.body,
                "labels": context.note.facts.labels,
                "people": context.note.facts.people,
                "workstreams": context.note.facts.workstreams,
                "tasks": context.note.facts.tasks.iter().map(task_json).collect::<Vec<_>>(),
            },
            "backlinks": context.backlinks.iter().map(note_summary_json).collect::<Vec<_>>(),
            "related": context.related.iter().map(note_summary_json).collect::<Vec<_>>(),
            "sources": context.sources.iter().map(|source| {
                json!({
                    "note_id": source.id,
                    "title": source.title,
                    "anchor": source.anchor,
                })
            }).collect::<Vec<_>>(),
        }),
        sources,
        None,
    )
}

fn list_tasks(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let limit = limit_arg(call, 12);
    let tasks = if let Some(note_id) = optional_arg(call, "note_id").filter(|id| !id.is_empty()) {
        backend
            .note_context(&note_id)
            .map_err(|err| tool_error(call, err))?
            .note
            .facts
            .tasks
    } else {
        backend
            .task_list(&Filter {
                person: optional_arg(call, "person").unwrap_or_default(),
                status: optional_arg(call, "status").unwrap_or_default(),
                ..Default::default()
            })
            .map_err(|err| tool_error(call, err))?
    };
    let rows = tasks.iter().take(limit).map(task_json).collect::<Vec<_>>();
    let sources = tasks
        .iter()
        .take(limit)
        .map(|task| SourceRef::Task {
            task_id: task.id.clone(),
        })
        .collect();
    result(call.tool.clone(), json!({ "tasks": rows }), sources, None)
}

fn find_related_notes(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let note_id = required_arg(call, "note_id")?;
    let related = backend
        .related_notes(&note_id, limit_arg(call, 8))
        .map_err(|err| tool_error(call, err))?;
    let sources = related
        .iter()
        .map(|note| SourceRef::Note {
            note_id: note.id.clone(),
        })
        .collect();
    result(
        call.tool.clone(),
        json!({
            "related": related.iter().map(|note| {
                json!({
                    "note_id": note.id,
                    "title": note.title,
                    "updated": note.updated,
                    "shared": note.shared,
                })
            }).collect::<Vec<_>>(),
        }),
        sources,
        None,
    )
}

fn list_note_revisions(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let note_id = required_arg(call, "note_id")?;
    let revisions = crate::note_history(backend, &note_id, limit_arg(call, 8))
        .map_err(|message| tool_error(call, message))?;
    result(
        call.tool.clone(),
        json!({ "revisions": revisions.iter().map(|revision| {
            json!({
                "revision_id": revision.id,
                "note_id": revision.note_id,
                "created": revision.created,
                "actor": revision.actor,
                "operation": revision.operation,
                "title": revision.title,
                "summary": revision.summary,
                "proposal_id": revision.proposal_id,
                "model_id": revision.model_id,
            })
        }).collect::<Vec<_>>() }),
        vec![SourceRef::Note { note_id }],
        None,
    )
}

fn load_note_revision(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let revision_id = required_arg(call, "revision_id")?;
    let revision = crate::note_revision_detail(backend, &revision_id)
        .map_err(|message| tool_error(call, message))?
        .ok_or_else(|| tool_error(call, "revision not found"))?;
    result(
        call.tool.clone(),
        json!({
            "revision_id": revision.id,
            "note_id": revision.note_id,
            "created": revision.created,
            "actor": revision.actor,
            "operation": revision.operation,
            "title": revision.title,
            "proposal_id": revision.proposal_id,
            "model_id": revision.model_id,
            "rationale": revision.rationale,
            "before": revision.before_content,
            "after": revision.after_content,
            "diff": revision.diff,
        }),
        vec![SourceRef::Note {
            note_id: revision.note_id,
        }],
        None,
    )
}

fn load_one_on_one_agenda_context(
    backend: &Backend,
    call: &NoetToolCall,
) -> AiResult<NoetToolResult> {
    let person = required_arg(call, "person")?;
    let context = backend
        .one_on_one_context(&person)
        .map_err(|err| tool_error(call, err))?;
    let mut sources = context
        .history
        .iter()
        .map(|note| SourceRef::Note {
            note_id: note.id.clone(),
        })
        .collect::<Vec<_>>();
    sources.extend(context.open_items.iter().map(|task| SourceRef::Task {
        task_id: task.id.clone(),
    }));
    result(
        call.tool.clone(),
        json!({
            "person": context.person,
            "current_note": context.current_note.as_ref().map(|ctx| note_summary_json(&ctx.note_summary())),
            "history": context.history.iter().map(note_summary_json).collect::<Vec<_>>(),
            "open_items": context.open_items.iter().map(task_json).collect::<Vec<_>>(),
            "followups": context.followups.iter().map(task_json).collect::<Vec<_>>(),
            "delegated": context.delegated.iter().map(task_json).collect::<Vec<_>>(),
            "waiting": context.waiting.iter().map(task_json).collect::<Vec<_>>(),
        }),
        sources,
        None,
    )
}

fn suggest_labels(call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let note_id = required_arg(call, "note_id")?;
    let reason =
        optional_arg(call, "reason").unwrap_or_else(|| "Suggested by local AI tool".into());
    let suggestions = csv_arg(call, "labels")
        .into_iter()
        .map(|label| LabelSuggestion {
            label: label.trim_start_matches('#').to_string(),
            reason: reason.clone(),
            sources: vec![SourceRef::Note {
                note_id: note_id.clone(),
            }],
        })
        .collect::<Vec<_>>();
    if suggestions.is_empty() {
        return Err(tool_error(call, "labels is required"));
    }
    let proposal = AiProposal {
        kind: ProposalKind::AddLabels,
        target: ProposalTarget::Note {
            note_id: note_id.clone(),
        },
        payload: ProposalPayload::AddLabels(LabelSuggestions { suggestions }),
        rationale: reason,
        confidence: confidence_arg(call),
        requires_confirmation: true,
    };
    proposal_result(call.tool.clone(), proposal)
}

fn suggest_task_extraction(call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let note_id = required_arg(call, "note_id")?;
    let text = required_arg(call, "text")?;
    let proposal = AiProposal {
        kind: ProposalKind::ExtractTasks,
        target: ProposalTarget::Note {
            note_id: note_id.clone(),
        },
        payload: ProposalPayload::ExtractTasks(TaskExtractions {
            tasks: vec![TaskExtraction {
                text,
                person: optional_arg(call, "person").filter(|value| !value.is_empty()),
                due: optional_arg(call, "due").filter(|value| !value.is_empty()),
                labels: csv_arg(call, "labels"),
                source: SourceRef::Note { note_id },
            }],
        }),
        rationale: optional_arg(call, "rationale")
            .unwrap_or_else(|| "Extracted from note context by local AI tool".into()),
        confidence: confidence_arg(call),
        requires_confirmation: true,
    };
    proposal_result(call.tool.clone(), proposal)
}

fn propose_task_promotion(backend: &Backend, call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let task_id = required_arg(call, "task_id")?;
    let todo = backend
        .get_todo(&task_id)
        .map_err(|err| tool_error(call, err))?;
    let title = optional_arg(call, "title")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| todo.text.clone());
    let body = optional_arg(call, "body")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("# {title}\n\nPromoted from {task_id}.\n"));
    let proposal = AiProposal {
        kind: ProposalKind::PromoteTask,
        target: ProposalTarget::Task {
            task_id: task_id.clone(),
        },
        payload: ProposalPayload::PromoteTask(TaskPromotionProposal {
            source_task_id: task_id.clone(),
            proposed_title: title,
            proposed_body: body,
            source: SourceRef::Task { task_id },
        }),
        rationale: optional_arg(call, "rationale")
            .unwrap_or_else(|| "Promote task into a dedicated note".into()),
        confidence: confidence_arg(call),
        requires_confirmation: true,
    };
    proposal_result(call.tool.clone(), proposal)
}

fn propose_note_patch(call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let note_id = required_arg(call, "note_id")?;
    let patch = required_arg(call, "patch")?;
    let proposal = AiProposal {
        kind: ProposalKind::PatchNote,
        target: ProposalTarget::Note {
            note_id: note_id.clone(),
        },
        payload: ProposalPayload::PatchNote(NotePatchProposal {
            note_id: note_id.clone(),
            patch,
            sources: vec![SourceRef::Note {
                note_id: note_id.clone(),
            }],
        }),
        rationale: optional_arg(call, "rationale")
            .unwrap_or_else(|| "Patch proposed by local AI tool".into()),
        confidence: confidence_arg(call),
        requires_confirmation: true,
    };
    proposal_result(call.tool.clone(), proposal)
}

fn propose_task_state_change(call: &NoetToolCall) -> AiResult<NoetToolResult> {
    let task_id = required_arg(call, "task_id")?;
    let proposed_state = match required_arg(call, "state")?
        .to_ascii_lowercase()
        .replace(['_', '-', ' '], "")
        .as_str()
    {
        "resolve" | "resolved" | "done" => ProposedTaskState::Resolve,
        "carryforward" | "carry" | "reopen" => ProposedTaskState::CarryForward,
        "demotetosomeday" | "someday" | "defer" => ProposedTaskState::DemoteToSomeday,
        "keepopen" | "open" => ProposedTaskState::KeepOpen,
        other => return Err(tool_error(call, format!("unknown task state {other}"))),
    };
    let proposal = AiProposal {
        kind: ProposalKind::ChangeTaskState,
        target: ProposalTarget::Task {
            task_id: task_id.clone(),
        },
        payload: ProposalPayload::ChangeTaskState(TaskStateChangeProposal {
            task_id: task_id.clone(),
            proposed_state,
            source: SourceRef::Task { task_id },
        }),
        rationale: optional_arg(call, "rationale")
            .unwrap_or_else(|| "Task state proposed by local AI tool".into()),
        confidence: confidence_arg(call),
        requires_confirmation: true,
    };
    proposal_result(call.tool.clone(), proposal)
}

fn result(
    tool: NoetTool,
    content: serde_json::Value,
    sources: Vec<SourceRef>,
    proposal: Option<AiProposal>,
) -> AiResult<NoetToolResult> {
    let content = serde_json::to_string(&content).map_err(|err| AiRuntimeError::ToolFailed {
        tool: tool.clone(),
        message: err.to_string(),
    })?;
    Ok(NoetToolResult {
        tool,
        content,
        sources,
        proposal,
    })
}

fn proposal_result(tool: NoetTool, proposal: AiProposal) -> AiResult<NoetToolResult> {
    let sources = proposal_sources(&proposal);
    result(
        tool,
        json!({
            "proposal_kind": format!("{:?}", proposal.kind),
            "target": proposal_target_json(&proposal.target),
            "rationale": proposal.rationale,
            "requires_confirmation": proposal.requires_confirmation,
        }),
        sources,
        Some(proposal),
    )
}

fn proposal_sources(proposal: &AiProposal) -> Vec<SourceRef> {
    match &proposal.payload {
        ProposalPayload::AddLabels(labels) => labels
            .suggestions
            .iter()
            .flat_map(|label| label.sources.clone())
            .collect(),
        ProposalPayload::ExtractTasks(tasks) => {
            tasks.tasks.iter().map(|task| task.source.clone()).collect()
        }
        ProposalPayload::PromoteTask(task) => vec![task.source.clone()],
        ProposalPayload::PatchNote(patch) => patch.sources.clone(),
        ProposalPayload::ChangeTaskState(change) => vec![change.source.clone()],
        ProposalPayload::DraftAgenda(draft) => draft
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .flat_map(|item| item.sources.clone())
            .collect(),
        ProposalPayload::ReviewNote(review) => review
            .findings
            .iter()
            .flat_map(|finding| finding.sources.clone())
            .chain(
                review
                    .label_suggestions
                    .iter()
                    .flat_map(|label| label.sources.clone()),
            )
            .chain(
                review
                    .task_extractions
                    .iter()
                    .map(|task| task.source.clone()),
            )
            .collect(),
    }
}

fn proposal_target_json(target: &ProposalTarget) -> serde_json::Value {
    match target {
        ProposalTarget::Note { note_id } => json!({ "note_id": note_id }),
        ProposalTarget::Task { task_id } => json!({ "task_id": task_id }),
        ProposalTarget::Person { name } => json!({ "person": name }),
        ProposalTarget::Vault => json!({ "vault": true }),
    }
}

fn note_summary_json(note: &noet_core::backend::NoteSummary) -> serde_json::Value {
    json!({
        "note_id": note.id,
        "title": note.title,
        "updated": note.updated,
        "labels": note.labels,
        "people": note.people,
        "workstreams": note.workstreams,
    })
}

trait NoteContextSummary {
    fn note_summary(&self) -> noet_core::backend::NoteSummary;
}

impl NoteContextSummary for noet_core::backend::NoteContext {
    fn note_summary(&self) -> noet_core::backend::NoteSummary {
        noet_core::backend::NoteSummary {
            id: self.note.note.id.clone(),
            title: self.note.title.clone(),
            updated: self.note.note.updated.clone(),
            labels: self.note.facts.labels.clone(),
            people: self.note.facts.people.clone(),
            workstreams: self.note.facts.workstreams.clone(),
        }
    }
}

fn task_json(task: &noet_core::backend::TaskFact) -> serde_json::Value {
    json!({
        "task_id": task.id,
        "note_id": task.source.note_id,
        "note_title": task.source.note_title,
        "text": task.text,
        "status": task.status.as_str(),
        "workflow": task.workflow.as_str(),
        "people": task.people,
        "workstreams": task.workstreams,
        "labels": task.labels,
        "due": task.due,
        "priority": task.priority,
        "line_no": task.source.line_no,
    })
}

fn required_arg(call: &NoetToolCall, name: &str) -> AiResult<String> {
    optional_arg(call, name)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| tool_error(call, format!("missing required argument {name}")))
}

fn optional_arg(call: &NoetToolCall, name: &str) -> Option<String> {
    call.arguments
        .iter()
        .find(|arg| arg.name == name)
        .map(|arg| arg.value.trim().to_string())
}

fn csv_arg(call: &NoetToolCall, name: &str) -> Vec<String> {
    optional_arg(call, name)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn limit_arg(call: &NoetToolCall, default: usize) -> usize {
    optional_arg(call, "limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
        .clamp(1, 50)
}

fn confidence_arg(call: &NoetToolCall) -> f32 {
    optional_arg(call, "confidence")
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.7)
        .clamp(0.0, 1.0)
}

fn excerpt(body: &str) -> String {
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("---"))
        .unwrap_or_default()
        .chars()
        .take(240)
        .collect()
}

fn tool_error(call: &NoetToolCall, message: impl ToString) -> AiRuntimeError {
    AiRuntimeError::ToolFailed {
        tool: call.tool.clone(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noet_ai::ToolArgument;
    use noet_core::Backend;
    use std::path::PathBuf;

    #[test]
    fn noet_tool_host_reads_notes_tasks_and_history() {
        let (mut backend, dir) = backend();
        let note = backend.new_note().unwrap();
        backend
            .save_note(
                &note.id,
                "Tool Note",
                "# Tool Note\n\nBody line.\n- [ ] Confirm tool contract @[[Jane]] #followup\n",
            )
            .unwrap();
        backend.add_tag(&note.id, "meeting").unwrap();
        let host = NoetToolHost::new(&backend);

        let search = host
            .call_tool(call(NoetTool::SearchNotes, &[("query", "Body")]))
            .unwrap();
        assert!(search.content.contains("Tool Note"));
        assert!(search.proposal.is_none());

        let context = host
            .call_tool(call(NoetTool::LoadNoteContext, &[("note_id", &note.id)]))
            .unwrap();
        assert!(context.content.contains("Confirm tool contract"));
        assert!(context
            .sources
            .iter()
            .any(|source| matches!(source, SourceRef::Note { note_id } if note_id == &note.id)));

        let history = host
            .call_tool(call(NoetTool::ListNoteRevisions, &[("note_id", &note.id)]))
            .unwrap();
        assert!(history.content.contains("add_tag"));
        assert!(history.proposal.is_none());

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn noet_tool_host_returns_reviewable_proposals_for_mutation_tools() {
        let (mut backend, dir) = backend();
        let note = backend.new_note().unwrap();
        backend
            .save_note(&note.id, "Tool Note", "# Tool Note\n\nDiscuss launch.\n")
            .unwrap();
        let host = NoetToolHost::new(&backend);

        let labels = host
            .call_tool(call(
                NoetTool::SuggestLabels,
                &[("note_id", &note.id), ("labels", "meeting,launch")],
            ))
            .unwrap();
        assert!(matches!(
            labels.proposal.as_ref().map(|proposal| &proposal.payload),
            Some(ProposalPayload::AddLabels(_))
        ));

        let task = host
            .call_tool(call(
                NoetTool::SuggestTaskExtraction,
                &[
                    ("note_id", &note.id),
                    ("text", "Confirm launch owner"),
                    ("person", "Jane"),
                    ("labels", "followup"),
                ],
            ))
            .unwrap();
        assert!(matches!(
            task.proposal.as_ref().map(|proposal| &proposal.payload),
            Some(ProposalPayload::ExtractTasks(_))
        ));
        assert!(backend
            .load_note(&note.id)
            .unwrap()
            .body
            .contains("Discuss launch"));
        assert!(
            !backend
                .load_note(&note.id)
                .unwrap()
                .body
                .contains("Confirm launch owner"),
            "tool proposals must not mutate Markdown before explicit accept"
        );

        std::fs::remove_dir_all(dir).ok();
    }

    fn call(tool: NoetTool, args: &[(&str, &str)]) -> NoetToolCall {
        NoetToolCall {
            tool,
            arguments: args
                .iter()
                .map(|(name, value)| ToolArgument {
                    name: (*name).into(),
                    value: (*value).into(),
                })
                .collect(),
        }
    }

    fn backend() -> (Backend, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "noet-ai-tools-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        let mut backend = Backend::open_at(dir.clone(), dir.join("cache")).unwrap();
        backend.reindex_all().unwrap();
        (backend, dir)
    }
}
