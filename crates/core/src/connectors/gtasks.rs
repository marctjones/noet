//! Google Tasks connector. Reuses the shared Google OAuth from [`super::gmail`]
//! (same credentials/refresh token — the consent already requests the Tasks
//! scope), so connecting Google once covers both Gmail and Tasks.
//!
//! Pure parts (task parsing + note shaping) are unit-tested; the HTTP is thin IO.

use super::gmail::{self, GmailConfig};
use anyhow::Result;

const API: &str = "https://tasks.googleapis.com/tasks/v1";

/// The `external` prefix linking a note/todo back to a Google Task.
pub const GTASK_REF_PREFIX: &str = "src:gtask:";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoogleTask {
    pub id: String,
    pub title: String,
    pub notes: String,
    /// Due date as YYYY-MM-DD (Google Tasks dues are date-only), or empty.
    pub due: String,
    /// The task list this came from (mapped to a workstream).
    pub list_title: String,
}

/// Parse a Google Tasks `tasks.get`/`list` item; `list_title` is the parent list.
pub(crate) fn parse_task(json: &serde_json::Value, list_title: &str) -> GoogleTask {
    let s = |k: &str| {
        json.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    GoogleTask {
        id: s("id"),
        title: s("title"),
        notes: s("notes"),
        // due is RFC3339 (e.g. 2026-06-10T00:00:00.000Z) — keep the date part.
        due: s("due").get(..10).unwrap_or("").to_string(),
        list_title: list_title.to_string(),
    }
}

/// List incomplete tasks across all of the account's task lists.
pub fn list_tasks(cfg: &GmailConfig, max_per_list: u32) -> Result<Vec<GoogleTask>> {
    let token = gmail::access_token(cfg)?;
    let lists = gmail::get_json(
        ureq::get(&format!("{API}/users/@me/lists"))
            .set("Authorization", &format!("Bearer {token}")),
    )?;
    let lists = lists
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for list in &lists {
        let list_id = list.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let list_title = list
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Tasks");
        if list_id.is_empty() {
            continue;
        }
        let tasks = gmail::get_json(
            ureq::get(&format!("{API}/lists/{list_id}/tasks"))
                .set("Authorization", &format!("Bearer {token}"))
                .query("showCompleted", "false")
                .query("maxResults", &max_per_list.to_string()),
        )?;
        if let Some(items) = tasks.get("items").and_then(|v| v.as_array()) {
            for t in items {
                out.push(parse_task(t, list_title));
            }
        }
    }
    Ok(out)
}

/// Render a Google Task into a Noet note `(title, body)`: the task notes as text,
/// then a task-list item filed under the list (`[[List]]`) with the due date and
/// a `src:gtask:` back-link.
pub fn task_to_note(task: &GoogleTask) -> (String, String) {
    let title = if task.title.trim().is_empty() {
        "Task".to_string()
    } else {
        task.title.trim().to_string()
    };
    let mut body = String::new();
    if !task.notes.trim().is_empty() {
        body.push_str(task.notes.trim());
        body.push_str("\n\n");
    }
    let mut todo = format!("- [ ] {title}");
    if !task.list_title.trim().is_empty() {
        todo.push_str(&format!(" [[{}]]", task.list_title.trim()));
    }
    if !task.due.trim().is_empty() {
        todo.push_str(&format!(" due:{}", task.due.trim()));
    }
    if !task.id.trim().is_empty() {
        todo.push_str(&format!(" {GTASK_REF_PREFIX}{}", task.id.trim()));
    }
    body.push_str(&todo);
    body.push('\n');
    (title, body)
}

/// Web URL for Google Tasks (there's no stable per-task deep link).
pub fn tasks_url() -> &'static str {
    "https://tasks.google.com/"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_task_and_trims_due() {
        let json = serde_json::json!({
            "id": "T1", "title": "Renew domain", "notes": "via registrar",
            "due": "2026-07-01T00:00:00.000Z", "status": "needsAction"
        });
        let t = parse_task(&json, "Work");
        assert_eq!(t.id, "T1");
        assert_eq!(t.title, "Renew domain");
        assert_eq!(t.notes, "via registrar");
        assert_eq!(t.due, "2026-07-01"); // date part only
        assert_eq!(t.list_title, "Work");
    }

    #[test]
    fn task_to_note_files_under_list_with_due_and_ref() {
        let t = GoogleTask {
            id: "T1".into(),
            title: "Renew domain".into(),
            notes: "via registrar".into(),
            due: "2026-07-01".into(),
            list_title: "Work".into(),
        };
        let (title, body) = task_to_note(&t);
        assert_eq!(title, "Renew domain");
        assert!(body.contains("via registrar"));
        assert!(body.contains("- [ ] Renew domain [[Work]] due:2026-07-01 src:gtask:T1"));
    }
}
