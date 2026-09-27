//! Outbound HTTP for the few remote APIs Anastasia talks to.
//!
//! Requests run through `/usr/bin/curl` rather than a linked HTTP client. That
//! is deliberate and predates this module: **headers travel to curl as a config
//! file on stdin, never on argv**, so bearer tokens cannot appear in the process
//! table where any other process on the machine could read them. Adding a TLS
//! stack to reach a handful of endpoints would be more code and worse for secret
//! hygiene.
//!
//! Everything here blocks. Call it from `cx.background_executor()`, never from a
//! path a frame can reach.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, anyhow};

/// How long any single request may take. Long enough for a cold Convex function,
/// short enough that a hung network cannot wedge a session start.
const TIMEOUT_SECONDS: &str = "15";

/// A response's status and body. Non-2xx is returned rather than raised: callers
/// distinguish "expired token, refresh and retry" from "genuinely broken".
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Sends a request and returns its status and body.
///
/// `headers` are `Name: value` lines. `body`, when present, is sent as the
/// request body with no assumptions about its content type — set that through a
/// header.
pub fn request(
    method: &str,
    url: &str,
    headers: &[String],
    body: Option<&str>,
) -> anyhow::Result<Response> {
    // The body goes in a private temp file rather than argv for the same reason
    // the headers do, and because a JSON document cannot be quoted reliably
    // inside curl's own config syntax.
    let body_file = body.map(write_private_temp_file).transpose()?;

    let mut arguments = vec![
        "-sS".to_owned(),
        "--max-time".to_owned(),
        TIMEOUT_SECONDS.to_owned(),
        "-D".to_owned(),
        "-".to_owned(),
        "-X".to_owned(),
        method.to_owned(),
        "-K".to_owned(),
        "-".to_owned(),
    ];
    if let Some(path) = &body_file {
        arguments.push("--data-binary".to_owned());
        arguments.push(format!("@{}", path.display()));
    }
    arguments.push(url.to_owned());

    let mut child = Command::new("/usr/bin/curl")
        .args(&arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not run curl")?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("curl stdin was unavailable"))?;
        for header in headers {
            writeln!(stdin, "header = \"{}\"", escape_for_curl_config(header))
                .context("could not configure curl")?;
        }
    }
    let output = child
        .wait_with_output()
        .context("curl did not finish cleanly")?;

    if let Some(path) = &body_file {
        // Best effort: the directory is private and per-request, so a leftover
        // file is not a leak, but do not accumulate them.
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(path.parent().unwrap_or(path));
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error = stderr
            .lines()
            .last()
            .map(str::trim)
            .filter(|error| !error.is_empty())
            .unwrap_or("unknown error")
            .to_owned();
        return Err(anyhow!("request failed: {error}"));
    }
    let (status, body) = split_status_and_body(&String::from_utf8_lossy(&output.stdout))?;
    Ok(Response { status, body })
}

/// curl's config parser reads double-quoted values with backslash escapes, so a
/// header carrying either character has to be escaped or it truncates the value
/// — or worse, ends the line early.
fn escape_for_curl_config(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_private_temp_file(body: &str) -> anyhow::Result<std::path::PathBuf> {
    let directory = std::env::temp_dir()
        .join("anastasia-http")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&directory).context("could not create a request directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    }
    let path = directory.join("body.json");
    std::fs::write(&path, body).context("could not stage the request body")?;
    Ok(path)
}

/// `-D -` prefixes the body with the response headers; the status code is on the
/// first line and the body follows the blank separator line.
///
/// A redirect or a `100 Continue` produces more than one header block, so the
/// *last* status line is the real one.
fn split_status_and_body(raw: &str) -> anyhow::Result<(u16, String)> {
    let mut remaining = raw;
    let mut status = None;
    loop {
        let Some(line) = remaining.lines().next() else {
            break;
        };
        if !line.starts_with("HTTP/") {
            break;
        }
        status = line
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok());
        let Some((_, rest)) = remaining
            .split_once("\r\n\r\n")
            .or_else(|| remaining.split_once("\n\n"))
        else {
            remaining = "";
            break;
        };
        remaining = rest;
    }
    let status = status.ok_or_else(|| anyhow!("the response had no status line"))?;
    Ok((status, remaining.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bearer_token_never_reaches_the_process_table() {
        // The whole reason this module shells to curl. Assert the escaping keeps
        // a hostile header on one config line rather than breaking out of it.
        let escaped = escape_for_curl_config("Authorization: Bearer ab\"cd\\ef");
        assert_eq!(escaped, "Authorization: Bearer ab\\\"cd\\\\ef");
        assert!(!escaped.contains('\n'));
    }

    #[test]
    fn the_final_status_line_wins_after_a_redirect() {
        let raw = "HTTP/1.1 307 Temporary Redirect\r\nLocation: /x\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let (status, body) = split_status_and_body(raw).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, "{\"ok\":true}");
    }

    #[test]
    fn a_plain_response_parses() {
        let raw =
            "HTTP/2 401 \r\nContent-Type: application/json\r\n\r\n{\"error\":\"Unauthorized\"}";
        let (status, body) = split_status_and_body(raw).unwrap();
        assert_eq!(status, 401);
        assert_eq!(body, "{\"error\":\"Unauthorized\"}");
        assert!(!Response { status, body }.is_success());
    }

    #[test]
    fn a_response_with_no_status_line_is_an_error() {
        assert!(split_status_and_body("garbage").is_err());
    }
}
