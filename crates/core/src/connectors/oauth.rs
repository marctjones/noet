//! Reusable OAuth 2.0 for native apps (RFC 8252): the **loopback redirect +
//! PKCE** flow. No app secret needs to be confidential; the redirect is a
//! `http://127.0.0.1:<port>` listener this process owns.
//!
//! Shared by any "register your own app" connector (Gmail today). The pure parts
//! — PKCE, the authorize URL, base64url, and parsing the redirect + token
//! responses — are unit-tested; the network calls and the loopback listener are
//! the thin IO on top.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// How long [`wait_for_code`] waits for the browser redirect before giving up.
const LOOPBACK_TIMEOUT: Duration = Duration::from_secs(300);

/// A PKCE verifier/challenge pair (S256).
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// Generate a PKCE pair. The verifier is high-entropy (two ULIDs → 52 chars from
/// the unreserved set), the challenge is `base64url(sha256(verifier))`.
pub fn pkce() -> Pkce {
    let verifier = format!("{}{}", ulid::Ulid::new(), ulid::Ulid::new());
    let challenge = base64url(&Sha256::digest(verifier.as_bytes()));
    Pkce { verifier, challenge }
}

/// URL-safe base64 without padding (RFC 4648 §5) — what PKCE/JWT use.
pub(crate) fn base64url(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        }
    }
    out
}

/// Decode URL-safe base64 (tolerant: accepts standard `+/` too, ignores padding
/// and whitespace). Used for Gmail message bodies.
pub(crate) fn base64url_decode(input: &str) -> Vec<u8> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'-' | b'+' => Some(62),
            b'_' | b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in input.as_bytes() {
        if let Some(v) = val(c) {
            buf = (buf << 6) | v as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buf >> bits) as u8);
            }
        }
    }
    out
}

fn enc(s: &str) -> String {
    // Minimal percent-encoding for query values (encode everything not unreserved).
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => o.push(b as char),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// Build the authorization URL for the loopback + PKCE flow. `scopes` are joined
/// with spaces; `access_type=offline` + `prompt=consent` ask for a refresh token.
pub fn authorize_url(
    auth_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[&str],
    challenge: &str,
    state: &str,
) -> String {
    format!(
        "{auth_endpoint}?response_type=code&client_id={}&redirect_uri={}&scope={}\
         &code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=consent",
        enc(client_id),
        enc(redirect_uri),
        enc(&scopes.join(" ")),
        enc(challenge),
        enc(state),
    )
}

/// Tokens from an authorization-code exchange or refresh.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

/// Parse a token endpoint JSON response. `refresh_token` is absent on refreshes.
pub(crate) fn parse_token_response(json: &serde_json::Value) -> Tokens {
    Tokens {
        access_token: json.get("access_token").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        refresh_token: json.get("refresh_token").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        expires_in: json.get("expires_in").and_then(|v| v.as_i64()).unwrap_or(0),
    }
}

/// Extract the `code` from a loopback redirect's HTTP request line
/// (`GET /?code=...&state=... HTTP/1.1`), verifying `state`. Pure + tested.
pub(crate) fn parse_redirect(request_line: &str, expected_state: &str) -> Result<String> {
    let path = request_line.split_whitespace().nth(1).context("malformed request line")?;
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        match pair.split_once('=') {
            Some(("code", v)) => code = Some(percent_decode(v)),
            Some(("state", v)) => state = Some(percent_decode(v)),
            Some(("error", v)) => anyhow::bail!("authorization denied: {}", percent_decode(v)),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        anyhow::bail!("OAuth state mismatch (possible CSRF) — try again");
    }
    code.context("no authorization code in redirect")
}

fn percent_decode(s: &str) -> String {
    let b = s.replace('+', " ");
    let bytes = b.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&b[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Bind a loopback listener on an ephemeral port; returns it plus its
/// `http://127.0.0.1:<port>` redirect URI (needed *before* building the auth URL).
pub fn loopback() -> Result<(TcpListener, String)> {
    let listener = TcpListener::bind("127.0.0.1:0").context("couldn't open a loopback port")?;
    let port = listener.local_addr()?.port();
    Ok((listener, format!("http://127.0.0.1:{port}")))
}

fn respond(stream: &mut std::net::TcpStream, body: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
}

/// Block until the browser hits the loopback redirect, return the auth `code`
/// (verifying `state`), and show a "you can close this tab" page. Robust to
/// stray requests (e.g. `/favicon.ico`) and times out after [`LOOPBACK_TIMEOUT`]
/// so a never-completed flow can't wedge the worker thread.
pub fn wait_for_code(listener: &TcpListener, expected_state: &str) -> Result<String> {
    listener.set_nonblocking(true).ok();
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for the browser sign-in — try Connect again");
        }
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e).context("loopback accept failed"),
        };
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let first = req.lines().next().unwrap_or("");
        let path = first.split_whitespace().nth(1).unwrap_or("");
        // Ignore anything that isn't the OAuth redirect (favicon, probes, …).
        if !path.contains("code=") && !path.contains("error=") {
            respond(&mut stream, "Waiting for authorization…");
            continue;
        }
        let result = parse_redirect(first, expected_state);
        respond(
            &mut stream,
            if result.is_ok() {
                "Noet is connected. You can close this tab."
            } else {
                "Authorization failed. Close this tab and try again."
            },
        );
        return result;
    }
}

/// POST a form to an OAuth endpoint, surfacing the provider's own error body on
/// failure (Google returns `error` / `error_description`, e.g. `invalid_client`,
/// `redirect_uri_mismatch`) so setup mistakes are legible instead of "status 400".
fn post_form(endpoint: &str, params: &[(&str, &str)]) -> Result<serde_json::Value> {
    match ureq::post(endpoint).send_form(params) {
        Ok(resp) => resp.into_json().context("unexpected OAuth token response"),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            let detail = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|j| {
                    j.get("error_description")
                        .or_else(|| j.get("error"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| body.chars().take(200).collect());
            anyhow::bail!("OAuth error (HTTP {code}): {detail}")
        }
        Err(e) => anyhow::bail!("network error talking to the OAuth server: {e}"),
    }
}

/// Exchange an authorization code for tokens (RFC 6749 §4.1.3).
pub fn exchange_code(
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<Tokens> {
    let resp = post_form(
        token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code_verifier", verifier),
            ("redirect_uri", redirect_uri),
        ],
    )?;
    Ok(parse_token_response(&resp))
}

/// Trade a refresh token for a fresh access token (RFC 6749 §6).
pub fn refresh_access(
    token_endpoint: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<Tokens> {
    let resp = post_form(
        token_endpoint,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ],
    )?;
    Ok(parse_token_response(&resp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_no_padding_urlsafe() {
        // standard base64 would be "Pj4-Pz8" territory; ensure -/_ and no '='
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64url(&[0xfb, 0xff, 0xbf]), "-_-_"); // exercises - and _
        assert!(!base64url(b"any").contains('='));
    }

    #[test]
    fn base64url_decode_roundtrips_and_tolerates_padding() {
        assert_eq!(base64url_decode("Zm9vYmFy"), b"foobar");
        assert_eq!(base64url_decode("Zg"), b"f"); // no padding
        assert_eq!(base64url_decode("Zm8="), b"fo"); // padding ignored
        assert_eq!(base64url_decode(&base64url(b"hello world")), b"hello world");
        // Gmail bodies may wrap with newlines — those are skipped
        assert_eq!(base64url_decode("Zm9v\nYmFy"), b"foobar");
    }

    #[test]
    fn pkce_s256_matches_rfc7636_vector() {
        // RFC 7636 Appendix B
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = base64url(&Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        // generated pairs are well-formed and self-consistent
        let p = pkce();
        assert!(p.verifier.len() >= 43 && p.verifier.len() <= 128);
        assert_eq!(p.challenge, base64url(&Sha256::digest(p.verifier.as_bytes())));
    }

    #[test]
    fn authorize_url_has_required_params() {
        let u = authorize_url(
            "https://accounts.google.com/o/oauth2/v2/auth",
            "cid.apps.googleusercontent.com",
            "http://127.0.0.1:5123",
            &["https://www.googleapis.com/auth/gmail.readonly"],
            "CHAL",
            "STATE",
        );
        assert!(u.contains("response_type=code"));
        assert!(u.contains("client_id=cid.apps.googleusercontent.com"));
        assert!(u.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5123"));
        assert!(u.contains("code_challenge=CHAL&code_challenge_method=S256"));
        assert!(u.contains("state=STATE"));
        assert!(u.contains("access_type=offline"));
        assert!(u.contains("scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fgmail.readonly"));
    }

    #[test]
    fn parse_redirect_extracts_code_and_checks_state() {
        let line = "GET /?state=xyz&code=4%2F0Adeu HTTP/1.1";
        assert_eq!(parse_redirect(line, "xyz").unwrap(), "4/0Adeu"); // %2F decoded
        assert!(parse_redirect(line, "other").is_err()); // state mismatch
        assert!(parse_redirect("GET /?error=access_denied&state=xyz HTTP/1.1", "xyz").is_err());
    }

    #[test]
    fn parse_token_response_handles_exchange_and_refresh() {
        let exch = serde_json::json!({"access_token":"AT","refresh_token":"RT","expires_in":3599});
        let t = parse_token_response(&exch);
        assert_eq!(t.access_token, "AT");
        assert_eq!(t.refresh_token, "RT");
        assert_eq!(t.expires_in, 3599);
        // refresh responses omit refresh_token
        let refr = serde_json::json!({"access_token":"AT2","expires_in":3599});
        assert_eq!(parse_token_response(&refr).refresh_token, "");
    }
}
