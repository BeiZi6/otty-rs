//! Pseudo-terminal and child-process management.
//!
//! One [`Pty`] owns one shell. It is deliberately thin: the OS-specific traps
//! live in `portable-pty`, and the parts that are Otty's own behaviour — shell
//! integration injection, `$TERM`, the working-directory rules, the session log,
//! and the live-resize signal delay — live here.
//!
//! # The window-size contract
//!
//! A drag sends one `SIGWINCH` per frame on some platforms, which makes a
//! full-screen TUI redraw dozens of times a second. `live-resize-sigwinch-delay-ms`
//! says how long the size must hold still before a *mid-drag* signal fires; the
//! drag end always fires one. [`ResizeGate`] implements that policy and is pure
//! enough to test without a terminal.

pub mod shell;
#[cfg(windows)]
mod win_proc;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use otty_core::expand_path;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use shell::{IntegrationEnv, ShellKind};

pub use shell::{detect_login_shell, is_executable, shell_invocation, ShellKind as Shell};

/// Everything needed to start one shell.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// The shell or command. Defaults to `command`, whose documented default is
    /// `$SHELL`, else `/bin/zsh` on POSIX.
    pub program: String,
    /// Extra arguments. A login shell's `-<name>` is added by
    /// [`shell_invocation`] when `login_greeting` is set.
    pub args: Vec<String>,
    /// `env = KEY=VALUE` entries, applied on top of the inherited environment.
    pub env: BTreeMap<String, String>,
    /// The initial working directory. `None` means the inherited one.
    pub cwd: Option<PathBuf>,
    pub cols: u16,
    pub rows: u16,
    /// `term` — exported as `$TERM`.
    pub term: String,
    /// `login-greeting`.
    pub login_greeting: bool,
    /// `shell-integration`.
    pub shell_integration: bool,
    /// The directory holding the integration payload.
    pub integration_dir: Option<PathBuf>,
    pub integration: IntegrationEnv,
    /// `live-resize-sigwinch-delay-ms`.
    pub sigwinch_delay: Duration,
    /// `session-log-mode` / `session-log-size-mb`; `None` disables logging.
    pub log: Option<LogConfig>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            program: detect_login_shell(),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            cols: 80,
            rows: 24,
            term: otty_core::Platform::host().default_term().to_owned(),
            login_greeting: false,
            shell_integration: true,
            integration_dir: None,
            integration: IntegrationEnv::default(),
            sigwinch_delay: Duration::from_millis(50),
            log: None,
        }
    }
}

/// `session-log-mode` and `session-log-size-mb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogConfig {
    pub mode: LogMode,
    /// The cap in bytes, derived from `session-log-size-mb`.
    pub max_bytes: u64,
}

impl LogConfig {
    /// Build from the two config keys.
    pub fn from_mb(mode: LogMode, megabytes: u64) -> Self {
        Self { mode, max_bytes: megabytes.saturating_mul(1024 * 1024) }
    }
}

/// How a session log treats secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogMode {
    /// Store output verbatim.
    Plain,
    /// Replace detected secrets with equal-width asterisks.
    Redacted,
}

impl LogMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "plain" => Some(Self::Plain),
            "redacted" => Some(Self::Redacted),
            _ => None,
        }
    }
}

/// Why a PTY could not be created.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("could not open a pseudo-terminal: {0}")]
    Open(#[source] anyhow::Error),
    #[error("could not start `{program}`: {source}")]
    Spawn { program: String, source: anyhow::Error },
    #[error("could not resize the pseudo-terminal: {0}")]
    Resize(#[source] anyhow::Error),
    #[error("could not write to the shell: {0}")]
    Write(#[source] std::io::Error),
    #[error("the working directory `{0}` does not exist")]
    NoSuchDirectory(String),
}

/// A running shell.
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    pid: Option<u32>,
    cols: u16,
    rows: u16,
    /// The integration directory, if injection happened — kept so a later
    /// `otty config set shell-integration false` can be explained.
    integration: Option<PathBuf>,
    log: Option<SessionLog>,
}

impl Pty {
    /// Spawn a shell.
    pub fn spawn(config: &PtyConfig) -> Result<Self, PtyError> {
        if let Some(dir) = &config.cwd {
            if !dir.is_dir() {
                return Err(PtyError::NoSuchDirectory(dir.display().to_string()));
            }
        }

        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(PtyError::Open)?;

        let (program, args) = shell_invocation(&config.program, config.login_greeting);
        let mut cmd = CommandBuilder::new(&program);
        for a in &args {
            cmd.arg(a);
        }
        for a in &config.args {
            cmd.arg(a);
        }
        if let Some(dir) = &config.cwd {
            cmd.cwd(dir);
        }
        cmd.env("TERM", &config.term);
        // The variables every Otty-aware script reads.
        cmd.env("TERM_PROGRAM", "Otty");
        cmd.env("TERM_PROGRAM_VERSION", otty_core::VERSION);
        if let Some(id) = &config.integration.pane_id {
            cmd.env("OTTY_PANE_ID", id);
        }
        cmd.env("OTTY", "1");
        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        // Shell integration: point the shell at Otty's payload, and hand it the
        // user's own root so the payload can give it back.
        let mut integration = None;
        let inject = config.shell_integration
            && config.integration.should_inject(&program, true)
            && config.integration_dir.is_some();
        if inject {
            let dir = config.integration_dir.clone().expect("checked above");
            let user_zdotdir = std::env::var_os("ZDOTDIR").map(PathBuf::from);
            // `to_env` writes into the same map the caller passed, so the
            // inherited environment is the base and Otty's additions win.
            let mut extra = BTreeMap::new();
            extra.insert("OTTY_TERM_INIT".into(), "1".into());
            config.integration.to_env(
                ShellKind::of(&program),
                &dir,
                &mut extra,
                user_zdotdir.as_deref(),
            );
            for (k, v) in extra {
                cmd.env(k, v);
            }
            integration = Some(dir);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|source| PtyError::Spawn { program: program.clone(), source })?;
        let pid = child.process_id();
        drop(pair.slave);

        let writer = pair.master.take_writer().map_err(PtyError::Open)?;
        Ok(Self {
            master: pair.master,
            child,
            writer,
            pid,
            cols: config.cols,
            rows: config.rows,
            integration,
            log: config.log.map(SessionLog::new),
        })
    }

    /// The child's process id, for ancestry-based pane matching.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// The device path of the pty's slave side, e.g. `/dev/ttys003`.
    ///
    /// AppleScript's `tty of t` returns exactly this string, and so does
    /// `otty pane tty`. `portable-pty` does not expose it on `MasterPty`, so this
    /// asks `ps` the same way [`foreground_processes`] already does — one
    /// mechanism for "which terminal is this process attached to", not two.
    ///
    /// Windows has no controlling-device path, so `None` is the honest answer
    /// there rather than a fabricated one.
    pub fn tty_name(&self) -> Option<String> {
        #[cfg(unix)]
        {
            let pid = self.pid?;
            let out = std::process::Command::new("ps")
                .args(["-o", "tty=", "-p", &pid.to_string()])
                .output()
                .ok()?;
            let name = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            // `ps` prints `??` for a process with no controlling terminal.
            if name.is_empty() || name == "??" || name == "?" {
                return None;
            }
            // `ps` prints the bare device name (`ttys003`); the AppleScript
            // property and the CLI both document a full path.
            Some(if name.starts_with('/') { name } else { format!("/dev/{name}") })
        }
        #[cfg(not(unix))]
        {
            None
        }
    }

    /// A reader for the shell's output. Call once; the reader is not shareable.
    pub fn reader(&self) -> std::io::Result<Box<dyn Read + Send>> {
        self.master.try_clone_reader().map_err(std::io::Error::other)
    }

    /// Write input to the shell.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.writer.write_all(bytes).map_err(PtyError::Write)?;
        self.writer.flush().map_err(PtyError::Write)
    }

    /// Resize the pty and its window size.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.master
            .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(PtyError::Resize)
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Whether the foreground process is something other than the shell — the
    /// `busy` property AppleScript exposes.
    pub fn is_busy(&self) -> bool {
        !self.processes().is_empty()
    }

    /// The foreground process names, for `processes of t`.
    pub fn processes(&self) -> Vec<String> {
        self.pid.map(foreground_processes).unwrap_or_default()
    }

    /// Try to reap the child without blocking. `Some` once it has exited.
    pub fn try_wait(&mut self) -> Option<portable_pty::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    /// Block until the child exits.
    pub fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        self.child.wait().map_err(std::io::Error::other)
    }

    /// Terminate the child.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

    /// Append output to the session log, if one is configured.
    pub fn log_output(&mut self, bytes: &[u8]) {
        if let Some(log) = &mut self.log {
            log.append(bytes);
        }
    }

    /// The session log path, when logging is on.
    pub fn log_path(&self) -> Option<&Path> {
        self.log.as_ref().map(|l| l.path.as_path())
    }

    /// Whether shell integration is active for this shell.
    pub fn integration_dir(&self) -> Option<&Path> {
        self.integration.as_deref()
    }
}

/// The children of `pid` that currently own the terminal.
///
/// On Unix, `ps -o pid=,ppid=,tty=,comm=` is the portable way to ask; a process
/// whose controlling terminal matches the shell's tty is running in the
/// foreground. Windows has no tty, so [`win_proc`] walks descendants of the
/// shell instead.
pub fn foreground_processes(pid: u32) -> Vec<String> {
    #[cfg(unix)]
    {
        unix_foreground_processes(pid)
    }
    #[cfg(windows)]
    {
        win_proc::foreground_processes(pid)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        Vec::new()
    }
}

#[cfg(unix)]
fn unix_foreground_processes(pid: u32) -> Vec<String> {
    use std::process::Command;
    let Ok(out) = Command::new("ps")
        .args(["-o", "pid=,ppid=,tty=,comm=", "-ax"])
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut tty: Option<String> = None;
    let mut procs: Vec<(u32, u32, String, String)> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(p), Some(pp), Some(t), comm) = (it.next(), it.next(), it.next(), it.collect::<Vec<_>>().join(" "))
        else {
            continue;
        };
        let (Ok(p), Ok(pp)) = (p.parse::<u32>(), pp.parse::<u32>()) else {
            continue;
        };
        if p == pid {
            tty = Some(t.to_owned());
        }
        procs.push((p, pp, t.to_owned(), comm));
    }
    let Some(tty) = tty else { return Vec::new() };
    procs
        .into_iter()
        .filter(|(p, pp, t, _)| *p != pid && *pp != 0 && t == &tty && *pp == pid)
        .map(|(_, _, _, comm)| {
            Path::new(&comm)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(comm)
        })
        .collect()
}

/// The `live-resize-sigwinch-delay-ms` policy.
///
/// Pure on purpose: a drag is a stream of size reports, and the gate decides
/// which of them deserve a signal.
#[derive(Debug, Clone)]
pub struct ResizeGate {
    delay: Duration,
    last_change: Option<Instant>,
    pending: bool,
    fired_for: Option<(u16, u16)>,
}

impl ResizeGate {
    pub fn new(delay: Duration) -> Self {
        Self { delay, last_change: None, pending: false, fired_for: None }
    }

    /// Report a new size. Returns `true` when the pty should be resized now.
    ///
    /// A size that has not changed is never re-signalled. A size that just
    /// changed is recorded; the signal fires either when the size has held still
    /// for `delay`, or — immediately — when `delay` is zero.
    pub fn report(&mut self, cols: u16, rows: u16, now: Instant, settled: bool) -> bool {
        if self.pending && self.last_change.is_some_and(|t| now.duration_since(t) >= self.delay) {
            self.pending = false;
            self.fired_for = Some((cols, rows));
            return true;
        }
        if settled {
            // The drag ended: fire once for the final size.
            self.pending = false;
            if self.fired_for == Some((cols, rows)) {
                return false;
            }
            self.fired_for = Some((cols, rows));
            return true;
        }
        self.last_change = Some(now);
        if self.delay.is_zero() {
            if self.fired_for == Some((cols, rows)) {
                return false;
            }
            self.fired_for = Some((cols, rows));
            return true;
        }
        self.pending = true;
        false
    }

    /// Whether a signal is still owed.
    pub fn is_pending(&self) -> bool {
        self.pending
    }
}

/// The per-session output log, capped at `session-log-size-mb`.
#[derive(Debug)]
pub struct SessionLog {
    pub path: PathBuf,
    pub mode: LogMode,
    max_bytes: u64,
    written: u64,
}

impl SessionLog {
    pub fn new(config: LogConfig) -> Self {
        let path = otty_core::app_support_dir()
            .join("logs")
            .join(format!("session-{}.log", std::process::id()));
        Self { path, mode: config.mode, max_bytes: config.max_bytes, written: 0 }
    }

    /// Append, redacting first when the mode asks for it.
    pub fn append(&mut self, bytes: &[u8]) {
        if self.written >= self.max_bytes {
            return;
        }
        let payload = match self.mode {
            LogMode::Plain => bytes.to_vec(),
            LogMode::Redacted => redact(bytes),
        };
        let budget = (self.max_bytes - self.written) as usize;
        let slice = &payload[..payload.len().min(budget)];
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            if f.write_all(slice).is_ok() {
                self.written += slice.len() as u64;
            }
        }
    }
}

/// Replace anything that looks like a credential with asterisks of the same
/// width, so redaction never shifts a column.
pub fn redact(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(text.len());
    for token in split_keeping_whitespace(&text) {
        if looks_secret(token) {
            out.extend(std::iter::repeat('*').take(token.chars().count()));
        } else {
            out.push_str(token);
        }
    }
    out.into_bytes()
}

fn split_keeping_whitespace(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = s.starts_with(char::is_whitespace);
    for (i, c) in s.char_indices() {
        let ws = c.is_whitespace();
        if ws != in_space {
            out.push(&s[start..i]);
            start = i;
            in_space = ws;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

fn looks_secret(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '=');
    if t.len() < 8 {
        return false;
    }
    if t.starts_with("sk-") || t.starts_with("ghp_") || t.starts_with("gho_") || t.starts_with("ghs_")
        || t.starts_with("AKIA") || t.starts_with("xoxb-") || t.starts_with("eyJ")
    {
        return true;
    }
    // `KEY=value` where the name says it is a credential.
    if let Some((k, v)) = t.split_once('=') {
        let upper = k.to_ascii_uppercase();
        let named = ["TOKEN", "SECRET", "PASSWORD", "PASSWD", "API_KEY", "APIKEY", "PRIVATE_KEY",
                     "ACCESS_KEY", "AUTH", "CREDENTIAL"]
            .iter()
            .any(|n| upper.contains(n));
        return named && v.len() >= 6;
    }
    // A bare long high-entropy run.
    t.len() >= 32 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Resolve `working-directory` the way its three spellings ask.
pub fn resolve_working_directory(
    value: &str,
    inherited: Option<&Path>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    match value.trim() {
        "" | "inherit" => inherited.map(Path::to_path_buf),
        "home" => home.map(Path::to_path_buf),
        other => {
            let expanded = expand_path(other);
            let p = PathBuf::from(expanded);
            if p.is_absolute() {
                Some(p)
            } else {
                inherited.map(|i| i.join(&p))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_mode_parses_the_two_documented_values() {
        assert_eq!(LogMode::parse("plain"), Some(LogMode::Plain));
        assert_eq!(LogMode::parse("Redacted"), Some(LogMode::Redacted));
        assert_eq!(LogMode::parse("off"), None);
    }

    #[test]
    fn log_config_converts_megabytes_to_bytes() {
        assert_eq!(LogConfig::from_mb(LogMode::Plain, 5).max_bytes, 5 * 1024 * 1024);
    }

    #[test]
    fn redaction_keeps_the_column_width() {
        let line = b"export TOKEN=abcdefghijklmnop done\n";
        let out = redact(line);
        assert_eq!(out.len(), line.len(), "redaction must not shift columns");
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("********"), "{text}");
        assert!(text.ends_with(" done\n"));
    }

    #[test]
    fn redaction_leaves_ordinary_text_alone() {
        let line = b"cargo build --release --workspace\n";
        assert_eq!(redact(line), line);
    }

    #[test]
    fn redaction_catches_the_common_credential_shapes() {
        for s in [
            "sk-abcdefghijklmnopqrstuvwxyz",
            "ghp_0123456789abcdefghij",
            "API_KEY=0123456789abcdef",
            "MY_SECRET=something-long",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            assert!(looks_secret(s), "{s} should look like a secret");
        }
        for s in ["--workspace", "hello", "TOKEN=s", "short"] {
            assert!(!looks_secret(s), "{s} should not look like a secret");
        }
    }

    #[test]
    fn the_resize_gate_coalesces_a_drag() {
        let mut gate = ResizeGate::new(Duration::from_millis(50));
        let t0 = Instant::now();
        // Three frames of a drag: none of them signals immediately.
        assert!(!gate.report(80, 24, t0, false));
        assert!(!gate.report(90, 24, t0 + Duration::from_millis(10), false));
        assert!(!gate.report(100, 24, t0 + Duration::from_millis(20), false));
        assert!(gate.is_pending());
        // The size then holds still long enough.
        assert!(gate.report(100, 24, t0 + Duration::from_millis(200), false));
        assert!(!gate.is_pending());
    }

    #[test]
    fn the_resize_gate_always_signals_at_the_end_of_a_drag() {
        let mut gate = ResizeGate::new(Duration::from_millis(50));
        let t0 = Instant::now();
        assert!(!gate.report(120, 30, t0, false));
        assert!(gate.report(120, 30, t0 + Duration::from_millis(5), true));
        // …but not twice for the same size.
        assert!(!gate.report(120, 30, t0 + Duration::from_millis(6), true));
    }

    #[test]
    fn a_zero_delay_signals_on_every_change() {
        let mut gate = ResizeGate::new(Duration::ZERO);
        let t0 = Instant::now();
        assert!(gate.report(80, 24, t0, false));
        assert!(!gate.report(80, 24, t0, false), "an unchanged size never re-signals");
        assert!(gate.report(81, 24, t0, false));
    }

    #[test]
    fn working_directory_honours_all_three_spellings() {
        let home = Path::new("/home/u");
        let inherited = Path::new("/work/otty");
        assert_eq!(
            resolve_working_directory("inherit", Some(inherited), Some(home)),
            Some(inherited.to_path_buf())
        );
        assert_eq!(
            resolve_working_directory("home", Some(inherited), Some(home)),
            Some(home.to_path_buf())
        );
        assert_eq!(
            resolve_working_directory("/tmp", Some(inherited), Some(home)),
            Some(PathBuf::from("/tmp"))
        );
        assert_eq!(
            resolve_working_directory("sub/dir", Some(inherited), Some(home)),
            Some(PathBuf::from("/work/otty/sub/dir"))
        );
        assert_eq!(resolve_working_directory("inherit", None, Some(home)), None);
    }

    #[test]
    fn spawning_a_shell_reports_a_missing_working_directory() {
        let cfg = PtyConfig {
            program: "/bin/true".into(),
            cwd: Some(PathBuf::from("/definitely/not/here")),
            ..Default::default()
        };
        assert!(matches!(Pty::spawn(&cfg), Err(PtyError::NoSuchDirectory(_))));
    }

    #[test]
    fn the_default_config_matches_the_documented_defaults() {
        let cfg = PtyConfig::default();
        assert_eq!((cfg.cols, cfg.rows), (80, 24));
        assert!(!cfg.login_greeting);
        assert!(cfg.shell_integration);
        assert_eq!(cfg.sigwinch_delay, Duration::from_millis(50));
        assert!(cfg.term.starts_with("xterm"));
    }
}
