use crate::{AppWindow, State};
use noet_app::{AppCommand, CommandOutcome};
use serde_json::{json, Value};
use slint::Model;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::Instant,
};

static TRACE: OnceLock<UiTrace> = OnceLock::new();

struct UiTrace {
    file: Option<Mutex<File>>,
    include_content: bool,
    started: Instant,
    seq: AtomicU64,
}

pub fn init(vault: &Path) {
    let trace = UiTrace::from_env();
    let enabled = trace.enabled();
    let include_content = trace.include_content;
    let _ = TRACE.set(trace);
    if enabled {
        event(
            "trace_started",
            json!({
                "vault": vault.display().to_string(),
                "include_content": include_content,
            }),
        );
    }
}

pub fn event(event: &str, fields: Value) {
    if let Some(trace) = TRACE.get() {
        trace.write(event, fields);
    }
}

pub fn ui_event(event: &str, ui: &AppWindow, state: &State, fields: Value) {
    if let Some(trace) = TRACE.get() {
        trace.write(
            event,
            json!({
                "fields": fields,
                "ui": snapshot(ui, Some(state), trace.include_content),
            }),
        );
    }
}

pub fn command(
    event: &str,
    ui: &AppWindow,
    state: &State,
    command: &AppCommand,
    outcome: &CommandOutcome,
) {
    if let Some(trace) = TRACE.get() {
        trace.write(
            event,
            json!({
                "command": command,
                "outcome": outcome,
                "ui": snapshot(ui, Some(state), trace.include_content),
            }),
        );
    }
}

pub fn refresh(ui: &AppWindow, state: &State) {
    if let Some(trace) = TRACE.get() {
        trace.write(
            "refresh",
            json!({
                "ui": snapshot(ui, Some(state), trace.include_content),
            }),
        );
    }
}

impl UiTrace {
    fn from_env() -> Self {
        let raw = std::env::var("NOET_UI_TRACE").unwrap_or_default();
        let enabled = !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "off" | "no"
        );
        let include_content = matches!(
            std::env::var("NOET_UI_TRACE_CONTENT")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "on" | "yes"
        );
        let file = if enabled {
            trace_path(&raw).and_then(|path| {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .ok()
                    .map(Mutex::new)
            })
        } else {
            None
        };
        Self {
            file,
            include_content,
            started: Instant::now(),
            seq: AtomicU64::new(1),
        }
    }

    fn enabled(&self) -> bool {
        self.file.is_some()
    }

    fn write(&self, event: &str, fields: Value) {
        let Some(file) = &self.file else {
            return;
        };
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let elapsed_ms = self.started.elapsed().as_millis();
        let line = json!({
            "seq": seq,
            "ts": chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "elapsed_ms": elapsed_ms,
            "event": event,
            "data": fields,
        });
        if let Ok(mut file) = file.lock() {
            let _ = serde_json::to_writer(&mut *file, &line);
            let _ = writeln!(file);
            let _ = file.flush();
        }
    }
}

fn trace_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if matches!(
        trimmed.to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    ) {
        let root = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
        return Some(root.join("noet").join(format!(
            "ui-trace-{}-{}.jsonl",
            chrono::Local::now().format("%Y%m%d-%H%M%S"),
            std::process::id()
        )));
    }
    Some(PathBuf::from(trimmed))
}

fn snapshot(ui: &AppWindow, state: Option<&State>, include_content: bool) -> Value {
    let workspace = state.and_then(|s| s.app.workspaces.active());
    let panes = workspace
        .map(|workspace| {
            workspace
                .panes
                .values()
                .map(|pane| {
                    json!({
                        "id": pane.id,
                        "role": format!("{:?}", pane.role),
                        "placement": format!("{:?}", pane.placement),
                        "surface": pane.surface.id(),
                        "open": pane.open,
                        "collapsed": pane.collapsed,
                        "size": pane.size.current,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut value = json!({
        "view": ui.get_view().to_string(),
        "workspace": {
            "id": ui.get_workspace_id().to_string(),
            "primary": ui.get_workspace_primary().to_string(),
            "title": ui.get_workspace_title().to_string(),
            "nav_surface": ui.get_workspace_nav_surface().to_string(),
            "left_open": ui.get_workspace_left_open(),
            "right_open": ui.get_workspace_right_open(),
            "bottom_open": ui.get_workspace_bottom_open(),
            "left_effective_open": ui.get_workspace_left_eff_open(),
            "right_effective_open": ui.get_workspace_right_eff_open(),
            "bottom_effective_open": ui.get_workspace_bottom_eff_open(),
            "nav_collapsed": ui.get_nav_collapsed(),
            "panes": panes,
        },
        "selection": {
            "current_id": ui.get_current_id().to_string(),
            "current_title": ui.get_current_title().to_string(),
            "selected_person": ui.get_selected_person().to_string(),
        },
        "counts": {
            "notes": ui.get_notes().row_count(),
            "current_todos": ui.get_current_todo_count(),
            "tasks": ui.get_tasks().row_count(),
            "ai_proposals": ui.get_ai_proposals().row_count(),
            "semantic_results": ui.get_ai_semantic_results().row_count(),
        },
        "mode": {
            "editing": ui.get_editing(),
            "source_mode": ui.get_source_mode(),
            "focus_mode": ui.get_focus_mode(),
        },
        "status": ui.get_status_text().to_string(),
        "ai": {
            "status": ui.get_ai_status().to_string(),
            "progress_active": ui.get_ai_progress_active(),
            "progress_label": ui.get_ai_progress_label().to_string(),
            "progress_detail": ui.get_ai_progress_detail().to_string(),
        },
    });
    if include_content {
        value["visible_content"] = json!({
            "search": truncate(&ui.get_search().to_string(), 500),
            "current_body_excerpt": truncate(&ui.get_current_body().to_string(), 4000),
        });
    }
    value
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}
