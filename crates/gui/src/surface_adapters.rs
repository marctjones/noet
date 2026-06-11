use chrono::NaiveDate;
use noet_core::backend;
use noet_core::backend::Filter;

use crate::{FacetItem, GanttItem, NoteItem, TodoItem};

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
    use backend::{PropertyFact, SourceSpan, TaskFact, TaskSource, TaskStatus, TaskWorkflow};
    use std::path::PathBuf;

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
