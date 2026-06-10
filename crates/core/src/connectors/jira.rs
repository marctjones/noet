//! Jira connector — works with both Cloud and Server/Data Center.
//!
//! Auth: Cloud uses `email` + an API token (HTTP Basic `email:token`); Server/DC
//! uses a Personal Access Token (HTTP `Bearer`). If `email` is set we assume
//! Cloud, otherwise Bearer. Tokens live in macOS Keychain on macOS, or in a
//! private `jira.json` fallback elsewhere. They never live in the vault.
//!
//! Everything except the single network call ([`fetch_issue`]) is pure and
//! unit-tested: key parsing, URL building, the auth header, and response
//! parsing. The connector degrades gracefully — with no config you can still
//! build/open browse URLs, just not fetch live issue data.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct JiraConfig {
    /// Site base, e.g. `https://acme.atlassian.net` (Cloud) or your Server URL.
    pub base_url: String,
    /// Cloud account email. Empty ⇒ treat `token` as a Bearer PAT (Server/DC).
    #[serde(default)]
    pub email: String,
    /// API token (Cloud) or Personal Access Token (Server/DC).
    #[serde(default)]
    pub token: String,
}

impl JiraConfig {
    #[cfg(target_os = "macos")]
    const TOKEN_KEY: &'static str = "jira.token";

    /// `<config dir>/noet/jira.json`. `None` if the platform has no config dir.
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("noet").join("jira.json"))
    }

    pub fn load() -> Option<JiraConfig> {
        let path = Self::path()?;
        let mut cfg = Self::load_from(&path)?;
        #[cfg(target_os = "macos")]
        {
            let disk_token = cfg.token.clone();
            if let Some(token) = super::keychain::get(Self::TOKEN_KEY) {
                cfg.token = token;
            } else if !disk_token.is_empty() {
                let _ = super::keychain::set(Self::TOKEN_KEY, &disk_token);
            }
        }
        Some(cfg)
    }

    pub fn load_from(path: &Path) -> Option<JiraConfig> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("no OS config dir to store jira.json")?;
        #[cfg(target_os = "macos")]
        {
            super::keychain::set(Self::TOKEN_KEY, &self.token)?;
            let disk = JiraConfig {
                base_url: self.base_url.clone(),
                email: self.email.clone(),
                token: String::new(),
            };
            return super::write_private_json(&path, &disk);
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.save_to(&path)
        }
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        super::write_private_json(path, self)
    }

    /// Enough to make API calls (a base URL + a token).
    pub fn is_configured(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.token.trim().is_empty()
    }

    /// The `Authorization` header value: Basic (Cloud, email+token) or Bearer
    /// (Server/DC PAT, when no email is set).
    pub fn auth_header(&self) -> String {
        if self.email.trim().is_empty() {
            format!("Bearer {}", self.token.trim())
        } else {
            let creds = format!("{}:{}", self.email.trim(), self.token.trim());
            format!("Basic {}", base64(creds.as_bytes()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraIssue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub url: String,
}

/// Extract a Jira issue key (e.g. `PROJ-12`) from an `external` token like
/// `jira:PROJ-12` (or a bare `PROJ-12`). Returns the upper-cased key, or `None`
/// if it isn't a valid `LETTERS[ALNUM]-NUMBER` key.
pub fn parse_key(external: &str) -> Option<String> {
    let raw = external.trim();
    let raw = raw.strip_prefix("jira:").unwrap_or(raw).trim();
    let (proj, num) = raw.split_once('-')?;
    if proj.is_empty() || num.is_empty() {
        return None;
    }
    if !proj.chars().next()?.is_ascii_alphabetic() {
        return None;
    }
    if !proj.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    if !num.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}-{}", proj.to_uppercase(), num))
}

fn trim_base(base: &str) -> &str {
    base.trim().trim_end_matches('/')
}

/// Human-facing URL to open the issue in a browser.
pub fn browse_url(base: &str, key: &str) -> String {
    format!("{}/browse/{}", trim_base(base), key)
}

/// REST endpoint for the issue (v2 works on both Cloud and Server/DC).
pub fn api_url(base: &str, key: &str) -> String {
    format!(
        "{}/rest/api/2/issue/{}?fields=summary,status",
        trim_base(base),
        key
    )
}

/// Pull the fields we care about out of a Jira issue JSON body.
pub fn parse_issue(base: &str, key: &str, body: &serde_json::Value) -> JiraIssue {
    JiraIssue {
        key: key.to_string(),
        summary: body
            .pointer("/fields/summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        status: body
            .pointer("/fields/status/name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        url: browse_url(base, key),
    }
}

/// Fetch an issue's summary + status from Jira. The only non-pure function here;
/// runs a blocking HTTP GET, so call it off the UI thread.
pub fn fetch_issue(cfg: &JiraConfig, key_or_ref: &str) -> Result<JiraIssue> {
    if !cfg.is_configured() {
        anyhow::bail!("Jira isn't configured — set the base URL + token in Settings");
    }
    let key = parse_key(key_or_ref).context("not a valid Jira issue key")?;
    let body: serde_json::Value = ureq::get(&api_url(&cfg.base_url, &key))
        .set("Authorization", &cfg.auth_header())
        .set("Accept", "application/json")
        .call()
        .map_err(|e| anyhow::anyhow!("Jira request failed: {e}"))?
        .into_json()
        .context("unexpected Jira response")?;
    Ok(parse_issue(&cfg.base_url, &key, &body))
}

/// Minimal standard base64 encoder (for the Basic auth header) — avoids pulling
/// in a crate for ~20 lines. Tested below.
fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys() {
        assert_eq!(parse_key("jira:PROJ-12").as_deref(), Some("PROJ-12"));
        assert_eq!(parse_key("PROJ-12").as_deref(), Some("PROJ-12"));
        assert_eq!(parse_key("jira:abc-7").as_deref(), Some("ABC-7")); // upper-cased
        assert_eq!(parse_key("jira:AB1-99").as_deref(), Some("AB1-99")); // alnum project
                                                                         // invalid shapes
        assert_eq!(parse_key("jira:PROJ"), None); // no number
        assert_eq!(parse_key("jira:12-12"), None); // project must start alpha
        assert_eq!(parse_key("jira:PROJ-x"), None); // number must be digits
        assert_eq!(parse_key("gh:owner/repo#1"), None);
    }

    #[test]
    fn builds_urls() {
        assert_eq!(
            browse_url("https://x.atlassian.net/", "P-1"),
            "https://x.atlassian.net/browse/P-1"
        );
        assert_eq!(
            api_url("https://x.atlassian.net", "P-1"),
            "https://x.atlassian.net/rest/api/2/issue/P-1?fields=summary,status"
        );
    }

    #[test]
    fn auth_header_picks_basic_or_bearer() {
        let cloud = JiraConfig {
            base_url: "u".into(),
            email: "a@b.com".into(),
            token: "tok".into(),
        };
        // Basic base64("a@b.com:tok")
        assert_eq!(
            cloud.auth_header(),
            format!("Basic {}", base64(b"a@b.com:tok"))
        );
        let server = JiraConfig {
            base_url: "u".into(),
            email: "".into(),
            token: "PAT".into(),
        };
        assert_eq!(server.auth_header(), "Bearer PAT");
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn parses_issue_json() {
        let body = serde_json::json!({
            "key": "P-1",
            "fields": { "summary": "Ship it", "status": { "name": "In Progress" } }
        });
        let i = parse_issue("https://x.atlassian.net", "P-1", &body);
        assert_eq!(i.summary, "Ship it");
        assert_eq!(i.status, "In Progress");
        assert_eq!(i.url, "https://x.atlassian.net/browse/P-1");
        // missing fields degrade to empty strings, not panics
        let empty = parse_issue("https://x", "P-2", &serde_json::json!({}));
        assert_eq!(empty.summary, "");
        assert_eq!(empty.status, "");
    }

    #[test]
    fn config_roundtrip_and_is_configured() {
        let dir = std::env::temp_dir().join(format!("noet-jira-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("jira.json");
        assert!(JiraConfig::load_from(&path).is_none());

        let cfg = JiraConfig {
            base_url: "https://x".into(),
            email: "a@b".into(),
            token: "t".into(),
        };
        cfg.save_to(&path).unwrap();
        let back = JiraConfig::load_from(&path).unwrap();
        assert_eq!(back.base_url, "https://x");
        assert!(back.is_configured());
        assert!(!JiraConfig::default().is_configured());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_accepts_redacted_token() {
        let cfg: JiraConfig =
            serde_json::from_str(r#"{"base_url":"https://x","email":"a@b"}"#).unwrap();
        assert_eq!(cfg.base_url, "https://x");
        assert_eq!(cfg.email, "a@b");
        assert!(cfg.token.is_empty());
        assert!(!cfg.is_configured());
    }
}
