//! Outlook connector — import the currently selected **Classic** Outlook email
//! as a Noet note. Classic Outlook exposes a COM Object Model; the new
//! (web-based) Outlook does not, so this is Classic-only and Windows-only.
//!
//! Rather than bind COM through FFI, we drive it through a short PowerShell
//! script (`New-Object -ComObject Outlook.Application`) that emits the selected
//! message as JSON. That keeps the Rust side small and cross-platform-compilable
//! — the platform gate is just whether we run the script. The data shaping
//! ([`parse_mail_json`], [`mail_to_note`]) is pure and unit-tested everywhere.

use anyhow::{Context, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutlookMail {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub sender_email: String,
    #[serde(default)]
    pub received: String,
    #[serde(default)]
    pub body: String,
    /// Outlook's stable per-item id. Lets Noet link back to the exact message
    /// (`src:outlook:<entry_id>`) and reopen it via COM. Empty if unknown.
    #[serde(default)]
    pub entry_id: String,
    /// The flag's due date, if any (YYYY-MM-DD); maps to the review todo's `due:`.
    #[serde(default)]
    pub due: String,
}

/// The `external` prefix used to link a note/todo back to an Outlook message.
pub const OUTLOOK_REF_PREFIX: &str = "src:outlook:";

/// If `external` is an Outlook back-link (`src:outlook:<EntryID>`), return the
/// EntryID. Pure + tested; used to decide how to open a ref.
pub fn entry_id_of(external: &str) -> Option<&str> {
    let id = external.trim().strip_prefix(OUTLOOK_REF_PREFIX)?.trim();
    (!id.is_empty()).then_some(id)
}

/// Whether Outlook import is available here (Classic Outlook COM is Windows-only).
/// The GUI uses this to decide whether to even attempt an import.
pub fn is_supported() -> bool {
    cfg!(windows)
}

/// The PowerShell that drives Classic Outlook's COM model and prints the selected
/// mail item as compact JSON. Kept as a pure string so it's inspectable/testable.
#[cfg_attr(not(windows), allow(dead_code))] // run only on Windows; exercised by tests
pub(crate) fn ps_script() -> &'static str {
    r#"
$ErrorActionPreference = 'Stop'
$ol = New-Object -ComObject Outlook.Application
$explorer = $ol.ActiveExplorer()
if ($null -eq $explorer) { Write-Error 'No active Outlook window'; exit 1 }
$sel = $explorer.Selection
if ($null -eq $sel -or $sel.Count -lt 1) { Write-Error 'No message selected in Outlook'; exit 1 }
$m = $sel.Item(1)
[pscustomobject]@{
  subject      = [string]$m.Subject
  sender       = [string]$m.SenderName
  sender_email = [string]$m.SenderEmailAddress
  received     = if ($m.ReceivedTime) { $m.ReceivedTime.ToString('s') } else { '' }
  body         = [string]$m.Body
  entry_id     = [string]$m.EntryID
  due          = if ($m.TaskDueDate) { $m.TaskDueDate.ToString('yyyy-MM-dd') } else { '' }
} | ConvertTo-Json -Compress
"#
}

/// Parse the JSON the PowerShell bridge prints into an [`OutlookMail`].
#[cfg_attr(not(windows), allow(dead_code))] // used on Windows + by tests
pub(crate) fn parse_mail_json(json: &str) -> Result<OutlookMail> {
    serde_json::from_str(json.trim()).context("could not parse the Outlook message JSON")
}

/// Import the currently selected Classic-Outlook email. Windows-only; elsewhere
/// it returns an error (callers gate on [`is_supported`] and surface the message).
pub fn import_selected() -> Result<OutlookMail> {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps_script()])
            .output()
            .context("failed to launch PowerShell")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!(
                "Outlook import failed: {}",
                err.lines().last().unwrap_or("is Classic Outlook running?").trim()
            );
        }
        parse_mail_json(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Outlook import is only available on Windows (Classic Outlook)")
    }
}

/// Render an imported email into a Noet note: a `(title, body)` pair. The sender
/// becomes an `@[[Person]]` mention and a `TODO(followup)` is seeded so the email
/// lands as an actionable note. Pure + tested.
pub fn mail_to_note(mail: &OutlookMail) -> (String, String) {
    let subject = mail.subject.trim();
    let title = if subject.is_empty() { "Email".to_string() } else { subject.to_string() };

    // Prefer the display name for the @mention; fall back to the address.
    let who = if !mail.sender.trim().is_empty() {
        mail.sender.trim()
    } else {
        mail.sender_email.trim()
    };

    let mut body = String::new();
    let from = match (mail.sender.trim(), mail.sender_email.trim()) {
        ("", "") => String::new(),
        ("", email) => email.to_string(),
        (name, "") => name.to_string(),
        (name, email) => format!("{name} <{email}>"),
    };
    if !from.is_empty() {
        body.push_str(&format!("**From:** {from}\n"));
    }
    if !mail.received.trim().is_empty() {
        body.push_str(&format!("**Received:** {}\n", mail.received.trim()));
    }
    body.push('\n');
    if !mail.body.trim().is_empty() {
        body.push_str(mail.body.trim());
        body.push_str("\n\n");
    }
    // A follow-up todo so the email becomes actionable; mention the sender, link
    // back to the live message (src:outlook:<EntryID>), and carry the flag due.
    let subj_for_todo = if subject.is_empty() { "this email".to_string() } else { format!("\"{subject}\"") };
    let mut todo = format!("TODO(followup) reply to {subj_for_todo}");
    if !who.is_empty() {
        todo.push_str(&format!(" @[[{who}]]"));
    }
    if !mail.due.trim().is_empty() {
        todo.push_str(&format!(" due:{}", mail.due.trim()));
    }
    if !mail.entry_id.trim().is_empty() {
        todo.push_str(&format!(" {OUTLOOK_REF_PREFIX}{}", mail.entry_id.trim()));
    }
    body.push_str(&todo);
    body.push('\n');
    (title, body)
}

// ---- Reopen an imported message in Outlook --------------------------------

/// Escape a value for embedding in a PowerShell single-quoted string (`'` → `''`).
fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

/// PowerShell that reopens a message by EntryID through the Outlook COM model.
#[cfg_attr(not(windows), allow(dead_code))] // run on Windows; exercised by tests
pub(crate) fn open_script(entry_id: &str) -> String {
    format!(
        "$ErrorActionPreference='Stop'; \
         $ol=New-Object -ComObject Outlook.Application; \
         $ol.Session.GetItemFromID('{}').Display($false)",
        ps_quote(entry_id)
    )
}

/// Open the linked Outlook message (the `src:outlook:<EntryID>` on a todo) in
/// Classic Outlook. Windows-only; errors elsewhere so the GUI can report it.
pub fn open_in_outlook(entry_id: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &open_script(entry_id)])
            .output()
            .context("failed to launch PowerShell")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!(
                "Couldn't open the message in Outlook: {}",
                err.lines().last().unwrap_or("message not found?").trim()
            );
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = entry_id;
        anyhow::bail!("Opening Outlook messages is only available on Windows (Classic Outlook)")
    }
}

// ---- Flag/category-driven sync --------------------------------------------

/// PowerShell that lists Inbox items flagged for follow-up OR categorized `Noet`
/// as JSON (subject/sender/received/body/entry_id/due).
#[cfg_attr(not(windows), allow(dead_code))] // run on Windows; exercised by tests
pub(crate) fn flagged_script() -> &'static str {
    r#"
$ErrorActionPreference = 'Stop'
$ol = New-Object -ComObject Outlook.Application
$ns = $ol.GetNamespace('MAPI')
$inbox = $ns.GetDefaultFolder(6)  # olFolderInbox
# olFlagMarked = 2 ; category 'Noet' opts an item in explicitly
$marked = $inbox.Items.Restrict("[FlagStatus] = 2 OR [Categories] = 'Noet'")
$out = @()
foreach ($m in $marked) {
  if ($m.Class -ne 43) { continue }  # 43 = olMail
  $out += [pscustomobject]@{
    subject      = [string]$m.Subject
    sender       = [string]$m.SenderName
    sender_email = [string]$m.SenderEmailAddress
    received     = if ($m.ReceivedTime) { $m.ReceivedTime.ToString('s') } else { '' }
    body         = [string]$m.Body
    entry_id     = [string]$m.EntryID
    due          = if ($m.TaskDueDate -and $m.TaskDueDate.Year -lt 4500) { $m.TaskDueDate.ToString('yyyy-MM-dd') } else { '' }
  }
}
$out | ConvertTo-Json -Compress
"#
}

/// Parse the JSON the flagged-items query prints. PowerShell's `ConvertTo-Json`
/// emits a bare object for a single item and `null`/empty for none, so handle
/// object, array, and empty.
#[cfg_attr(not(windows), allow(dead_code))] // used on Windows + by tests
pub(crate) fn parse_mail_list(json: &str) -> Result<Vec<OutlookMail>> {
    let trimmed = json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    let v: serde_json::Value =
        serde_json::from_str(trimmed).context("could not parse the Outlook items JSON")?;
    match v {
        serde_json::Value::Array(items) => items
            .into_iter()
            .map(|x| serde_json::from_value(x).context("bad Outlook item"))
            .collect(),
        serde_json::Value::Object(_) => {
            Ok(vec![serde_json::from_value(v).context("bad Outlook item")?])
        }
        _ => Ok(Vec::new()),
    }
}

/// Fetch the flagged/`Noet`-categorized Inbox items. Windows-only.
pub fn fetch_flagged() -> Result<Vec<OutlookMail>> {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", flagged_script()])
            .output()
            .context("failed to launch PowerShell")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!(
                "Outlook sync failed: {}",
                err.lines().last().unwrap_or("is Classic Outlook running?").trim()
            );
        }
        parse_mail_list(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Outlook sync is only available on Windows (Classic Outlook)")
    }
}

/// PowerShell that marks a message's follow-up flag complete (push-back).
#[cfg_attr(not(windows), allow(dead_code))] // run on Windows; exercised by tests
pub(crate) fn complete_flag_script(entry_id: &str) -> String {
    format!(
        "$ErrorActionPreference='Stop'; \
         $ol=New-Object -ComObject Outlook.Application; \
         $m=$ol.Session.GetItemFromID('{}'); \
         $m.MarkComplete(); $m.Save()",
        ps_quote(entry_id)
    )
}

/// Mark the Outlook follow-up flag complete (push-back when you finish the review
/// todo in Noet). Windows-only; best-effort, errors elsewhere.
pub fn mark_flag_complete(entry_id: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &complete_flag_script(entry_id)])
            .output()
            .context("failed to launch PowerShell")?;
        if !out.status.success() {
            anyhow::bail!(
                "Couldn't update the Outlook flag: {}",
                String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").trim()
            );
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = entry_id;
        anyhow::bail!("Outlook push-back is only available on Windows (Classic Outlook)")
    }
}

/// What a sync should do for one message. The flag/category in Outlook is the
/// source of truth; Noet mirrors it, and a review todo finished in Noet pushes
/// back. Pure — [`reconcile`] decides; [`sync_into`] applies via `Backend`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction {
    /// Flagged in Outlook, not yet in Noet → create a review note.
    Create(Box<OutlookMail>),
    /// No longer flagged in Outlook → mark the review todo done + archive in Noet.
    ResolveInNoet(String),
    /// Re-flagged in Outlook after we'd resolved+archived it → reopen in Noet
    /// (un-archive + set the review todo back to open).
    Reopen(String),
    /// Still flagged in Outlook but the review todo is done in Noet (and not
    /// archived, i.e. you finished it yourself) → mark the Outlook flag complete
    /// (push-back) and archive the note.
    CompleteInOutlook(String),
}

/// Diff the live flagged set against what Noet already imported. `imported` is
/// `(entry_id, done_in_noet, archived_in_noet)` for each note carrying a
/// `src:outlook:` link. The `archived` bit distinguishes "Outlook cleared the
/// flag" (we archive) from "re-flagged after we resolved it" (we reopen) — both
/// look like `done` otherwise. Pure and fully unit-tested; COM/IO lives in
/// [`fetch_flagged`]/[`sync_into`].
pub fn reconcile(flagged: &[OutlookMail], imported: &[(String, bool, bool)]) -> Vec<SyncAction> {
    use std::collections::HashSet;
    let live: HashSet<&str> =
        flagged.iter().map(|m| m.entry_id.trim()).filter(|s| !s.is_empty()).collect();
    let known: HashSet<&str> = imported.iter().map(|(id, _, _)| id.as_str()).collect();

    let mut actions = Vec::new();
    for m in flagged {
        let id = m.entry_id.trim();
        if !id.is_empty() && !known.contains(id) {
            actions.push(SyncAction::Create(Box::new(m.clone())));
        }
    }
    for (id, done, archived) in imported {
        if !live.contains(id.as_str()) {
            // Outlook cleared the flag. Resolve it (once) if we haven't already.
            if !archived {
                actions.push(SyncAction::ResolveInNoet(id.clone()));
            }
        } else if *archived {
            // It's flagged again but we'd archived it → the user re-flagged to
            // resurface it. Reopen rather than push back.
            actions.push(SyncAction::Reopen(id.clone()));
        } else if *done {
            // Flagged, live, finished in Noet, still active here → push back.
            actions.push(SyncAction::CompleteInOutlook(id.clone()));
        }
    }
    actions
}

/// Outcome of a sync, for a status message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncSummary {
    pub created: usize,
    pub resolved: usize,
    pub reopened: usize,
    pub pushed_back: usize,
}

/// Apply [`reconcile`] against a vault: create review notes for newly-flagged
/// mail, resolve (done + archive) ones Outlook cleared, and push completed Noet
/// reviews back to Outlook. Testable with a temp `Backend` (the push-back COM
/// call is best-effort and a no-op off Windows).
pub fn sync_into(backend: &mut crate::backend::Backend, flagged: &[OutlookMail]) -> Result<SyncSummary> {
    let existing = backend.todos_by_external_prefix(OUTLOOK_REF_PREFIX)?;
    let imported: Vec<(String, bool, bool)> = existing
        .iter()
        .filter_map(|t| {
            entry_id_of(&t.external).map(|id| {
                let archived = backend.note_archived(&t.note_id).unwrap_or(false);
                (id.to_string(), t.done, archived)
            })
        })
        .collect();
    let find = |id: &str| existing.iter().find(|t| entry_id_of(&t.external) == Some(id));

    let mut summary = SyncSummary::default();
    for action in reconcile(flagged, &imported) {
        match action {
            SyncAction::Create(mail) => {
                let (title, body) = mail_to_note(&mail);
                let note = backend.new_note()?;
                backend.save_note(&note.id, &title, &body)?;
                summary.created += 1;
            }
            SyncAction::ResolveInNoet(id) => {
                if let Some(t) = find(&id) {
                    let (tid, nid) = (t.id.clone(), t.note_id.clone());
                    let _ = backend.set_todo_status(&tid, "done");
                    let _ = backend.archive_note(&nid, true);
                    summary.resolved += 1;
                }
            }
            SyncAction::Reopen(id) => {
                if let Some(t) = find(&id) {
                    let (tid, nid) = (t.id.clone(), t.note_id.clone());
                    let _ = backend.archive_note(&nid, false); // un-archive first
                    let _ = backend.set_todo_status(&tid, "todo");
                    summary.reopened += 1;
                }
            }
            SyncAction::CompleteInOutlook(id) => {
                if let Some(t) = find(&id) {
                    let _ = mark_flag_complete(&id); // best-effort, Windows-only
                    let _ = backend.archive_note(&t.note_id.clone(), true);
                    summary.pushed_back += 1;
                }
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_supported_matches_platform() {
        assert_eq!(is_supported(), cfg!(windows));
    }

    #[test]
    fn ps_script_uses_classic_outlook_com() {
        let s = ps_script();
        assert!(s.contains("New-Object -ComObject Outlook.Application"));
        assert!(s.contains("ActiveExplorer"));
        assert!(s.contains("ConvertTo-Json"));
    }

    #[test]
    fn parses_mail_json_with_and_without_fields() {
        let m = parse_mail_json(
            r#"{"subject":"Q3 plan","sender":"Jane Doe","sender_email":"jane@x.com","received":"2026-06-04T09:00:00","body":"Let's sync."}"#,
        )
        .unwrap();
        assert_eq!(m.subject, "Q3 plan");
        assert_eq!(m.sender, "Jane Doe");
        assert_eq!(m.sender_email, "jane@x.com");

        // missing fields default to empty (no panic)
        let m2 = parse_mail_json(r#"{"subject":"hi"}"#).unwrap();
        assert_eq!(m2.subject, "hi");
        assert_eq!(m2.sender, "");

        // invalid JSON is an error, not a panic
        assert!(parse_mail_json("not json").is_err());
    }

    #[test]
    fn mail_to_note_builds_followup_and_mention() {
        let mail = OutlookMail {
            subject: "Budget review".into(),
            sender: "Jane Doe".into(),
            sender_email: "jane@x.com".into(),
            received: "2026-06-04T09:00:00".into(),
            body: "Numbers attached.".into(),
            ..Default::default()
        };
        let (title, body) = mail_to_note(&mail);
        assert_eq!(title, "Budget review");
        assert!(body.contains("**From:** Jane Doe <jane@x.com>"));
        assert!(body.contains("**Received:** 2026-06-04T09:00:00"));
        assert!(body.contains("Numbers attached."));
        assert!(body.contains(r#"TODO(followup) reply to "Budget review" @[[Jane Doe]]"#));
    }

    #[test]
    fn mail_to_note_degrades_when_fields_missing() {
        let (title, body) = mail_to_note(&OutlookMail::default());
        assert_eq!(title, "Email"); // empty subject -> fallback title
        assert!(body.contains("TODO(followup) reply to this email"));
        assert!(!body.contains("@[[")); // no sender -> no mention
        assert!(!body.contains("**From:**"));
        assert!(!body.contains("src:outlook:")); // no entry id -> no back-link
    }

    #[test]
    fn mail_to_note_embeds_backlink_and_due() {
        let mail = OutlookMail {
            subject: "Renewal".into(),
            entry_id: "00000000DEADBEEF".into(),
            due: "2026-07-01".into(),
            ..Default::default()
        };
        let (_t, body) = mail_to_note(&mail);
        assert!(body.contains("due:2026-07-01"));
        assert!(body.contains("src:outlook:00000000DEADBEEF"));
    }

    #[test]
    fn entry_id_round_trips_through_ref() {
        assert_eq!(entry_id_of("src:outlook:ABC123"), Some("ABC123"));
        assert_eq!(entry_id_of("src:outlook:"), None); // empty id
        assert_eq!(entry_id_of("jira:PROJ-1"), None);
        assert_eq!(entry_id_of("https://x"), None);
    }

    #[test]
    fn com_scripts_use_the_right_surface() {
        let open = open_script("AB'CD"); // quote must be escaped
        assert!(open.contains("GetItemFromID('AB''CD')"));
        assert!(open.contains("Display"));

        let complete = complete_flag_script("ID1");
        assert!(complete.contains("GetItemFromID('ID1')"));
        assert!(complete.contains("MarkComplete"));

        let flagged = flagged_script();
        assert!(flagged.contains("Outlook.Application"));
        assert!(flagged.contains("FlagStatus"));
        assert!(flagged.contains("Categories"));
        assert!(flagged.contains("ConvertTo-Json"));
    }

    #[test]
    fn parse_mail_list_handles_array_object_and_empty() {
        // array of two
        let two = parse_mail_list(r#"[{"subject":"a","entry_id":"1"},{"subject":"b","entry_id":"2"}]"#).unwrap();
        assert_eq!(two.len(), 2);
        // single object (PowerShell collapses a 1-element array)
        let one = parse_mail_list(r#"{"subject":"solo","entry_id":"9"}"#).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].entry_id, "9");
        // empty / null
        assert_eq!(parse_mail_list("").unwrap().len(), 0);
        assert_eq!(parse_mail_list("null").unwrap().len(), 0);
    }

    #[test]
    fn reconcile_covers_create_resolve_complete_leave() {
        let a = OutlookMail { entry_id: "A".into(), ..Default::default() };
        let b = OutlookMail { entry_id: "B".into(), ..Default::default() };
        let d = OutlookMail { entry_id: "D".into(), ..Default::default() };
        let e = OutlookMail { entry_id: "E".into(), ..Default::default() };
        let flagged = vec![a.clone(), b.clone(), d.clone(), e.clone()];
        // imported (id, done, archived):
        //   B done, flagged, not archived -> push back
        //   C absent from flags, not archived -> resolve
        //   D not done, flagged, not archived -> leave
        //   E flagged but archived (re-flagged after resolve) -> reopen
        //   F absent from flags, already archived -> leave (already resolved)
        //   A is new -> create
        let imported = vec![
            ("B".to_string(), true, false),
            ("C".to_string(), false, false),
            ("D".to_string(), false, false),
            ("E".to_string(), true, true),
            ("F".to_string(), true, true),
        ];
        let actions = reconcile(&flagged, &imported);
        assert!(actions.contains(&SyncAction::Create(Box::new(a))));
        assert!(actions.contains(&SyncAction::CompleteInOutlook("B".into())));
        assert!(actions.contains(&SyncAction::ResolveInNoet("C".into())));
        assert!(actions.contains(&SyncAction::Reopen("E".into())));
        assert_eq!(actions.len(), 4); // D and F are left alone
    }

    #[test]
    fn sync_into_creates_dedups_resolves_reopens_and_pushes_back() {
        use crate::backend::Backend;
        let dir = std::env::temp_dir().join(format!("noet-osync-{}", ulid::Ulid::new()));
        let mut b = Backend::open_at(dir.clone(), dir.join(".index")).unwrap();

        let m1 = OutlookMail { subject: "A".into(), entry_id: "ID1".into(), ..Default::default() };
        let m2 = OutlookMail { subject: "B".into(), entry_id: "ID2".into(), ..Default::default() };

        // first sync creates two review notes
        assert_eq!(sync_into(&mut b, &[m1.clone(), m2.clone()]).unwrap().created, 2);
        // re-sync with the same flags is a no-op (dedup by EntryID)
        assert_eq!(sync_into(&mut b, &[m1.clone(), m2.clone()]).unwrap().created, 0);

        // m1 un-flagged in Outlook -> resolved (todo done + note archived)
        let s = sync_into(&mut b, &[m2.clone()]).unwrap();
        assert_eq!(s.resolved, 1);
        let t1 = b.todos_by_external_prefix("src:outlook:ID1").unwrap();
        assert!(t1[0].done && b.note_archived(&t1[0].note_id).unwrap());

        // re-syncing without m1 doesn't re-resolve it (it's already archived)
        assert_eq!(sync_into(&mut b, &[m2.clone()]).unwrap().resolved, 0);

        // m1 re-flagged in Outlook -> reopened (un-archived + todo back to open)
        let s = sync_into(&mut b, &[m1.clone(), m2.clone()]).unwrap();
        assert_eq!(s.reopened, 1);
        let t1 = b.todos_by_external_prefix("src:outlook:ID1").unwrap();
        assert!(!t1[0].done && !b.note_archived(&t1[0].note_id).unwrap());

        // finish m2's review in Noet while still flagged -> push-back
        let t2 = b.todos_by_external_prefix("src:outlook:ID2").unwrap();
        b.set_todo_status(&t2[0].id, "done").unwrap();
        let s = sync_into(&mut b, &[m1.clone(), m2.clone()]).unwrap();
        assert_eq!(s.pushed_back, 1);

        std::fs::remove_dir_all(&dir).ok();
    }
}
