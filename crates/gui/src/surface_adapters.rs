use chrono::{Datelike, Duration, NaiveDate};
use noet_core::backend;
use noet_core::backend::Filter;
use slint::{Image, ModelRc, SharedString, VecModel};

use crate::{
    BoardColumn, CalCell, FacetItem, FilterChip, GanttItem, MdBlock, NoteItem, NoteRef, NoteTab,
    RelatedRef, Segment, TodoItem,
};

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

pub fn note_items(notes: &[backend::Note]) -> Vec<NoteItem> {
    notes.iter().map(note_item).collect()
}

pub fn recent_note_items(notes: &[backend::Note], limit: usize) -> Vec<NoteItem> {
    notes.iter().take(limit).map(note_item).collect()
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

pub fn label_review_items(review: &backend::LabelReview, active: &str) -> Vec<FacetItem> {
    let items = review
        .labels
        .iter()
        .map(|label| backend::Project {
            name: label.name.clone(),
            count: label.note_count,
        })
        .collect::<Vec<_>>();
    facet_tree_items(&items, active)
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

pub fn todo_items(todos: &[backend::Todo]) -> Vec<TodoItem> {
    todos.iter().map(todo_item).collect()
}

pub fn task_items(tasks: &[backend::TaskFact]) -> Vec<TodoItem> {
    tasks.iter().map(todo_item_from_fact).collect()
}

fn empty_segments() -> ModelRc<Segment> {
    ModelRc::new(VecModel::from(Vec::<Segment>::new()))
}

fn empty_task_fields() -> (
    SharedString,
    SharedString,
    SharedString,
    SharedString,
    SharedString,
) {
    (
        SharedString::from(""),
        SharedString::from(""),
        SharedString::from(""),
        SharedString::from(""),
        SharedString::from(""),
    )
}

fn block_level(kind: &str) -> i32 {
    match kind {
        "h1" => 1,
        "h2" => 2,
        "h3" => 3,
        _ => 99,
    }
}

#[allow(clippy::too_many_arguments)]
fn markdown_block(
    kind: impl Into<SharedString>,
    text: impl Into<SharedString>,
    indent: i32,
    img: Image,
    todo_id: impl Into<SharedString>,
    done: bool,
    status: impl Into<SharedString>,
    task_kind: SharedString,
    project: SharedString,
    person: SharedString,
    due: SharedString,
    priority: SharedString,
    segments: ModelRc<Segment>,
    block_id: i32,
    folded: bool,
) -> MdBlock {
    MdBlock {
        kind: kind.into(),
        text: text.into(),
        indent,
        img,
        todo_id: todo_id.into(),
        done,
        status: status.into(),
        task_kind,
        project,
        person,
        due,
        priority,
        segments,
        block_id,
        folded,
    }
}

pub fn markdown_blocks_model<F>(
    note_id: &str,
    body: &str,
    render_typst: bool,
    folded: &std::collections::HashSet<usize>,
    mut typst_image: F,
) -> ModelRc<MdBlock>
where
    F: FnMut(&str) -> Option<Image>,
{
    let todos = backend::parse_todos(note_id, body);
    let mut todo_iter = todos.iter();
    let empty = Image::default();
    let mut out = Vec::new();
    let mut hide_level: Option<i32> = None;

    for (idx, block) in backend::markdown_blocks(body).into_iter().enumerate() {
        let level = block_level(&block.kind);
        let todo = if block.kind == "todo" {
            todo_iter.next().cloned()
        } else {
            None
        };
        let hidden = match hide_level {
            Some(active_level) if level <= active_level => {
                hide_level = None;
                false
            }
            Some(_) => true,
            None => false,
        };
        if hidden {
            continue;
        }

        let block_id = idx as i32;
        if block.kind == "typst" {
            if render_typst {
                if let Some(img) = typst_image(&block.text) {
                    let (task_kind, project, person, due, priority) = empty_task_fields();
                    out.push(markdown_block(
                        "typst",
                        "",
                        block.indent,
                        img,
                        "",
                        false,
                        "",
                        task_kind,
                        project,
                        person,
                        due,
                        priority,
                        empty_segments(),
                        block_id,
                        false,
                    ));
                    continue;
                }
            }
            let (task_kind, project, person, due, priority) = empty_task_fields();
            out.push(markdown_block(
                "code",
                block.text,
                block.indent,
                empty.clone(),
                "",
                false,
                "",
                task_kind,
                project,
                person,
                due,
                priority,
                empty_segments(),
                block_id,
                false,
            ));
        } else if block.kind == "todo" {
            let t = todo.as_ref();
            let id = t.map(|t| t.id.clone()).unwrap_or_default();
            let done = t.map(|t| t.done).unwrap_or(false);
            let status = t.map(|t| t.status.clone()).unwrap_or_default();
            let priority = t.map(|t| t.priority.clone()).unwrap_or_default();
            out.push(markdown_block(
                "todo",
                t.map(|t| t.text.clone())
                    .unwrap_or_else(|| block.text.clone()),
                block.indent,
                empty.clone(),
                id,
                done,
                status,
                t.map(|t| t.kind.clone()).unwrap_or_default().into(),
                t.map(|t| t.project.clone()).unwrap_or_default().into(),
                t.map(|t| t.person.clone()).unwrap_or_default().into(),
                t.map(|t| t.due.clone()).unwrap_or_default().into(),
                priority.into(),
                empty_segments(),
                block_id,
                false,
            ));
        } else if block.kind == "code" || block.kind == "rule" {
            let (task_kind, project, person, due, priority) = empty_task_fields();
            out.push(markdown_block(
                block.kind,
                block.text,
                block.indent,
                empty.clone(),
                "",
                false,
                "",
                task_kind,
                project,
                person,
                due,
                priority,
                empty_segments(),
                block_id,
                false,
            ));
        } else if level <= 3 {
            let is_folded = folded.contains(&idx);
            let (task_kind, project, person, due, priority) = empty_task_fields();
            out.push(markdown_block(
                block.kind,
                backend::clean_inline(&block.text),
                block.indent,
                empty.clone(),
                "",
                false,
                "",
                task_kind,
                project,
                person,
                due,
                priority,
                empty_segments(),
                block_id,
                is_folded,
            ));
            if is_folded {
                hide_level = Some(level);
            }
        } else {
            let inline = matches!(
                block.kind.as_str(),
                "para" | "bullet" | "numbered" | "quote"
            );
            let segments = if inline {
                backend::line_segments(&block.text)
            } else {
                Vec::new()
            };
            let has_link = segments.iter().any(|segment| !segment.kind.is_empty());
            let (task_kind, project, person, due, priority) = empty_task_fields();
            if has_link && block.text.chars().count() < 160 {
                let segments = segments
                    .iter()
                    .map(|segment| Segment {
                        text: segment.text.clone().into(),
                        kind: segment.kind.clone().into(),
                        value: segment.value.clone().into(),
                    })
                    .collect::<Vec<_>>();
                out.push(markdown_block(
                    block.kind,
                    "",
                    block.indent,
                    empty.clone(),
                    "",
                    false,
                    "",
                    task_kind,
                    project,
                    person,
                    due,
                    priority,
                    ModelRc::new(VecModel::from(segments)),
                    block_id,
                    false,
                ));
            } else {
                out.push(markdown_block(
                    block.kind,
                    backend::clean_inline(&block.text),
                    block.indent,
                    empty.clone(),
                    "",
                    false,
                    "",
                    task_kind,
                    project,
                    person,
                    due,
                    priority,
                    empty_segments(),
                    block_id,
                    false,
                ));
            }
        }
    }

    ModelRc::new(VecModel::from(out))
}

#[derive(Clone, Default)]
pub struct AgendaSurface {
    pub overdue: Vec<TodoItem>,
    pub today: Vec<TodoItem>,
    pub week: Vec<TodoItem>,
    pub later: Vec<TodoItem>,
}

pub fn agenda_surface(items: &[backend::Todo], today: NaiveDate) -> AgendaSurface {
    let today_label = today.format("%Y-%m-%d").to_string();
    let week = (today + Duration::days(7)).format("%Y-%m-%d").to_string();
    let mut surface = AgendaSurface::default();
    for todo in items {
        let row = todo_item(todo);
        if todo.due.as_str() < today_label.as_str() {
            surface.overdue.push(row);
        } else if todo.due == today_label {
            surface.today.push(row);
        } else if todo.due.as_str() <= week.as_str() {
            surface.week.push(row);
        } else {
            surface.later.push(row);
        }
    }
    surface
}

#[derive(Clone, Default)]
pub struct WorkstreamSurface {
    pub todos: Vec<TodoItem>,
    pub notes: Vec<NoteRef>,
}

pub fn workstream_surface(todos: &[backend::Todo], notes: &[backend::Note]) -> WorkstreamSurface {
    WorkstreamSurface {
        todos: todo_items(todos),
        notes: note_refs_from_notes(notes),
    }
}

#[derive(Clone, Default)]
pub struct LabelContextSurface {
    pub label: String,
    pub notes: Vec<NoteRef>,
    pub open_tasks: Vec<TodoItem>,
}

pub fn label_context_surface(context: &backend::LabelContext) -> LabelContextSurface {
    LabelContextSurface {
        label: context.label.clone(),
        notes: note_refs_from_summaries(&context.notes),
        open_tasks: task_items(&context.open_tasks),
    }
}

fn month_name(month: u32) -> &'static str {
    [
        "",
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ]
    .get(month as usize)
    .copied()
    .unwrap_or("")
}

#[derive(Clone)]
pub struct CalendarSurface {
    pub label: String,
    pub cells: Vec<CalCell>,
}

pub fn calendar_surface(
    todos: &[backend::Todo],
    year: i32,
    month: u32,
    today: NaiveDate,
) -> CalendarSurface {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_default();
    let start_pad = first.weekday().num_days_from_monday() as usize;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let days = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap_or_default()
        .signed_duration_since(first)
        .num_days() as usize;
    let prefix = format!("{year}-{month:02}-");
    let mut by_date: std::collections::HashMap<String, Vec<TodoItem>> =
        std::collections::HashMap::new();
    for todo in todos {
        if todo.due.starts_with(&prefix) {
            by_date
                .entry(todo.due.clone())
                .or_default()
                .push(todo_item(todo));
        }
    }

    let today = today.format("%Y-%m-%d").to_string();
    let empty = || ModelRc::new(VecModel::from(Vec::<TodoItem>::new()));
    let mut cells = Vec::with_capacity(42);
    for idx in 0..42usize {
        if idx < start_pad || idx >= start_pad + days {
            cells.push(CalCell {
                day: 0,
                date: "".into(),
                today: false,
                items: empty(),
            });
        } else {
            let day = (idx - start_pad + 1) as u32;
            let date = format!("{year}-{month:02}-{day:02}");
            let items = by_date.remove(&date).unwrap_or_default();
            cells.push(CalCell {
                day: day as i32,
                date: date.clone().into(),
                today: date == today,
                items: ModelRc::new(VecModel::from(items)),
            });
        }
    }

    CalendarSurface {
        label: format!("{} {year}", month_name(month)),
        cells,
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

pub fn trash_note_refs(notes: &[(String, String)]) -> Vec<NoteRef> {
    notes
        .iter()
        .map(|(id, title)| NoteRef {
            id: id.clone().into(),
            title: title.clone().into(),
        })
        .collect()
}

pub fn note_tabs(pinned: &[(String, String)], recents: &[(String, String)]) -> Vec<NoteTab> {
    let pinned_ids = pinned
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut tabs = pinned
        .iter()
        .map(|(id, title)| NoteTab {
            id: id.clone().into(),
            title: title.clone().into(),
            pinned: true,
        })
        .collect::<Vec<_>>();
    tabs.extend(
        recents
            .iter()
            .filter(|(id, _)| !pinned_ids.contains(id.as_str()))
            .map(|(id, title)| NoteTab {
                id: id.clone().into(),
                title: title.clone().into(),
                pinned: false,
            }),
    );
    tabs
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

pub fn active_filter_chips(f: &Filter) -> Vec<FilterChip> {
    let mut chips = Vec::new();
    let mut chip = |label: String, dim: &str| {
        chips.push(FilterChip {
            label: label.into(),
            dim: dim.into(),
        });
    };
    if !f.project.is_empty() {
        chip(format!("▸ {}", f.project), "project");
    }
    if !f.person.is_empty() {
        chip(format!("@ {}", f.person), "person");
    }
    if !f.tag.is_empty() {
        chip(format!("# {}", f.tag), "tag");
    }
    if !f.kind.is_empty() {
        chip(format!("workflow: {}", f.kind), "kind");
    }
    if !f.priority.is_empty() {
        chip(format!("priority {}", f.priority), "priority");
    }
    if !f.due_bucket.is_empty() {
        chip(format!("due: {}", due_display(&f.due_bucket)), "due");
    }
    if !f.status.is_empty() {
        chip(format!("status: {}", f.status), "status");
    }
    if !f.search.is_empty() {
        chip(format!("search: {}", f.search), "search");
    }
    chips
}

pub fn filter_value_or_any(value: &str) -> String {
    if value.is_empty() {
        "any".into()
    } else {
        value.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backend::{
        LabelContext, LabelReview, LabelSummary, NoteSummary, PropertyFact, RelatedNote, SourceRef,
        SourceSpan, TaskFact, TaskReview, TaskSource, TaskStatus, TaskWorkflow, WaitingGroup,
        WaitingReview,
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
            workstreams: vec!["workstream/client-acme".into()],
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

    fn note(id: &str, title: &str, updated: &str) -> backend::Note {
        backend::Note {
            id: id.into(),
            title: title.into(),
            created: "2026-06-01T09:00:00".into(),
            updated: updated.into(),
            kind: "markdown".into(),
            body: String::new(),
            path: PathBuf::from(format!("{id}.md")),
        }
    }

    fn todo(id: &str, due: &str, text: &str) -> backend::Todo {
        backend::Todo {
            id: id.into(),
            note_id: "n1".into(),
            kind: "followup".into(),
            status: "todo".into(),
            text: text.into(),
            project: "workstream/client-acme".into(),
            person: "Jane".into(),
            start: String::new(),
            due: due.into(),
            external: String::new(),
            priority: "B".into(),
            repeat: String::new(),
            done: false,
            line_no: 3,
            anchor: String::new(),
            span: SourceSpan {
                line_no: 3,
                byte_start: 10,
                byte_end: 72,
            },
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
            project: "workstream/acme".into(),
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
    fn agenda_surface_buckets_by_due_date() {
        let items = vec![
            todo("n1:1", "2026-06-10", "Overdue"),
            todo("n1:2", "2026-06-11", "Today"),
            todo("n1:3", "2026-06-18", "This week"),
            todo("n1:4", "2026-06-19", "Later"),
        ];

        let surface = agenda_surface(&items, NaiveDate::from_ymd_opt(2026, 6, 11).unwrap());
        assert_eq!(surface.overdue[0].text.to_string(), "Overdue");
        assert_eq!(surface.today[0].text.to_string(), "Today");
        assert_eq!(surface.week[0].text.to_string(), "This week");
        assert_eq!(surface.later[0].text.to_string(), "Later");
    }

    #[test]
    fn list_workstream_and_trash_surfaces_are_stable() {
        let notes = vec![
            note("n1", "Newest", "2026-06-11T10:30:00"),
            note("n2", "Older", "2026-06-10T10:30:00"),
        ];
        let todos = vec![todo("n1:1", "2026-06-20", "Follow up")];

        let hub = workstream_surface(&todos, &notes);
        assert_eq!(hub.todos[0].text.to_string(), "Follow up");
        assert_eq!(hub.notes[0].title.to_string(), "Newest");

        let rows = note_items(&notes);
        assert_eq!(rows.len(), 2);
        assert_eq!(recent_note_items(&notes, 1)[0].title.to_string(), "Newest");
        assert_eq!(
            todo_items(&todos)[0].project.to_string(),
            "workstream/client-acme"
        );

        let trash = trash_note_refs(&[("old.md".into(), "Deleted note".into())]);
        assert_eq!(trash[0].id.to_string(), "old.md");
        assert_eq!(trash[0].title.to_string(), "Deleted note");
    }

    #[test]
    fn note_tabs_put_pins_before_recents_and_skip_duplicate_recents() {
        let pinned = vec![("n1".into(), "Pinned".into())];
        let recents = vec![
            ("n2".into(), "Recent".into()),
            ("n1".into(), "Pinned duplicate".into()),
        ];

        let tabs = note_tabs(&pinned, &recents);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].id.to_string(), "n1");
        assert_eq!(tabs[0].title.to_string(), "Pinned");
        assert!(tabs[0].pinned);
        assert_eq!(tabs[1].id.to_string(), "n2");
        assert_eq!(tabs[1].title.to_string(), "Recent");
        assert!(!tabs[1].pinned);
    }

    #[test]
    fn calendar_surface_builds_fixed_month_grid() {
        let todos = vec![
            todo("n1:1", "2026-06-11", "Due today"),
            todo("n1:2", "2026-07-01", "Outside month"),
        ];

        let surface = calendar_surface(
            &todos,
            2026,
            6,
            NaiveDate::from_ymd_opt(2026, 6, 11).unwrap(),
        );
        assert_eq!(surface.label, "June 2026");
        assert_eq!(surface.cells.len(), 42);
        assert_eq!(surface.cells[0].day, 1);

        let today = surface
            .cells
            .iter()
            .find(|cell| cell.date.to_string() == "2026-06-11")
            .unwrap();
        assert!(today.today);
        assert_eq!(today.items.row_count(), 1);
        assert_eq!(
            today.items.row_data(0).unwrap().text.to_string(),
            "Due today"
        );

        let july_items = surface
            .cells
            .iter()
            .filter(|cell| cell.items.row_count() > 0)
            .count();
        assert_eq!(july_items, 1);
    }

    #[test]
    fn active_filter_chips_are_deterministic() {
        let filter = Filter {
            project: "workstream/client-acme".into(),
            person: "Jane".into(),
            tag: "meeting".into(),
            kind: "followup".into(),
            priority: "A".into(),
            due_bucket: "week".into(),
            status: "open".into(),
            search: "risk".into(),
            show_archived: false,
        };

        let labels = active_filter_chips(&filter)
            .iter()
            .map(|chip| chip.label.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                "▸ workstream/client-acme",
                "@ Jane",
                "# meeting",
                "workflow: followup",
                "priority A",
                "due: this week",
                "status: open",
                "search: risk"
            ]
        );
        assert_eq!(filter_value_or_any(""), "any");
        assert_eq!(filter_value_or_any("A"), "A");
    }

    #[test]
    fn markdown_blocks_model_resolves_todos_segments_and_folds() {
        let mut folded = std::collections::HashSet::new();
        folded.insert(1);
        let body = "\
# Heading [[Client/Acme]]
## Folded
Hidden detail
# Visible
- [x] Close task #mine @[[Jane]] [[Client/Acme]] #workstream/client-acme due:2026-06-20 priority:A
See [[Client/Acme]] and https://example.test
";

        let blocks = markdown_blocks_model("n1", body, false, &folded, |_| None);
        assert_eq!(blocks.row_count(), 5);
        let h1 = blocks.row_data(0).unwrap();
        assert_eq!(h1.kind.to_string(), "h1");
        assert_eq!(h1.text.to_string(), "Heading Client/Acme");
        assert!(!h1.folded);

        let h2 = blocks.row_data(1).unwrap();
        assert_eq!(h2.kind.to_string(), "h2");
        assert_eq!(h2.text.to_string(), "Folded");
        assert!(h2.folded);

        let visible = blocks.row_data(2).unwrap();
        assert_eq!(visible.kind.to_string(), "h1");
        assert_eq!(visible.text.to_string(), "Visible");

        let todo = blocks.row_data(3).unwrap();
        assert_eq!(todo.kind.to_string(), "todo");
        assert_eq!(todo.text.to_string(), "Close task");
        assert_eq!(todo.todo_id.to_string(), "n1:4");
        assert!(todo.done);
        assert_eq!(todo.status.to_string(), "done");
        assert_eq!(todo.task_kind.to_string(), "mine");
        assert_eq!(todo.project.to_string(), "workstream/client-acme");
        assert_eq!(todo.person.to_string(), "Jane");
        assert_eq!(todo.due.to_string(), "2026-06-20");
        assert_eq!(todo.priority.to_string(), "A");

        let para = blocks.row_data(4).unwrap();
        assert_eq!(para.kind.to_string(), "para");
        assert_eq!(para.text.to_string(), "");
        assert!(para.segments.row_count() > 1);
        assert!(para
            .segments
            .row_data(0)
            .map(|segment| segment.text.to_string().contains("See"))
            .unwrap_or(false));
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
            workstreams: vec!["workstream/client-acme".into()],
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
        assert_eq!(row.project.to_string(), "workstream/client-acme");
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
    fn label_review_and_context_surfaces_are_stable() {
        let review = LabelReview {
            labels: vec![
                LabelSummary {
                    name: "meeting".into(),
                    note_count: 2,
                    open_task_count: 0,
                },
                LabelSummary {
                    name: "meeting/one-on-one".into(),
                    note_count: 3,
                    open_task_count: 2,
                },
            ],
        };

        let rows = label_review_items(&review, "meeting/one-on-one");
        let meeting = rows
            .iter()
            .find(|row| row.name.to_string() == "meeting")
            .unwrap();
        assert_eq!(meeting.count, 5);
        assert_eq!(meeting.label.to_string(), "meeting");
        let one_on_one = rows
            .iter()
            .find(|row| row.name.to_string() == "meeting/one-on-one")
            .unwrap();
        assert_eq!(one_on_one.depth, 1);
        assert!(one_on_one.active);

        let context = LabelContext {
            label: "meeting/one-on-one".into(),
            notes: vec![note_summary("n1", "Jane 1:1")],
            open_tasks: vec![task(
                "n1:4",
                "n1",
                "Follow up",
                TaskWorkflow::Followup,
                TaskStatus::Todo,
            )],
        };
        let surface = label_context_surface(&context);
        assert_eq!(surface.label, "meeting/one-on-one");
        assert_eq!(surface.notes[0].title.to_string(), "Jane 1:1");
        assert_eq!(surface.open_tasks[0].text.to_string(), "Follow up");
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
            project: "workstream/acme".into(),
            person: "Jane".into(),
            tag: "followup".into(),
            status: "open".into(),
            ..Default::default()
        });
        assert!(summary.contains("▸workstream/acme"));
        assert!(summary.contains("@Jane"));
        assert_eq!(board_group_key("workflow"), "kind");
        assert_eq!(due_display("week"), "this week");
    }
}
