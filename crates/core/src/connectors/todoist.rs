//! Todoist connector. Read tasks via the REST v2 API using a **personal API
//! token** (Settings → Integrations → Developer in Todoist) — no OAuth, no IT.
//! Tasks map naturally onto Noet's typed todos: priority → `[#A/B/C]`, project →
//! `+[[Workstream]]`, labels → `#tags`, due → `due:`, plus a `src:todoist:` ref.
//!
//! Pure parts (config, parsing, note shaping) are unit-tested; HTTP is thin IO.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const API: &str = "https://api.todoist.com/rest/v2";

/// The `external` prefix linking a note/todo back to a Todoist task.
pub const TODOIST_REF_PREFIX: &str = "src:todoist:";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TodoistConfig {
    /// Personal API token from Todoist (Settings → Integrations → Developer).
    pub token: String,
}

impl TodoistConfig {
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("noet").join("todoist.json"))
    }
    pub fn load() -> Option<TodoistConfig> {
        Self::load_from(&Self::path()?)
    }
    pub fn load_from(path: &Path) -> Option<TodoistConfig> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path().context("no OS config dir for todoist.json")?)
    }
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
    pub fn is_configured(&self) -> bool {
        !self.token.trim().is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TodoistTask {
    pub id: String,
    pub content: String,
    pub description: String,
    pub project_id: String,
    pub project_name: String, // resolved from the projects map
    pub priority: i64,        // Todoist: 1 (normal) .. 4 (urgent)
    pub due: String,          // YYYY-MM-DD or empty
    pub labels: Vec<String>,
}

/// Map a Todoist priority (1..4, 4 = urgent) to Noet's org-style priority letter.
pub(crate) fn priority_letter(p: i64) -> &'static str {
    match p {
        4 => "A",
        3 => "B",
        2 => "C",
        _ => "",
    }
}

/// Parse a Todoist REST v2 task object (project name is resolved separately).
pub(crate) fn parse_task(json: &serde_json::Value) -> TodoistTask {
    TodoistTask {
        id: json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        content: json.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        description: json.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        project_id: json.get("project_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        project_name: String::new(),
        priority: json.get("priority").and_then(|v| v.as_i64()).unwrap_or(1),
        due: json
            .pointer("/due/date")
            .and_then(|v| v.as_str())
            .map(|d| d.get(..10).unwrap_or(d).to_string())
            .unwrap_or_default(),
        labels: json
            .get("labels")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|l| l.as_str().map(String::from)).collect())
            .unwrap_or_default(),
    }
}

fn get_json(cfg: &TodoistConfig, url: &str) -> Result<serde_json::Value> {
    match ureq::get(url).set("Authorization", &format!("Bearer {}", cfg.token.trim())).call() {
        Ok(r) => r.into_json().context("unexpected Todoist response"),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            anyhow::bail!("Todoist API error (HTTP {code}): {}", body.chars().take(200).collect::<String>())
        }
        Err(e) => anyhow::bail!("network error talking to Todoist: {e}"),
    }
}

/// List active tasks, with project ids resolved to names. `filter` is an optional
/// Todoist filter query (e.g. `today | overdue`); empty means all active tasks.
pub fn list_tasks(cfg: &TodoistConfig, filter: &str) -> Result<Vec<TodoistTask>> {
    if !cfg.is_configured() {
        anyhow::bail!("Todoist isn't configured — add your API token in Settings");
    }
    // project id -> name
    let projects = get_json(cfg, &format!("{API}/projects"))?;
    let names: HashMap<String, String> = projects
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some((
                        p.get("id")?.as_str()?.to_string(),
                        p.get("name")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let url = if filter.trim().is_empty() {
        format!("{API}/tasks")
    } else {
        format!("{API}/tasks?filter={}", urlencode(filter.trim()))
    };
    let tasks = get_json(cfg, &url)?;
    let mut out = Vec::new();
    if let Some(items) = tasks.as_array() {
        for t in items {
            let mut task = parse_task(t);
            task.project_name = names.get(&task.project_id).cloned().unwrap_or_default();
            out.push(task);
        }
    }
    Ok(out)
}

fn urlencode(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// Render a Todoist task into a Noet note `(title, body)`: description as text,
/// then a typed todo with priority, project (`+[[…]]`), labels (`#…`), due, and a
/// `src:todoist:` back-link.
pub fn task_to_note(task: &TodoistTask) -> (String, String) {
    let title = if task.content.trim().is_empty() { "Task".to_string() } else { task.content.trim().to_string() };
    let mut body = String::new();
    if !task.description.trim().is_empty() {
        body.push_str(task.description.trim());
        body.push_str("\n\n");
    }
    let mut todo = String::from("TODO(do) ");
    let prio = priority_letter(task.priority);
    if !prio.is_empty() {
        todo.push_str(&format!("[#{prio}] "));
    }
    todo.push_str(&title);
    if !task.project_name.trim().is_empty() {
        todo.push_str(&format!(" +[[{}]]", task.project_name.trim()));
    }
    for label in &task.labels {
        if !label.trim().is_empty() {
            todo.push_str(&format!(" #{}", label.trim()));
        }
    }
    if !task.due.trim().is_empty() {
        todo.push_str(&format!(" due:{}", task.due.trim()));
    }
    if !task.id.trim().is_empty() {
        todo.push_str(&format!(" {TODOIST_REF_PREFIX}{}", task.id.trim()));
    }
    body.push_str(&todo);
    body.push('\n');
    (title, body)
}

/// Web/app URL that opens a Todoist task.
pub fn task_url(id: &str) -> String {
    format!("https://app.todoist.com/app/task/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_maps_to_letters() {
        assert_eq!(priority_letter(4), "A");
        assert_eq!(priority_letter(3), "B");
        assert_eq!(priority_letter(2), "C");
        assert_eq!(priority_letter(1), "");
    }

    #[test]
    fn parses_task_fields() {
        let json = serde_json::json!({
            "id": "678",
            "content": "Draft Q3 plan",
            "description": "with the leads",
            "project_id": "220",
            "priority": 4,
            "labels": ["work", "urgent"],
            "due": { "date": "2026-07-01" }
        });
        let t = parse_task(&json);
        assert_eq!(t.id, "678");
        assert_eq!(t.content, "Draft Q3 plan");
        assert_eq!(t.priority, 4);
        assert_eq!(t.due, "2026-07-01");
        assert_eq!(t.labels, vec!["work", "urgent"]);
        // datetime due is trimmed to the date
        let dt = parse_task(&serde_json::json!({"id":"1","due":{"date":"2026-07-01T09:00:00"}}));
        assert_eq!(dt.due, "2026-07-01");
        // no due
        assert_eq!(parse_task(&serde_json::json!({"id":"2"})).due, "");
    }

    #[test]
    fn task_to_note_maps_priority_project_labels_due_ref() {
        let t = TodoistTask {
            id: "678".into(),
            content: "Draft Q3 plan".into(),
            description: "with the leads".into(),
            project_id: "220".into(),
            project_name: "Planning".into(),
            priority: 4,
            due: "2026-07-01".into(),
            labels: vec!["work".into(), "urgent".into()],
        };
        let (title, body) = task_to_note(&t);
        assert_eq!(title, "Draft Q3 plan");
        assert!(body.contains("with the leads"));
        assert!(body.contains("TODO(do) [#A] Draft Q3 plan +[[Planning]] #work #urgent due:2026-07-01 src:todoist:678"));
    }

    #[test]
    fn config_roundtrip() {
        let dir = std::env::temp_dir().join(format!("noet-todoist-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("todoist.json");
        assert!(TodoistConfig::load_from(&path).is_none());
        TodoistConfig { token: "abc".into() }.save_to(&path).unwrap();
        assert!(TodoistConfig::load_from(&path).unwrap().is_configured());
        std::fs::remove_dir_all(&dir).ok();
    }
}
