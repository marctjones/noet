use chrono::NaiveDate;
use noet_core::backend;
use noet_core::backend::Filter;
use slint::{ModelRc, VecModel};

use crate::{BoardColumn, FacetItem, GanttItem, NoteItem, NoteRef, RelatedRef, TodoItem};

pub fn note_item(n: &backend::Note) -> NoteItem {
    NoteItem {
        id: n.id.clone().into(),
        title: n.title.clone().into(),
        subtitle: n.updated.replace('T', " ").into(),
    }
}

pub fn note_item_from_summary(n: &backend::NoteSummary) -> NoteItem {
    NoteItem {
        id: n.id.clone().into(),
        title: n.title.clone().into(),
        subtitle: n.updated.replace('T', " ").into(),
    }
}

pub fn facet_items(items: &[backend::Project], active: &str) -> Vec<FacetItem> {
    items
        .iter()
        .map(|p| FacetItem {
            name: p.name.clone().into(),
            label: p.name.clone().into(),
            depth: 0,
            count: p.count as i32,
            active: p.name == active,
        })
        .collect()
}

/// Expand a flat list of `a/b/c` names into a sorted hierarchy. Parent nodes are
/// synthesized and counts roll up over each subtree.
pub fn facet_tree_items(items: &[backend::Project], active: &str) -> Vec<FacetItem> {
    use std::collections::BTreeSet;

    let mut nodes: BTreeSet<String> = BTreeSet::new();
    for p in items {
        let parts: Vec<&str> = p.name.split('/').collect();
        for i in 0..parts.len() {
            nodes.insert(parts[..=i].join("/"));
        }
    }

    nodes
        .iter()
        .map(|node| {
            let prefix = format!("{node}/");
            let count: i64 = items
                .iter()
                .filter(|p| &p.name == node || p.name.starts_with(&prefix))
                .map(|p| p.count)
                .sum();
            FacetItem {
                name: node.clone().into(),
                label: node.rsplit('/').next().unwrap_or(node).into(),
                depth: node.matches('/').count() as i32,
                count: count as i32,
                active: node == active,
            }
        })
        .collect()
}

pub fn todo_item(t: &backend::Todo) -> TodoItem {
    TodoItem {
        id: t.id.clone().into(),
        note_id: t.note_id.clone().into(),
        kind: t.kind.clone().into(),
        status: t.status.clone().into(),
        text: t.text.clone().into(),
        project: t.project.clone().into(),
        person: t.person.clone().into(),
        due: t.due.clone().into(),
        external: t.external.clone().into(),
        priority: t.priority.clone().into(),
        done: t.done,
    }
}

pub fn todo_item_from_fact(t: &backend::TaskFact) -> TodoItem {
    TodoItem {
        id: t.id.clone().into(),
        note_id: t.source.note_id.clone().into(),
        kind: t.workflow.as_str().into(),
        status: t.status.as_str().into(),
        text: t.text.clone().into(),
        project: t.workstreams.first().cloned().unwrap_or_default().into(),
        person: t.people.first().cloned().unwrap_or_default().into(),
        due: t.due.clone().into(),
        external: t.external.clone().into(),
        priority: t.priority.clone().into(),
        done: !t.status.is_open(),
    }
}

#[derive(Clone)]
pub struct OneOnOneSurface {
    pub discuss: Vec<TodoItem>,
    pub delegated: Vec<TodoItem>,
    pub delegated_history: Vec<TodoItem>,
    pub other: Vec<TodoItem>,
    pub current_id: String,
    pub current_title: String,
    pub last_title: String,
    pub prev_id: String,
    pub next_id: String,
    pub index: i32,
    pub count: i32,
    pub last_followups: Vec<TodoItem>,
    pub history_notes: Vec<NoteItem>,
}

impl Default for OneOnOneSurface {
    fn default() -> Self {
        Self {
            discuss: Vec::new(),
            delegated: Vec::new(),
            delegated_history: Vec::new(),
            other: Vec::new(),
            current_id: String::new(),
            current_title: String::new(),
            last_title: String::new(),
            prev_id: String::new(),
            next_id: String::new(),
            index: 0,
            count: 0,
            last_followups: Vec::new(),
            history_notes: Vec::new(),
        }
    }
}

fn is_followup_history_workflow(workflow: &backend::TaskWorkflow) -> bool {
    matches!(
        workflow,
        backend::TaskWorkflow::Delegated
            | backend::TaskWorkflow::Followup
            | backend::TaskWorkflow::Waiting
    )
}

pub fn one_on_one_surface(
    context: Option<&backend::OneOnOneContext>,
    current_note_id: &str,
) -> OneOnOneSurface {
    let Some(context) = context else {
        return OneOnOneSurface::default();
    };

    let mut discuss = context.followups.clone();
    discuss.extend(context.waiting.clone());

    let history = &context.history;
    let current_idx = history
        .iter()
        .position(|n| n.id == current_note_id)
        .unwrap_or(0);
    let current = history.get(current_idx);
    let prev = if current_idx > 0 {
        history.get(current_idx - 1)
    } else {
        None
    };
    let next = history.get(current_idx + 1);
    let last = next;
    let last_followups = last
        .map(|last_note| {
            context
                .open_items
                .iter()
                .filter(|task| {
                    task.source.note_id == last_note.id
                        && is_followup_history_workflow(&task.workflow)
                })
                .map(todo_item_from_fact)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    OneOnOneSurface {
        discuss: discuss.iter().map(todo_item_from_fact).collect(),
        delegated: context.delegated.iter().map(todo_item_from_fact).collect(),
        delegated_history: context
            .open_items
            .iter()
            .filter(|task| is_followup_history_workflow(&task.workflow))
            .map(todo_item_from_fact)
            .collect(),
        other: context
            .open_items
            .iter()
            .filter(|task| !is_followup_history_workflow(&task.workflow))
            .map(todo_item_from_fact)
            .collect(),
        current_id: current.map(|n| n.id.clone()).unwrap_or_default(),
        current_title: current.map(|n| n.title.clone()).unwrap_or_default(),
        last_title: last.map(|n| n.title.clone()).unwrap_or_default(),
        prev_id: prev.map(|n| n.id.clone()).unwrap_or_default(),
        next_id: next.map(|n| n.id.clone()).unwrap_or_default(),
        index: current_idx as i32,
        count: history.len() as i32,
        last_followups,
        history_notes: history.iter().map(note_item_from_summary).collect(),
    }
}

pub fn board_columns(board: &backend::BoardModel) -> Vec<BoardColumn> {
    board
        .columns
        .iter()
        .map(|col| {
            let cards = col
                .tasks
                .iter()
                .map(todo_item_from_fact)
                .collect::<Vec<_>>();
            BoardColumn {
                title: col.label.clone().into(),
                key: col.key.clone().into(),
                count: cards.len() as i32,
                cards: ModelRc::new(VecModel::from(cards)),
            }
        })
        .collect()
}

#[derive(Clone, Default)]
pub struct TaskReviewSurface {
    pub overdue: Vec<TodoItem>,
    pub due: Vec<TodoItem>,
    pub stale: Vec<TodoItem>,
    pub followups: Vec<TodoItem>,
    pub someday: Vec<TodoItem>,
}

pub fn task_review_surface(review: &backend::TaskReview) -> TaskReviewSurface {
    let overdue_ids = review
        .overdue
        .iter()
        .map(|task| task.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    TaskReviewSurface {
        overdue: review.overdue.iter().map(todo_item_from_fact).collect(),
        due: review
            .due
            .iter()
            .filter(|task| !overdue_ids.contains(task.id.as_str()))
            .map(todo_item_from_fact)
            .collect(),
        stale: review.stale.iter().map(todo_item_from_fact).collect(),
        followups: review.followups.iter().map(todo_item_from_fact).collect(),
        someday: review.someday.iter().map(todo_item_from_fact).collect(),
    }
}

pub fn waiting_review_items(waiting: &backend::WaitingReview) -> Vec<TodoItem> {
    let mut tasks = waiting
        .groups
        .iter()
        .flat_map(|group| group.tasks.iter())
        .map(todo_item_from_fact)
        .collect::<Vec<_>>();
    tasks.extend(waiting.unassigned.iter().map(todo_item_from_fact));
    tasks
}

pub fn note_refs_from_summaries(notes: &[backend::NoteSummary]) -> Vec<NoteRef> {
    notes
        .iter()
        .map(|n| NoteRef {
            id: n.id.clone().into(),
            title: n.title.clone().into(),
        })
        .collect()
}

pub fn note_refs_from_notes(notes: &[backend::Note]) -> Vec<NoteRef> {
    notes
        .iter()
        .map(|n| NoteRef {
            id: n.id.clone().into(),
            title: n.title.clone().into(),
        })
        .collect()
}

pub fn related_refs(notes: &[backend::RelatedNote]) -> Vec<RelatedRef> {
    notes
        .iter()
        .map(|r| RelatedRef {
            id: r.id.clone().into(),
            title: r.title.clone().into(),
            via: r.shared.join(", ").into(),
        })
        .collect()
}

pub fn source_refs(sources: &[backend::SourceRef]) -> Vec<RelatedRef> {
    sources
        .iter()
        .map(|source| RelatedRef {
            id: source.id.clone().into(),
            title: source.title.clone().into(),
            via: if source.anchor.is_empty() {
                "source".into()
            } else {
                format!("^{}", source.anchor).into()
            },
        })
        .collect()
}

fn day(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn gantt_items(todos: &[backend::Todo]) -> Vec<GanttItem> {
    let mut min: Option<NaiveDate> = None;
    let mut max: Option<NaiveDate> = None;
    for t in todos {
        for d in [t.start.as_str(), t.due.as_str()] {
            if let Some(nd) = day(d) {
                min = Some(min.map_or(nd, |m| m.min(nd)));
                max = Some(max.map_or(nd, |m| m.max(nd)));
            }
        }
    }
    let (Some(min), Some(max)) = (min, max) else {
        return Vec::new();
    };
    let span_days = (max - min).num_days().max(1) as f32;
    todos
        .iter()
        .filter_map(|t| {
            let due = day(&t.due)?;
            let start = day(&t.start).unwrap_or(due);
            let s = (start - min).num_days() as f32 / span_days;
            let e = (due - min).num_days() as f32 / span_days;
            Some(GanttItem {
                id: t.id.clone().into(),
                text: t.text.clone().into(),
                date: t.due[5..].to_string().into(),
                kind: t.kind.clone().into(),
                start_frac: s.clamp(0.0, 1.0),
                span_frac: (e - s).clamp(0.0, 1.0),
            })
        })
        .collect()
}

pub fn due_display(bucket: &str) -> &'static str {
    match bucket {
        "overdue" => "overdue",
        "week" => "this week",
        "hasdate" => "has date",
        "nodate" => "no date",
        _ => "any",
    }
}

pub fn board_group_key(group_by: &str) -> &str {
    match group_by {
        "workflow" => "kind",
        other => other,
    }
}

pub fn active_summary(f: &Filter) -> String {
    let mut parts = Vec::new();
    if !f.project.is_empty() {
        parts.push(format!("▸{}", f.project));
    }
    if !f.tag.is_empty() {
        parts.push(format!("#{}", f.tag));
    }
    if !f.person.is_empty() {
        parts.push(format!("@{}", f.person));
    }
    if !f.search.is_empty() {
        parts.push(format!("“{}”", f.search));
    }
    if !f.status.is_empty() {
        parts.push(f.status.clone());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("Filtered: {}", parts.join("  ·  "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backend::{
        NoteSummary, PropertyFact, RelatedNote, SourceRef, SourceSpan, TaskFact, TaskReview,
        TaskSource, TaskStatus, TaskWorkflow, WaitingGroup, WaitingReview,
    };
    use slint::Model;
    use std::path::PathBuf;

    fn source(note_id: &str, title: &str) -> TaskSource {
        TaskSource {
            note_id: note_id.into(),
            note_title: title.into(),
            note_updated: "2026-06-11T10:30:00".into(),
            line_no: 4,
            anchor: format!("{note_id}-anchor"),
            span: SourceSpan {
                line_no: 4,
                byte_start: 100,
                byte_end: 140,
            },
        }
    }

    fn task(
        id: &str,
        note_id: &str,
        text: &str,
        workflow: TaskWorkflow,
        status: TaskStatus,
    ) -> TaskFact {
        TaskFact {
            id: id.into(),
            source: source(note_id, note_id),
            text: text.into(),
            status,
            workflow,
            people: vec!["Jane".into()],
            workstreams: vec!["Client/Acme".into()],
            labels: Vec::new(),
            properties: Vec::new(),
            start: String::new(),
            due: "2026-06-20".into(),
            priority: "B".into(),
            external: String::new(),
            repeat: String::new(),
        }
    }

    fn note_summary(id: &str, title: &str) -> NoteSummary {
        NoteSummary {
            id: id.into(),
            title: title.into(),
            updated: "2026-06-11T10:30:00".into(),
            labels: Vec::new(),
            people: Vec::new(),
            workstreams: Vec::new(),
        }
    }

    #[test]
    fn note_and_todo_rows_are_stable() {
        let note = backend::Note {
            id: "n1".into(),
            title: "Weekly 1:1".into(),
            created: "2026-06-01T09:00:00".into(),
            updated: "2026-06-11T10:30:00".into(),
            kind: "markdown".into(),
            body: String::new(),
            path: PathBuf::from("n1.md"),
        };
        let row = note_item(&note);
        assert_eq!(row.id.to_string(), "n1");
        assert_eq!(row.title.to_string(), "Weekly 1:1");
        assert_eq!(row.subtitle.to_string(), "2026-06-11 10:30:00");

        let todo = backend::Todo {
            id: "n1:3".into(),
            note_id: "n1".into(),
            kind: "followup".into(),
            status: "todo".into(),
            text: "Ask Jane about launch risks".into(),
            project: "Acme".into(),
            person: "Jane".into(),
            start: String::new(),
            due: "2026-06-17".into(),
            external: "ref:https://example.test".into(),
            priority: "A".into(),
            repeat: String::new(),
            done: false,
            line_no: 3,
            anchor: String::new(),
            span: SourceSpan {
                line_no: 3,
                byte_start: 10,
                byte_end: 72,
            },
        };
        let row = todo_item(&todo);
        assert_eq!(row.id.to_string(), "n1:3");
        assert_eq!(row.kind.to_string(), "followup");
        assert_eq!(row.person.to_string(), "Jane");
        assert!(!row.done);
    }

    #[test]
    fn one_on_one_surface_groups_tasks_and_history() {
        let prior_followup = task(
            "old:1",
            "old",
            "Revisit budget",
            TaskWorkflow::Followup,
            TaskStatus::Todo,
        );
        let waiting = task(
            "cur:2",
            "cur",
            "Waiting on plan",
            TaskWorkflow::Waiting,
            TaskStatus::Todo,
        );
        let delegated = task(
            "cur:3",
            "cur",
            "Send draft",
            TaskWorkflow::Delegated,
            TaskStatus::Doing,
        );
        let mine = task(
            "cur:4",
            "cur",
            "My prep",
            TaskWorkflow::Mine,
            TaskStatus::Todo,
        );
        let context = backend::OneOnOneContext {
            person: "Jane".into(),
            current_note: None,
            history: vec![
                note_summary("cur", "Current 1:1"),
                note_summary("old", "Old 1:1"),
            ],
            previous_notes: vec![note_summary("old", "Old 1:1")],
            open_items: vec![
                prior_followup.clone(),
                waiting.clone(),
                delegated.clone(),
                mine.clone(),
            ],
            followups: vec![prior_followup],
            delegated: vec![delegated],
            waiting: vec![waiting],
        };

        let surface = one_on_one_surface(Some(&context), "cur");
        assert_eq!(surface.current_id, "cur");
        assert_eq!(surface.current_title, "Current 1:1");
        assert_eq!(surface.next_id, "old");
        assert_eq!(surface.last_title, "Old 1:1");
        assert_eq!(surface.count, 2);
        assert_eq!(surface.discuss.len(), 2);
        assert_eq!(surface.delegated.len(), 1);
        assert_eq!(surface.delegated_history.len(), 3);
        assert_eq!(surface.other.len(), 1);
        assert_eq!(surface.other[0].text.to_string(), "My prep");
        assert_eq!(surface.last_followups.len(), 1);
        assert_eq!(surface.last_followups[0].text.to_string(), "Revisit budget");
        assert_eq!(surface.history_notes[0].title.to_string(), "Current 1:1");

        let empty = one_on_one_surface(None, "");
        assert_eq!(empty.count, 0);
        assert!(empty.discuss.is_empty());
    }

    #[test]
    fn board_review_and_waiting_surfaces_are_deterministic() {
        let overdue = task(
            "n1:1",
            "n1",
            "Overdue",
            TaskWorkflow::Mine,
            TaskStatus::Todo,
        );
        let due = task(
            "n2:1",
            "n2",
            "Due soon",
            TaskWorkflow::Followup,
            TaskStatus::Todo,
        );
        let someday = task(
            "n3:1",
            "n3",
            "Someday",
            TaskWorkflow::Someday,
            TaskStatus::Todo,
        );

        let board = backend::BoardModel {
            group_by: "kind".into(),
            columns: vec![backend::BoardColumn {
                label: "Mine".into(),
                key: "mine".into(),
                tasks: vec![overdue.clone(), due.clone()],
            }],
        };
        let columns = board_columns(&board);
        assert_eq!(columns.len(), 1);
        assert_eq!(columns[0].title.to_string(), "Mine");
        assert_eq!(columns[0].count, 2);
        assert_eq!(columns[0].cards.row_count(), 2);

        let review = TaskReview {
            open: vec![overdue.clone(), due.clone(), someday.clone()],
            overdue: vec![overdue.clone()],
            due: vec![overdue.clone(), due.clone()],
            stale: vec![due.clone()],
            mine: vec![overdue.clone()],
            followups: vec![due.clone()],
            delegated: Vec::new(),
            waiting: Vec::new(),
            someday: vec![someday.clone()],
        };
        let surface = task_review_surface(&review);
        assert_eq!(surface.overdue.len(), 1);
        assert_eq!(surface.due.len(), 1, "overdue tasks are excluded from due");
        assert_eq!(surface.due[0].text.to_string(), "Due soon");
        assert_eq!(surface.someday[0].text.to_string(), "Someday");

        let waiting = WaitingReview {
            groups: vec![WaitingGroup {
                person: "Jane".into(),
                tasks: vec![due.clone()],
            }],
            unassigned: vec![someday],
        };
        let waiting_rows = waiting_review_items(&waiting);
        assert_eq!(waiting_rows.len(), 2);
        assert_eq!(waiting_rows[0].text.to_string(), "Due soon");
        assert_eq!(waiting_rows[1].text.to_string(), "Someday");
    }

    #[test]
    fn note_context_refs_are_stable() {
        let summaries = vec![note_summary("n1", "Backlink")];
        let refs = note_refs_from_summaries(&summaries);
        assert_eq!(refs[0].id.to_string(), "n1");
        assert_eq!(refs[0].title.to_string(), "Backlink");

        let related = vec![RelatedNote {
            id: "n2".into(),
            title: "Related".into(),
            updated: "2026-06-11T10:30:00".into(),
            shared: vec!["Jane".into(), "Client/Acme".into()],
        }];
        let related_rows = related_refs(&related);
        assert_eq!(related_rows[0].via.to_string(), "Jane, Client/Acme");

        let sources = vec![
            SourceRef {
                id: "n3".into(),
                title: "Meeting".into(),
                anchor: "followup".into(),
            },
            SourceRef {
                id: "n4".into(),
                title: "Plain Source".into(),
                anchor: String::new(),
            },
        ];
        let source_rows = source_refs(&sources);
        assert_eq!(source_rows[0].via.to_string(), "^followup");
        assert_eq!(source_rows[1].via.to_string(), "source");
    }

    #[test]
    fn task_fact_row_uses_typed_workflow_fields() {
        let task = TaskFact {
            id: "n1:4".into(),
            source: TaskSource {
                note_id: "n1".into(),
                note_title: "Note".into(),
                note_updated: "2026-06-11T10:30:00".into(),
                line_no: 4,
                anchor: "send-draft".into(),
                span: SourceSpan {
                    line_no: 4,
                    byte_start: 100,
                    byte_end: 140,
                },
            },
            text: "Send draft".into(),
            status: TaskStatus::Doing,
            workflow: TaskWorkflow::Delegated,
            people: vec!["Sam".into()],
            workstreams: vec!["Client/Acme".into()],
            labels: vec!["delegated".into()],
            properties: vec![PropertyFact {
                key: "due".into(),
                value: "2026-06-20".into(),
            }],
            start: String::new(),
            due: "2026-06-20".into(),
            priority: "B".into(),
            external: String::new(),
            repeat: String::new(),
        };

        let row = todo_item_from_fact(&task);
        assert_eq!(row.status.to_string(), "doing");
        assert_eq!(row.kind.to_string(), "delegated");
        assert_eq!(row.project.to_string(), "Client/Acme");
        assert_eq!(row.person.to_string(), "Sam");
        assert!(!row.done);
    }

    #[test]
    fn facet_tree_rolls_counts_into_parent_nodes() {
        let rows = facet_tree_items(
            &[
                backend::Project {
                    name: "meeting".into(),
                    count: 2,
                },
                backend::Project {
                    name: "meeting/one-on-one".into(),
                    count: 3,
                },
                backend::Project {
                    name: "work/client".into(),
                    count: 4,
                },
            ],
            "meeting/one-on-one",
        );

        let meeting = rows
            .iter()
            .find(|row| row.name.to_string() == "meeting")
            .unwrap();
        assert_eq!(meeting.count, 5);
        assert_eq!(meeting.depth, 0);
        assert!(!meeting.active);

        let one_on_one = rows
            .iter()
            .find(|row| row.name.to_string() == "meeting/one-on-one")
            .unwrap();
        assert_eq!(one_on_one.label.to_string(), "one-on-one");
        assert_eq!(one_on_one.depth, 1);
        assert!(one_on_one.active);
    }

    #[test]
    fn gantt_items_normalize_date_ranges() {
        let todos = vec![
            backend::Todo {
                id: "n1:1".into(),
                note_id: "n1".into(),
                kind: "do".into(),
                status: "todo".into(),
                text: "First".into(),
                project: String::new(),
                person: String::new(),
                start: "2026-06-10".into(),
                due: "2026-06-12".into(),
                external: String::new(),
                priority: String::new(),
                repeat: String::new(),
                done: false,
                line_no: 1,
                anchor: String::new(),
                span: SourceSpan {
                    line_no: 1,
                    byte_start: 0,
                    byte_end: 60,
                },
            },
            backend::Todo {
                id: "n1:2".into(),
                note_id: "n1".into(),
                kind: "do".into(),
                status: "todo".into(),
                text: "Second".into(),
                project: String::new(),
                person: String::new(),
                start: "2026-06-12".into(),
                due: "2026-06-14".into(),
                external: String::new(),
                priority: String::new(),
                repeat: String::new(),
                done: false,
                line_no: 2,
                anchor: String::new(),
                span: SourceSpan {
                    line_no: 2,
                    byte_start: 61,
                    byte_end: 121,
                },
            },
        ];

        let rows = gantt_items(&todos);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date.to_string(), "06-12");
        assert_eq!(rows[0].start_frac, 0.0);
        assert!(rows[0].span_frac > 0.0);
        assert_eq!(rows[1].date.to_string(), "06-14");
        assert!(rows[1].start_frac > rows[0].start_frac);
    }

    #[test]
    fn filter_summary_and_board_group_are_deterministic() {
        let summary = active_summary(&Filter {
            project: "Acme".into(),
            person: "Jane".into(),
            tag: "followup".into(),
            status: "open".into(),
            ..Default::default()
        });
        assert!(summary.contains("▸Acme"));
        assert!(summary.contains("@Jane"));
        assert_eq!(board_group_key("workflow"), "kind");
        assert_eq!(due_display("week"), "this week");
    }
}
