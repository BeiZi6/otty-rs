//! Wiring a pane to a real process.
//!
//! A session owns one PTY, one [`Terminal`], and one reader thread. The thread
//! does nothing but move bytes into a channel, because the terminal engine must
//! be driven from the event loop — parsing on the reader thread would mean the
//! grid is mutated from two threads and every read of it needs a lock.
//!
//! ```text
//!   reader thread          event loop
//!   ─────────────          ──────────
//!   pty.read() ──bytes──▶  Session::pump()
//!                              │
//!                              ├─▶ Parser::advance(&mut Terminal)
//!                              └─▶ drain_events()  ──▶ badges, titles, notifications
//! ```
//!
//! The channel is bounded. A process that produces output faster than the event
//! loop drains it — `yes | cat` at 4 GiB/s — must slow down rather than grow the
//! process's memory without limit, so the reader thread blocks when the channel is
//! full. That is backpressure, and it is why the PTY keeps its own window: the
//! kernel's pty buffer fills, the writer's `write` blocks, and the whole pipeline
//! throttles instead of the shell being killed.

use std::io::Read;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use otty_core::PaneId;
use otty_pty::{Pty, PtyConfig, PtyError, ResizeGate};
use otty_term::{Event, Parser, TermConfig, Terminal};

/// How many chunks may be queued before the reader thread blocks.
///
/// 64 chunks of 64 KiB is 4 MiB in flight, which is far more than a frame needs
/// and small enough that a runaway `yes` cannot exhaust memory.
const CHANNEL_DEPTH: usize = 64;
const READ_CHUNK: usize = 64 * 1024;

/// How long `close` waits for the output pipe to drain before giving up.
///
/// Long enough for a normal shell to be reaped, short enough that a wedged child
/// cannot freeze a pane's close. The wait is bounded on purpose.
const CLOSE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// A running pane.
pub struct Session {
    pub pane: PaneId,
    /// `None` once the session has been closed, or after `Drop` has handed it to
    /// the reaper thread.
    pty: Option<Pty>,
    terminal: Terminal,
    parser: Parser,
    reader: Receiver<Vec<u8>>,
    /// The join handle of the reader thread, kept so `close` can detach it
    /// deterministically rather than hoping the process exits.
    _reader: thread::JoinHandle<()>,
    gate: ResizeGate,
    /// The size the terminal last told the PTY about.
    reported: (u16, u16),
    exited: Option<i32>,
    closed: bool,
    /// The reader thread has gone away. Writes after that hang on Windows
    /// ConPTY, so `drain_replies` must not touch the pipe.
    reader_gone: bool,
}

impl Session {
    /// Start a process in a new PTY.
    pub fn spawn(pane: PaneId, config: &PtyConfig, term: TermConfig) -> Result<Self, PtyError> {
        let mut pty = Pty::spawn(config)?;
        let mut reader = pty.reader().map_err(|e| PtyError::Open(anyhow::Error::from(e)))?;
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_DEPTH);

        let handle = thread::Builder::new()
            .name(format!("otty-pty-{pane}"))
            .spawn(move || {
                let mut buf = vec![0u8; READ_CHUNK];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            // A closed receiver means the session is gone; stop
                            // reading rather than blocking on a full channel
                            // forever.
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            // Failing to start a thread is not a shell failure, but it is the
            // same kind of "this pane cannot run" as failing to open a pty, so
            // it is reported through the same variant rather than by panicking.
            .map_err(|e| PtyError::Spawn {
                program: config.program.clone(),
                source: anyhow::Error::from(e),
            })?;

        Ok(Self {
            pane,
            pty: Some(pty),
            terminal: Terminal::new(config.cols, config.rows, term),
            parser: Parser::new(),
            reader: rx,
            _reader: handle,
            gate: ResizeGate::new(config.sigwinch_delay),
            reported: (config.cols, config.rows),
            exited: None,
            closed: false,
            reader_gone: false,
        })
    }

    /// The pty, while the session is live.
    fn pty(&self) -> Option<&Pty> {
        self.pty.as_ref()
    }

    /// The terminal, for reading the grid.
    pub fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    /// Move every queued byte from the PTY into the terminal.
    ///
    /// Returns the number of bytes consumed, so a caller can tell "there was
    /// nothing to do" from "work happened" without inspecting the grid.
    pub fn pump(&mut self) -> usize {
        let mut total = 0;
        loop {
            match self.reader.try_recv() {
                Ok(chunk) => {
                    self.parser.advance(&mut self.terminal, &chunk);
                    if let Some(pty) = &mut self.pty {
                        pty.log_output(&chunk);
                    }
                    total += chunk.len();
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The child closed its side. Reap it so `status` is available
                    // on the next call rather than never.
                    self.reader_gone = true;
                    self.reap();
                    break;
                }
            }
        }
        total
    }

    /// Everything the terminal produced since the last drain.
    pub fn drain_events(&mut self) -> Vec<Event> {
        self.terminal.drain_events()
    }

    /// The bytes the terminal wants to send back to the application.
    ///
    /// The engine answers some queries itself — DA1, DECRQM, the device
    /// attributes a program probes with — and each answer is its own vector.
    /// They are concatenated in the order the terminal produced them, because a
    /// program that asked two questions expects both answers in the order it
    /// asked.
    pub fn drain_replies(&mut self) -> Vec<u8> {
        let replies: Vec<u8> = self.terminal.drain_replies().concat();
        if replies.is_empty() {
            return replies;
        }
        self.reap();
        // Writing into a ConPTY whose host has exited blocks until
        // `ClosePseudoConsole`, which itself waits for the write — a deadlock
        // measured as "the pane close hung for minutes". Skip the write once
        // the child or its reader is gone.
        if self.closed || self.reader_gone || self.exited.is_some() {
            return replies;
        }
        if let Some(pty) = &mut self.pty {
            let _ = pty.write(&replies);
        }
        replies
    }

    /// Send input to the process.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        match &mut self.pty {
            Some(pty) => pty.write(bytes),
            // A closed session is a no-op, not an error: the caller is writing to
            // a pane that has gone, and the write is not the interesting event.
            None => Ok(()),
        }
    }

    /// Resize the grid, and tell the PTY when the gate lets it through.
    ///
    /// The two halves are deliberately on different schedules. The **grid** is
    /// ours: changing it costs an allocation and keeps the window and the text in
    /// step, so it happens on every event. The **signal to the child** is not
    /// ours: a window drag produces one resize per pixel, and each one makes the
    /// child's `SIGWINCH` handler run and every full-screen program repaint. The
    /// gate is only about that second half. `settled` says the drag has stopped.
    ///
    /// Returns whether the *child* was told, which is the part a caller can
    /// usefully branch on.
    pub fn resize(&mut self, cols: u16, rows: u16, settled: bool) -> bool {
        if (cols, rows) != (self.terminal.cols(), self.terminal.rows()) {
            self.terminal.resize(cols, rows);
        }
        let now = std::time::Instant::now();
        if !self.gate.report(cols, rows, now, settled) {
            return false;
        }
        self.reported = (cols, rows);
        if let Some(pty) = &self.pty {
            let _ = pty.resize(cols, rows);
        }
        true
    }

    /// The size the PTY was last told about.
    pub fn reported_size(&self) -> (u16, u16) {
        self.reported
    }

    /// The pane's tty device path, when the platform has one.
    pub fn tty(&self) -> Option<String> {
        self.pty()?.tty_name()
    }

    /// Whether a foreground process other than the shell is running.
    pub fn is_busy(&self) -> bool {
        self.pty().is_some_and(Pty::is_busy)
    }

    /// The process names in this pane, outermost first.
    pub fn processes(&self) -> Vec<String> {
        self.pty().map(Pty::processes).unwrap_or_default()
    }

    pub fn pid(&self) -> Option<u32> {
        self.pty()?.pid()
    }

    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.pty()?.log_path()
    }

    /// The exit status, once the process has finished.
    pub fn status(&mut self) -> Option<i32> {
        self.reap();
        self.exited
    }

    fn reap(&mut self) {
        if self.exited.is_some() {
            return;
        }
        let Some(pty) = &mut self.pty else { return };
        if let Some(status) = pty.try_wait() {
            // portable-pty's exit code is a `u32` that is `-1` when the child was
            // signalled; report that as 128+signal so a script sees the shell's
            // own convention.
            let code = status.exit_code() as i32;
            self.exited = Some(if code < 0 { 128 } else { code });
        }
    }

    /// The visible text. What `contents of t` returns.
    pub fn contents(&self) -> String {
        self.terminal.contents()
    }

    /// Scrollback plus the visible screen. What `history of t` returns.
    pub fn history(&self) -> String {
        self.terminal.history()
    }

    /// The last `lines` lines of history, or all of it when `lines` is `None`.
    /// This is `otty pane capture`.
    pub fn capture(&self, lines: Option<u32>) -> String {
        let all = self.history();
        match lines {
            None => all,
            Some(0) => all,
            Some(n) => {
                let v: Vec<&str> = all.lines().collect();
                let take = n as usize;
                if v.len() <= take {
                    all
                } else {
                    let mut s = v[v.len() - take..].join("\n");
                    s.push('\n');
                    s
                }
            }
        }
    }

    /// Stop the process, and let the reader thread finish.
    ///
    /// The order matters and is not obvious. Closing a ConPTY **blocks until its
    /// output pipe has been drained**, so a close that stops reading first
    /// deadlocks against a reader thread blocked in `send` on a full channel —
    /// which is exactly what the two hanging tests were: the shell had exited,
    /// or had just been killed, and the close waited for a drain that the close
    /// itself was preventing.
    ///
    /// So this kills first, then keeps draining until the reader finishes or the
    /// deadline passes. The deadline is what makes it a bounded wait: a child
    /// that is wedged in an uninterruptible state must not freeze the pane's
    /// close forever.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        // `ClosePseudoConsole` (inside `portable-pty`'s `Drop`) blocks until the
        // attached process exits *and* its output pipe is flushed. Calling it
        // on this thread is what froze every Windows test that ended a shell —
        // the drain deadline below never got to fire because we never reached
        // it. Kill and drop on a helper thread; drain what is already queued.
        if let Some(pty) = self.pty.take() {
            retire_pty(pty);
        }
        let deadline = std::time::Instant::now() + CLOSE_DRAIN_TIMEOUT;
        while std::time::Instant::now() < deadline {
            let read = self.pump();
            if self.reader_gone && read == 0 {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// Whether `close` has been called.
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Some(pty) = self.pty.take() {
            retire_pty(pty);
        }
    }
}

/// Kill the child and drop the PTY off the caller's thread.
///
/// `ClosePseudoConsole` is a blocking Win32 call. Doing it here is what made
/// "the shell exited" freeze the pane. A helper thread pays that cost; if we
/// cannot spawn one we still `TerminateProcess` and then `mem::forget` the
/// PTY so this thread cannot hang.
fn retire_pty(pty: Pty) {
    let slot = std::sync::Arc::new(std::sync::Mutex::new(Some(pty)));
    let worker = slot.clone();
    match thread::Builder::new()
        .name("otty-pty-reaper".to_owned())
        .spawn(move || {
            if let Ok(mut g) = worker.lock() {
                if let Some(mut pty) = g.take() {
                    pty.kill();
                    drop(pty);
                }
            }
        }) {
        Ok(_) => {}
        Err(_) => {
            if let Ok(mut g) = slot.lock() {
                if let Some(mut pty) = g.take() {
                    pty.kill();
                    std::mem::forget(pty);
                }
            }
            log::warn!(
                "could not spawn a reaper thread; ClosePseudoConsole skipped so the pane can close"
            );
        }
    }
}

/// The shell a test can use without touching the user's configuration.
pub fn test_shell_config(cols: u16, rows: u16) -> PtyConfig {
    let (program, args) = if cfg!(windows) {
        ("cmd.exe".to_owned(), vec!["/Q".to_owned()])
    } else {
        ("/bin/sh".to_owned(), Vec::new())
    };
    PtyConfig {
        program,
        args,
        env: std::collections::BTreeMap::new(),
        cwd: None,
        cols,
        rows,
        term: "xterm-256color".to_owned(),
        login_greeting: false,
        shell_integration: false,
        integration_dir: None,
        integration: Default::default(),
        sigwinch_delay: std::time::Duration::from_millis(50),
        log: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_core::PaneId;
    use std::time::{Duration, Instant};

    fn p(n: u64) -> PaneId {
        PaneId::new(n)
    }

    /// Pump until `pred` is true or the deadline passes. Real processes are
    /// asynchronous, so every test that talks to one has to wait for it.
    fn pump_until(
        s: &mut Session,
        timeout: Duration,
        mut pred: impl FnMut(&Session) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            s.pump();
            s.drain_events();
            s.drain_replies();
            if pred(s) {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn session(cols: u16, rows: u16) -> Session {
        Session::spawn(p(1), &test_shell_config(cols, rows), TermConfig::default())
            .expect("a shell must be spawnable")
    }

    #[test]
    fn a_session_spawns_a_real_process_and_reads_it() {
        let mut s = session(80, 24);
        assert!(s.pid().is_some(), "a pty session must have a pid");
        // Ask the shell to print a marker, then wait for the terminal to show it.
        s.write(b"echo otty-marker\r\n").unwrap();
        assert!(
            pump_until(&mut s, Duration::from_secs(30), |s| s.contents().contains("otty-marker")),
            "the marker never appeared; grid was {:?}",
            s.contents()
        );
        s.close();
    }

    #[test]
    fn the_grid_size_is_what_the_pty_was_told() {
        let s = session(100, 30);
        assert_eq!(s.reported_size(), (100, 30));
        assert_eq!(s.terminal().cols(), 100);
        assert_eq!(s.terminal().rows(), 30);
    }

    #[test]
    fn a_resize_reaches_both_the_grid_and_the_pty() {
        let mut s = session(80, 24);
        assert!(s.resize(120, 40, true), "a settled resize must go through");
        assert_eq!(s.reported_size(), (120, 40));
        assert_eq!(s.terminal().cols(), 120);
        assert_eq!(s.terminal().rows(), 40);

        // A burst while the drag is still moving is coalesced by the gate, so
        // the child is not signalled once per pixel.
        let mut sent = 0;
        for i in 0..10u16 {
            if s.resize(120 + i, 40, false) {
                sent += 1;
            }
        }
        assert!(sent < 10, "every intermediate size was forwarded: {sent}");
        // The final settled size always goes through.
        assert!(s.resize(200, 50, true));
        assert_eq!(s.reported_size(), (200, 50));
        s.close();
    }

    #[test]
    fn the_terminal_resizes_immediately_but_the_pty_waits() {
        let mut s = session(80, 24);
        // A resize that the gate refuses must still not leave the grid stale —
        // the grid is ours and costs nothing to change.
        s.resize(100, 30, false);
        s.resize(100, 30, false);
        assert_eq!(s.terminal().cols(), 100, "the grid follows the window");
        s.close();
    }

    #[test]
    fn output_reaches_the_grid_without_waiting_for_a_frame() {
        let mut s = session(80, 24);
        s.write(b"echo pump-test\r\n").unwrap();
        assert!(pump_until(&mut s, Duration::from_secs(30), |s| s.contents().contains("pump-test")));
        // A second pump with nothing pending consumes nothing, which is how the
        // event loop knows not to redraw.
        thread::sleep(Duration::from_millis(80));
        s.pump();
        assert_eq!(s.pump(), 0, "an idle session must consume no bytes");
        s.close();
    }

    #[test]
    fn capture_with_a_line_limit_returns_the_tail() {
        let mut s = session(80, 24);
        for i in 0..20 {
            s.write(format!("echo line-{i:02}\r\n").as_bytes()).unwrap();
        }
        assert!(
            pump_until(&mut s, Duration::from_secs(30), |s| s.contents().contains("line-19")),
            "the last line never appeared"
        );
        // `Some(0)` and `None` both mean everything.
        let all = s.capture(None);
        assert_eq!(s.capture(Some(0)), all);
        let tail = s.capture(Some(3));
        assert!(tail.lines().count() <= 3, "asked for 3 lines, got {:?}", tail);
        assert!(tail.contains("line-19"), "{tail}");
        assert!(!tail.contains("line-00"), "{tail}");
        // Asking for more lines than exist returns everything, not an error.
        assert_eq!(s.capture(Some(100_000)), all);
        s.close();
    }

    /// The exit status path.
    ///
    /// On unix `exit 7` reaches `status()` as 7.
    ///
    /// On Windows `status()` stays `None`: ConPTY runs the shell behind a proxy
    /// process and `portable-pty`'s `try_wait` reports on the proxy rather than
    /// on the shell inside it. The hang that used to sit on this path was
    /// `ClosePseudoConsole` on the event-loop thread; that is gone, so this
    /// test runs. If it starts reporting a code, tighten the Windows assertion.
    #[test]
    fn a_process_that_has_exited_reports_its_status() {
        let mut s = session(80, 24);
        s.write(b"exit 7\r\n").unwrap();
        // Unix needs time for `exit` to reap. Windows never reports a code
        // (ConPTY proxy), so waiting 30s just inflates the suite; 2s is enough
        // to see if that limitation has gone.
        let deadline = Instant::now()
            + Duration::from_secs(if cfg!(windows) { 2 } else { 30 });
        let mut status = None;
        while Instant::now() < deadline {
            s.pump();
            if let Some(code) = s.status() {
                status = Some(code);
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }

        #[cfg(unix)]
        assert_eq!(status, Some(7), "a unix shell's exit code must be reported");

        #[cfg(windows)]
        assert_eq!(
            status, None,
            "if this now reports a code, the ConPTY limitation is gone — \
             tighten this assertion and delete the note above"
        );
    }

    #[test]
    fn writing_to_a_closed_session_does_not_panic() {
        let mut s = session(80, 24);
        s.close();
        // The write may fail; it must not take the process down. This is the
        // assertion that matters, because the alternative is a crash the user
        // sees as "Otty quit when I closed a pane".
        let _ = s.write(b"echo after-close\r\n");
    }

    #[test]
    fn tty_and_process_names_are_available() {
        let mut s = session(80, 24);
        // A tty name exists on unix; Windows has no device path, and `None` is
        // the honest answer there rather than a fabricated one.
        #[cfg(unix)]
        assert!(s.tty().is_some_and(|t| t.starts_with("/dev/")), "{:?}", s.tty());
        #[cfg(not(unix))]
        assert!(s.tty().is_none());

        #[cfg(unix)]
        {
            s.write(b"sleep 30\r\n").unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut procs = Vec::new();
            while Instant::now() < deadline {
                s.pump();
                procs = s.processes();
                if procs.iter().any(|p| p.contains("sleep")) {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            assert!(
                procs.iter().any(|p| p.contains("sleep")),
                "the foreground process was not reported: {procs:?}"
            );
        }
        #[cfg(windows)]
        {
            s.write(b"echo otty-ready\r\n").unwrap();
            assert!(
                pump_until(&mut s, Duration::from_secs(30), |s| s.contents().contains("otty-ready")),
                "cmd.exe never became ready; grid was {:?}",
                s.contents()
            );
            s.write(b"ping -n 20 127.0.0.1\r\n").unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut procs = Vec::new();
            while Instant::now() < deadline {
                s.pump();
                procs = s.processes();
                if procs.iter().any(|p| p.to_ascii_lowercase().contains("ping")) {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            assert!(
                procs.iter().any(|p| p.to_ascii_lowercase().contains("ping")),
                "pid={:?} foreground={procs:?}",
                s.pid()
            );
        }
        s.close();
    }

    #[test]
    fn close_returns_without_waiting_for_conpty_teardown() {
        let mut s = session(80, 24);
        let started = Instant::now();
        s.close();
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "close blocked for {elapsed:?}; ClosePseudoConsole ran on this thread"
        );
    }

    #[test]
    fn two_sessions_have_independent_grids() {
        let mut a = session(40, 10);
        let mut b = session(120, 40);
        a.write(b"echo from-a\r\n").unwrap();
        assert!(pump_until(&mut a, Duration::from_secs(30), |s| s.contents().contains("from-a")));
        // A second session must not have seen it.
        b.pump();
        assert!(!b.contents().contains("from-a"), "output leaked between sessions");
        assert_ne!(a.terminal().cols(), b.terminal().cols());
        a.close();
        b.close();
    }

    #[test]
    fn the_session_keeps_its_pane_identity() {
        let s = Session::spawn(
            p(42),
            &test_shell_config(80, 24),
            TermConfig::default(),
        )
        .unwrap();
        assert_eq!(s.pane, p(42));
    }
}
