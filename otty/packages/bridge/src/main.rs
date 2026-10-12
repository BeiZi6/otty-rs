//! `otty-bridge`: one Electron window's backend, speaking the JSON lines of
//! `frontend/electron/PROTOCOL.md` over stdio.
//!
//! This file is the part that cannot be tested without a shell: the pipes, the
//! two listener threads and the live sessions. What a request means is in
//! [`backend`].

mod backend;

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use backend::{Backend, Panes};
use base64::Engine;
use otty_config::theme::Theme;
use otty_config::Config;
use otty_core::PaneId;
use otty_ui::ipc::PaneIo;
use otty_ui::TerminalView;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Command {
    Input { pane: Option<String>, text: String },
    Paste { pane: Option<String>, text: String },
    Resize { pane: String, width: u32, height: u32 },
    Request {
        id: Value,
        method: String,
        #[serde(default)]
        params: Value,
    },
    Close,
}

/// What the main loop wakes for.
enum Msg {
    Line(Command),
    /// A CLI request from the control socket, and where its answer goes.
    Ipc(otty_ipc::Request, Sender<otty_ipc::Response>),
}

/// The model's terminal panes, live: one view — PTY, engine, pixels — each.
struct Views {
    map: HashMap<PaneId, TerminalView>,
    scale: f32,
    /// The last size the frontend gave any pane. A new pane starts there and
    /// is resized as soon as the frontend has laid it out.
    size: (u32, u32),
    /// Panes whose pixels changed since their last frame.
    owed: HashSet<PaneId>,
    /// Panes whose shell has not printed anything yet, with the input held for
    /// them: a shell still starting up flushes its tty and eats what was typed.
    fresh: HashMap<PaneId, (Instant, Vec<(bool, String)>)>,
}

impl Views {
    /// The pane a command names, or the focused one when it names none.
    fn target(&mut self, pane: Option<&str>, focused: Option<PaneId>) -> Option<(PaneId, &mut TerminalView)> {
        let id = match pane {
            Some(slug) => slug.parse().ok(),
            None => focused,
        }?;
        self.map.get_mut(&id).map(|view| (id, view))
    }

    /// Keys (`paste = false`) or a paste for a pane, held while it is fresh.
    fn feed(&mut self, pane: Option<&str>, focused: Option<PaneId>, paste: bool, text: String) {
        let Some(id) = self.target(pane, focused).map(|(id, _)| id) else { return };
        if let Some((_, held)) = self.fresh.get_mut(&id) {
            held.push((paste, text));
        } else if let Some(view) = self.map.get_mut(&id) {
            report(if paste { view.paste(&text) } else { view.handle_key(&text) });
        }
    }

    /// Release a pane's held input once its shell has drawn something.
    // ponytail: "drew something" is the readiness signal, with a 5 s cap; a
    // shell-integration prompt mark (OSC 133 A) is the exact one.
    fn release(&mut self, id: PaneId) {
        let Some(view) = self.map.get_mut(&id) else { return };
        let Some((since, _)) = self.fresh.get(&id) else { return };
        let drew = (0..view.grid_size().1).any(|row| !view.line_text(row).trim().is_empty());
        if !drew && since.elapsed() < Duration::from_secs(5) {
            return;
        }
        for (paste, text) in self.fresh.remove(&id).map(|(_, held)| held).unwrap_or_default() {
            report(if paste { view.paste(&text) } else { view.handle_key(&text) });
        }
    }
}

impl PaneIo for Views {
    fn write(&mut self, pane: PaneId, bytes: &[u8]) -> bool {
        self.map.get_mut(&pane).is_some_and(|v| v.write(bytes).is_ok())
    }
    fn screen(&self, pane: PaneId) -> Option<Vec<String>> {
        self.map.get(&pane).and_then(|v| PaneIo::screen(v, pane))
    }
    fn dimensions(&self, pane: PaneId) -> Option<(u16, u16)> {
        self.map.get(&pane).map(|v| v.grid_size())
    }
    fn busy(&self, pane: PaneId) -> Option<bool> {
        self.map.get(&pane).map(|v| v.is_busy())
    }
    fn processes(&self, pane: PaneId) -> Vec<String> {
        self.map.get(&pane).map(|v| v.processes()).unwrap_or_default()
    }
    fn tty(&self, pane: PaneId) -> Option<String> {
        self.map.get(&pane).and_then(|v| v.tty())
    }
}

impl Panes for Views {
    fn sync(&mut self, panes: &[(PaneId, Option<String>)], config: &Config) -> Vec<(PaneId, String)> {
        // Dropping a view retires its PTY on a helper thread; `close` would
        // wait for the drain on this one.
        self.map.retain(|id, _| panes.iter().any(|(pane, _)| pane == id));
        self.fresh.retain(|id, _| self.map.contains_key(id));
        let mut failed = Vec::new();
        for (id, cwd) in panes {
            if self.map.contains_key(id) {
                continue;
            }
            let cwd = cwd.as_deref().map(Path::new);
            match TerminalView::new_in(config, *id, self.size, self.scale, cwd) {
                Ok(view) => {
                    self.map.insert(*id, view);
                    self.owed.insert(*id);
                    self.fresh.insert(*id, (Instant::now(), Vec::new()));
                }
                Err(e) => failed.push((*id, e.to_string())),
            }
        }
        failed
    }

    fn selected_text(&self, pane: PaneId, from: (u16, u16), to: (u16, u16)) -> Option<String> {
        self.map.get(&pane).map(|v| v.selected_text(from, to))
    }

    fn history(&self, pane: PaneId) -> Option<Vec<String>> {
        self.map.get(&pane).map(|v| v.history_lines())
    }

    fn set_theme(&mut self, theme: &Theme) {
        for (id, view) in &mut self.map {
            view.set_theme(theme);
            self.owed.insert(*id);
        }
    }

    fn set_font(&mut self, config: &Config) {
        for (id, view) in &mut self.map {
            if let Err(e) = view.apply_font_size(config, self.scale) {
                eprintln!("otty-bridge: {id}: {e}");
            }
            self.owed.insert(*id);
        }
    }
}

fn main() {
    let scale = std::env::var("OTTY_SCALE_FACTOR")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);
    let config = Config::load();
    let gates = otty_ipc::Gates {
        allow_send_keys: config.get_bool("ipc-allow-send-keys").unwrap_or(false),
        allow_sensitive_sessions: config.get_bool("ipc-allow-sensitive-sessions").unwrap_or(false),
    };
    let cwd = std::env::var("OTTY_CWD")
        .ok()
        .or_else(|| std::env::current_dir().ok().map(|dir| dir.to_string_lossy().into_owned()));
    let mut backend = Backend::new(config, cwd);
    backend.store = otty_state::Store::open_default().ok();
    let mut views = Views { map: HashMap::new(), scale, size: (1200, 700), owed: HashSet::new(), fresh: HashMap::new() };

    let inbox = listen();
    emit(&backend.ready());

    let mut last_state = String::new();
    let mut shown = Vec::new();
    let mut last_blink = false;
    let mut icons_at: Option<Instant> = None;
    'window: loop {
        // ponytail: a 16 ms poll, so an idle window still wakes 60 times a
        // second to pump its PTYs. Have the reader threads wake this channel
        // when idle CPU shows up in a profile.
        let first = inbox.recv_timeout(Duration::from_millis(16)).ok();
        for msg in first.into_iter().chain(inbox.try_iter()) {
            let focused = backend.d.focused_pane_id();
            match msg {
                Msg::Line(Command::Input { pane, text }) => {
                    // Hint and Vi mode keep their keys away from the shell.
                    if backend.d.handle_mode_key(&text) {
                        continue;
                    }
                    views.feed(pane.as_deref(), focused, false, text);
                }
                Msg::Line(Command::Paste { pane, text }) => {
                    views.feed(pane.as_deref(), focused, true, text);
                }
                Msg::Line(Command::Resize { pane, width, height }) => {
                    let size = (width.clamp(16, 8192), height.clamp(16, 8192));
                    if let Some((id, view)) = views.target(Some(&pane), None) {
                        view.resize(size.0, size.1, true);
                        views.owed.insert(id);
                        views.size = size;
                    }
                }
                Msg::Line(Command::Request { id, method, params }) => {
                    emit(&match backend.request(&mut views, &method, &params) {
                        Ok(result) => json!({"type": "response", "id": id, "ok": true, "result": result}),
                        Err(error) => json!({"type": "response", "id": id, "ok": false, "error": error}),
                    });
                }
                Msg::Line(Command::Close) => break 'window,
                Msg::Ipc(request, reply) => {
                    let _ = reply.send(otty_ui::ipc::handle(&mut backend.d, &mut views, gates, request));
                }
            }
        }

        // Every pane is pumped, shown or not: a hidden shell still has to drain.
        for (id, view) in &mut views.map {
            if view.pump() {
                views.owed.insert(*id);
            }
            for event in view.take_events() {
                backend.on_event(*id, event);
            }
            if view.exited() {
                backend.close_pane(*id);
            }
        }
        for id in views.fresh.keys().copied().collect::<Vec<_>>() {
            views.release(id);
        }
        if icons_at.is_none_or(|at| at.elapsed() >= Duration::from_millis(500)) {
            icons_at = Some(Instant::now());
            backend.refresh_icons(&views);
        }
        backend.settle(&mut views);
        for event in backend.out.drain(..) {
            emit(&event);
        }
        let state = backend.state(&views).to_string();
        if state != last_state {
            emit_line(&state);
            last_state = state;
        }

        // A frame goes out for a pane that changed, that just came into view,
        // or — all of them — when the cursor's blink phase toggled.
        let blink = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .is_ok_and(|time| (time.as_millis() / 500) % 2 == 0);
        let blinked = blink != last_blink;
        last_blink = blink;
        let visible = backend.visible();
        for id in &visible {
            let Some(view) = views.map.get_mut(id) else { continue };
            let owed = views.owed.remove(id);
            if owed || blinked || !shown.contains(id) {
                view.set_cursor_blink(blink);
                view.frame();
                let (width, height) = view.buffer_size();
                // Written by hand: base64 needs no escaping, and a `Value`
                // would copy megabytes of it once more.
                emit_line(&format!(
                    r#"{{"type":"frame","pane":"{id}","width":{width},"height":{height},"pixels":"{}"}}"#,
                    base64::engine::general_purpose::STANDARD.encode(view.pixels()),
                ));
            }
        }
        shown = visible;

        if backend.quit {
            emit(&json!({"type": "quit"}));
            break;
        }
    }
}

/// Start the two listeners — stdin and the CLI's control socket — and return
/// the one channel they both feed.
fn listen() -> Receiver<Msg> {
    let (sender, receiver) = mpsc::channel();

    let socket = otty_ipc::socket_path(std::env::var_os("OTTY_SOCKET").as_deref().map(Path::new));
    let ipc = sender.clone();
    // The listener thread only ferries: the views are not `Send`, so each
    // request is answered by the main loop.
    if let Err(e) = otty_ipc::serve(&socket, move |request| {
        let (reply, answer) = mpsc::channel();
        let _ = ipc.send(Msg::Ipc(request, reply));
        answer.recv_timeout(Duration::from_secs(2)).unwrap_or_else(|_| {
            otty_ipc::Response::error(otty_ipc::code::TIMEOUT, "Otty did not answer")
        })
    }) {
        // Usually another window's bridge owns it; this one still works.
        eprintln!("otty-bridge: ipc: not listening on {}: {e}", socket.display());
    }

    thread::Builder::new()
        .name("otty-bridge-stdin".to_owned())
        .spawn(move || {
            for line in io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                match serde_json::from_str::<Command>(&line) {
                    Ok(command) => {
                        let closing = matches!(command, Command::Close);
                        if sender.send(Msg::Line(command)).is_err() || closing {
                            return;
                        }
                    }
                    Err(error) => emit(&json!({"type": "error", "message": error.to_string()})),
                }
            }
            // The frontend went away; so does this window.
            let _ = sender.send(Msg::Line(Command::Close));
        })
        .expect("stdin reader thread must start");
    receiver
}

/// A write to a pane's shell that failed is the frontend's to show.
fn report<E: std::fmt::Display>(result: Result<(), E>) {
    if let Err(error) = result {
        emit(&json!({"type": "error", "message": error.to_string()}));
    }
}

fn emit(value: &Value) {
    emit_line(&value.to_string());
}

fn emit_line(line: &str) {
    let mut output = io::stdout().lock();
    let _ = writeln!(output, "{line}");
    let _ = output.flush();
}
