//! External-system connectors. Each is optional and degrades gracefully when
//! unconfigured or unsupported on the platform. Today: [`jira`] (HTTP, any OS)
//! and [`outlook`] (Classic Outlook COM via PowerShell, Windows-only — a no-op
//! that errors elsewhere).

pub mod gmail;
pub mod jira;
pub mod oauth;
pub mod outlook;

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
    if let Some(rest) = ext.strip_prefix("jira:") {
        let key = jira::parse_key(rest)?;
        let base = jira_cfg.map(|c| c.base_url.trim()).filter(|b| !b.is_empty())?;
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

    #[test]
    fn resolves_external_refs() {
        let cfg = jira::JiraConfig { base_url: "https://x.atlassian.net".into(), ..Default::default() };

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
