//! Workflow read models built from indexed Markdown facts.
//!
//! These objects are the contract between the vault/index layer and application
//! workspaces. They keep the GUI from re-deriving 1:1, review, board, and label
//! state from ad hoc note/todo queries.

use super::parse::{parse_links, parse_mentions, parse_properties, parse_tags, parse_todos};
use super::{Backend, Filter, Note, Todo};
use anyhow::Result;
use chrono::Local;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyFact {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Done => "done",
        }
    }

    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Done)
    }
}

impl From<&str> for TaskStatus {
    fn from(value: &str) -> Self {
        match value {
            "doing" => Self::Doing,
            "done" => Self::Done,
            _ => Self::Todo,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskWorkflow {
    Do,
    Mine,
    Followup,
    Delegated,
    Waiting,
    Someday,
    Reading,
}

impl TaskWorkflow {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Do => "do",
            Self::Mine => "mine",
            Self::Followup => "followup",
            Self::Delegated => "delegated",
            Self::Waiting => "waiting",
            Self::Someday => "someday",
            Self::Reading => "reading",
        }
    }
}

impl From<&str> for TaskWorkflow {
    fn from(value: &str) -> Self {
        match value {
            "mine" => Self::Mine,
            "followup" => Self::Followup,
            "delegated" => Self::Delegated,
            "waiting" => Self::Waiting,
            "someday" => Self::Someday,
            "reading" => Self::Reading,
            _ => Self::Do,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSource {
    pub note_id: String,
    pub note_title: String,
    pub note_updated: String,
    pub line_no: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFact {
    pub id: String,
    pub source: TaskSource,
    pub text: String,
    pub status: TaskStatus,
    pub workflow: TaskWorkflow,
    pub people: Vec<String>,
    pub workstreams: Vec<String>,
    pub labels: Vec<String>,
    pub properties: Vec<PropertyFact>,
    pub start: String,
    pub due: String,
    pub priority: String,
    pub external: String,
    pub repeat: String,
}

impl TaskFact {
    pub fn is_open(&self) -> bool {
        self.status.is_open()
    }

    pub fn property(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|p| p.key == key)
            .map(|p| p.value.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct NoteFacts {
    pub labels: Vec<String>,
    pub people: Vec<String>,
    pub workstreams: Vec<String>,
    pub properties: Vec<PropertyFact>,
    pub tasks: Vec<TaskFact>,
    pub primary_task: Option<TaskFact>,
}

#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub note: Note,
    pub title: String,
    pub facts: NoteFacts,
}

#[derive(Debug, Clone)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub updated: String,
    pub labels: Vec<String>,
    pub people: Vec<String>,
    pub workstreams: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    pub id: String,
    pub title: String,
    pub anchor: String,
}

#[derive(Debug, Clone)]
pub struct NoteContext {
    pub note: ParsedNote,
    pub backlinks: Vec<NoteSummary>,
    pub related: Vec<NoteSummary>,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone)]
pub struct OneOnOneContext {
    pub person: String,
    pub current_note: Option<NoteContext>,
    pub history: Vec<NoteSummary>,
    pub previous_notes: Vec<NoteSummary>,
    pub open_items: Vec<TaskFact>,
    pub followups: Vec<TaskFact>,
    pub delegated: Vec<TaskFact>,
    pub waiting: Vec<TaskFact>,
}

#[derive(Debug, Clone)]
pub struct TaskReview {
    pub open: Vec<TaskFact>,
    pub overdue: Vec<TaskFact>,
    pub due: Vec<TaskFact>,
    pub stale: Vec<TaskFact>,
    pub mine: Vec<TaskFact>,
    pub followups: Vec<TaskFact>,
    pub delegated: Vec<TaskFact>,
    pub waiting: Vec<TaskFact>,
    pub someday: Vec<TaskFact>,
}

#[derive(Debug, Clone)]
pub struct WaitingGroup {
    pub person: String,
    pub tasks: Vec<TaskFact>,
}

#[derive(Debug, Clone)]
pub struct WaitingReview {
    pub groups: Vec<WaitingGroup>,
    pub unassigned: Vec<TaskFact>,
}

#[derive(Debug, Clone)]
pub struct BoardColumn {
    pub label: String,
    pub key: String,
    pub tasks: Vec<TaskFact>,
}

#[derive(Debug, Clone)]
pub struct BoardModel {
    pub group_by: String,
    pub columns: Vec<BoardColumn>,
}

#[derive(Debug, Clone)]
pub struct LabelSummary {
    pub name: String,
    pub note_count: i64,
    pub open_task_count: usize,
}

#[derive(Debug, Clone)]
pub struct LabelReview {
    pub labels: Vec<LabelSummary>,
}

#[derive(Debug, Clone)]
pub struct LabelContext {
    pub label: String,
    pub notes: Vec<NoteSummary>,
    pub open_tasks: Vec<TaskFact>,
}

impl Backend {
    pub fn parsed_note(&self, note_id: &str) -> Result<ParsedNote> {
        let note = self.load_note(note_id)?;
        Ok(parsed_note_from_note(note))
    }

    pub fn note_context(&self, note_id: &str) -> Result<NoteContext> {
        let note = self.parsed_note(note_id)?;
        let backlinks = self
            .backlinks(&note.title)?
            .into_iter()
            .map(|note| self.note_summary(note))
            .collect::<Result<Vec<_>>>()?;
        let related = self
            .related_notes(note_id, 12)?
            .into_iter()
            .filter(|related| related.id != note_id)
            .map(|related| self.note_summary_by_id(&related.id))
            .collect::<Result<Vec<_>>>()?;
        let sources = self.source_refs_for_note(&note.note.id, &note.note.body)?;
        Ok(NoteContext {
            note,
            backlinks,
            related,
            sources,
        })
    }

    pub fn one_on_one_context(&self, person: &str) -> Result<OneOnOneContext> {
        let meeting_filter = Filter {
            person: person.to_string(),
            tag: "meeting/one-on-one".into(),
            ..Default::default()
        };
        let history = self
            .query_notes(&meeting_filter)?
            .into_iter()
            .map(|note| self.note_summary(note))
            .collect::<Result<Vec<_>>>()?;

        let current_note = history
            .first()
            .map(|summary| self.note_context(&summary.id))
            .transpose()?;
        let current_id = current_note.as_ref().map(|ctx| ctx.note.note.id.clone());
        let previous_notes = history
            .iter()
            .filter(|summary| Some(summary.id.as_str()) != current_id.as_deref())
            .cloned()
            .collect();

        let open_items = self.task_facts_for_filter(&Filter {
            person: person.to_string(),
            status: "open".into(),
            ..Default::default()
        })?;
        let by_workflow = |workflow: TaskWorkflow| -> Vec<TaskFact> {
            open_items
                .iter()
                .filter(|task| task.workflow == workflow)
                .cloned()
                .collect()
        };

        Ok(OneOnOneContext {
            person: person.to_string(),
            current_note,
            history,
            previous_notes,
            followups: by_workflow(TaskWorkflow::Followup),
            delegated: by_workflow(TaskWorkflow::Delegated),
            waiting: by_workflow(TaskWorkflow::Waiting),
            open_items,
        })
    }

    pub fn task_review(&self) -> Result<TaskReview> {
        let open = self.task_facts_for_filter(&Filter {
            status: "open".into(),
            ..Default::default()
        })?;
        let today = Local::now().format("%Y-%m-%d").to_string();
        let workflow = |kind: TaskWorkflow| -> Vec<TaskFact> {
            open.iter()
                .filter(|task| task.workflow == kind)
                .cloned()
                .collect()
        };
        let stale = self.task_facts_for_todos(self.stale_todos()?)?;
        Ok(TaskReview {
            overdue: open
                .iter()
                .filter(|task| !task.due.is_empty() && task.due.as_str() < today.as_str())
                .cloned()
                .collect(),
            due: open
                .iter()
                .filter(|task| !task.due.is_empty())
                .cloned()
                .collect(),
            stale,
            mine: workflow(TaskWorkflow::Mine),
            followups: workflow(TaskWorkflow::Followup),
            delegated: workflow(TaskWorkflow::Delegated),
            waiting: workflow(TaskWorkflow::Waiting),
            someday: workflow(TaskWorkflow::Someday),
            open,
        })
    }

    pub fn task_list(&self, filter: &Filter) -> Result<Vec<TaskFact>> {
        self.task_facts_for_filter(filter)
    }

    pub fn waiting_review(&self) -> Result<WaitingReview> {
        let review = self.task_review()?;
        let waiting_tasks = review
            .delegated
            .into_iter()
            .chain(review.waiting)
            .collect::<Vec<_>>();
        let mut grouped: BTreeMap<String, Vec<TaskFact>> = BTreeMap::new();
        let mut unassigned = Vec::new();
        for task in waiting_tasks {
            let Some(person) = task.people.first().cloned().filter(|p| !p.is_empty()) else {
                unassigned.push(task);
                continue;
            };
            grouped.entry(person).or_default().push(task);
        }
        let groups = grouped
            .into_iter()
            .map(|(person, tasks)| WaitingGroup { person, tasks })
            .collect();
        Ok(WaitingReview { groups, unassigned })
    }

    pub fn board_model(&self, group_by: &str, filter: &Filter) -> Result<BoardModel> {
        let columns = self
            .board(group_by, filter)?
            .into_iter()
            .map(|(label, key, todos)| {
                Ok(BoardColumn {
                    label,
                    key,
                    tasks: self.task_facts_for_todos(todos)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(BoardModel {
            group_by: group_by.to_string(),
            columns,
        })
    }

    pub fn label_review(&self) -> Result<LabelReview> {
        let labels = self
            .list_tags()?
            .into_iter()
            .map(|label| {
                let open_task_count = self
                    .task_facts_for_filter(&Filter {
                        tag: label.name.clone(),
                        status: "open".into(),
                        ..Default::default()
                    })?
                    .len();
                Ok(LabelSummary {
                    name: label.name,
                    note_count: label.count,
                    open_task_count,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(LabelReview { labels })
    }

    pub fn label_context(&self, label: &str) -> Result<LabelContext> {
        let notes = self
            .query_notes(&Filter {
                tag: label.to_string(),
                ..Default::default()
            })?
            .into_iter()
            .map(|note| self.note_summary(note))
            .collect::<Result<Vec<_>>>()?;
        let open_tasks = self.task_facts_for_filter(&Filter {
            tag: label.to_string(),
            status: "open".into(),
            ..Default::default()
        })?;
        Ok(LabelContext {
            label: label.to_string(),
            notes,
            open_tasks,
        })
    }

    fn note_summary_by_id(&self, note_id: &str) -> Result<NoteSummary> {
        self.note_summary(self.load_note(note_id)?)
    }

    fn source_refs_for_note(&self, note_id: &str, body: &str) -> Result<Vec<SourceRef>> {
        let all_notes = self.query_notes(&Filter {
            show_archived: true,
            ..Default::default()
        })?;
        let mut out = Vec::new();
        for source in source_link_targets(body) {
            let (title, anchor) = split_source_target(&source);
            let Some(note) = all_notes
                .iter()
                .find(|note| note.id != note_id && note.title == title)
            else {
                continue;
            };
            if !out.iter().any(|src: &SourceRef| src.id == note.id) {
                out.push(SourceRef {
                    id: note.id.clone(),
                    title: note.title.clone(),
                    anchor: anchor.clone(),
                });
            }
        }
        Ok(out)
    }

    fn note_summary(&self, note: Note) -> Result<NoteSummary> {
        let parsed = parsed_note_from_note(note);
        Ok(NoteSummary {
            id: parsed.note.id,
            title: parsed.title,
            updated: parsed.note.updated,
            labels: parsed.facts.labels,
            people: parsed.facts.people,
            workstreams: parsed.facts.workstreams,
        })
    }

    fn task_facts_for_filter(&self, filter: &Filter) -> Result<Vec<TaskFact>> {
        self.task_facts_for_todos(self.query_todos(filter)?)
    }

    fn task_facts_for_todos(&self, todos: Vec<Todo>) -> Result<Vec<TaskFact>> {
        let mut parsed_by_note: HashMap<String, ParsedNote> = HashMap::new();
        let mut out = Vec::new();
        for todo in todos {
            let parsed = if let Some(parsed) = parsed_by_note.get(&todo.note_id) {
                parsed
            } else {
                parsed_by_note.insert(todo.note_id.clone(), self.parsed_note(&todo.note_id)?);
                parsed_by_note.get(&todo.note_id).unwrap()
            };
            if let Some(task) = parsed.facts.tasks.iter().find(|task| task.id == todo.id) {
                out.push(task.clone());
            }
        }
        Ok(out)
    }
}

fn source_link_targets(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("source:[[")?;
            let target = rest.split("]]").next()?.trim();
            if target.is_empty() {
                None
            } else {
                Some(target.to_string())
            }
        })
        .collect()
}

fn split_source_target(target: &str) -> (String, String) {
    match target.split_once("#^") {
        Some((title, anchor)) => (title.trim().to_string(), anchor.trim().to_string()),
        None => (target.trim().to_string(), String::new()),
    }
}

fn parsed_note_from_note(note: Note) -> ParsedNote {
    let title = note.title.clone();
    let labels = parse_tags(&note.body);
    let people = parse_mentions(&note.body);
    let workstreams = parse_links(&note.body);
    let properties = parse_properties(&note.body)
        .into_iter()
        .map(|(key, value)| PropertyFact { key, value })
        .collect();
    let tasks = task_facts_from_note(&note);
    let primary_task = first_content_line(&note.body).and_then(|(line_no, _)| {
        tasks
            .iter()
            .find(|task| task.source.line_no == line_no)
            .cloned()
    });

    ParsedNote {
        note,
        title,
        facts: NoteFacts {
            labels,
            people,
            workstreams,
            properties,
            tasks,
            primary_task,
        },
    }
}

fn task_facts_from_note(note: &Note) -> Vec<TaskFact> {
    let line_by_no: HashMap<usize, &str> = note.body.lines().enumerate().collect();
    parse_todos(&note.id, &note.body)
        .into_iter()
        .map(|todo| {
            let raw_line = line_by_no.get(&todo.line_no).copied().unwrap_or("");
            TaskFact {
                id: todo.id.clone(),
                source: TaskSource {
                    note_id: note.id.clone(),
                    note_title: note.title.clone(),
                    note_updated: note.updated.clone(),
                    line_no: todo.line_no,
                },
                text: todo.text.clone(),
                status: TaskStatus::from(todo.status.as_str()),
                workflow: TaskWorkflow::from(todo.kind.as_str()),
                people: parse_mentions(raw_line),
                workstreams: parse_links(raw_line),
                labels: parse_tags(raw_line),
                properties: parse_properties(raw_line)
                    .into_iter()
                    .map(|(key, value)| PropertyFact { key, value })
                    .collect(),
                start: todo.start,
                due: todo.due,
                priority: todo.priority,
                external: todo.external,
                repeat: todo.repeat,
            }
        })
        .collect()
}

fn first_content_line(body: &str) -> Option<(usize, &str)> {
    let mut in_frontmatter = body.starts_with("---\n");
    for (line_no, line) in body.lines().enumerate() {
        if in_frontmatter {
            if line.trim() == "---" && line_no > 0 {
                in_frontmatter = false;
            }
            continue;
        }
        if !line.trim().is_empty() {
            return Some((line_no, line));
        }
    }
    None
}
