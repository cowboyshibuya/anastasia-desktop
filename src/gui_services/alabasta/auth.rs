//! Signing in to Alabasta from the desktop, and keeping the credential safe.
//!
//! Anastasia is a public OAuth client with no secret, so it uses authorization
//! code + PKCE with a **loopback redirect** (RFC 8252 §7.3): the app binds an
//! ephemeral port on 127.0.0.1, opens the system browser, and receives the code
//! back on that socket. That deliberately avoids registering a custom URL
//! scheme — nothing to add to `Info.plist`, no AppKit URL handler, and no
//! collision with `alabasta://`, which Alabasta already uses as its internal
//! resource-URI namespace.
//!
//! The refresh token is the only long-lived secret and lives in the login
//! keychain. Access tokens last an hour and are never written to disk.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::gui_services::http;

/// Must match the entry in Alabasta's `lib/oauth-clients.ts` registry.
pub const CLIENT_ID: &str = "alabasta-anastasia";
/// Read is required; write covers marking a task in progress.
pub const SCOPE: &str = "alabasta.read alabasta.write";
const REDIRECT_PATH: &str = "/callback";
const KEYCHAIN_SERVICE: &str = "Anastasia — Alabasta";

/// How long the browser leg may take before the listener gives up. Long enough
/// to sign in and pick a workspace, short enough not to hold a port forever.
const AUTHORIZATION_TIMEOUT: Duration = Duration::from_secs(300);

/// Refresh this long before actual expiry, so a request that takes a moment to
/// reach Convex cannot arrive with a just-expired token.
const REFRESH_SKEW_SECONDS: u64 = 60;

/// Tokens as Alabasta issues them.
#[derive(Clone, Debug, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: u64,
}

/// An in-flight browser authorization holding the loopback socket.
pub struct PendingAuthorization {
    /// Open this in the user's browser.
    pub url: String,
    listener: TcpListener,
    redirect_uri: String,
    verifier: String,
    state: String,
}

/// Starts an authorization: binds the loopback socket and builds the URL.
///
/// `workspace_hint` preselects a workspace on the consent screen when the user
/// already chose one; Alabasta binds the code to whichever workspace they
/// actually confirm.
pub fn begin(site_url: &str, authorize_base: &str) -> anyhow::Result<PendingAuthorization> {
    // Port 0 lets the OS pick a free ephemeral port. Binding before building the
    // URL is what makes the redirect knowable.
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .context("could not open a local port for the Alabasta sign-in callback")?;
    listener
        .set_nonblocking(false)
        .context("could not configure the sign-in callback socket")?;
    let port = listener
        .local_addr()
        .context("the sign-in callback socket has no address")?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}{REDIRECT_PATH}");

    let verifier = random_token();
    let challenge = code_challenge(&verifier);
    let state = random_token();

    let url = format!(
        "{}/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method=S256&scope={}&state={}",
        authorize_base.trim_end_matches('/'),
        urlencode(CLIENT_ID),
        urlencode(&redirect_uri),
        urlencode(&challenge),
        urlencode(SCOPE),
        urlencode(&state),
    );
    let _ = site_url;
    Ok(PendingAuthorization {
        url,
        listener,
        redirect_uri,
        verifier,
        state,
    })
}

impl PendingAuthorization {
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Blocks until the browser redirects back, then exchanges the code.
    ///
    /// Runs on a background thread; it parks on `accept` for up to five minutes.
    pub fn complete(self, site_url: &str) -> anyhow::Result<Tokens> {
        let code = self.wait_for_code()?;
        exchange(site_url, &code, &self.verifier, &self.redirect_uri)
    }

    fn wait_for_code(&self) -> anyhow::Result<String> {
        self.listener
            .set_nonblocking(false)
            .context("could not configure the sign-in callback socket")?;
        let deadline = std::time::Instant::now() + AUTHORIZATION_TIMEOUT;
        loop {
            if std::time::Instant::now() > deadline {
                bail!("the Alabasta sign-in was not completed in time");
            }
            let (mut stream, _) = self
                .listener
                .accept()
                .context("the Alabasta sign-in callback failed")?;
            match self.read_callback(&mut stream) {
                Ok(code) => {
                    respond(&mut stream, true);
                    return Ok(code);
                }
                Err(error) => {
                    // A browser will happily request /favicon.ico on the same
                    // port. Answer and keep waiting rather than failing sign-in.
                    respond(&mut stream, false);
                    if error.to_string().contains("not the callback") {
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    fn read_callback(&self, stream: &mut TcpStream) -> anyhow::Result<String> {
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        let mut line = String::new();
        BufReader::new(stream.try_clone()?)
            .read_line(&mut line)
            .context("could not read the sign-in callback")?;
        let target = line
            .split_whitespace()
            .nth(1)
            .ok_or_else(|| anyhow!("malformed sign-in callback request"))?;
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if path != REDIRECT_PATH {
            bail!("not the callback path");
        }
        let mut code = None;
        let mut state = None;
        let mut error = None;
        for (key, value) in query.split('&').filter_map(|pair| pair.split_once('=')) {
            match key {
                "code" => code = Some(urldecode(value)),
                "state" => state = Some(urldecode(value)),
                "error" => error = Some(urldecode(value)),
                _ => {}
            }
        }
        if let Some(error) = error {
            bail!("Alabasta declined the sign-in: {error}");
        }
        // Without this check a third party could feed the app an authorization
        // code of their choosing by luring the browser to the loopback URL.
        if state.as_deref() != Some(self.state.as_str()) {
            bail!("the Alabasta sign-in response did not match this request");
        }
        code.ok_or_else(|| anyhow!("the Alabasta sign-in returned no authorization code"))
    }
}

fn respond(stream: &mut TcpStream, success: bool) {
    let body = if success {
        "<!doctype html><meta charset=utf-8><title>Anastasia</title><body style=\"font:15px -apple-system,sans-serif;padding:3rem;text-align:center\"><p>Anastasia is connected to Alabasta.<p style=\"color:#888\">You can close this tab."
    } else {
        "<!doctype html><meta charset=utf-8><title>Anastasia</title><body style=\"font:15px -apple-system,sans-serif;padding:3rem;text-align:center\"><p>Sign-in could not be completed.<p style=\"color:#888\">Return to Anastasia and try again."
    };
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

/// Exchanges an authorization code for tokens.
pub fn exchange(
    site_url: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> anyhow::Result<Tokens> {
    let body = format!(
        "grant_type=authorization_code&code={}&code_verifier={}&client_id={}&redirect_uri={}",
        urlencode(code),
        urlencode(verifier),
        urlencode(CLIENT_ID),
        urlencode(redirect_uri),
    );
    post_form(site_url, &body)
}

/// Trades a refresh token for a new pair. Alabasta rotates refresh tokens, so
/// the caller must persist the new one or the next refresh fails.
pub fn refresh(site_url: &str, refresh_token: &str) -> anyhow::Result<Tokens> {
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}",
        urlencode(refresh_token),
        urlencode(CLIENT_ID),
    );
    post_form(site_url, &body)
}

fn post_form(site_url: &str, body: &str) -> anyhow::Result<Tokens> {
    let url = format!("{}/oauth/token", site_url.trim_end_matches('/'));
    let response = http::request(
        "POST",
        &url,
        &["Content-Type: application/x-www-form-urlencoded".to_owned()],
        Some(body),
    )?;
    if !response.is_success() {
        let described = serde_json::from_str::<serde_json::Value>(&response.body)
            .ok()
            .and_then(|value| {
                value
                    .get("error_description")
                    .or_else(|| value.get("error"))?
                    .as_str()
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| format!("HTTP {}", response.status));
        bail!("Alabasta rejected the sign-in: {described}");
    }
    serde_json::from_str(&response.body).context("Alabasta returned an unreadable token response")
}

/// Absolute unix second at which an access token should be replaced.
pub fn expiry_from(expires_in: u64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    now + expires_in.saturating_sub(REFRESH_SKEW_SECONDS)
}

pub fn is_expired(expires_at: u64) -> bool {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
        >= expires_at
}

// ── Keychain ────────────────────────────────────────────────────────────────
//
// `security add-generic-password -w <secret>` would put the refresh token in
// argv, where any process on the machine can read it out of the process table.
// `security -i` reads its command from stdin instead, so the secret never
// appears as an argument.

pub fn store_refresh_token(account: &str, token: &str) -> anyhow::Result<()> {
    let mut child = Command::new("/usr/bin/security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not run the keychain tool")?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("the keychain tool did not accept input"))?;
        writeln!(
            stdin,
            "add-generic-password -a {} -s {} -w {} -U",
            quote(account),
            quote(KEYCHAIN_SERVICE),
            quote(token)
        )
        .context("could not write to the keychain")?;
    }
    let output = child
        .wait_with_output()
        .context("the keychain tool did not finish")?;
    if !output.status.success() {
        bail!(
            "could not save the Alabasta credential: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

pub fn load_refresh_token(account: &str) -> Option<String> {
    let output = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-a",
            account,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!token.is_empty()).then_some(token)
}

pub fn delete_refresh_token(account: &str) {
    let _ = Command::new("/usr/bin/security")
        .args([
            "delete-generic-password",
            "-a",
            account,
            "-s",
            KEYCHAIN_SERVICE,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// `security -i` parses its line like a shell would, so a value containing a
/// quote or backslash has to be escaped or it changes the command.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

// ── PKCE and URL helpers ────────────────────────────────────────────────────

/// 256 bits of CSPRNG entropy, base64url without padding — the RFC 7636 shape.
///
/// `Uuid::new_v4` draws from the OS CSPRNG, so two of them give 32 random bytes
/// without pulling in a separate RNG crate.
fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    bytes[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
    base64_url(&bytes)
}

fn code_challenge(verifier: &str) -> String {
    base64_url(&Sha256::digest(verifier.as_bytes()))
}

fn base64_url(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn urldecode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                match u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pkce_challenge_matches_the_rfc_7636_vector() {
        // RFC 7636 appendix B: the one worked example, so an encoding slip here
        // shows up as a failing test rather than a rejected sign-in.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            code_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifiers_are_unpredictable_and_correctly_shaped() {
        let first = random_token();
        let second = random_token();
        assert_ne!(first, second);
        // RFC 7636 requires 43..=128 characters from the unreserved set.
        assert_eq!(first.len(), 43);
        assert!(
            first
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
    }

    #[test]
    fn url_encoding_round_trips_reserved_characters() {
        for value in ["a b", "a+b", "a/b=c", "naïve — ✓", "code&state=x"] {
            assert_eq!(urldecode(&urlencode(value)), value);
        }
        // A redirect URI must survive intact, since Alabasta compares it byte
        // for byte against the one recorded with the code.
        assert_eq!(
            urlencode("http://127.0.0.1:5173/callback"),
            "http%3A%2F%2F127.0.0.1%3A5173%2Fcallback"
        );
    }

    #[test]
    fn keychain_arguments_cannot_break_out_of_their_quoting() {
        // The secret reaches `security -i` on stdin, but it is still parsed as a
        // shell-ish line, so a token containing a quote must not end the value.
        assert_eq!(quote(r#"ab"cd\ef"#), r#""ab\"cd\\ef""#);
        // The property that matters: only the two delimiters are unescaped, so
        // the value cannot terminate early and turn the rest into arguments.
        let quoted = quote(r#"a"b\c"#);
        let unescaped = quoted
            .char_indices()
            .filter(|(index, character)| *character == '"' && !quoted[..*index].ends_with('\\'))
            .count();
        assert_eq!(unescaped, 2, "{quoted}");
    }

    #[test]
    fn an_expiry_is_reached_before_the_token_actually_dies() {
        // A token good for an hour must be refreshed early enough that a slow
        // request cannot arrive after it expired.
        let expires_at = expiry_from(3600);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(expires_at > now);
        assert!(expires_at <= now + 3600 - REFRESH_SKEW_SECONDS);
        assert!(is_expired(expiry_from(0)));
    }
}
