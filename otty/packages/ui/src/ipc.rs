//! Answers `otty-ipc` requests against the model.
//!
//! The shell owns the socket and the thread; this module is the part that
//! decides what a request *means*, so it is the part a test drives. Selector
//! misses are exit 4, gate refusals 13, and a request the shell cannot honour
//! yet is a usage error that says so rather than a silent success.

use std::fmt::Display;

use otty_core::{AgentState, PaneId, TabId, WindowId};
use otty_ipc::{
    check_privileges, code, BadgeKind, Gates, PaneInfo, PaneRequest, Request, Response, Selector,
    SplitDirection, TabRequest, WindowRequest,
};
use serde_json::{json, Value};

use crate::dispatch::Dispatcher;
use crate::model::{App, Badge};
use crate::view::TerminalView;

/// The live sessions behind the model's panes. A pane the shell has no session
/// for answers `None`/`false`, which the handler reports as "no live session".
pub trait PaneIo {
    fn write(&mut self, pane: PaneId, bytes: &[u8]) -> bool;
    /// The visible grid, one string per row.
    fn screen(&self, pane: PaneId) -> Option<Vec<String>>;
    /// `(columns, rows)`.
    fn dimensions(&self, pane: PaneId) -> Option<(u16, u16)>;
    fn busy(&self, pane: PaneId) -> Option<bool>;
    fn processes(&self, pane: PaneId) -> Vec<String>;
    fn tty(&self, pane: PaneId) -> Option<String>;
}

impl PaneIo for TerminalView {
    fn write(&mut self, pane: PaneId, bytes: &[u8]) -> bool {
        pane == self.pane_id() && TerminalView::write(self, bytes).is_ok()
    }

    fn screen(&self, pane: PaneId) -> Option<Vec<String>> {
        (pane == self.pane_id())
            .then(|| (0..self.grid_size().1).map(|r| self.line_text(r)).collect())
    }

    fn dimensions(&self, pane: PaneId) -> Option<(u16, u16)> {
        (pane == self.pane_id()).then(|| self.grid_size())
    }

    fn busy(&self, pane: PaneId) -> Option<bool> {
        (pane == self.pane_id()).then(|| self.is_busy())
    }

    fn processes(&self, pane: PaneId) -> Vec<String> {
        if pane == self.pane_id() {
            TerminalView::processes(self)
        } else {
            Vec::new()
        }
    }

    fn tty(&self, pane: PaneId) -> Option<String> {
        (pane == self.pane_id()).then(|| TerminalView::tty(self)).flatten()
    }
}

/// Answer `req` for the shell's one live view: copy what the view learned from
/// its shell (title, cwd) into the dispatcher's model first, so `list` is true.
pub fn handle_view(
    d: &mut Dispatcher,
    view: &mut TerminalView,
    gates: Gates,
    req: Request,
) -> Response {
    let id = view.pane_id();
    if let (Some(from), Some(to)) = (view.app().pane(id), d.app.pane_mut(id)) {
        to.cwd.clone_from(&from.cwd);
        to.shell_title.clone_from(&from.shell_title);
    }
    handle(d, view, gates, req)
}

/// Answer one request.
pub fn handle(d: &mut Dispatcher, io: &mut dyn PaneIo, gates: Gates, mut req: Request) -> Response {
    // Resolve "the focused pane" and index selectors to a concrete id before
    // the gates run, so an ssh pane is refused however it was named.
    let mut target = None;
    if let Request::Pane(p) = &mut req {
        if let Some(sel) = pane_selector(p) {
            let id = match sel {
                Some(s) => match pick(&d.app.all_panes(), s, "pane") {
                    Ok(id) => id,
                    Err(r) => return r,
                },
                None => match d.focused_pane_id() {
                    Some(id) => id,
                    None => return no_match("pane"),
                },
            };
            *sel = Some(Selector::Id(id.slug()));
            target = Some(id);
        }
    }
    let lookup = |id: PaneId| {
        let processes = io.processes(id);
        Some(PaneInfo {
            id,
            sensitive: processes
                .iter()
                .any(|n| matches!(n.trim_end_matches(".exe"), "ssh" | "sudo" | "su" | "doas")),
            foreground: processes.last().cloned(),
        })
    };
    if let Err(refusal) = check_privileges(&req, gates, lookup) {
        return refusal;
    }

    let answer = match req {
        Request::Status => {
            let (windows, tabs, _) = d.app.counts();
            Ok(Response::ok(json!({
                "version": env!("CARGO_PKG_VERSION"),
                "windows": windows,
                "tabs": tabs,
                "panes": d.app.all_panes().len(),
                "agents": d.app.agents,
            })))
        }
        Request::Window(w) => window(d, w),
        Request::Tab(t) => tab(d, t),
        Request::Pane(p) => pane(d, io, p, target),
        Request::Badge { tab, kind, .. } => set_badge(d, &tab, kind),
        Request::AgentState { agent, session_id, state, cwd, .. } => {
            d.app.agents.insert(session_id, state);
            // ponytail: matched by cwd, then focus; the original walks the
            // process tree up from `agent-pid` to the pane's shell.
            let pane = d
                .app
                .all_panes()
                .into_iter()
                .find(|id| {
                    cwd.is_some() && d.app.pane(*id).and_then(|p| p.cwd.as_deref()) == cwd.as_deref()
                })
                .or(d.focused_pane_id());
            if let Some(id) = pane {
                if let Some(p) = d.app.pane_mut(id) {
                    p.agent = Some((agent.as_str().to_owned(), state));
                }
                if let Some(t) = d.app.tab_mut_of(id) {
                    t.badge = Some(match state {
                        AgentState::Processing | AgentState::Awaiting => Badge::Running,
                        AgentState::Idle => Badge::Success,
                        AgentState::Error => Badge::Error,
                    });
                }
            }
            Ok(Response::empty())
        }
        Request::ReloadConfig | Request::Notify { .. } | Request::SetTheme { .. } => {
            Err(unsupported("this request"))
        }
    };
    answer.unwrap_or_else(|refusal| refusal)
}

type Answer = Result<Response, Response>;

fn no_match(what: &str) -> Response {
    Response::error(code::NO_MATCH, format!("No {what} matched selector"))
}

fn unsupported(what: &str) -> Response {
    Response::error(code::USAGE, format!("{what} is not supported by this shell yet"))
}

fn no_session() -> Response {
    Response::error(code::NO_MATCH, "pane has no live session")
}

/// New panes are model-only in this shell: there is no session to run it in.
fn no_command(command: &Option<String>) -> Result<(), Response> {
    match command {
        Some(_) => Err(unsupported("starting a command in a new pane")),
        None => Ok(()),
    }
}

/// Ids are matched by slug (`pane-3`) or by 1-based position in `ids`.
fn pick<T: Copy + Display>(ids: &[T], sel: &Selector, what: &str) -> Result<T, Response> {
    ids.iter()
        .enumerate()
        .find(|(i, id)| sel.matches(&id.to_string(), *i as u32 + 1))
        .map(|(_, id)| *id)
        .ok_or_else(|| no_match(what))
}

fn row(rows: Vec<Value>, id: &str) -> Answer {
    rows.into_iter()
        .find(|r| r["id"] == id)
        .map(Response::ok)
        .ok_or_else(|| no_match("row"))
}

fn window_ids(app: &App) -> Vec<WindowId> {
    app.windows().iter().map(|w| w.id).collect()
}

fn tab_ids(app: &App) -> Vec<TabId> {
    app.windows().iter().flat_map(|w| w.tabs().iter().map(|t| t.id)).collect()
}

fn focused_of_tab(app: &App, tab: TabId) -> Option<PaneId> {
    app.windows()
        .iter()
        .flat_map(|w| w.tabs())
        .find(|t| t.id == tab)
        .map(|t| t.focused)
}

fn window_rows(app: &App) -> Vec<Value> {
    let active = app.active_window().map(|w| w.id);
    app.windows()
        .iter()
        .map(|w| json!({"id": w.id.slug(), "tabs": w.tabs().len(), "active": active == Some(w.id)}))
        .collect()
}

fn tab_rows(app: &App) -> Vec<Value> {
    let mut rows = Vec::new();
    for w in app.windows() {
        for (i, t) in w.tabs().iter().enumerate() {
            let title = app.pane(t.focused).and_then(|p| p.title()).unwrap_or_default();
            let badge = t.badge.map(|b| match b {
                Badge::Running => "running".to_owned(),
                Badge::Success => "completed".to_owned(),
                Badge::Error => "error".to_owned(),
                Badge::Progress(p) => format!("{p}%"),
            });
            rows.push(json!({
                "id": t.id.slug(),
                "window": w.id.slug(),
                "title": format!("{}{title}", t.prefix.as_deref().unwrap_or_default()),
                "panes": t.tree.len(),
                "active": w.active_index() == i,
                "badge": badge,
            }));
        }
    }
    rows
}

fn pane_rows(app: &App) -> Vec<Value> {
    app.all_panes()
        .into_iter()
        .map(|id| {
            let p = app.pane(id);
            json!({
                "id": id.slug(),
                "tab": app.tab_of(id).map(TabId::slug),
                "window": app.window_of(id).map(WindowId::slug),
                "title": p.and_then(|p| p.title()),
                "cwd": p.and_then(|p| p.cwd.as_deref()),
                "agent": p.and_then(|p| p.agent.as_ref()).map(|(a, s)| format!("{a}:{}", s.as_str())),
            })
        })
        .collect()
}

fn window(d: &mut Dispatcher, req: WindowRequest) -> Answer {
    let ids = window_ids(&d.app);
    let active_tab = |app: &App, id: WindowId| {
        app.windows().iter().find(|w| w.id == id).and_then(|w| w.active_tab()).map(|t| (t.id, t.focused))
    };
    Ok(match req {
        WindowRequest::List => Response::ok(json!(window_rows(&d.app))),
        WindowRequest::Show(sel) => {
            row(window_rows(&d.app), &pick(&ids, &sel, "window")?.slug())?
        }
        WindowRequest::New { cwd, command, title, .. } => {
            no_command(&command)?;
            d.activate("new_window");
            let pane = d.focused_pane_id().ok_or_else(|| no_match("pane"))?;
            if let Some(p) = d.app.pane_mut(pane) {
                p.cwd = cwd;
                p.custom_title = title;
            }
            Response::ok(json!({"id": d.app.window_of(pane).map(WindowId::slug), "pane": pane.slug()}))
        }
        WindowRequest::Close(sel) => {
            d.app.close_window(pick(&ids, &sel, "window")?);
            Response::empty()
        }
        WindowRequest::Focus(sel) => {
            let (_, pane) = active_tab(&d.app, pick(&ids, &sel, "window")?)
                .ok_or_else(|| no_match("tab"))?;
            d.app.reveal_pane(pane);
            Response::empty()
        }
        WindowRequest::SelectedTab(sel) => {
            let (tab, _) = active_tab(&d.app, pick(&ids, &sel, "window")?)
                .ok_or_else(|| no_match("tab"))?;
            row(tab_rows(&d.app), &tab.slug())?
        }
        WindowRequest::Rename { .. } => return Err(unsupported("renaming a window")),
    })
}

fn tab(d: &mut Dispatcher, req: TabRequest) -> Answer {
    let ids = tab_ids(&d.app);
    let focused = |app: &App, sel: &Selector| -> Result<(TabId, PaneId), Response> {
        let id = pick(&ids, sel, "tab")?;
        Ok((id, focused_of_tab(app, id).ok_or_else(|| no_match("tab"))?))
    };
    Ok(match req {
        TabRequest::List => Response::ok(json!(tab_rows(&d.app))),
        TabRequest::Show(sel) => row(tab_rows(&d.app), &pick(&ids, &sel, "tab")?.slug())?,
        TabRequest::New { window, cwd, command, title, .. } => {
            no_command(&command)?;
            if let Some(sel) = window {
                let wid = pick(&window_ids(&d.app), &sel, "window")?;
                let pane = d
                    .app
                    .windows()
                    .iter()
                    .find(|w| w.id == wid)
                    .and_then(|w| w.active_tab())
                    .map(|t| t.focused)
                    .ok_or_else(|| no_match("tab"))?;
                d.app.reveal_pane(pane);
            }
            let before = d.app.all_panes().len();
            d.activate("new_tab");
            let pane = d
                .focused_pane_id()
                .filter(|_| d.app.all_panes().len() > before)
                .ok_or_else(|| no_match("window"))?;
            if let Some(p) = d.app.pane_mut(pane) {
                p.cwd = cwd;
                p.custom_title = title;
            }
            Response::ok(json!({"id": d.app.tab_of(pane).map(TabId::slug), "pane": pane.slug()}))
        }
        TabRequest::Close(sel) => {
            let (id, pane) = focused(&d.app, &sel)?;
            d.app.reveal_pane(pane);
            if let Some(w) = d.app.active_window_mut() {
                w.close_tab(id);
            }
            Response::empty()
        }
        TabRequest::Focus(sel) => {
            let (_, pane) = focused(&d.app, &sel)?;
            d.app.reveal_pane(pane);
            Response::empty()
        }
        TabRequest::Rename { tab, title } => {
            let (_, pane) = focused(&d.app, &tab)?;
            if let Some(p) = d.app.pane_mut(pane) {
                p.custom_title = Some(title);
            }
            Response::empty()
        }
        TabRequest::Badge { tab, kind } => set_badge(d, &tab, kind)?,
        TabRequest::Move { .. } => return Err(unsupported("moving a tab")),
    })
}

fn set_badge(d: &mut Dispatcher, sel: &Selector, kind: BadgeKind) -> Answer {
    let id = pick(&tab_ids(&d.app), sel, "tab")?;
    let badge = match kind {
        BadgeKind::Running => Badge::Running,
        BadgeKind::Completed | BadgeKind::Finished => Badge::Success,
        BadgeKind::Error => Badge::Error,
        BadgeKind::Unread | BadgeKind::AwaitingInput => return Err(unsupported("that badge kind")),
    };
    let pane = focused_of_tab(&d.app, id).ok_or_else(|| no_match("tab"))?;
    if let Some(t) = d.app.tab_mut_of(pane) {
        t.badge = Some(badge);
    }
    Ok(Response::empty())
}

/// The `Option<Selector>` of the pane requests that default to the focused pane.
fn pane_selector(req: &mut PaneRequest) -> Option<&mut Option<Selector>> {
    match req {
        PaneRequest::Split { pane, .. }
        | PaneRequest::Resize { pane, .. }
        | PaneRequest::SendKeys { pane, .. }
        | PaneRequest::SendText { pane, .. }
        | PaneRequest::Run { pane, .. }
        | PaneRequest::Exec { pane, .. }
        | PaneRequest::Capture { pane, .. }
        | PaneRequest::Tty { pane }
        | PaneRequest::Processes { pane }
        | PaneRequest::Busy { pane }
        | PaneRequest::Dimensions { pane }
        | PaneRequest::Wait { pane, .. } => Some(pane),
        PaneRequest::List
        | PaneRequest::Show(_)
        | PaneRequest::Close(_)
        | PaneRequest::Focus(_)
        | PaneRequest::Zoom(_) => None,
    }
}

fn pane(d: &mut Dispatcher, io: &mut dyn PaneIo, req: PaneRequest, target: Option<PaneId>) -> Answer {
    let all = d.app.all_panes();
    let target = || target.ok_or_else(|| no_match("pane"));
    let mut send = |pane: PaneId, bytes: &[u8]| {
        if io.write(pane, bytes) {
            Ok(Response::empty())
        } else {
            Err(no_session())
        }
    };
    Ok(match req {
        PaneRequest::List => Response::ok(json!(pane_rows(&d.app))),
        PaneRequest::Show(sel) => row(pane_rows(&d.app), &pick(&all, &sel, "pane")?.slug())?,
        PaneRequest::Close(sel) => {
            d.app.reveal_pane(pick(&all, &sel, "pane")?);
            // Closes the focused pane, or the tab when it is the last one.
            d.activate("close_tab");
            Response::empty()
        }
        PaneRequest::Focus(sel) => {
            d.app.reveal_pane(pick(&all, &sel, "pane")?);
            Response::empty()
        }
        PaneRequest::Zoom(sel) => {
            d.app.toggle_zoom(pick(&all, &sel, "pane")?);
            Response::empty()
        }
        PaneRequest::Split { direction, size, command, cwd, .. } => {
            no_command(&command)?;
            let from = target()?;
            d.app.reveal_pane(from);
            d.activate(match direction {
                SplitDirection::Right => "split_right",
                SplitDirection::Left => "split_left",
                SplitDirection::Up => "split_up",
                SplitDirection::Down => "split_down",
            });
            let new = d.focused_pane_id().filter(|n| *n != from).ok_or_else(|| no_match("pane"))?;
            if let Some(share) = size {
                // `--size` is the new pane's share; the tree stores the first child's.
                let new_is_second = matches!(direction, SplitDirection::Right | SplitDirection::Down);
                let first = if new_is_second { 100 - share.min(90) } else { share };
                if let Some(t) = d.app.tab_mut_of(new) {
                    t.tree.set_ratio_for(new, first);
                }
            }
            if let Some(p) = d.app.pane_mut(new) {
                p.cwd = cwd;
            }
            Response::ok(json!({"id": new.slug()}))
        }
        PaneRequest::SendText { text, .. } => send(target()?, &unescape(&text))?,
        PaneRequest::SendKeys { keys, .. } => {
            let mut bytes = Vec::new();
            for key in &keys {
                bytes.extend(key_bytes(key)?);
            }
            send(target()?, &bytes)?
        }
        // ponytail: submits and returns; waiting for the exit status needs the
        // OSC 133 D mark. Until then `otty pane wait` is the way to block.
        PaneRequest::Run { command, .. } => send(target()?, format!("{command}\r").as_bytes())?,
        PaneRequest::Capture { lines, .. } => {
            let mut rows = io.screen(target()?).ok_or_else(no_session)?;
            while rows.last().is_some_and(|r| r.trim().is_empty()) {
                rows.pop();
            }
            if let Some(n) = lines {
                rows.drain(..rows.len().saturating_sub(n as usize));
            }
            Response::ok_text(rows.join("\n"))
        }
        PaneRequest::Tty { .. } => io
            .tty(target()?)
            .map(Response::ok_text)
            .ok_or_else(|| Response::error(code::NO_MATCH, "pane has no tty device"))?,
        PaneRequest::Processes { .. } => {
            let names: Vec<Value> =
                io.processes(target()?).into_iter().map(|n| json!({"name": n})).collect();
            Response::ok(json!(names))
        }
        PaneRequest::Busy { .. } => {
            Response::ok(json!({"busy": io.busy(target()?).ok_or_else(no_session)?}))
        }
        PaneRequest::Dimensions { .. } => {
            let (columns, rows) = io.dimensions(target()?).ok_or_else(no_session)?;
            Response::ok(json!({"rows": rows, "columns": columns}))
        }
        // One poll: 0 idle, 9 still busy, 6 when neither side can say. The
        // CLI repeats it until the deadline. An agent's hook beats the shell.
        PaneRequest::Wait { .. } => {
            let id = target()?;
            let by_hook =
                d.app.pane(id).and_then(|p| p.agent.as_ref()).map(|(_, s)| *s == AgentState::Idle);
            match by_hook.or_else(|| io.busy(id).map(|busy| !busy)) {
                Some(true) => Response::empty(),
                Some(false) => return Err(Response::error(code::TIMEOUT, "pane is busy")),
                None => {
                    return Err(Response::error(code::NO_INTEGRATION, "pane cannot report idle"))
                }
            }
        }
        PaneRequest::Exec { .. } => return Err(unsupported("pane exec")),
        PaneRequest::Resize { .. } => return Err(unsupported("pane resize")),
    })
}

/// `send-text` escapes: `\n \r \t \e \\ \xHH`. Anything else is literal.
fn unescape(text: &str) -> Vec<u8> {
    let src = text.as_bytes();
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        let simple = match src.get(i + 1).filter(|_| src[i] == b'\\') {
            Some(b'n') => Some(b'\n'),
            Some(b'r') => Some(b'\r'),
            Some(b't') => Some(b'\t'),
            Some(b'e') => Some(0x1b),
            Some(b'\\') => Some(b'\\'),
            _ => None,
        };
        let hex = (src[i] == b'\\' && src.get(i + 1) == Some(&b'x'))
            .then(|| text.get(i + 2..i + 4).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        if let Some(b) = simple {
            out.push(b);
            i += 2;
        } else if let Some(b) = hex {
            out.push(b);
            i += 4;
        } else {
            out.push(src[i]);
            i += 1;
        }
    }
    out
}

/// `key:Enter`-style names; anything without the prefix is literal text.
fn key_bytes(key: &str) -> Result<Vec<u8>, Response> {
    let Some(name) = key.strip_prefix("key:") else { return Ok(key.as_bytes().to_vec()) };
    let name = name.to_ascii_lowercase();
    Ok(match name.as_str() {
        "enter" | "return" => b"\r".to_vec(),
        "tab" => b"\t".to_vec(),
        "escape" | "esc" => b"\x1b".to_vec(),
        "backspace" => b"\x7f".to_vec(),
        "space" => b" ".to_vec(),
        "up" => b"\x1b[A".to_vec(),
        "down" => b"\x1b[B".to_vec(),
        "right" => b"\x1b[C".to_vec(),
        "left" => b"\x1b[D".to_vec(),
        other => match other.strip_prefix("ctrl+").map(str::as_bytes) {
            Some([c]) if c.is_ascii_lowercase() => vec![c - b'a' + 1],
            _ => return Err(Response::error(code::USAGE, format!("`{key}` is not a key"))),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pane 1 has a session; every other pane is model-only.
    #[derive(Default)]
    struct Fake {
        written: Vec<u8>,
        busy: bool,
        processes: Vec<String>,
    }

    impl PaneIo for Fake {
        fn write(&mut self, pane: PaneId, bytes: &[u8]) -> bool {
            self.written.extend_from_slice(bytes);
            pane == PaneId::new(1)
        }
        fn screen(&self, pane: PaneId) -> Option<Vec<String>> {
            (pane == PaneId::new(1))
                .then(|| vec!["$ ls".into(), "a  b".into(), "$".into(), "".into(), " ".into()])
        }
        fn dimensions(&self, pane: PaneId) -> Option<(u16, u16)> {
            (pane == PaneId::new(1)).then_some((80, 24))
        }
        fn busy(&self, pane: PaneId) -> Option<bool> {
            (pane == PaneId::new(1)).then_some(self.busy)
        }
        fn processes(&self, _: PaneId) -> Vec<String> {
            self.processes.clone()
        }
        fn tty(&self, _: PaneId) -> Option<String> {
            None
        }
    }

    const OPEN: Gates = Gates { allow_send_keys: true, allow_sensitive_sessions: false };

    fn code_of(r: &Response) -> i32 {
        match r {
            Response::Ok { .. } => 0,
            Response::Err { code, .. } => *code,
        }
    }

    fn data(r: Response) -> Value {
        match r {
            Response::Ok { data: Some(d), .. } => d,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lists_follow_the_model_and_a_missing_selector_is_exit_4() {
        let (mut d, mut io) = (Dispatcher::new(), Fake::default());
        let rows = data(handle(&mut d, &mut io, Gates::default(), Request::Window(WindowRequest::List)));
        assert_eq!(rows, json!([{"id": "window-1", "tabs": 1, "active": true}]));

        let new = Request::Tab(TabRequest::New {
            window: None,
            profile: None,
            cwd: Some("/work".into()),
            command: None,
            title: Some("api".into()),
        });
        let made = data(handle(&mut d, &mut io, Gates::default(), new));
        assert_eq!(made, json!({"id": "tab-2", "pane": "pane-2"}));
        let panes = data(handle(&mut d, &mut io, Gates::default(), Request::Pane(PaneRequest::List)));
        assert_eq!(panes[1]["cwd"], "/work");
        assert_eq!(panes[1]["title"], "api");

        let split = Request::Pane(PaneRequest::Split {
            pane: Some(Selector::Index(1)),
            direction: SplitDirection::Right,
            size: Some(30),
            command: None,
            cwd: None,
        });
        assert_eq!(data(handle(&mut d, &mut io, Gates::default(), split)), json!({"id": "pane-3"}));
        let status = data(handle(&mut d, &mut io, Gates::default(), Request::Status));
        assert_eq!((status["tabs"].clone(), status["panes"].clone()), (json!(2), json!(3)));
        // The split revealed tab 1, and pane-3 sits in it.
        let tabs = data(handle(&mut d, &mut io, Gates::default(), Request::Tab(TabRequest::List)));
        assert_eq!((tabs[0]["panes"].clone(), tabs[0]["active"].clone()), (json!(2), json!(true)));

        let miss = Request::Pane(PaneRequest::Focus(Selector::Id("pane-9".into())));
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), miss)), code::NO_MATCH);
        let miss = Request::Tab(TabRequest::Close(Selector::Index(7)));
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), miss)), code::NO_MATCH);
    }

    #[test]
    fn send_text_is_gated_unescaped_and_refused_on_an_ssh_pane() {
        let (mut d, mut io) = (Dispatcher::new(), Fake::default());
        let send = || Request::Pane(PaneRequest::SendText { pane: None, text: r"echo hi\n\x03".into() });
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), send())), code::REFUSED);
        assert!(io.written.is_empty());

        assert_eq!(code_of(&handle(&mut d, &mut io, OPEN, send())), 0);
        assert_eq!(io.written, b"echo hi\n\x03");

        // No selector at all still resolves to the focused pane for the gate.
        io.processes = vec!["pwsh.exe".into(), "ssh.exe".into()];
        assert_eq!(code_of(&handle(&mut d, &mut io, OPEN, send())), code::REFUSED);
        let capture = Request::Pane(PaneRequest::Capture { pane: None, lines: None, escapes: false });
        assert_eq!(code_of(&handle(&mut d, &mut io, OPEN, capture)), code::REFUSED);

        io.processes.clear();
        let keys = Request::Pane(PaneRequest::SendKeys {
            pane: None,
            keys: vec!["ls".into(), "key:Enter".into(), "key:ctrl+c".into()],
        });
        io.written.clear();
        assert_eq!(code_of(&handle(&mut d, &mut io, OPEN, keys)), 0);
        assert_eq!(io.written, b"ls\r\x03");
    }

    #[test]
    fn capture_trims_blank_rows_and_a_model_only_pane_has_no_session() {
        let (mut d, mut io) = (Dispatcher::new(), Fake::default());
        let capture = |lines| Request::Pane(PaneRequest::Capture { pane: None, lines, escapes: false });
        match handle(&mut d, &mut io, Gates::default(), capture(Some(2))) {
            Response::Ok { text: Some(t), .. } => assert_eq!(t, "a  b\n$"),
            other => panic!("{other:?}"),
        }
        d.activate("split_right");
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), capture(None))), code::NO_MATCH);
    }

    #[test]
    fn wait_polls_the_shell_until_an_agent_hook_takes_over() {
        let (mut d, mut io) = (Dispatcher::new(), Fake::default());
        let wait = || Request::Pane(PaneRequest::Wait { pane: None, timeout_ms: None });
        io.busy = true;
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), wait())), code::TIMEOUT);
        io.busy = false;
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), wait())), 0);

        let report = |state| Request::AgentState {
            agent: otty_core::AgentId::Claude,
            session_id: "s1".into(),
            state,
            cwd: None,
            bypass: false,
            agent_pid: None,
            error_kind: None,
        };
        handle(&mut d, &mut io, Gates::default(), report(AgentState::Processing));
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), wait())), code::TIMEOUT);
        let tabs = data(handle(&mut d, &mut io, Gates::default(), Request::Tab(TabRequest::List)));
        assert_eq!(tabs[0]["badge"], "running");

        handle(&mut d, &mut io, Gates::default(), report(AgentState::Idle));
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), wait())), 0);
        let status = data(handle(&mut d, &mut io, Gates::default(), Request::Status));
        assert_eq!(status["agents"]["s1"], "idle", "what `otty watch:<agent>` reads");

        // A pane with neither a session nor a hook cannot answer.
        d.activate("split_right");
        assert_eq!(code_of(&handle(&mut d, &mut io, Gates::default(), wait())), code::NO_INTEGRATION);
    }
}
