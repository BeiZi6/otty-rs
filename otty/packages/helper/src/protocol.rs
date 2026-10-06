//! The line-delimited JSON protocol `otty-helper` speaks.
//!
//! Kept as a library as well as a binary so the local side and the remote side
//! cannot drift: the app depends on `otty-helper` for these types, and the two
//! ends are compiled from one definition.

use serde::{Deserialize, Serialize};

/// Returned by the helper when the local side asks for the ident.
pub const PROTOCOL_VERSION: u32 = 1;

/// A request from the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Who and what is on the far end, and which helper build answered.
    Identify,
    /// Resource usage.
    Stats,
    /// Read a file, or a byte range of one.
    Read {
        path: String,
        /// Byte offset; `None` means the whole file.
        offset: Option<u64>,
        /// How many bytes; `None` means to the end.
        length: Option<u64>,
    },
    /// Write a file, creating its parents.
    Write { path: String, data: String, mode: Option<u32> },
    /// List a directory.
    List { path: String },
    /// Stat a path.
    Stat { path: String },
    /// `mkdir -p`.
    Mkdir { path: String },
    /// Remove a file or an empty directory.
    Remove { path: String },
    /// Install Otty's shell integration for the named shell.
    InstallShellIntegration { shell: String, dir: String },
    /// Remove it again.
    RemoveShellIntegration { dir: String },
    /// Whether a shadowing terminfo entry exists for `term`, and what removing
    /// it would change.
    TerminfoCheck { term: String },
    /// Move that entry aside into `~/.terminfo-otty-backup`.
    TerminfoClean { term: String },
    /// Stop reading.
    Quit,
}

/// A response from the helper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Err(RpcError),
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Self::Ok { data: Some(data), text: None }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::Ok { data: None, text: Some(text.into()) }
    }

    pub fn empty() -> Self {
        Self::Ok { data: None, text: None }
    }

    pub fn err(code: &str, message: impl Into<String>) -> Self {
        Self::Err(RpcError { code: code.to_owned(), message: message.into() })
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

/// A failure the helper reports rather than crashing on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    /// A stable code the local side can branch on.
    pub code: String,
    pub message: String,
}

impl RpcError {
    pub const IO: &'static str = "io";
    pub const NO_SUCH_PATH: &'static str = "no_such_path";
    pub const NOT_A_FILE: &'static str = "not_a_file";
    pub const TOO_LARGE: &'static str = "too_large";
    pub const BAD_REQUEST: &'static str = "bad_request";
    pub const UNSUPPORTED: &'static str = "unsupported";
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// One line of the wire, in either direction.
pub fn encode<T: Serialize>(value: &T) -> std::io::Result<String> {
    let mut s = serde_json::to_string(value)?;
    // A newline terminates every frame, and a serialiser cannot emit one inside
    // a JSON string unescaped, so the framing is unambiguous.
    s.push('\n');
    Ok(s)
}

/// Parse one line of the wire.
pub fn decode<T: for<'de> Deserialize<'de>>(line: &str) -> Result<T, RpcError> {
    serde_json::from_str(line.trim()).map_err(|e| RpcError {
        code: RpcError::BAD_REQUEST.to_owned(),
        message: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_round_trips_through_one_line() {
        for req in [
            Request::Identify,
            Request::Stats,
            Request::Read { path: "/etc/hosts".into(), offset: Some(10), length: Some(20) },
            Request::Write { path: "/tmp/a".into(), data: "hello".into(), mode: Some(0o644) },
            Request::List { path: "/tmp".into() },
            Request::TerminfoCheck { term: "xterm-otty".into() },
            Request::Quit,
        ] {
            let line = encode(&req).unwrap();
            assert!(line.ends_with('\n'));
            assert_eq!(line.matches('\n').count(), 1, "one frame is one line");
            assert_eq!(decode::<Request>(&line).unwrap(), req);
        }
    }

    #[test]
    fn a_path_with_a_newline_stays_on_one_frame() {
        // A filename may contain a newline on every POSIX system; the framing
        // has to survive it, which is why the newline is JSON-escaped.
        let req = Request::Read { path: "/tmp/two\nlines".into(), offset: None, length: None };
        let line = encode(&req).unwrap();
        assert_eq!(line.matches('\n').count(), 1);
        assert_eq!(decode::<Request>(&line).unwrap(), req);
    }

    #[test]
    fn responses_carry_a_code_the_local_side_can_branch_on() {
        let r = Response::err(RpcError::NO_SUCH_PATH, "no such file");
        let line = encode(&r).unwrap();
        match decode::<Response>(&line).unwrap() {
            Response::Err(e) => {
                assert_eq!(e.code, "no_such_path");
                assert_eq!(e.message, "no such file");
            }
            other => panic!("{other:?}"),
        }
        assert!(Response::empty().is_ok());
        assert!(!r.is_ok());
    }

    #[test]
    fn a_malformed_line_is_a_bad_request_not_a_panic() {
        let e = decode::<Request>("{not json").unwrap_err();
        assert_eq!(e.code, RpcError::BAD_REQUEST);
    }

    #[test]
    fn the_wire_spelling_is_snake_case() {
        let line = encode(&Request::InstallShellIntegration {
            shell: "zsh".into(),
            dir: "/home/u/.otty".into(),
        })
        .unwrap();
        assert!(line.contains("\"op\":\"install_shell_integration\""), "{line}");
        let line = encode(&Request::TerminfoCheck { term: "x".into() }).unwrap();
        assert!(line.contains("\"op\":\"terminfo_check\""), "{line}");
    }

    #[test]
    fn the_protocol_version_is_declared() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}
