//! External-system connectors. Each is optional and degrades gracefully when
//! unconfigured or unsupported on the platform. Today: [`jira`] (HTTP, any OS)
//! and [`outlook`] (Classic Outlook COM via PowerShell, Windows-only — a no-op
//! that errors elsewhere).

pub mod gmail;
pub mod gtasks;
pub mod jira;
pub mod oauth;
pub mod outlook;
pub mod todoist;

use anyhow::Result;
use std::io::Write;
use std::path::Path;

#[cfg(target_os = "macos")]
pub(crate) mod keychain {
    use anyhow::Result;

    const SERVICE: &str = "Noet";

    pub(crate) fn get(account: &str) -> Option<String> {
        let service = service();
        let bytes = security_framework::passwords::get_generic_password(&service, account).ok()?;
        String::from_utf8(bytes).ok()
    }

    pub(crate) fn set(account: &str, secret: &str) -> Result<()> {
        if secret.is_empty() {
            let _ = delete(account);
            return Ok(());
        }
        let service = service();
        security_framework::passwords::set_generic_password(&service, account, secret.as_bytes())?;
        Ok(())
    }

    pub(crate) fn delete(account: &str) -> Result<()> {
        let service = service();
        match security_framework::passwords::delete_generic_password(&service, account) {
            Ok(()) => Ok(()),
            Err(_) => Ok(()),
        }
    }

    fn service() -> String {
        std::env::var("NOET_KEYCHAIN_SERVICE").unwrap_or_else(|_| SERVICE.to_string())
    }
}

/// Write connector configuration to JSON with restrictive local permissions.
///
/// This is still a fallback storage path, not a substitute for platform secret
/// stores. Until Keychain / Credential Manager / Secret Service support lands,
/// keep token-bearing files owner-only and keep the containing config directory
/// private where the platform supports Unix permissions.
pub(crate) fn write_private_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let json = serde_json::to_vec_pretty(value)?;
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(&json)?;
    file.write_all(b"\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Resolve an `external` ref (the token Noet lifts off a todo line, e.g.
/// `jira:PROJ-12`, `gh:owner/repo#3`, or a bare URL) into a browsable URL.
/// Returns `None` when it can't be resolved (e.g. a `jira:` ref with no
/// configured base URL).
pub fn resolve_external_url(external: &str, jira_cfg: Option<&jira::JiraConfig>) -> Option<String> {
    let ext = external.trim();

    if ext.starts_with("http://") || ext.starts_with("https://") {
        return Some(ext.to_string());
    }
    if let Some(id) = ext.strip_prefix(gmail::GMAIL_REF_PREFIX) {
        let id = id.trim();
        if !id.is_empty() {
            return Some(gmail::message_url(id));
        }
    }
    if ext.starts_with(gtasks::GTASK_REF_PREFIX) {
        return Some(gtasks::tasks_url().to_string());
    }
    if let Some(id) = ext.strip_prefix(todoist::TODOIST_REF_PREFIX) {
        let id = id.trim();
        if !id.is_empty() {
            return Some(todoist::task_url(id));
        }
    }
    if let Some(rest) = ext.strip_prefix("jira:") {
        let key = jira::parse_key(rest)?;
        let base = jira_cfg
            .map(|c| c.base_url.trim())
            .filter(|b| !b.is_empty())?;
        return Some(jira::browse_url(base, &key));
    }
    if let Some(rest) = ext.strip_prefix("gh:") {
        // gh:owner/repo#123 -> issue URL ; gh:owner/repo -> repo URL
        let (repo, num) = match rest.split_once('#') {
            Some((r, n)) => (r, Some(n)),
            None => (rest, None),
        };
        let base = format!("https://github.com/{}", repo.trim_end_matches('/'));
        return Some(match num {
            Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => {
                format!("{base}/issues/{n}")
            }
            _ => base,
        });
    }
    if let Some(rest) = ext.strip_prefix("ref:") {
        let r = rest.trim();
        if r.starts_with("http://") || r.starts_with("https://") {
            return Some(r.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_json_uses_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("noet-private-json-{}", ulid::Ulid::new()));
        let path = dir.join("secret.json");
        write_private_json(&path, &serde_json::json!({ "token": "secret" })).unwrap();

        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolves_external_refs() {
        let cfg = jira::JiraConfig {
            base_url: "https://x.atlassian.net".into(),
            ..Default::default()
        };

        // jira ref needs a configured base
        assert_eq!(
            resolve_external_url("jira:PROJ-9", Some(&cfg)).as_deref(),
            Some("https://x.atlassian.net/browse/PROJ-9")
        );
        assert_eq!(resolve_external_url("jira:PROJ-9", None), None);

        // github refs
        assert_eq!(
            resolve_external_url("gh:rust-lang/rust#123", None).as_deref(),
            Some("https://github.com/rust-lang/rust/issues/123")
        );
        assert_eq!(
            resolve_external_url("gh:rust-lang/rust", None).as_deref(),
            Some("https://github.com/rust-lang/rust")
        );

        // gmail back-link -> Gmail web URL
        assert_eq!(
            resolve_external_url("src:gmail:18abc", None).as_deref(),
            Some("https://mail.google.com/mail/u/0/#all/18abc")
        );
        // google task -> Tasks web app; todoist -> the task URL
        assert_eq!(
            resolve_external_url("src:gtask:T1", None).as_deref(),
            Some("https://tasks.google.com/")
        );
        assert_eq!(
            resolve_external_url("src:todoist:678", None).as_deref(),
            Some("https://app.todoist.com/app/task/678")
        );

        // bare + ref: URLs pass through
        assert_eq!(
            resolve_external_url("https://example.com/x", None).as_deref(),
            Some("https://example.com/x")
        );
        assert_eq!(
            resolve_external_url("ref:https://example.com/y", None).as_deref(),
            Some("https://example.com/y")
        );

        // unresolvable
        assert_eq!(resolve_external_url("ref:not-a-url", None), None);
        assert_eq!(resolve_external_url("", None), None);
    }
}
