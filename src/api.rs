//! Client for tailscaled's LocalAPI: plain HTTP over a Unix socket.
//!
//! Requests are HTTP/1.0 with one connection each, so the daemon answers with a plain
//! body and closes; no chunked decoding or connection pooling is needed. Every call
//! blocks, so call these from a worker thread, never from the GTK main loop.

use serde::de::DeserializeOwned;
use std::fmt;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::model::*;

const DEFAULT_SOCKET: &str = "/var/run/tailscale/tailscaled.sock";

#[derive(Debug, Clone)]
pub enum Error {
    /// The socket is missing or refused the connection: tailscaled is not running.
    NoDaemon,
    /// The daemon answered with an error status. 403 usually means "not the operator".
    Http(u16, String),
    Io(String),
    Json(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoDaemon => write!(f, "Tailscale is not running on this machine"),
            Error::Http(403, _) => write!(
                f,
                "Not allowed. Run once: sudo tailscale set --operator=$USER"
            ),
            Error::Http(code, body) => {
                let body = body.trim();
                if body.is_empty() {
                    write!(f, "Tailscale answered with error {code}")
                } else {
                    write!(f, "{}", body.lines().next().unwrap_or(body))
                }
            }
            Error::Io(e) => write!(f, "Could not talk to Tailscale: {e}"),
            Error::Json(e) => write!(f, "Unexpected answer from Tailscale: {e}"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn socket_path() -> String {
    std::env::var("PIXELSCALE_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET.to_string())
}

fn connect(timeout: Duration) -> Result<UnixStream> {
    let s = UnixStream::connect(socket_path()).map_err(|_| Error::NoDaemon)?;
    let _ = s.set_read_timeout(Some(timeout));
    let _ = s.set_write_timeout(Some(timeout));
    Ok(s)
}

fn head(method: &str, path: &str, content_length: Option<u64>) -> String {
    let mut h = format!(
        "{method} /localapi/v0/{path} HTTP/1.0\r\nHost: local-tailscaled.sock\r\nSec-Tailscale: localapi\r\nConnection: close\r\n"
    );
    if let Some(n) = content_length {
        h.push_str(&format!("Content-Length: {n}\r\n"));
    }
    h.push_str("\r\n");
    h
}

/// Split a raw HTTP response into (status code, body).
fn parse(raw: Vec<u8>) -> Result<(u16, Vec<u8>)> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| Error::Io("truncated response".into()))?;
    let header = String::from_utf8_lossy(&raw[..split]);
    let code = header
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or_else(|| Error::Io("bad status line".into()))?;
    Ok((code, raw[split + 4..].to_vec()))
}

fn finish(mut s: UnixStream) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    s.read_to_end(&mut raw)
        .map_err(|e| Error::Io(e.to_string()))?;
    let (code, body) = parse(raw)?;
    if (200..300).contains(&code) {
        Ok(body)
    } else {
        Err(Error::Http(
            code,
            String::from_utf8_lossy(&body).into_owned(),
        ))
    }
}

pub fn request(
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut s = connect(timeout)?;
    let len = body
        .map(|b| b.len() as u64)
        .or(if method == "GET" { None } else { Some(0) });
    s.write_all(head(method, path, len).as_bytes())
        .map_err(|e| Error::Io(e.to_string()))?;
    if let Some(b) = body {
        s.write_all(b).map_err(|e| Error::Io(e.to_string()))?;
    }
    finish(s)
}

fn get<T: DeserializeOwned>(path: &str) -> Result<T> {
    let body = request("GET", path, None, Duration::from_secs(5))?;
    serde_json::from_slice(&body).map_err(|e| Error::Json(e.to_string()))
}

/// Percent-encode one URL path segment or query value.
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn status() -> Result<Status> {
    get("status")
}

pub fn prefs() -> Result<Prefs> {
    get("prefs")
}

/// Change preferences. `patch` is a MaskedPrefs object: each field needs its `<Field>Set: true`.
pub fn edit_prefs(patch: serde_json::Value) -> Result<()> {
    let body = serde_json::to_vec(&patch).map_err(|e| Error::Json(e.to_string()))?;
    request("PATCH", "prefs", Some(&body), Duration::from_secs(10)).map(|_| ())
}

pub fn set_running(on: bool) -> Result<()> {
    edit_prefs(serde_json::json!({ "WantRunning": on, "WantRunningSet": true }))
}

/// `id` is a peer's stable node ID, or "" for no exit node.
pub fn set_exit_node(id: &str) -> Result<()> {
    edit_prefs(serde_json::json!({
        "ExitNodeID": id, "ExitNodeIDSet": true,
        "ExitNodeIP": "", "ExitNodeIPSet": true
    }))
}

pub fn set_bool_pref(field: &str, value: bool) -> Result<()> {
    let mut m = serde_json::Map::new();
    m.insert(field.to_string(), serde_json::Value::Bool(value));
    m.insert(format!("{field}Set"), serde_json::Value::Bool(true));
    edit_prefs(serde_json::Value::Object(m))
}

pub fn profiles() -> Result<Vec<Profile>> {
    get::<Option<Vec<Profile>>>("profiles/").map(Option::unwrap_or_default)
}

pub fn current_profile() -> Result<Profile> {
    get("profiles/current")
}

pub fn switch_profile(id: &str) -> Result<()> {
    request(
        "POST",
        &format!("profiles/{}", encode(id)),
        None,
        Duration::from_secs(15),
    )
    .map(|_| ())
}

/// Start an interactive login; the URL to open then appears as `Status::auth_url`.
pub fn login_interactive() -> Result<()> {
    request("POST", "login-interactive", None, Duration::from_secs(10)).map(|_| ())
}

pub fn ping(ip: &str) -> Result<PingResult> {
    let body = request(
        "POST",
        &format!("ping?ip={}&type=disco", encode(ip)),
        None,
        Duration::from_secs(12),
    )?;
    serde_json::from_slice(&body).map_err(|e| Error::Json(e.to_string()))
}

pub fn file_targets() -> Result<Vec<FileTarget>> {
    get::<Option<Vec<FileTarget>>>("file-targets").map(Option::unwrap_or_default)
}

pub fn waiting_files() -> Result<Vec<WaitingFile>> {
    get::<Option<Vec<WaitingFile>>>("files/").map(Option::unwrap_or_default)
}

/// Download one received file into `dir`, never overwriting: "name (1).ext" and so on.
/// Returns the path written. The file stays in Tailscale's inbox until `delete_file`.
pub fn save_file(name: &str, dir: &Path) -> Result<std::path::PathBuf> {
    let mut s = connect(Duration::from_secs(60))?;
    s.write_all(head("GET", &format!("files/{}", encode(name)), None).as_bytes())
        .map_err(|e| Error::Io(e.to_string()))?;
    let body = finish(s)?;
    std::fs::create_dir_all(dir).map_err(|e| Error::Io(e.to_string()))?;
    // Only the final component: a sender must not be able to choose the directory.
    let safe = Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("received-file")
        .to_string();
    let (stem, ext) = match safe.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (safe.clone(), String::new()),
    };
    let mut path = dir.join(&safe);
    let mut n = 1;
    while path.exists() {
        path = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    std::fs::write(&path, body).map_err(|e| Error::Io(e.to_string()))?;
    Ok(path)
}

pub fn delete_file(name: &str) -> Result<()> {
    request(
        "DELETE",
        &format!("files/{}", encode(name)),
        None,
        Duration::from_secs(10),
    )
    .map(|_| ())
}

/// Send a file to a peer, streaming it from disk. `sent` is updated as bytes go out.
pub fn send_file(stable_id: &str, path: &Path, sent: &AtomicU64) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::Io("bad file name".into()))?;
    let mut file = std::fs::File::open(path).map_err(|e| Error::Io(e.to_string()))?;
    let size = file.metadata().map_err(|e| Error::Io(e.to_string()))?.len();
    let mut s = connect(Duration::from_secs(120))?;
    s.write_all(
        head(
            "PUT",
            &format!("file-put/{}/{}", encode(stable_id), encode(name)),
            Some(size),
        )
        .as_bytes(),
    )
    .map_err(|e| Error::Io(e.to_string()))?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        s.write_all(&buf[..n])
            .map_err(|e| Error::Io(e.to_string()))?;
        sent.fetch_add(n as u64, Ordering::Relaxed);
    }
    finish(s).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_response() {
        let raw = b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{\"a\":1}".to_vec();
        let (code, body) = parse(raw).unwrap();
        assert_eq!(code, 200);
        assert_eq!(body, b"{\"a\":1}");
    }

    #[test]
    fn rejects_truncated_response() {
        assert!(parse(b"HTTP/1.0 200 OK\r\n".to_vec()).is_err());
    }

    #[test]
    fn encodes_path_segments() {
        assert_eq!(encode("a b/c.txt"), "a%20b%2Fc.txt");
        assert_eq!(encode("100.64.0.1"), "100.64.0.1");
        assert_eq!(encode("fd7a:115c::1"), "fd7a%3A115c%3A%3A1");
    }

    #[test]
    fn forbidden_explains_operator() {
        let msg = Error::Http(403, "access denied".into()).to_string();
        assert!(msg.contains("--operator"));
    }
}
