use crate::{ui_trace, AppWindow, PaletteItem};
use noet_core::backend::{Backend, Filter};

const PALETTE_VIEWS: &[(&str, &str)] = &[
    ("today", "Today"),
    ("agenda", "Agenda"),
    ("calendar", "Calendar"),
    ("tasks", "Tasks"),
    ("board", "Board"),
    ("gantt", "Gantt"),
    ("waiting", "Waiting on"),
    ("notes", "Notes"),
    ("people", "People"),
    ("labels", "Labels"),
    ("inbox", "Inbox"),
    ("trash", "Trash"),
    ("settings", "Settings"),
    ("about", "About / open-source licenses"),
];

const PALETTE_CMDS: &[(&str, &str)] = &[
    ("new-note", "New note"),
    ("new-meeting", "New meeting note"),
    ("capture", "Quick capture"),
    ("reindex", "Reindex vault"),
    ("clear-filters", "Clear all filters"),
    ("rail", "Toggle filter rail"),
    ("nav", "Toggle sidebar"),
    ("ai-draft-agenda", "AI draft agenda"),
    ("ai-review-note", "AI review note"),
    ("ai-refresh-embeddings", "AI refresh embeddings"),
    ("ai-semantic-search", "AI semantic search"),
];

pub(crate) fn results(b: &Backend, query: &str) -> Vec<PaletteItem> {
    let q = query.trim().to_lowercase();
    let hit = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
    let mk = |id: String, label: String, kind: &str, hint: String| PaletteItem {
        id: id.into(),
        label: label.into(),
        kind: kind.into(),
        hint: hint.into(),
    };
    let mut out: Vec<PaletteItem> = Vec::new();
    for (v, l) in PALETTE_VIEWS {
        if hit(l) {
            out.push(mk(format!("v:{v}"), (*l).into(), "VIEW", String::new()));
        }
    }
    for (c, l) in PALETTE_CMDS {
        if hit(l) {
            out.push(mk(format!("c:{c}"), (*l).into(), "COMMAND", String::new()));
        }
    }
    if let Ok(notes) = b.query_notes(&Filter::default()) {
        let limit = if q.is_empty() { 8 } else { 60 };
        for n in notes.iter().filter(|n| hit(&n.title)).take(limit) {
            out.push(mk(
                format!("n:{}", n.id),
                n.title.clone(),
                "NOTE",
                n.updated.replace('T', " "),
            ));
        }
    }
    if !q.is_empty() {
        if let Ok(ps) = b.list_projects() {
            for p in ps.iter().filter(|p| hit(&p.name)) {
                out.push(mk(
                    format!("p:{}", p.name),
                    format!("▸ {}", p.name),
                    "PROJECT",
                    format!("{} notes", p.count),
                ));
            }
        }
        if let Ok(ts) = b.list_tags() {
            for p in ts.iter().filter(|p| hit(&p.name)) {
                out.push(mk(
                    format!("t:{}", p.name),
                    format!("# {}", p.name),
                    "TAG",
                    String::new(),
                ));
            }
        }
        if let Ok(pe) = b.list_people() {
            for p in pe.iter().filter(|p| hit(&p.name)) {
                out.push(mk(
                    format!("@:{}", p.name),
                    format!("@ {}", p.name),
                    "PERSON",
                    String::new(),
                ));
            }
        }
    }
    out.truncate(80);
    out
}

pub(crate) fn activate(ui: &AppWindow, id: &str) {
    ui_trace::event("callback.palette_activate", serde_json::json!({ "id": id }));
    if let Some(v) = id.strip_prefix("v:") {
        ui.invoke_set_view(v.into());
    } else if let Some(nid) = id.strip_prefix("n:") {
        ui.invoke_set_view("notes".into());
        ui.invoke_select_note(nid.into());
    } else if let Some(name) = id.strip_prefix("p:") {
        ui.invoke_clear_filter("project".into());
        ui.invoke_toggle_project(name.into());
        ui.invoke_workspace_switch("notes".into());
    } else if let Some(name) = id.strip_prefix("t:") {
        ui.invoke_toggle_tag(name.into());
        ui.invoke_set_view("notes".into());
    } else if let Some(name) = id.strip_prefix("@:") {
        ui.invoke_toggle_person(name.into());
        ui.invoke_set_view("notes".into());
    } else if let Some(c) = id.strip_prefix("c:") {
        match c {
            "new-note" => ui.invoke_new_note(),
            "new-meeting" => crate::dispatch_cmd(ui, "new-meeting"),
            "capture" => crate::dispatch_cmd(ui, "capture"),
            "reindex" => ui.invoke_reindex(),
            "clear-filters" => ui.invoke_clear_filters(),
            "rail" => ui.set_rail_hidden(!ui.get_rail_hidden()),
            "nav" => ui.set_nav_collapsed(!ui.get_nav_collapsed()),
            "ai-draft-agenda" => ui.invoke_ai_draft_agenda(),
            "ai-review-note" => ui.invoke_ai_review_note(),
            "ai-refresh-embeddings" => ui.invoke_ai_refresh_embeddings(),
            "ai-semantic-search" => ui.invoke_ai_semantic_search(ui.get_search()),
            _ => {}
        }
    }
}
