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
    // A follow-up todo so the email becomes actionable; mention the sender.
    let subj_for_todo = if subject.is_empty() { "this email".to_string() } else { format!("\"{subject}\"") };
    if who.is_empty() {
        body.push_str(&format!("TODO(followup) reply to {subj_for_todo}\n"));
    } else {
        body.push_str(&format!("TODO(followup) reply to {subj_for_todo} @[[{who}]]\n"));
    }
    (title, body)
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
    }
}
