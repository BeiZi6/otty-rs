//! The control channel between the CLI, the agent hooks and the running app.
//!
//! One listening socket per user, one request per connection, one JSON object
//! each way, newline-terminated. That is intentionally boring: the CLI is a
//! short-lived process that has to talk to a long-lived app, possibly from a
//! script, possibly from inside a pane, and neither end should need a runtime.
//!
//! The transport is a Unix domain socket on POSIX and a named pipe on Windows; a
//! Windows named pipe can only be opened by a process at the same or higher
//! integrity level, but it cannot be placed on the filesystem, so the security
//! property comes from the pipe ACL rather than from a directory mode.
//!
//! # Privilege
//!
//! Two gates, both off by default and both enforced here rather than at the
//! call site:
//!
//! * `ipc-allow-send-keys` — without it, `pane send-keys` is refused, so a
//!   process inside one pane cannot type into another.
//! * `ipc-allow-sensitive-sessions` — without it, `send-keys` and `capture` are
//!   refused on a pane whose foreground process is `ssh` or `sudo`, which is
//!   what stops a local script from driving a remote shell.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use otty_core::{AgentId, AgentState, PaneId, SessionId, TabId, WindowId};
use serde::{Deserialize, Serialize};

/// A request from a client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// `otty window …`
    Window(WindowRequest),
    /// `otty tab …`
    Tab(TabRequest),
    /// `otty pane …`
    Pane(PaneRequest),
    /// `otty state:<agent> …` — what the agent hooks call.
    AgentState {
        agent: AgentId,
        #[serde(rename = "session-id")]
        session_id: String,
        state: AgentState,
        cwd: Option<String>,
        #[serde(default)]
        bypass: bool,
        #[serde(rename = "agent-pid")]
        agent_pid: Option<u32>,
        #[serde(rename = "error-kind")]
        error_kind: Option<String>,
    },
    /// `otty config reload`
    ReloadConfig,
    /// Set a tab badge directly.
    Badge { tab: Selector, kind: BadgeKind, text: Option<String> },
    /// Post a notification on behalf of the shell integration.
    Notify { title: Option<String>, body: String, pane: Option<PaneId> },
    /// Change the active theme.
    SetTheme { name: String },
    /// Ask what the app knows about itself — `otty doctor`, `otty version`.
    Status,
}

/// A `window` subcommand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowRequest {
    List,
    Show(Selector),
    New { profile: Option<String>, cwd: Option<String>, command: Option<String>, title: Option<String> },
    Close(Selector),
    Focus(Selector),
    Rename { window: Selector, title: String },
    /// The frontmost tab of a window — `selected tab of front window`.
    SelectedTab(Selector),
}

/// A `tab` subcommand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabRequest {
    List,
    Show(Selector),
    New { window: Option<Selector>, profile: Option<String>, cwd: Option<String>, command: Option<String>, title: Option<String> },
    Close(Selector),
    Focus(Selector),
    Rename { tab: Selector, title: String },
    /// `otty tab move` — reorder or move to another window.
    Move { tab: Selector, to_window: Option<Selector>, position: Option<u32> },
    /// `otty tab badge --kind running`
    Badge { tab: Selector, kind: BadgeKind },
}

/// A `pane` subcommand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneRequest {
    List,
    Show(Selector),
    Split {
        pane: Option<Selector>,
        direction: SplitDirection,
        /// The new pane's share, 10–90.
        size: Option<u8>,
        command: Option<String>,
        cwd: Option<String>,
    },
    Close(Selector),
    Focus(Selector),
    Zoom(Selector),
    /// `--right 10 --down 5`, in cells.
    Resize { pane: Option<Selector>, right: Option<i32>, down: Option<i32>, left: Option<i32>, up: Option<i32> },
    SendKeys { pane: Option<Selector>, keys: Vec<String> },
    SendText { pane: Option<Selector>, text: String },
    /// `otty pane run` — send text then Enter.
    Run { pane: Option<Selector>, command: String },
    /// `otty pane exec` — replace the pane's command.
    Exec { pane: Option<Selector>, command: String },
    /// `otty pane capture --lines 50`
    Capture { pane: Option<Selector>, lines: Option<u32>, escapes: bool },
    /// The pane's tty device path. AppleScript's `tty of t` returns this, and so
    /// does `otty pane tty` — a script that attaches a tool to a pane needs the
    /// device, not the pane id.
    Tty { pane: Option<Selector> },
    /// The names of the processes running in the pane. `processes of t` in the
    /// AppleScript dictionary.
    Processes { pane: Option<Selector> },
    /// Whether a foreground process other than the shell is running — the
    /// `busy` property AppleScript exposes, and what `otty pane wait` keys on
    /// for a shell pane.
    Busy { pane: Option<Selector> },
    /// The pane's grid size, for `number of rows` / `number of columns`.
    Dimensions { pane: Option<Selector> },
    /// `otty watch` — wait for the pane to become idle.
    Wait { pane: Option<Selector>, timeout_ms: Option<u64> },
}

/// How a pane is split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    Right,
    Left,
    Up,
    Down,
}

/// The `running`/`completed`/`finished`/`unread`/`error`/`awaiting-input` kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BadgeKind {
    Running,
    Completed,
    Finished,
    Unread,
    Error,
    AwaitingInput,
}

impl BadgeKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "finished" => Self::Finished,
            "unread" => Self::Unread,
            "error" => Self::Error,
            "awaiting-input" | "awaiting_input" => Self::AwaitingInput,
            _ => return None,
        })
    }
}

/// A window/tab/pane selector: an id or an index.
///
/// A selector that does not match is an error (exit 4), never a silent fallback
/// to the active window — a script that asks for pane 3 and gets the focused
/// pane instead is a script that corrupts the wrong terminal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Selector {
    Id(String),
    Index(u32),
}

impl Selector {
    pub fn parse(s: &str) -> Self {
        match s.parse::<u32>() {
            Ok(i) => Self::Index(i),
            Err(_) => Self::Id(s.to_owned()),
        }
    }

    /// Match against a candidate id and its 1-based index.
    pub fn matches(&self, id: &str, index: u32) -> bool {
        match self {
            Self::Index(i) => *i == index,
            Self::Id(s) => s == id,
        }
    }
}

/// The answer to a [`Request`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
    Err {
        code: i32,
        message: String,
    },
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Self::Ok { data: Some(data), text: None }
    }

    pub fn ok_text(text: impl Into<String>) -> Self {
        Self::Ok { data: None, text: Some(text.into()) }
    }

    pub fn empty() -> Self {
        Self::Ok { data: None, text: None }
    }

    pub fn error(code: i32, message: impl Into<String>) -> Self {
        Self::Err { code, message: message.into() }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

/// The exit codes the CLI reference documents for a refusal.
pub mod code {
    /// The request was malformed or used an unknown key.
    pub const USAGE: i32 = 2;
    /// The app is not running, or the socket could not be reached.
    pub const NO_APP: i32 = 3;
    /// A selector matched nothing.
    pub const NO_MATCH: i32 = 4;
    /// The waiting command never saw a usable state.
    pub const NO_STATE: i32 = 4;
    /// The agent has no integration installed.
    pub const NO_INTEGRATION: i32 = 6;
    /// The request timed out.
    pub const TIMEOUT: i32 = 9;
    /// The action was refused by a privilege gate.
    pub const REFUSED: i32 = 13;
}

/// The privilege gates this channel enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gates {
    /// `ipc-allow-send-keys`.
    pub allow_send_keys: bool,
    /// `ipc-allow-sensitive-sessions`.
    pub allow_sensitive_sessions: bool,
}

/// What the app knows about one pane, used to answer a gate's question.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneInfo {
    pub id: PaneId,
    /// The foreground process name, if any.
    pub foreground: Option<String>,
    /// Whether the pane's tty is driven by a remote or privileged program.
    pub sensitive: bool,
}

impl PaneInfo {
    /// Whether the foreground program is one whose input must not be forged.
    pub fn is_sensitive(&self) -> bool {
        self.sensitive
            || self
                .foreground
                .as_deref()
                .is_some_and(|f| matches!(f, "ssh" | "sudo" | "su" | "doas"))
    }
}

/// Whether `req` may run, given `gates` and what `lookup` says about the panes.
///
/// Kept as a free function so the policy is one testable place rather than a
/// condition scattered through the handler.
pub fn check_privileges<F>(
    req: &Request,
    gates: Gates,
    mut lookup: F,
) -> Result<(), Response>
where
    F: FnMut(PaneId) -> Option<PaneInfo>,
{
    let pane_of = |sel: &Option<Selector>| -> Option<PaneId> {
        match sel {
            Some(Selector::Id(s)) => s.parse().ok(),
            _ => None,
        }
    };

    match req {
        Request::Pane(PaneRequest::SendKeys { pane, .. })
        | Request::Pane(PaneRequest::SendText { pane, .. })
        | Request::Pane(PaneRequest::Run { pane, .. })
        | Request::Pane(PaneRequest::Exec { pane, .. }) => {            if !gates.allow_send_keys {
                return Err(Response::error(
                    code::REFUSED,
                    "writing to a pane is disabled — set `ipc-allow-send-keys = true` in config.toml \
                     if you intend a script to type into Otty",
                ));
            }
            if !gates.allow_sensitive_sessions {
                if let Some(info) = pane_of(pane).and_then(&mut lookup) {
                    if info.is_sensitive() {
                        return Err(Response::error(
                            code::REFUSED,
                            format!(
                                "pane {} is running {}, and `ipc-allow-sensitive-sessions` is off",
                                info.id,
                                info.foreground.as_deref().unwrap_or("a privileged program")
                            ),
                        ));
                    }
                }
            }
        }
        Request::Pane(PaneRequest::Capture { pane, .. }) if !gates.allow_sensitive_sessions => {
            if let Some(info) = pane_of(pane).and_then(&mut lookup) {
                if info.is_sensitive() {
                    return Err(Response::error(
                        code::REFUSED,
                        format!(
                            "capturing pane {} would read {} output",
                            info.id,
                            info.foreground.as_deref().unwrap_or("a privileged program")
                        ),
                    ));
                }
            }
        }
        // The shell integration posts notifications and badges legitimately;
        // they are gated by the `privilege-*` config keys on the app side, not
        // by the IPC gates.
        _ => {}
    }
    Ok(())
}

/// Where the socket lives.
///
/// `--socket <path>` wins; otherwise a per-user path that both the app and the
/// CLI can derive without configuration.
pub fn socket_path(override_path: Option<&Path>) -> PathBuf {
    if let Some(p) = override_path {
        return p.to_path_buf();
    }
    #[cfg(target_os = "windows")]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".into());
        PathBuf::from(format!(r"\\.\pipe\otty-{user}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            return PathBuf::from(dir).join("otty.sock");
        }
        otty_core::app_support_dir().join("otty.sock")
    }
}

/// A client for the channel.
#[derive(Debug, Clone)]
pub struct Client {
    path: PathBuf,
    timeout: Duration,
}

impl Client {
    /// A client for the default socket with the documented 3 s timeout.
    pub fn new(override_path: Option<&Path>) -> Self {
        Self { path: socket_path(override_path), timeout: Duration::from_millis(3000) }
    }

    /// Override the timeout (the CLI's `--timeout <ms>`).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    /// Send a request and read the answer.
    ///
    /// A missing socket is [`code::NO_APP`], not an I/O error: "the app is not
    /// running" is a normal outcome for `otty pane list`.
    pub fn request(&self, req: &Request) -> Result<Response, ClientError> {
        let stream = self.connect()?;
        let mut writer = stream.try_clone().map_err(ClientError::Io)?;
        let mut payload = serde_json::to_vec(req).map_err(ClientError::Encode)?;
        payload.push(b'\n');
        writer.write_all(&payload).map_err(ClientError::Io)?;
        writer.flush().map_err(ClientError::Io)?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(ClientError::Io)?;
        if line.trim().is_empty() {
            return Err(ClientError::EmptyResponse);
        }
        serde_json::from_str(&line).map_err(ClientError::Decode)
    }

    #[cfg(unix)]
    fn connect(&self) -> Result<std::os::unix::net::UnixStream, ClientError> {
        use std::os::unix::net::UnixStream;
        if !self.path.exists() {
            return Err(ClientError::NoApp);
        }
        let stream = UnixStream::connect(&self.path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                ClientError::NoApp
            }
            _ => ClientError::Io(e),
        })?;
        stream.set_read_timeout(Some(self.timeout)).map_err(ClientError::Io)?;
        stream.set_write_timeout(Some(self.timeout)).map_err(ClientError::Io)?;
        Ok(stream)
    }

    #[cfg(not(unix))]
    fn connect(&self) -> Result<std::fs::File, ClientError> {
        use std::fs::OpenOptions;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => ClientError::NoApp,
                _ => ClientError::Io(e),
            })
    }

    /// Poll until the app answers, or `deadline` passes. Backs `otty pane wait`
    /// and `otty watch:<agent>`.
    pub fn poll<F>(
        &self,
        req: &Request,
        deadline: Option<Duration>,
        interval: Duration,
        mut ready: F,
    ) -> Result<Response, ClientError>
    where
        F: FnMut(&Response) -> bool,
    {
        let start = Instant::now();
        loop {
            match self.request(req) {
                Ok(r) if ready(&r) => return Ok(r),
                Ok(_) | Err(ClientError::NoApp) => {}
                Err(e) => return Err(e),
            }
            if deadline.is_some_and(|d| start.elapsed() >= d) {
                return Err(ClientError::Timeout);
            }
            std::thread::sleep(interval);
        }
    }
}

/// The listening end: one connection at a time, one request per connection.
#[cfg(unix)]
struct Listener(std::os::unix::net::UnixListener);

#[cfg(unix)]
impl Listener {
    fn bind(path: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::{UnixListener, UnixStream};
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let listener = match UnixListener::bind(path) {
            // A socket file nobody answers on is what a crashed app leaves behind.
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if UnixStream::connect(path).is_ok() {
                    return Err(e);
                }
                std::fs::remove_file(path)?;
                UnixListener::bind(path)?
            }
            other => other?,
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self(listener))
    }

    fn accept(&mut self) -> std::io::Result<std::os::unix::net::UnixStream> {
        let (stream, _) = self.0.accept()?;
        // A client that connects and says nothing must not stall the channel.
        stream.set_read_timeout(Some(Duration::from_millis(3000)))?;
        Ok(stream)
    }
}

#[cfg(windows)]
mod pipe {
    use std::ffi::c_void;

    pub const PIPE_ACCESS_DUPLEX: u32 = 0x3;
    pub const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
    pub const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
    pub const PIPE_UNLIMITED_INSTANCES: u32 = 255;
    pub const ERROR_PIPE_CONNECTED: i32 = 535;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn CreateNamedPipeW(
            name: *const u16,
            open_mode: u32,
            pipe_mode: u32,
            max_instances: u32,
            out_buffer: u32,
            in_buffer: u32,
            default_timeout: u32,
            security: *mut c_void,
        ) -> *mut c_void;
        pub fn ConnectNamedPipe(pipe: *mut c_void, overlapped: *mut c_void) -> i32;
    }
}

/// On Windows every connection consumes one pipe instance, so the listener
/// holds the instance the next client will land on.
#[cfg(windows)]
struct Listener {
    name: Vec<u16>,
    next: std::fs::File,
}

#[cfg(windows)]
impl Listener {
    fn instance(name: &[u16], first: bool) -> std::io::Result<std::fs::File> {
        use std::os::windows::io::FromRawHandle;
        // `first` makes a second app fail to bind instead of sharing the name.
        let mode = pipe::PIPE_ACCESS_DUPLEX
            | if first { pipe::FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        // The default security descriptor lets only the creating user write.
        let handle = unsafe {
            pipe::CreateNamedPipeW(
                name.as_ptr(),
                mode,
                pipe::PIPE_REJECT_REMOTE_CLIENTS,
                pipe::PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle as isize == -1 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(unsafe { std::fs::File::from_raw_handle(handle) })
    }

    fn bind(path: &Path) -> std::io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let next = Self::instance(&name, true)?;
        Ok(Self { name, next })
    }

    // ponytail: no read timeout on a pipe, so a client that connects and never
    // writes stalls the channel until it exits; overlapped I/O if that bites.
    fn accept(&mut self) -> std::io::Result<std::fs::File> {
        use std::os::windows::io::AsRawHandle;
        let ok = unsafe { pipe::ConnectNamedPipe(self.next.as_raw_handle(), std::ptr::null_mut()) };
        if ok == 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() != Some(pipe::ERROR_PIPE_CONNECTED) {
                return Err(e);
            }
        }
        let fresh = Self::instance(&self.name, false)?;
        Ok(std::mem::replace(&mut self.next, fresh))
    }
}

/// Read one request line, answer it, hang up.
fn answer<C, F>(mut conn: C, handler: &mut F) -> std::io::Result<()>
where
    C: std::io::Read + Write,
    F: FnMut(Request) -> Response,
{
    use std::io::Read;
    let mut line = String::new();
    BufReader::new((&mut conn).take(1 << 20)).read_line(&mut line)?;
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(req) => handler(req),
        Err(e) => Response::error(code::USAGE, format!("not a request: {e}")),
    };
    let mut payload = serde_json::to_vec(&response)?;
    payload.push(b'\n');
    conn.write_all(&payload)?;
    conn.flush()
}

/// Listen on `path` and answer every request with `handler`, on a new thread.
///
/// Binding happens before this returns, so "another Otty already owns the
/// socket" is an error the app sees at startup rather than a silent thread.
pub fn serve<F>(path: &Path, mut handler: F) -> std::io::Result<std::thread::JoinHandle<()>>
where
    F: FnMut(Request) -> Response + Send + 'static,
{
    let mut listener = Listener::bind(path)?;
    std::thread::Builder::new().name("otty-ipc".into()).spawn(move || loop {
        match listener.accept() {
            Ok(conn) => {
                if let Err(e) = answer(conn, &mut handler) {
                    log::debug!("ipc: connection dropped: {e}");
                }
            }
            Err(e) => {
                log::error!("ipc: listener stopped: {e}");
                return;
            }
        }
    })
}

/// A parsed `key=value` argument list, the shape `otty state:<agent>` uses.
pub fn parse_pairs<I, S>(args: I) -> BTreeMap<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .filter_map(|a| {
            let a = a.as_ref();
            a.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned()))
        })
        .collect()
}

/// Build a [`Request::AgentState`] from the arguments an `otty-hook.sh` passes.
pub fn agent_state_from_args(agent: &str, args: &[String]) -> Result<Request, String> {
    let pairs = parse_pairs(args);
    let get = |k: &str| pairs.get(k).cloned();
    let state = get("state")
        .and_then(|s| AgentState::parse(&s))
        .ok_or_else(|| "missing or unknown `state=`".to_owned())?;
    let session_id = get("session-id").ok_or_else(|| "missing `session-id=`".to_owned())?;
    Ok(Request::AgentState {
        agent: AgentId::parse(agent),
        session_id,
        state,
        cwd: get("cwd").filter(|s| !s.is_empty()),
        bypass: get("bypass").is_some_and(|b| b == "1" || b.eq_ignore_ascii_case("true")),
        agent_pid: get("agent-pid").and_then(|p| p.parse().ok()),
        error_kind: get("error-kind").filter(|s| !s.is_empty()),
    })
}

/// A session the app tracks, for the `list` commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub window: WindowId,
    pub tab: TabId,
    pub pane: PaneId,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub agent: Option<SessionId>,
}

/// Why a request could not be answered.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Otty is not running")]
    NoApp,
    #[error("no reply from Otty within the timeout")]
    Timeout,
    #[error("Otty closed the connection without answering")]
    EmptyResponse,
    #[error("could not reach Otty: {0}")]
    Io(std::io::Error),
    #[error("could not encode the request: {0}")]
    Encode(serde_json::Error),
    #[error("Otty sent something unreadable: {0}")]
    Decode(serde_json::Error),
}

impl ClientError {
    /// The exit code the CLI should use.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoApp => code::NO_APP,
            Self::Timeout => code::TIMEOUT,
            Self::EmptyResponse => code::NO_APP,
            Self::Io(_) | Self::Encode(_) | Self::Decode(_) => code::USAGE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_core::Modifiers;

    #[test]
    fn requests_round_trip_as_json() {
        let cases = vec![
            Request::Pane(PaneRequest::Split {
                pane: Some(Selector::Index(3)),
                direction: SplitDirection::Right,
                size: Some(30),
                command: Some("htop".into()),
                cwd: None,
            }),
            Request::Pane(PaneRequest::SendKeys {
                pane: Some(Selector::Id("pane-3".into())),
                keys: vec!["echo hi".into(), "key:Enter".into()],
            }),
            Request::Pane(PaneRequest::Capture { pane: None, lines: Some(50), escapes: false }),
            Request::Window(WindowRequest::List),
            Request::Tab(TabRequest::Badge { tab: Selector::Index(1), kind: BadgeKind::Running }),
            Request::ReloadConfig,
            Request::Status,
        ];
        for r in cases {
            let s = serde_json::to_string(&r).unwrap();
            let back: Request = serde_json::from_str(&s).unwrap();
            assert_eq!(back, r, "round trip of {s}");
        }
    }

    #[test]
    fn a_request_uses_the_wire_spelling_the_hooks_use() {
        let r = Request::AgentState {
            agent: AgentId::Claude,
            session_id: "9f3c1a2b".into(),
            state: AgentState::Processing,
            cwd: Some("/work".into()),
            bypass: false,
            agent_pid: Some(4242),
            error_kind: None,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"cmd\":\"agent_state\""), "{s}");
        assert!(s.contains("\"session-id\":\"9f3c1a2b\""), "{s}");
        assert!(s.contains("\"agent-pid\":4242"), "{s}");
    }

    #[test]
    fn responses_carry_the_documented_exit_codes() {
        assert_eq!(Response::error(code::NO_MATCH, "no pane").is_ok(), false);
        let r = Response::error(code::NO_MATCH, "No pane matched selector");
        match r {
            Response::Err { code, message } => {
                assert_eq!(code, 4);
                assert_eq!(message, "No pane matched selector");
            }
            _ => unreachable!(),
        }
        assert!(Response::ok(serde_json::json!({"panes": []})).is_ok());
    }

    #[test]
    fn selectors_prefer_an_index_and_keep_ids_opaque() {
        assert_eq!(Selector::parse("3"), Selector::Index(3));
        assert_eq!(Selector::parse("pane-3"), Selector::Id("pane-3".into()));
        assert!(Selector::Index(3).matches("pane-7", 3));
        assert!(Selector::Id("pane-7".into()).matches("pane-7", 1));
        assert!(!Selector::Id("pane-7".into()).matches("pane-8", 7));
    }

    #[test]
    fn badge_kinds_parse_their_documented_names() {
        for name in ["running", "completed", "finished", "unread", "error", "awaiting-input"] {
            assert!(BadgeKind::parse(name).is_some(), "{name}");
        }
        assert_eq!(BadgeKind::parse("awaiting_input"), Some(BadgeKind::AwaitingInput));
        assert_eq!(BadgeKind::parse("nope"), None);
    }

    #[test]
    fn send_keys_is_refused_until_the_gate_is_opened() {
        let req = Request::Pane(PaneRequest::SendKeys {
            pane: Some(Selector::Id("pane-1".into())),
            keys: vec!["rm -rf /".into()],
        });
        let closed = check_privileges(&req, Gates::default(), |_| None);
        assert!(closed.is_err());
        let open = check_privileges(
            &req,
            Gates { allow_send_keys: true, ..Default::default() },
            |_| None,
        );
        assert!(open.is_ok());
    }

    #[test]
    fn a_sensitive_pane_needs_its_own_gate_even_with_send_keys_on() {
        let req = Request::Pane(PaneRequest::SendText {
            pane: Some(Selector::Id("pane-2".into())),
            text: "whoami".into(),
        });
        let gates = Gates { allow_send_keys: true, allow_sensitive_sessions: false };
        let info = |_: PaneId| {
            Some(PaneInfo {
                id: PaneId::new(2),
                foreground: Some("ssh".into()),
                sensitive: false,
            })
        };
        let refused = check_privileges(&req, gates, info);
        assert!(refused.is_err(), "an ssh pane must not be typed into by default");

        let both = Gates { allow_send_keys: true, allow_sensitive_sessions: true };
        assert!(check_privileges(&req, both, info).is_ok());
    }

    #[test]
    fn a_pane_is_sensitive_by_its_foreground_program_too() {
        let local = PaneInfo { id: PaneId::new(1), foreground: Some("cargo".into()), sensitive: false };
        let ssh = PaneInfo { id: PaneId::new(2), foreground: Some("ssh".into()), sensitive: false };
        let flagged = PaneInfo { id: PaneId::new(3), foreground: None, sensitive: true };
        assert!(!local.is_sensitive());
        assert!(ssh.is_sensitive());
        assert!(flagged.is_sensitive());
    }

    #[test]
    fn capture_is_gated_on_sensitive_panes() {
        let req = Request::Pane(PaneRequest::Capture {
            pane: Some(Selector::Id("pane-9".into())),
            lines: None,
            escapes: false,
        });
        let info = |_: PaneId| {
            Some(PaneInfo { id: PaneId::new(9), foreground: Some("sudo".into()), sensitive: true })
        };
        assert!(check_privileges(&req, Gates::default(), info).is_err());
        let ok = Gates { allow_send_keys: false, allow_sensitive_sessions: true };
        assert!(check_privileges(&req, ok, info).is_ok());
    }

    #[test]
    fn harmless_requests_are_never_gated() {
        for req in [
            Request::Window(WindowRequest::List),
            Request::Status,
            Request::ReloadConfig,
            Request::Tab(TabRequest::List),
            Request::Pane(PaneRequest::List),
        ] {
            assert!(check_privileges(&req, Gates::default(), |_| None).is_ok());
        }
    }

    #[test]
    fn hook_arguments_become_a_state_report() {
        let args: Vec<String> = [
            "session-id=9f3c1a2b",
            "state=awaiting",
            "cwd=/work/otty",
            "bypass=1",
            "agent-pid=4242",
            "error-kind=",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        match agent_state_from_args("claude", &args).unwrap() {
            Request::AgentState { agent, session_id, state, cwd, bypass, agent_pid, error_kind } => {
                assert_eq!(agent, AgentId::Claude);
                assert_eq!(session_id, "9f3c1a2b");
                assert_eq!(state, AgentState::Awaiting);
                assert_eq!(cwd.as_deref(), Some("/work/otty"));
                assert!(bypass);
                assert_eq!(agent_pid, Some(4242));
                assert_eq!(error_kind, None, "an empty error-kind is not a kind");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_hook_report_without_a_state_is_rejected() {
        let args = vec!["session-id=x".to_owned()];
        assert!(agent_state_from_args("claude", &args).is_err());
        let args = vec!["state=idle".to_owned()];
        assert!(agent_state_from_args("claude", &args).is_err());
    }

    #[test]
    fn an_unknown_agent_still_reports() {
        let args = vec!["session-id=s1".to_owned(), "state=idle".to_owned()];
        match agent_state_from_args("aider", &args).unwrap() {
            Request::AgentState { agent, .. } => assert_eq!(agent.as_str(), "aider"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_missing_socket_is_reported_as_the_app_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let client = Client::new(Some(&dir.path().join("nothing.sock")));
        assert!(matches!(client.request(&Request::Status), Err(ClientError::NoApp)));
        assert_eq!(ClientError::NoApp.exit_code(), code::NO_APP);
        assert_eq!(ClientError::Timeout.exit_code(), code::TIMEOUT);
    }

    #[test]
    fn a_served_socket_answers_and_a_second_bind_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let path = PathBuf::from(format!(r"\\.\pipe\otty-test-{}", std::process::id()));
        #[cfg(not(windows))]
        let path = dir.path().join("otty.sock");
        let _ = &dir;

        serve(&path, |req| match req {
            Request::Status => Response::ok(serde_json::json!({"panes": 1})),
            _ => Response::error(code::NO_MATCH, "nope"),
        })
        .unwrap();
        assert!(serve(&path, |_| Response::empty()).is_err(), "one app per socket");

        let client = Client::new(Some(&path));
        // Twice: the listener must be ready for the next connection.
        for _ in 0..2 {
            assert_eq!(
                client.request(&Request::Status).unwrap(),
                Response::ok(serde_json::json!({"panes": 1}))
            );
        }
        match client.request(&Request::ReloadConfig).unwrap() {
            Response::Err { code, .. } => assert_eq!(code, code::NO_MATCH),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_default_socket_path_is_derived_not_configured() {
        let p = socket_path(None);
        assert!(!p.as_os_str().is_empty());
        let forced = socket_path(Some(Path::new("/tmp/x.sock")));
        assert_eq!(forced, PathBuf::from("/tmp/x.sock"));
    }

    #[test]
    fn the_default_timeout_is_the_documented_three_seconds() {
        let c = Client::new(None);
        assert_eq!(c.timeout, Duration::from_millis(3000));
        assert_eq!(c.with_timeout(Duration::from_millis(50)).timeout, Duration::from_millis(50));
    }

    #[test]
    fn key_arguments_tolerate_the_modifier_spelling_the_cli_uses() {
        // `key:Enter` is the CLI's spelling; it is forwarded verbatim and the
        // app resolves it through the same chord parser the keymap uses.
        let chord: otty_core::Chord = "cmd+shift+t".parse().unwrap();
        assert_eq!(chord.mods.contains(Modifiers::SUPER | Modifiers::SHIFT), true);
    }
}
