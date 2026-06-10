//! Gmail connector. Reads recent messages via the Gmail REST API using the
//! native-app loopback + PKCE flow (see [`super::oauth`]). You register your own
//! OAuth "Desktop app" client in Google Cloud; on a Workspace you administer you
//! can mark it **Internal**, which exempts it from Google verification. Creds +
//! the long-lived refresh token live in macOS Keychain on macOS, or in a
//! private `gmail.json` fallback elsewhere.
//!
//! Pure parts (config, message parsing, note shaping) are unit-tested; the OAuth
//! dance and HTTP are the thin IO.

use super::oauth;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// In-memory access-token cache (one Gmail account) so repeated API calls and
/// back-to-back imports don't each hit the token endpoint.
static ACCESS_CACHE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub(crate) const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub(crate) const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
/// Scopes requested at connect — read Gmail *and* Google Tasks, so one consent
/// covers both connectors.
const SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/tasks.readonly",
];
const API: &str = "https://gmail.googleapis.com/gmail/v1/users/me";

/// The `external` prefix linking a note/todo back to a Gmail message.
pub const GMAIL_REF_PREFIX: &str = "src:gmail:";

/// Google OAuth credentials. Despite the name this covers **both** Gmail and
/// Google Tasks — one Desktop-app client, one consent, one refresh token (the
/// scopes requested include both). Secret fields are stored in macOS Keychain on
/// macOS; other platforms use the private JSON fallback.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct GmailConfig {
    /// OAuth client id (`*.apps.googleusercontent.com`).
    #[serde(default)]
    pub client_id: String,
    /// OAuth client secret (Desktop clients require it; not actually confidential).
    #[serde(default)]
    pub client_secret: String,
    /// Long-lived refresh token, obtained once via [`connect`].
    #[serde(default)]
    pub refresh_token: String,
}

impl GmailConfig {
    #[cfg(target_os = "macos")]
    const CLIENT_SECRET_KEY: &'static str = "gmail.client_secret";
    #[cfg(target_os = "macos")]
    const REFRESH_TOKEN_KEY: &'static str = "gmail.refresh_token";

    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("noet").join("gmail.json"))
    }
    pub fn load() -> Option<GmailConfig> {
        let path = Self::path()?;
        let mut cfg = Self::load_from(&path)?;
        #[cfg(target_os = "macos")]
        {
            let disk_client_secret = cfg.client_secret.clone();
            let disk_refresh_token = cfg.refresh_token.clone();
            if let Some(secret) = super::keychain::get(Self::CLIENT_SECRET_KEY) {
                cfg.client_secret = secret;
            } else if !disk_client_secret.is_empty() {
                let _ = super::keychain::set(Self::CLIENT_SECRET_KEY, &disk_client_secret);
            }
            if let Some(token) = super::keychain::get(Self::REFRESH_TOKEN_KEY) {
                cfg.refresh_token = token;
            } else if !disk_refresh_token.is_empty() {
                let _ = super::keychain::set(Self::REFRESH_TOKEN_KEY, &disk_refresh_token);
            }
        }
        Some(cfg)
    }
    pub fn load_from(path: &Path) -> Option<GmailConfig> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
    pub fn save(&self) -> Result<()> {
        let path = Self::path().context("no OS config dir for gmail.json")?;
        #[cfg(target_os = "macos")]
        {
            super::keychain::set(Self::CLIENT_SECRET_KEY, &self.client_secret)?;
            super::keychain::set(Self::REFRESH_TOKEN_KEY, &self.refresh_token)?;
            let disk = GmailConfig {
                client_id: self.client_id.clone(),
                client_secret: String::new(),
                refresh_token: String::new(),
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
    /// Has an app registered (client id/secret) — ready to [`connect`].
    pub fn has_client(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }
    /// Fully connected (a refresh token is stored).
    pub fn is_connected(&self) -> bool {
        self.has_client() && !self.refresh_token.trim().is_empty()
    }
}

/// Run the one-time consent flow and store the refresh token. `open_browser` is
/// called with the authorize URL (the GUI passes the system opener) — kept out of
/// core so it stays GUI-free. Blocks on the loopback redirect, so call it off the
/// UI thread. Returns the updated, saved config.
pub fn connect(mut cfg: GmailConfig, open_browser: impl FnOnce(&str)) -> Result<GmailConfig> {
    if !cfg.has_client() {
        anyhow::bail!("set your Google OAuth client id + secret first");
    }
    let pkce = oauth::pkce();
    let state = ulid::Ulid::new().to_string();
    let (listener, redirect_uri) = oauth::loopback()?;
    let url = oauth::authorize_url(
        AUTH_ENDPOINT,
        &cfg.client_id,
        &redirect_uri,
        SCOPES,
        &pkce.challenge,
        &state,
    );
    open_browser(&url);
    let code = oauth::wait_for_code(&listener, &state)?;
    let tokens = oauth::exchange_code(
        TOKEN_ENDPOINT,
        &cfg.client_id,
        &cfg.client_secret,
        &code,
        &pkce.verifier,
        &redirect_uri,
    )?;
    if tokens.refresh_token.is_empty() {
        anyhow::bail!("Google returned no refresh token — revoke prior access and retry");
    }
    cfg.refresh_token = tokens.refresh_token;
    cfg.save()?;
    Ok(cfg)
}

/// A valid access token, reusing the cached one until ~1 min before it expires.
/// Shared by the Gmail and Google Tasks connectors (same Google account).
pub(crate) fn access_token(cfg: &GmailConfig) -> Result<String> {
    if !cfg.is_connected() {
        anyhow::bail!("Gmail isn't connected — connect it in Settings");
    }
    if let Some((tok, exp)) = ACCESS_CACHE.lock().unwrap().as_ref() {
        if Instant::now() < *exp {
            return Ok(tok.clone());
        }
    }
    let t = oauth::refresh_access(
        TOKEN_ENDPOINT,
        &cfg.client_id,
        &cfg.client_secret,
        &cfg.refresh_token,
    )?;
    if t.access_token.is_empty() {
        anyhow::bail!("Gmail refresh returned no access token");
    }
    let ttl = (t.expires_in.max(60) as u64).saturating_sub(60);
    *ACCESS_CACHE.lock().unwrap() = Some((
        t.access_token.clone(),
        Instant::now() + Duration::from_secs(ttl),
    ));
    Ok(t.access_token)
}

/// GET a Google API URL, surfacing Google's error `message` (e.g. "Gmail API has
/// not been used in project … or it is disabled") instead of a bare status.
/// Shared by the Gmail and Google Tasks connectors.
pub(crate) fn get_json(req: ureq::Request) -> Result<serde_json::Value> {
    match req.call() {
        Ok(r) => r.into_json().context("unexpected Gmail response"),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|j| {
                    j.pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| body.chars().take(200).collect());
            anyhow::bail!("Gmail API error (HTTP {code}): {detail}")
        }
        Err(e) => anyhow::bail!("network error talking to Gmail: {e}"),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GmailMessage {
    pub id: String,
    pub subject: String,
    pub from_name: String,
    pub from_email: String,
    pub date: String,
    pub snippet: String,
    /// Decoded plain-text body (HTML stripped if that's all there was). Empty if
    /// the message had no text part.
    pub body: String,
}

/// Walk a Gmail `payload` MIME tree and return the best text body: the first
/// `text/plain` part, else stripped `text/html`. Bodies are base64url-encoded.
fn extract_body(payload: &serde_json::Value) -> String {
    fn walk(node: &serde_json::Value, plain: &mut Option<String>, html: &mut Option<String>) {
        let mime = node.get("mimeType").and_then(|v| v.as_str()).unwrap_or("");
        if let Some(data) = node.pointer("/body/data").and_then(|v| v.as_str()) {
            let decoded = || String::from_utf8_lossy(&oauth::base64url_decode(data)).into_owned();
            if mime == "text/plain" && plain.is_none() {
                *plain = Some(decoded());
            } else if mime == "text/html" && html.is_none() {
                *html = Some(decoded());
            }
        }
        if let Some(parts) = node.get("parts").and_then(|v| v.as_array()) {
            for p in parts {
                walk(p, plain, html);
            }
        }
    }
    let (mut plain, mut html) = (None, None);
    walk(payload, &mut plain, &mut html);
    if let Some(p) = plain {
        return p.trim().to_string();
    }
    html.map(|h| strip_html(&h)).unwrap_or_default()
}

/// Crude HTML→text: drop tags, decode a few entities, collapse whitespace.
fn strip_html(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse a Gmail `messages.get` (format=full) JSON body.
pub(crate) fn parse_message(json: &serde_json::Value) -> GmailMessage {
    let header = |name: &str| -> String {
        json.pointer("/payload/headers")
            .and_then(|h| h.as_array())
            .and_then(|hs| {
                hs.iter().find(|h| {
                    h.get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| n.eq_ignore_ascii_case(name))
                        .unwrap_or(false)
                })
            })
            .and_then(|h| h.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let (from_name, from_email) = split_from(&header("From"));
    GmailMessage {
        id: json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        subject: header("Subject"),
        from_name,
        from_email,
        date: header("Date"),
        snippet: json
            .get("snippet")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        body: json.get("payload").map(extract_body).unwrap_or_default(),
    }
}

/// Split a `From:` header (`Jane Doe <jane@x.com>` or `jane@x.com`) into
/// (display-name, email).
fn split_from(from: &str) -> (String, String) {
    let from = from.trim();
    if let Some(open) = from.find('<') {
        let name = from[..open].trim().trim_matches('"').trim();
        let email = from[open + 1..].trim_end_matches('>').trim();
        (name.to_string(), email.to_string())
    } else if from.contains('@') {
        (String::new(), from.to_string())
    } else {
        (from.to_string(), String::new())
    }
}

/// List recent messages (newest first). `query` is a Gmail search (e.g.
/// `is:starred`, `label:follow-up`, or `""` for the inbox).
pub fn list_recent(cfg: &GmailConfig, query: &str, max: u32) -> Result<Vec<GmailMessage>> {
    let token = access_token(cfg)?;
    let list = get_json(
        ureq::get(&format!("{API}/messages"))
            .set("Authorization", &format!("Bearer {token}"))
            .query("maxResults", &max.to_string())
            .query("q", query),
    )?;
    let ids: Vec<String> = list
        .get("messages")
        .and_then(|m| m.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for id in ids {
        let msg = get_json(
            ureq::get(&format!("{API}/messages/{id}"))
                .set("Authorization", &format!("Bearer {token}"))
                .query("format", "full"),
        )?;
        out.push(parse_message(&msg));
    }
    Ok(out)
}

/// Render a Gmail message into a Noet note: a `(title, body)` pair with the sender
/// as an `@[[Person]]`, a `src:gmail:` back-link, and a follow-up todo. Pure.
pub fn message_to_note(msg: &GmailMessage) -> (String, String) {
    let subject = msg.subject.trim();
    let title = if subject.is_empty() {
        "Email".to_string()
    } else {
        subject.to_string()
    };
    let who = if !msg.from_name.trim().is_empty() {
        msg.from_name.trim()
    } else {
        msg.from_email.trim()
    };

    let mut body = String::new();
    let from = match (msg.from_name.trim(), msg.from_email.trim()) {
        ("", "") => String::new(),
        ("", email) => email.to_string(),
        (name, "") => name.to_string(),
        (name, email) => format!("{name} <{email}>"),
    };
    if !from.is_empty() {
        body.push_str(&format!("**From:** {from}\n"));
    }
    if !msg.date.trim().is_empty() {
        body.push_str(&format!("**Received:** {}\n", msg.date.trim()));
    }
    body.push('\n');
    // Prefer the full decoded body; fall back to the snippet preview.
    let content = if !msg.body.trim().is_empty() {
        msg.body.trim()
    } else {
        msg.snippet.trim()
    };
    if !content.is_empty() {
        body.push_str(content);
        body.push_str("\n\n");
    }
    let subj_for_todo = if subject.is_empty() {
        "this email".to_string()
    } else {
        format!("\"{subject}\"")
    };
    let mut todo = format!("TODO(followup) Follow up: {subj_for_todo}");
    if !who.is_empty() {
        todo.push_str(&format!(" @[[{who}]]"));
    }
    if !msg.id.trim().is_empty() {
        todo.push_str(&format!(" {GMAIL_REF_PREFIX}{}", msg.id.trim()));
    }
    body.push_str(&todo);
    body.push('\n');
    (title, body)
}

/// Web URL that opens a Gmail message in the browser.
pub fn message_url(id: &str) -> String {
    format!("https://mail.google.com/mail/u/0/#all/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_state_machine() {
        let mut c = GmailConfig::default();
        assert!(!c.has_client() && !c.is_connected());
        c.client_id = "x.apps.googleusercontent.com".into();
        c.client_secret = "secret".into();
        assert!(c.has_client() && !c.is_connected());
        c.refresh_token = "rt".into();
        assert!(c.is_connected());
    }

    #[test]
    fn config_roundtrip() {
        let dir = std::env::temp_dir().join(format!("noet-gmail-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gmail.json");
        assert!(GmailConfig::load_from(&path).is_none());
        GmailConfig {
            client_id: "cid".into(),
            client_secret: "s".into(),
            refresh_token: "rt".into(),
        }
        .save_to(&path)
        .unwrap();
        assert!(GmailConfig::load_from(&path).unwrap().is_connected());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_accepts_redacted_secret_fields() {
        let cfg: GmailConfig = serde_json::from_str(r#"{"client_id":"cid"}"#).unwrap();
        assert_eq!(cfg.client_id, "cid");
        assert!(cfg.client_secret.is_empty());
        assert!(cfg.refresh_token.is_empty());
    }

    #[test]
    fn parses_message_headers_and_from() {
        let json = serde_json::json!({
            "id": "18abc",
            "snippet": "Let's sync on the budget.",
            "payload": { "headers": [
                {"name": "Subject", "value": "Q3 budget"},
                {"name": "From", "value": "Jane Doe <jane@x.com>"},
                {"name": "Date", "value": "Thu, 5 Jun 2026 09:00:00 -0700"}
            ]}
        });
        let m = parse_message(&json);
        assert_eq!(m.id, "18abc");
        assert_eq!(m.subject, "Q3 budget");
        assert_eq!(m.from_name, "Jane Doe");
        assert_eq!(m.from_email, "jane@x.com");
        assert_eq!(m.snippet, "Let's sync on the budget.");
        // bare-address From
        let bare = parse_message(&serde_json::json!({
            "id":"1","payload":{"headers":[{"name":"From","value":"ops@x.com"}]}
        }));
        assert_eq!(bare.from_email, "ops@x.com");
        assert_eq!(bare.from_name, "");
    }

    #[test]
    fn message_to_note_uses_full_body_then_falls_back_to_snippet() {
        let m = GmailMessage {
            id: "18abc".into(),
            subject: "Q3 budget".into(),
            from_name: "Jane Doe".into(),
            from_email: "jane@x.com".into(),
            date: "Thu, 5 Jun 2026 09:00:00 -0700".into(),
            snippet: "preview…".into(),
            body: "The full email body.\nSecond line.".into(),
        };
        let (title, body) = message_to_note(&m);
        assert_eq!(title, "Q3 budget");
        assert!(body.contains("**From:** Jane Doe <jane@x.com>"));
        assert!(body.contains("The full email body."));
        assert!(
            !body.contains("preview…"),
            "full body should win over the snippet"
        );
        assert!(
            body.contains(r#"TODO(followup) Follow up: "Q3 budget" @[[Jane Doe]] src:gmail:18abc"#)
        );
        // with no body, the snippet is the fallback
        let m2 = GmailMessage {
            body: String::new(),
            ..m
        };
        assert!(message_to_note(&m2).1.contains("preview…"));
    }

    #[test]
    fn extract_body_walks_mime_and_decodes() {
        // multipart/alternative with text/plain (preferred) + text/html
        let plain = oauth::base64url(b"Hello in plain text.");
        let html = oauth::base64url(b"<p>Hello in <b>HTML</b></p>");
        let json = serde_json::json!({
            "id": "1",
            "payload": { "mimeType": "multipart/alternative", "parts": [
                { "mimeType": "text/plain", "body": { "data": plain } },
                { "mimeType": "text/html", "body": { "data": html } }
            ]}
        });
        assert_eq!(parse_message(&json).body, "Hello in plain text.");
        // html-only -> tags stripped
        let json2 = serde_json::json!({
            "id": "2",
            "payload": { "mimeType": "text/html", "body": { "data": html } }
        });
        assert_eq!(parse_message(&json2).body, "Hello in HTML");
    }

    #[test]
    fn message_url_points_at_gmail() {
        assert_eq!(
            message_url("18abc"),
            "https://mail.google.com/mail/u/0/#all/18abc"
        );
    }
}
