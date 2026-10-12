//! What a frontend request *means*, with no stdio, threads or PTY in sight.
//!
//! `main.rs` owns the pipes and the live sessions; this module owns the model
//! and answers the methods of `frontend/electron/PROTOCOL.md` against it,
//! reaching the sessions only through [`Panes`]. That is what lets a test
//! drive it.

use std::collections::HashMap;
use std::path::Path;

use otty_config::theme::{slugify, Mode, Theme};
use otty_config::{bundled_resources, Catalog, Config, KeyKind, SaveScope, Value as Setting};
use otty_core::{expand_path, PaneId, TabId};
use otty_render::theme::Chrome;
use otty_state::Store;
use otty_term::{find_next, Event, FindHit, Progress};
use otty_ui::ipc::PaneIo;
use otty_ui::mode::InputMode;
use otty_ui::quickly::now_secs;
use otty_ui::tabs::{git_root, TabGroupMode};
use otty_ui::{
    context_rows, filter_palette, palette_entries, pane_hit, query_quickly, shell_menu, Axis,
    Badge, Dispatcher, Node, OpenFilter, Pane, PaneKind, QuicklySources, SplitDirection, Tab,
};
use serde_json::{json, Map, Value};

/// How many closed tabs `tab.reopen` remembers.
const REOPEN_DEPTH: usize = 12;
/// `file.read` answers in one JSON line, so it refuses anything bigger.
const FILE_LIMIT: u64 = 2 << 20;

/// The live sessions, as the request logic needs them.
pub trait Panes: PaneIo {
    /// Make the sessions match the model: start one (in its cwd) for every
    /// listed pane that has none, end every session that is not listed.
    /// Returns the panes that could not start, and why.
    fn sync(&mut self, panes: &[(PaneId, Option<String>)], config: &Config) -> Vec<(PaneId, String)>;
    /// The text between two `(col, row)` cells.
    fn selected_text(&self, pane: PaneId, from: (u16, u16), to: (u16, u16)) -> Option<String>;
    /// Scrollback plus the screen, one string per grid line.
    fn history(&self, pane: PaneId) -> Option<Vec<String>>;
    fn set_theme(&mut self, theme: &Theme);
    /// The font keys changed; re-measure every pane.
    fn set_font(&mut self, config: &Config);
}

/// One window's model and everything a request can change.
pub struct Backend {
    pub d: Dispatcher,
    pub config: Config,
    /// The frecency store and prompt queue. `None` in tests.
    pub store: Option<Store>,
    /// Events owed to the frontend, in order. The caller drains them.
    pub out: Vec<Value>,
    /// The window is gone: `quit` ran, or its last tab closed.
    pub quit: bool,
    /// Cwds of closed tabs, newest last.
    closed: Vec<Option<String>>,
    /// The tabs as of the last `settle`, so a tab that vanished can be reopened.
    tabs: Vec<(TabId, Option<String>)>,
    /// The focused pane's cwd as of the last `settle`; a new pane starts there.
    last_cwd: Option<String>,
    icons: HashMap<PaneId, &'static str>,
    /// cwd → git root.
    projects: HashMap<String, String>,
}

type Answer = Result<Value, String>;

fn text<'a>(p: &'a Value, key: &str) -> Result<&'a str, String> {
    p[key].as_str().ok_or_else(|| format!("missing `{key}`"))
}

fn cell(p: &Value, key: &str) -> Result<(u16, u16), String> {
    let n = |i: usize| p[key][i].as_u64().map(|n| n as u16);
    n(0).zip(n(1)).ok_or_else(|| format!("`{key}` must be [col, row]"))
}

fn title(pane: Option<&Pane>) -> &str {
    pane.and_then(|p| p.custom_title.as_deref().filter(|t| !t.is_empty()).or(p.shell_title.as_deref()))
        .unwrap_or("Otty")
}

fn tree(node: &Node) -> Value {
    match node {
        Node::Leaf(id) => json!({"pane": id.slug()}),
        Node::Split { axis, ratio, first, second } => json!({
            "axis": match axis { Axis::Horizontal => "horizontal", Axis::Vertical => "vertical" },
            "ratio": ratio,
            "first": tree(first),
            "second": tree(second),
        }),
    }
}

fn setting(value: Setting) -> Value {
    match value {
        Setting::Bool(b) => json!(b),
        Setting::Int(i) => json!(i),
        Setting::Float(f) => json!(f),
        Setting::Str(s) => json!(s),
        Setting::List(l) => json!(l),
        color => json!(color.render()),
    }
}

/// OSC 7 carries `file://host/path`, percent-encoded; the model wants the path.
fn osc7_path(url: &str) -> String {
    let rest = url.strip_prefix("file://").map_or(url, |r| &r[r.find('/').unwrap_or(r.len())..]);
    let (src, mut out, mut i) = (rest.as_bytes(), Vec::with_capacity(rest.len()), 0);
    while i < src.len() {
        let hex = (src[i] == b'%')
            .then(|| rest.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()))
            .flatten();
        out.push(hex.unwrap_or(src[i]));
        i += if hex.is_some() { 3 } else { 1 };
    }
    let path = String::from_utf8_lossy(&out).into_owned();
    // `/C:/Users` is how a URL spells a drive.
    match path.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => path[1..].to_owned(),
        _ => path,
    }
}

/// `git` in `dir`, when it is a repository and git is installed.
fn git(dir: &str, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW: the bridge has no console, and git must not open one.
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Dotted keys, the spelling `Catalog::get` takes.
fn flatten(prefix: &str, node: &Value, out: &mut Map<String, Value>) {
    match node {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(&key, v, out);
            }
        }
        Value::String(_) => {
            out.insert(prefix.to_owned(), node.clone());
        }
        _ => {}
    }
}

impl Backend {
    /// A window with one tab, whose shell starts in `cwd`.
    pub fn new(config: Config, cwd: Option<String>) -> Self {
        let mut d = Dispatcher::new();
        if let Some(p) = d.focused_pane_id().and_then(|id| d.app.pane_mut(id)) {
            p.cwd = cwd;
        }
        Self {
            d,
            config,
            store: None,
            out: Vec::new(),
            quit: false,
            closed: Vec::new(),
            tabs: Vec::new(),
            last_cwd: None,
            icons: HashMap::new(),
            projects: HashMap::new(),
        }
    }

    pub fn ready(&self) -> Value {
        json!({
            "type": "ready",
            "version": env!("CARGO_PKG_VERSION"),
            "chrome": Chrome::from_theme(&self.config.active_theme(false)),
            "layout": self.config.get_str("window-layout").unwrap_or_else(|| "sidebar-left".to_owned()),
            "sidebar_visible": self.config.get_bool("sidebar-visible").unwrap_or(true),
            "platform": std::env::consts::OS,
        })
    }

    fn tab_list(&self) -> &[Tab] {
        self.d.app.active_window().map(|w| w.tabs()).unwrap_or_default()
    }

    /// The panes the frontend is showing: the active tab's, or its zoomed one.
    pub fn visible(&self) -> Vec<PaneId> {
        self.d
            .app
            .active_window()
            .and_then(|w| w.active_tab())
            .map(|t| t.zoomed.map_or_else(|| t.tree.panes(), |z| vec![z]))
            .unwrap_or_default()
    }

    /// The full snapshot. Cheap enough to build every tick and compare.
    pub fn state<P: Panes>(&mut self, io: &P) -> Value {
        let Self { d, projects, icons, .. } = self;
        let app = &d.app;
        let window = app.active_window();
        let (mut tabs, mut panes) = (Vec::new(), Map::new());
        for t in window.map(|w| w.tabs()).unwrap_or_default() {
            let focused = app.pane(t.focused);
            let cwd = focused.and_then(|p| p.cwd.clone()).unwrap_or_default();
            // ponytail: cached for the bridge's life, so a `git init` in an open
            // cwd is not noticed; drop the entry on a file watch if that matters.
            let project = projects.entry(cwd.clone()).or_insert_with(|| {
                let has_git = |dir: &str| Path::new(dir).join(".git").exists();
                Some(git_root(&cwd, has_git)).filter(|root| has_git(root)).unwrap_or_default()
            });
            let icon = match focused.map(|p| &p.kind) {
                Some(PaneKind::File { .. }) => "file",
                Some(PaneKind::Folder { .. }) => "folder",
                Some(PaneKind::Url { .. }) => "web",
                _ => focused
                    .and_then(|p| p.agent.as_ref())
                    .and_then(|(name, _)| otty_agent::icon_for_process(name))
                    .or(icons.get(&t.focused).copied())
                    .unwrap_or("terminal"),
            };
            tabs.push(json!({
                "id": t.id.slug(),
                "title": format!("{}{}", t.prefix.as_deref().unwrap_or_default(), title(focused)),
                "cwd": cwd,
                "project": project,
                "icon": icon,
                "badge": match t.badge {
                    None => Value::Null,
                    Some(Badge::Running) => json!("running"),
                    Some(Badge::Success) => json!("completed"),
                    Some(Badge::Error) => json!("error"),
                    Some(Badge::Progress(p)) => json!({"progress": p}),
                },
                "focused": t.focused.slug(),
                "zoomed": t.zoomed.map(PaneId::slug),
                "tree": tree(&t.tree),
            }));
            for id in t.tree.panes() {
                let Some(p) = app.pane(id) else { continue };
                let (cols, rows) = io.dimensions(id).unwrap_or_default();
                let mut row = json!({
                    "kind": "terminal",
                    "title": title(Some(p)),
                    "cwd": p.cwd.as_deref().unwrap_or_default(),
                    "agent": p.agent.as_ref().map(|(name, state)| json!({"name": name, "state": state.as_str()})),
                    "grid": {"cols": cols, "rows": rows},
                });
                // Not in PROTOCOL.md's example: a non-terminal pane says what it shows.
                match &p.kind {
                    PaneKind::Terminal => {}
                    PaneKind::File { path, .. } => (row["kind"], row["path"]) = (json!("file"), json!(path)),
                    PaneKind::Folder { path, .. } => (row["kind"], row["path"]) = (json!("folder"), json!(path)),
                    PaneKind::Url { url } => (row["kind"], row["url"]) = (json!("url"), json!(url)),
                }
                panes.insert(id.slug(), row);
            }
        }
        json!({
            "type": "state",
            "activeTab": window.and_then(|w| w.active_tab()).map(|t| t.id.slug()),
            "tabs": tabs,
            "panes": panes,
            "tabGroup": match d.tab_group {
                TabGroupMode::None => "none",
                TabGroupMode::Project => "project",
                TabGroupMode::Date => "date",
            },
            "dividers": d.dividers,
            "mode": match d.mode {
                InputMode::Shell => "shell",
                InputMode::Hint => "hint",
                InputMode::Vi => "vi",
                InputMode::Mark => "mark",
            },
        })
    }

    /// What a pane's shell said since the last pump.
    pub fn on_event(&mut self, pane: PaneId, event: Event) {
        let slug = pane.slug();
        match event {
            Event::Title(t) => {
                if let Some(p) = self.d.app.pane_mut(pane) {
                    p.shell_title = Some(t);
                }
            }
            Event::WorkingDirectory(url) => {
                if let Some(p) = self.d.app.pane_mut(pane) {
                    p.cwd = Some(osc7_path(&url));
                }
            }
            Event::Progress(progress) => {
                if let Some(t) = self.d.app.tab_mut_of(pane) {
                    t.badge = match progress {
                        Progress::Clear => None,
                        Progress::Set(p) | Progress::Paused(p) => Some(Badge::Progress(p)),
                        Progress::Indeterminate => Some(Badge::Running),
                        Progress::Error => Some(Badge::Error),
                    };
                }
            }
            Event::Bell => self.out.push(json!({"type": "bell", "pane": slug})),
            Event::Notify(n) if !n.silent => self.out.push(json!({
                "type": "notify", "title": n.title.unwrap_or_default(), "body": n.body, "pane": slug,
            })),
            Event::ClipboardStore { data, .. } => {
                self.out.push(json!({"type": "clipboard", "text": String::from_utf8_lossy(&data)}));
            }
            _ => {}
        }
    }

    /// Tab icons follow the foreground process, which costs a process walk —
    /// so on the caller's clock, not on every snapshot.
    pub fn refresh_icons<P: Panes>(&mut self, io: &P) {
        self.icons = self
            .d
            .app
            .all_panes()
            .into_iter()
            .map(|id| (id, otty_agent::icon_for_processes(&io.processes(id))))
            .collect();
    }

    /// Close a pane, and its tab when that was the last one in it.
    pub fn close_pane(&mut self, pane: PaneId) {
        if let Some(tab) = self.d.app.close_pane(pane) {
            if let Some(w) = self.d.app.active_window_mut() {
                w.take_tab(tab);
            }
        }
    }

    /// Bring everything outside the model back in step with it. Runs after
    /// every request and every tick; a no-op when nothing moved.
    pub fn settle<P: Panes>(&mut self, io: &mut P) {
        // ponytail: one bridge is one window, and the protocol has no "open a
        // window" event, so `new_window` and friends land as tabs. Give the
        // frontend that event and stop merging.
        if self.d.app.windows().len() > 1 {
            let focused = self.d.focused_pane_id();
            // Merge into the first window, so the newcomers go last.
            let home = self.d.app.windows()[0].active_tab().map(|t| t.focused);
            if let Some(pane) = home {
                self.d.app.reveal_pane(pane);
            }
            self.d.app.merge_all_windows();
            if let Some(pane) = focused {
                self.d.app.reveal_pane(pane);
            }
            self.d.pip = None;
        }

        // A new pane starts where the user was.
        for id in self.d.app.all_panes() {
            if let Some(p) = self.d.app.pane_mut(id).filter(|p| p.cwd.is_none()) {
                p.cwd.clone_from(&self.last_cwd);
            }
        }
        let app = &self.d.app;
        let cwd_of = |id: PaneId| app.pane(id).and_then(|p| p.cwd.clone());
        self.last_cwd = self.d.focused_pane_id().and_then(cwd_of);

        let tabs: Vec<_> = self.tab_list().iter().map(|t| (t.id, cwd_of(t.focused))).collect();
        for (id, cwd) in std::mem::take(&mut self.tabs) {
            if !tabs.iter().any(|(t, _)| *t == id) {
                self.closed.push(cwd);
            }
        }
        self.closed.drain(..self.closed.len().saturating_sub(REOPEN_DEPTH));
        self.tabs = tabs;

        let live: Vec<_> = app
            .all_panes()
            .into_iter()
            .filter(|id| app.pane(*id).is_some_and(|p| p.kind == PaneKind::Terminal))
            .map(|id| (id, cwd_of(id)))
            .collect();
        for (pane, why) in io.sync(&live, &self.config) {
            self.out.push(json!({"type": "error", "message": format!("{pane} could not start: {why}")}));
            self.close_pane(pane);
        }

        if let Some(text) = self.d.clipboard.take() {
            self.out.push(json!({"type": "clipboard", "text": text}));
        }
        if let Some(url) = self.d.last_opened_url.take() {
            self.out.push(json!({"type": "open", "url": url}));
        }
        // A prompt queued while the agent was busy goes out once it is idle.
        let focused = self.d.focused_pane_id();
        let agent = focused.and_then(|id| self.d.app.pane(id)).is_some_and(|p| p.agent.is_some());
        if let (Some(store), Some(pane), true) = (&self.store, focused, agent) {
            if let Some(text) = self.d.flush_queue(store) {
                io.write(pane, format!("{text}\r").as_bytes());
            }
        }
        self.quit |= self.d.quit || self.tab_list().is_empty();
    }

    /// Answer one `request`. `Err` is the `error` of an `ok:false` response.
    pub fn request<P: Panes>(&mut self, io: &mut P, method: &str, params: &Value) -> Answer {
        let answer = self.run(io, method, params);
        self.settle(io);
        answer
    }

    fn pane(&self, p: &Value) -> Result<PaneId, String> {
        match p["pane"].as_str() {
            Some(s) => s
                .parse()
                .ok()
                .filter(|id| self.d.app.all_panes().contains(id))
                .ok_or_else(|| format!("no pane `{s}`")),
            None => self.d.focused_pane_id().ok_or_else(|| "no focused pane".to_owned()),
        }
    }

    /// The tab named by `p[key]`, as `(id, focused pane)`.
    fn tab(&self, p: &Value, key: &str) -> Result<(TabId, PaneId), String> {
        let s = text(p, key)?;
        let id = s.parse::<TabId>().ok();
        self.tab_list()
            .iter()
            .find(|t| Some(t.id) == id)
            .map(|t| (t.id, t.focused))
            .ok_or_else(|| format!("no tab `{s}`"))
    }

    fn cwd(&self, p: &Value) -> Result<String, String> {
        Ok(self.d.app.pane(self.pane(p)?).and_then(|pane| pane.cwd.clone()).unwrap_or_default())
    }

    fn new_tab(&mut self, cwd: Option<String>) -> Answer {
        let before = self.d.focused_pane_id();
        self.d.activate("new_tab");
        let pane = self.d.focused_pane_id().filter(|p| Some(*p) != before).ok_or("no window to open a tab in")?;
        if let (Some(p), Some(_)) = (self.d.app.pane_mut(pane), &cwd) {
            p.cwd = cwd;
        }
        Ok(json!({"tab": self.d.app.tab_of(pane).map(TabId::slug), "pane": pane.slug()}))
    }

    fn catalog(&self) -> Option<Catalog> {
        self.config.load_catalog(&bundled_resources()).ok()
    }

    fn apply_theme<P: Panes>(&mut self, io: &mut P, theme: &Theme) {
        io.set_theme(theme);
        self.out.push(json!({"type": "chrome", "chrome": Chrome::from_theme(theme)}));
    }

    /// A setting changed on disk: tell the frontend, and apply what is live.
    fn config_changed<P: Panes>(&mut self, io: &mut P, key: &str) {
        self.out.push(json!({"type": "config", "key": key}));
        if key.starts_with("theme") {
            let theme = self.config.active_theme(false);
            self.apply_theme(io, &theme);
        }
        if key.starts_with("font") {
            io.set_font(&self.config);
        }
    }

    fn run<P: Panes>(&mut self, io: &mut P, method: &str, p: &Value) -> Answer {
        let write = |io: &mut P, pane: PaneId, bytes: &[u8]| {
            io.write(pane, bytes).then_some(()).ok_or("pane has no live session")
        };
        Ok(match method {
            "action.run" => {
                // Hint and Vi mode read the screen the action was run on.
                if let Some(rows) = self.d.focused_pane_id().and_then(|id| io.screen(id)) {
                    self.d.find_lines = rows;
                }
                json!({"handled": self.d.activate(text(p, "id")?)})
            }

            "tab.new" => self.new_tab(p["cwd"].as_str().map(str::to_owned))?,
            "tab.activate" => {
                self.d.app.reveal_pane(self.tab(p, "tab")?.1);
                Value::Null
            }
            "tab.close" => {
                let (id, _) = self.tab(p, "tab")?;
                self.d.app.active_window_mut().and_then(|w| w.take_tab(id));
                Value::Null
            }
            "tab.move" => {
                let ((id, _), (target, _)) = (self.tab(p, "tab")?, self.tab(p, "target")?);
                let after = p["after"].as_bool().unwrap_or(false) as usize;
                let active = self.d.focused_pane_id();
                let rest = self.tab_list().iter().filter(|t| t.id != id);
                if let Some(at) = rest.map(|t| t.id).position(|t| t == target) {
                    self.d.app.active_window_mut().map(|w| w.move_tab(id, at + after));
                    // `move_tab` activates what it moved; a drag does not.
                    active.map(|pane| self.d.app.reveal_pane(pane));
                }
                Value::Null
            }
            "tab.rename" => {
                let (_, pane) = self.tab(p, "tab")?;
                let title = Some(text(p, "title")?.to_owned()).filter(|t| !t.is_empty());
                self.d.app.pane_mut(pane).ok_or("tab has no pane")?.custom_title = title;
                Value::Null
            }
            "tab.reopen" => match self.closed.pop() {
                Some(cwd) => self.new_tab(cwd)?,
                None => Value::Null,
            },

            "pane.split" => {
                let from = self.pane(p)?;
                let direction = SplitDirection::parse(text(p, "direction")?)
                    .ok_or("direction must be right, left, up or down")?;
                self.d.app.reveal_pane(from);
                self.d.activate(&format!("split_{}", direction.as_str()));
                let new = self.d.focused_pane_id().filter(|n| *n != from).ok_or("pane could not be split")?;
                // A split made behind a zoom would be invisible.
                if let Some(t) = self.d.app.tab_mut_of(new) {
                    t.zoomed = None;
                }
                json!({"pane": new.slug()})
            }
            "pane.focus" => {
                self.d.app.reveal_pane(self.pane(p)?);
                Value::Null
            }
            "pane.close" => {
                self.close_pane(self.pane(p)?);
                Value::Null
            }
            "pane.zoom" => {
                self.d.app.toggle_zoom(self.pane(p)?);
                Value::Null
            }
            "pane.ratio" => {
                let pane = self.pane(p)?;
                let ratio = p["ratio"].as_f64().ok_or("missing `ratio`")?.clamp(10.0, 90.0) as u8;
                let with = p["with"].as_str().map(|s| self.pane(&json!({"pane": s}))).transpose()?;
                self.d.app.tab_mut_of(pane).map(|t| match with {
                    Some(other) => t.tree.set_ratio_between(pane, other, ratio),
                    None => t.tree.set_ratio_for(pane, ratio),
                });
                Value::Null
            }
            "pane.text" => {
                let mut rows = io.screen(self.pane(p)?).ok_or("pane has no live session")?;
                if let Some(n) = p["lines"].as_u64() {
                    while rows.last().is_some_and(|r| r.trim().is_empty()) {
                        rows.pop();
                    }
                    rows.drain(..rows.len().saturating_sub(n as usize));
                }
                json!({"rows": rows})
            }
            "pane.select" => {
                let selected = io
                    .selected_text(self.pane(p)?, cell(p, "from")?, cell(p, "to")?)
                    .ok_or("pane has no live session")?;
                // So `action.run copy` has something to copy.
                self.d.hit.selection = Some(selected.clone());
                json!({"text": selected})
            }
            "pane.context" => {
                let rows = io.screen(self.pane(p)?).unwrap_or_default();
                let line = rows.get(p["row"].as_u64().unwrap_or(0) as usize).map_or("", String::as_str);
                self.d.hit = pane_hit(line, p["col"].as_u64().unwrap_or(0) as usize, p["selection"].as_str());
                let rows = context_rows(&self.d.hit);
                json!(rows.iter().map(|(label, action)| json!({"label": label, "action": action})).collect::<Vec<_>>())
            }
            "find.next" => {
                let lines = io.history(self.pane(p)?).ok_or("pane has no live session")?;
                let key = |h: &FindHit| (h.line, h.col);
                // ponytail: every hit is collected on every call, O(scrollback)
                // per keystroke. Keep them per query when a profile says so.
                let mut hits: Vec<FindHit> = Vec::new();
                while let Some(hit) = find_next(&lines, text(p, "query")?, hits.last().copied()) {
                    if hits.last().is_some_and(|last| key(&hit) <= key(last)) {
                        break;
                    }
                    hits.push(hit);
                }
                let from = p["from"]["line"].as_u64().zip(p["from"]["col"].as_u64()).map(|(l, c)| (l as usize, c as usize));
                let hit = if p["backwards"].as_bool().unwrap_or(false) {
                    hits.iter().rev().find(|h| from.is_some_and(|f| key(h) < f)).or(hits.last())
                } else {
                    hits.iter().find(|h| from.is_none_or(|f| key(h) > f)).or(hits.first())
                };
                json!(hit.map(|h| json!({"line": h.line, "col": h.col})))
            }

            "palette.list" => {
                let entries = palette_entries(self.catalog().as_ref(), &self.config.keymap());
                let rows = filter_palette(&entries, p["query"].as_str().unwrap_or_default());
                json!(rows
                    .iter()
                    .map(|e| json!({"id": e.action, "title": e.label, "scope": e.scope.as_str(), "shortcut": e.shortcut}))
                    .collect::<Vec<_>>())
            }
            "quickly.query" => {
                let mut opened = self.d.opened.clone();
                opened.extend(self.tab_list().iter().filter_map(|t| self.d.app.pane(t.focused)?.cwd.clone()));
                opened.sort();
                opened.dedup();
                // `live` reads the default store and `~/.ssh/config`; tests have neither.
                let sources = match self.store {
                    Some(_) => QuicklySources::live(opened),
                    None => QuicklySources { opened, ..Default::default() },
                };
                let filter = OpenFilter::from_key(p["filter"].as_str().unwrap_or("all"));
                let rows = query_quickly(filter, p["query"].as_str().unwrap_or_default(), &sources);
                json!(rows
                    .iter()
                    .map(|e| {
                        let kind = e.kind.as_str();
                        let name = Path::new(&e.path).file_name().map(|n| n.to_string_lossy().into_owned());
                        match name.filter(|_| !matches!(kind, "ssh" | "agent" | "recipe")) {
                            Some(name) => json!({"title": name, "subtitle": e.path, "kind": kind, "value": e.path}),
                            None => json!({"title": e.title, "subtitle": "", "kind": kind, "value": e.path}),
                        }
                    })
                    .collect::<Vec<_>>())
            }
            "quickly.open" => {
                let value = text(p, "value")?;
                match text(p, "kind")? {
                    // A row's kind says where it came from; whether it opens a
                    // shell or a viewer is decided by what is on disk.
                    "path" | "folder" | "opened" | "recent" | "current" if !Path::new(value).is_file() => {
                        if !self.d.run_quickly(value, self.store.as_ref(), now_secs()) {
                            return Err(format!("{value} is not a directory"));
                        }
                        self.new_tab(self.d.last_opened.clone())?;
                    }
                    "file" | "path" | "folder" | "opened" | "recent" | "current" => {
                        if !self.d.open_view(value) {
                            return Err(format!("{value} could not be opened"));
                        }
                    }
                    "url" => self.out.push(json!({"type": "open", "url": value})),
                    "ssh" => {
                        // The host is typed into a shell, so it must look like one.
                        let ok = |c: char| c.is_ascii_alphanumeric() || "._-@:".contains(c);
                        if value.is_empty() || value.starts_with('-') || !value.chars().all(ok) {
                            return Err(format!("`{value}` is not a host name"));
                        }
                        let made = self.new_tab(None)?;
                        self.settle(io);
                        write(io, made["pane"].as_str().and_then(|s| s.parse().ok()).ok_or("no pane")?, format!("ssh {value}\r").as_bytes())?;
                    }
                    _ => return Err("not implemented".to_owned()),
                }
                Value::Null
            }
            "keymap.list" => json!(self
                .config
                .keymap()
                .iter()
                .map(|b| json!({"trigger": b.trigger.to_string(), "action": b.action}))
                .collect::<Vec<_>>()),
            "menu.list" => json!(shell_menu(self.catalog().as_ref(), &self.config.keymap())
                .iter()
                .map(|m| json!({
                    "title": m.title,
                    "items": m.items.iter().map(|i| json!({"label": i.label, "action": i.action, "shortcut": i.shortcut})).collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>()),

            "settings.schema" => json!(otty_config::spec::all()
                .iter()
                .filter(|k| k.applies_here())
                .map(|k| {
                    let mut row = json!({
                        "key": k.key,
                        "type": match k.kind {
                            KeyKind::Bool => "bool",
                            KeyKind::Int => "int",
                            KeyKind::Float => "float",
                            KeyKind::Str => "string",
                            KeyKind::Color => "color",
                            KeyKind::Enum => "enum",
                            KeyKind::List => "list",
                            KeyKind::Repeated => "repeated",
                            KeyKind::Pairs => "pairs",
                            KeyKind::Rule => "rule",
                        },
                        "default": k.default.value().map(setting),
                        "doc": k.doc,
                        "group": k.section,
                    });
                    if !k.enum_values.is_empty() {
                        row["values"] = json!(k.enum_values);
                    }
                    row
                })
                .collect::<Vec<_>>()),
            "settings.get" => {
                let key = text(p, "key")?;
                otty_config::spec::lookup(key).ok_or_else(|| format!("unknown key `{key}`"))?;
                json!({
                    "value": self.config.get(key).map(setting),
                    "origin": self.config.key_origin(key).map(|path| path.display().to_string()),
                })
            }
            "settings.set" => {
                let key = text(p, "key")?;
                let raw = match &p["value"] {
                    Value::String(s) => s.clone(),
                    Value::Null => return Err("missing `value`".to_owned()),
                    other => other.to_string(),
                };
                let scope = match p["scope"].as_str().unwrap_or("auto") {
                    "auto" => SaveScope::Auto,
                    "local" => SaveScope::Local,
                    "shared" => SaveScope::Global,
                    other => return Err(format!("scope `{other}` is not auto, local or shared")),
                };
                self.config.set_value(key, &raw, scope).map_err(|e| e.to_string())?;
                self.config_changed(io, key);
                Value::Null
            }
            "settings.reset" => {
                let key = text(p, "key")?;
                self.config.unset(key).map_err(|e| e.to_string())?;
                self.config_changed(io, key);
                Value::Null
            }
            "theme.list" => {
                let dir = Config::themes_dir();
                json!(Theme::available(Some(&dir))
                    .iter()
                    .map(|name| json!({
                        "name": name,
                        "dark": Theme::load(name, Some(&dir)).is_ok_and(|t| t.mode == Mode::Dark),
                    }))
                    .collect::<Vec<_>>())
            }
            "theme.set" => {
                let name = slugify(text(p, "name")?);
                let theme = Theme::load(&name, Some(&Config::themes_dir())).map_err(|e| e.to_string())?;
                // Kept for the next launch when the file can be written; live either way.
                let _ = self.config.set_value("theme", &name, SaveScope::Auto);
                self.apply_theme(io, &theme);
                Value::Null
            }

            "details.info" => {
                let pane = self.pane(p)?;
                let agent = self.d.app.pane(pane).and_then(|pane| pane.agent.as_ref());
                json!({
                    "cwd": self.cwd(p)?,
                    "processes": io.processes(pane),
                    // Nothing in the workspace maps a pane to its listening ports yet.
                    "ports": [],
                    "agent": agent.map(|(name, state)| json!({"name": name, "state": state.as_str()})),
                })
            }
            "details.git" => {
                let cwd = self.cwd(p)?;
                let root = Some(&cwd).filter(|c| !c.is_empty()).and_then(|c| git(c, &["rev-parse", "--show-toplevel"]));
                let Some(root) = root else { return Ok(Value::Null) };
                // `## main...origin/main [ahead 1]`, then one `XY path` per change.
                let status = git(&cwd, &["status", "--porcelain", "--branch"]).unwrap_or_default();
                let mut lines = status.lines();
                let head = lines.next().unwrap_or_default().trim_start_matches("## ");
                let branch = head.trim_start_matches("No commits yet on ").split("...").next();
                json!({
                    "root": root.trim(),
                    "branch": branch,
                    "changes": lines
                        .filter_map(|l| Some(json!({"path": l.get(3..)?, "status": l.get(..2)?.trim()})))
                        .collect::<Vec<_>>(),
                })
            }
            "details.files" => {
                let dir = std::fs::read_dir(expand_path(text(p, "path")?)).map_err(|e| e.to_string())?;
                let mut rows: Vec<(bool, String)> = dir
                    .flatten()
                    .map(|e| (!e.path().is_dir(), e.file_name().to_string_lossy().into_owned()))
                    .collect();
                // Folders first, then by name.
                rows.sort();
                json!(rows.iter().map(|(file, name)| json!({"name": name, "dir": !file})).collect::<Vec<_>>())
            }
            "file.read" => {
                let path = expand_path(text(p, "path")?);
                let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
                if size > FILE_LIMIT {
                    return Err(format!("{path} is {size} bytes; the limit is {FILE_LIMIT}"));
                }
                match String::from_utf8(std::fs::read(&path).map_err(|e| e.to_string())?) {
                    Ok(body) => {
                        let ext = Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or_default();
                        let language = otty_ui::syntax::bundled_syntax_set(&bundled_resources())
                            .ok()
                            .and_then(|set| set.find_syntax_by_extension(ext))
                            .map(|syntax| syntax.name.clone());
                        json!({"kind": "text", "body": body, "language": language})
                    }
                    Err(_) => json!({"kind": "binary", "body": ""}),
                }
            }

            "composer.send" => {
                let pane = self.pane(p)?;
                self.d.app.reveal_pane(pane);
                self.d.last_sent = None;
                self.d.send_composer(text(p, "text")?, self.store.as_ref(), now_secs());
                let sent = self.d.last_sent.take();
                match (&sent, &self.store) {
                    (Some(sent), _) => write(io, pane, format!("{sent}\r").as_bytes())?,
                    // The agent is busy and there is nowhere to queue.
                    (None, None) => return Err("the prompt queue is unavailable".to_owned()),
                    (None, Some(_)) => {}
                }
                json!({"queued": sent.is_none()})
            }

            "i18n.strings" => {
                let language = self.config.ui_language().resolve();
                let mut strings = Map::new();
                // English underneath, so a key the translation lacks still has text.
                for file in ["en.json", language.file_name()] {
                    if let Some(root) = std::fs::read(bundled_resources().join(file))
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                    {
                        flatten("", &root, &mut strings);
                    }
                }
                json!({"language": language.code(), "strings": strings})
            }

            // OSC 133 marks reach the bridge as events with no grid line, so
            // there is nothing to build an outline from yet.
            "details.outline" => return Err("not implemented".to_owned()),
            _ => return Err(format!("unknown method `{method}`")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sessions that are only a list: what was started, what was written.
    #[derive(Default)]
    struct Fake {
        live: Vec<(PaneId, Option<String>)>,
        written: Vec<(PaneId, String)>,
        screen: Vec<String>,
        themes: Vec<String>,
        fonts: usize,
    }

    impl PaneIo for Fake {
        fn write(&mut self, pane: PaneId, bytes: &[u8]) -> bool {
            self.written.push((pane, String::from_utf8_lossy(bytes).into_owned()));
            self.live.iter().any(|(id, _)| *id == pane)
        }
        fn screen(&self, _: PaneId) -> Option<Vec<String>> {
            Some(self.screen.clone())
        }
        fn dimensions(&self, _: PaneId) -> Option<(u16, u16)> {
            Some((80, 24))
        }
        fn busy(&self, _: PaneId) -> Option<bool> {
            Some(false)
        }
        fn processes(&self, _: PaneId) -> Vec<String> {
            vec!["claude".to_owned()]
        }
        fn tty(&self, _: PaneId) -> Option<String> {
            None
        }
    }

    impl Panes for Fake {
        fn sync(&mut self, panes: &[(PaneId, Option<String>)], _: &Config) -> Vec<(PaneId, String)> {
            // Like the real one, a session keeps the cwd it was started in.
            self.live.retain(|(id, _)| panes.iter().any(|(pane, _)| pane == id));
            for pane in panes {
                if !self.live.iter().any(|(id, _)| *id == pane.0) {
                    self.live.push(pane.clone());
                }
            }
            Vec::new()
        }
        fn selected_text(&self, _: PaneId, from: (u16, u16), to: (u16, u16)) -> Option<String> {
            let row = self.screen.get(from.1 as usize)?;
            Some(row.chars().skip(from.0 as usize).take((to.0 - from.0) as usize + 1).collect())
        }
        fn history(&self, _: PaneId) -> Option<Vec<String>> {
            Some(self.screen.clone())
        }
        fn set_theme(&mut self, theme: &Theme) {
            self.themes.push(theme.slug.clone());
        }
        fn set_font(&mut self, _: &Config) {
            self.fonts += 1;
        }
    }

    /// A backend on an empty config directory, its first pane started in `/home`.
    fn backend() -> (tempfile::TempDir, Backend, Fake) {
        let dir = tempfile::tempdir().unwrap();
        let mut b = Backend::new(Config::load_from(dir.path()), Some("/home".to_owned()));
        let mut io = Fake::default();
        b.settle(&mut io);
        (dir, b, io)
    }

    fn types(b: &mut Backend) -> Vec<String> {
        b.out.drain(..).map(|e| e["type"].as_str().unwrap().to_owned()).collect()
    }

    #[test]
    fn the_snapshot_after_a_new_tab_and_a_split_is_the_protocol_s_shape() {
        let (_dir, mut b, mut io) = backend();
        let made = b.request(&mut io, "tab.new", &json!({"cwd": "/work"})).unwrap();
        assert_eq!(made, json!({"tab": "tab-2", "pane": "pane-2"}));
        let split = b.request(&mut io, "pane.split", &json!({"direction": "right"})).unwrap();
        assert_eq!(split, json!({"pane": "pane-3"}));
        b.refresh_icons(&io);

        let state = b.state(&io);
        let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&state), ["activeTab", "dividers", "mode", "panes", "tabGroup", "tabs", "type"]);
        assert_eq!(state["activeTab"], "tab-2");
        assert_eq!((&state["tabGroup"], &state["dividers"], &state["mode"]), (&json!("none"), &json!([]), &json!("shell")));
        assert_eq!(state["tabs"][0]["tree"], json!({"pane": "pane-1"}));
        let mut tab = state["tabs"][1].clone();
        assert!(tab["project"].is_string());
        tab["project"] = json!("");
        assert_eq!(
            tab,
            json!({
                "id": "tab-2", "title": "Otty", "cwd": "/work", "project": "", "icon": "claude",
                "badge": null, "focused": "pane-3", "zoomed": null,
                "tree": {"axis": "horizontal", "ratio": 50,
                         "first": {"pane": "pane-2"}, "second": {"pane": "pane-3"}},
            })
        );
        // The split inherited the cwd of the pane it was made from.
        assert_eq!(
            state["panes"]["pane-3"],
            json!({"kind": "terminal", "title": "Otty", "cwd": "/work", "agent": null,
                   "grid": {"cols": 80, "rows": 24}})
        );
        // And every pane got a session, started where the model says.
        assert_eq!(io.live.len(), 3);
        assert_eq!(io.live[1], (PaneId::new(2), Some("/work".to_owned())));

        b.request(&mut io, "pane.ratio", &json!({"pane": "pane-2", "ratio": 30})).unwrap();
        b.request(&mut io, "pane.zoom", &json!({"pane": "pane-2"})).unwrap();
        let state = b.state(&io);
        assert_eq!(state["tabs"][1]["tree"]["ratio"], 30);
        assert_eq!(state["tabs"][1]["zoomed"], "pane-2");
        assert_eq!(b.visible(), [PaneId::new(2)]);
    }

    #[test]
    fn an_unknown_method_and_an_unbuilt_one_are_errors_not_crashes() {
        let (_dir, mut b, mut io) = backend();
        assert!(b.request(&mut io, "no.such", &json!({})).unwrap_err().contains("no.such"));
        assert_eq!(b.request(&mut io, "details.outline", &json!({})).unwrap_err(), "not implemented");
        assert!(b.request(&mut io, "pane.focus", &json!({"pane": "pane-9"})).is_err());
        assert!(b.request(&mut io, "tab.close", &Value::Null).is_err(), "params may be missing");
        assert_eq!(b.request(&mut io, "action.run", &json!({"id": "nope"})).unwrap(), json!({"handled": false}));
    }

    #[test]
    fn the_palette_filters_and_the_keymap_and_menu_are_listed() {
        let (_dir, mut b, mut io) = backend();
        let all = b.request(&mut io, "palette.list", &json!({"query": ""})).unwrap();
        assert_eq!(all.as_array().unwrap().len(), otty_config::actions::ACTIONS.len());
        let hits = b.request(&mut io, "palette.list", &json!({"query": "new_tab"})).unwrap();
        let hits = hits.as_array().unwrap();
        assert!(hits.len() < 10 && hits.iter().all(|h| h["id"].as_str().unwrap().contains("new_tab")));
        let row = hits.iter().find(|h| h["id"] == "new_tab").unwrap();
        assert_eq!((&row["scope"], &row["shortcut"]), (&json!("window"), &json!("cmd+t")));
        assert!(row["title"].is_string());

        let keys = b.request(&mut io, "keymap.list", &json!({})).unwrap();
        assert!(keys.as_array().unwrap().contains(&json!({"trigger": "cmd+t", "action": "new_tab"})));
        let menus = b.request(&mut io, "menu.list", &json!({})).unwrap();
        assert_eq!(menus[0]["title"], "File");
        assert!(menus[0]["items"].as_array().unwrap().iter().any(|i| i["action"] == "new_tab"));
    }

    #[test]
    fn a_setting_is_written_read_back_with_its_origin_and_reset() {
        let (dir, mut b, mut io) = backend();
        let schema = b.request(&mut io, "settings.schema", &json!({})).unwrap();
        let font = schema.as_array().unwrap().iter().find(|k| k["key"] == "font-size").unwrap();
        assert_eq!((&font["type"], &font["default"]), (&json!("float"), &json!(13.0)));
        let get = |b: &mut Backend, io: &mut Fake| b.request(io, "settings.get", &json!({"key": "font-size"})).unwrap();
        assert_eq!(get(&mut b, &mut io), json!({"value": 13.0, "origin": null}));

        let set = json!({"key": "font-size", "value": 18, "scope": "auto"});
        assert_eq!(b.request(&mut io, "settings.set", &set).unwrap(), Value::Null);
        assert_eq!(b.out, [json!({"type": "config", "key": "font-size"})]);
        assert_eq!(io.fonts, 1, "the panes were re-measured");
        let got = get(&mut b, &mut io);
        assert_eq!(got["value"], 18.0);
        assert_eq!(got["origin"], dir.path().join("config.toml").display().to_string());
        assert!(std::fs::read_to_string(dir.path().join("config.toml")).unwrap().contains("font-size = 18"));

        assert!(b.request(&mut io, "settings.set", &json!({"key": "no-such-key", "value": 1})).is_err());
        b.request(&mut io, "settings.reset", &json!({"key": "font-size"})).unwrap();
        assert_eq!(get(&mut b, &mut io), json!({"value": 13.0, "origin": null}));

        // A theme is a setting with a live half: the panes and the chrome.
        b.out.clear();
        let themes = b.request(&mut io, "theme.list", &json!({})).unwrap();
        let name = themes[0]["name"].as_str().unwrap();
        assert!(themes[0]["dark"].is_boolean());
        b.request(&mut io, "theme.set", &json!({"name": name})).unwrap();
        assert_eq!(io.themes, [name]);
        assert_eq!(types(&mut b), ["chrome"]);
        assert!(b.request(&mut io, "theme.set", &json!({"name": "no-such-theme"})).is_err());
    }

    #[test]
    fn a_closed_tab_ends_its_sessions_and_reopens_in_its_cwd() {
        let (_dir, mut b, mut io) = backend();
        b.request(&mut io, "tab.new", &json!({"cwd": "/work"})).unwrap();
        b.request(&mut io, "pane.split", &json!({"direction": "down"})).unwrap();
        b.request(&mut io, "tab.rename", &json!({"tab": "tab-2", "title": "api"})).unwrap();
        b.request(&mut io, "tab.activate", &json!({"tab": "tab-1"})).unwrap();
        b.request(&mut io, "tab.move", &json!({"tab": "tab-2", "target": "tab-1", "after": false})).unwrap();
        let state = b.state(&io);
        assert_eq!((&state["tabs"][0]["id"], &state["tabs"][0]["title"]), (&json!("tab-2"), &json!("api")));
        assert_eq!(state["activeTab"], "tab-1", "a drag does not change the active tab");

        b.request(&mut io, "tab.close", &json!({"tab": "tab-2"})).unwrap();
        assert_eq!(io.live, [(PaneId::new(1), Some("/home".to_owned()))]);
        assert_eq!(b.state(&io)["activeTab"], "tab-1");

        let back = b.request(&mut io, "tab.reopen", &json!({})).unwrap();
        assert_eq!(back, json!({"tab": "tab-3", "pane": "pane-4"}));
        assert_eq!(b.state(&io)["panes"]["pane-4"]["cwd"], "/work");
        assert_eq!(b.request(&mut io, "tab.reopen", &json!({})).unwrap(), Value::Null);

        // Closing the last pane of the last tab closes the window.
        b.request(&mut io, "pane.close", &json!({"pane": "pane-4"})).unwrap();
        assert!(!b.quit);
        b.request(&mut io, "action.run", &json!({"id": "close_tab"})).unwrap();
        assert!(b.quit && io.live.is_empty());
    }

    #[test]
    fn a_second_window_folds_into_a_tab_at_the_end() {
        let (_dir, mut b, mut io) = backend();
        b.request(&mut io, "tab.new", &json!({})).unwrap();
        b.request(&mut io, "tab.activate", &json!({"tab": "tab-1"})).unwrap();
        b.request(&mut io, "action.run", &json!({"id": "new_window"})).unwrap();
        let state = b.state(&io);
        let ids: Vec<_> = state["tabs"].as_array().unwrap().iter().map(|t| t["id"].clone()).collect();
        assert_eq!(ids, ["tab-1", "tab-2", "tab-3"]);
        assert_eq!(state["activeTab"], "tab-3");
        assert_eq!(b.d.app.counts().0, 1);
    }

    #[test]
    fn text_selection_context_and_find_read_the_pane() {
        let (_dir, mut b, mut io) = backend();
        io.screen = vec!["$ cat /etc/hosts".into(), "foo bar foo".into(), "".into(), "foo".into(), "".into()];
        let rows = b.request(&mut io, "pane.text", &json!({})).unwrap();
        assert_eq!(rows["rows"].as_array().unwrap().len(), 5);
        let rows = b.request(&mut io, "pane.text", &json!({"lines": 2})).unwrap();
        assert_eq!(rows, json!({"rows": ["", "foo"]}));

        let sel = b.request(&mut io, "pane.select", &json!({"from": [4, 1], "to": [6, 1]})).unwrap();
        assert_eq!(sel, json!({"text": "bar"}));
        b.request(&mut io, "action.run", &json!({"id": "copy"})).unwrap();
        assert_eq!(b.out, [json!({"type": "clipboard", "text": "bar"})]);

        let menu = b.request(&mut io, "pane.context", &json!({"col": 8, "row": 0, "selection": "bar"})).unwrap();
        assert_eq!(menu, json!([{"label": "Copy", "action": "copy"}, {"label": "Copy Path", "action": "copy_path"}]));

        let mut find = |params: Value| b.request(&mut io, "find.next", &params).unwrap();
        assert_eq!(find(json!({"query": "foo"})), json!({"line": 1, "col": 0}));
        assert_eq!(find(json!({"query": "foo", "from": {"line": 1, "col": 0}})), json!({"line": 1, "col": 8}));
        assert_eq!(find(json!({"query": "foo", "from": {"line": 3, "col": 0}})), json!({"line": 1, "col": 0}), "wraps");
        let back = json!({"query": "foo", "from": {"line": 1, "col": 0}, "backwards": true});
        assert_eq!(find(back), json!({"line": 3, "col": 0}), "wraps backwards");
        assert_eq!(find(json!({"query": "bar", "from": {"line": 1, "col": 4}})), json!({"line": 1, "col": 4}));
        assert_eq!(find(json!({"query": "nope"})), Value::Null);
    }

    #[test]
    fn shell_events_reach_the_model_and_the_frontend() {
        let (_dir, mut b, _io) = backend();
        let pane = PaneId::new(1);
        b.on_event(pane, Event::Title("zsh".into()));
        b.on_event(pane, Event::WorkingDirectory("file://host/C:/My%20Work".into()));
        b.on_event(pane, Event::Progress(Progress::Set(40)));
        b.on_event(pane, Event::Bell);
        b.on_event(pane, Event::ClipboardStore { selection: 'c', data: b"hi".to_vec() });
        let state = b.state(&Fake::default());
        assert_eq!(state["tabs"][0]["title"], "zsh");
        assert_eq!(state["tabs"][0]["cwd"], "C:/My Work");
        assert_eq!(state["tabs"][0]["badge"], json!({"progress": 40}));
        assert_eq!(b.out, [json!({"type": "bell", "pane": "pane-1"}), json!({"type": "clipboard", "text": "hi"})]);
        assert_eq!(osc7_path("file:///tmp/a%2Fb"), "/tmp/a/b");
        assert_eq!(osc7_path("/plain"), "/plain");
    }

    #[test]
    fn composer_text_and_an_ssh_row_are_typed_into_the_right_pane() {
        let (_dir, mut b, mut io) = backend();
        let sent = b.request(&mut io, "composer.send", &json!({"text": "hello"})).unwrap();
        assert_eq!(sent, json!({"queued": false}));
        assert_eq!(io.written, [(PaneId::new(1), "hello\r".to_owned())]);

        b.request(&mut io, "quickly.open", &json!({"kind": "ssh", "value": "web1"})).unwrap();
        assert_eq!(io.written[1], (PaneId::new(2), "ssh web1\r".to_owned()));
        let bad = json!({"kind": "ssh", "value": "x; rm -rf ~"});
        assert!(b.request(&mut io, "quickly.open", &bad).is_err());
        assert_eq!(io.written.len(), 2);

        // A folder row opens a tab there, and shows up under Opened afterwards.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        b.request(&mut io, "quickly.open", &json!({"kind": "folder", "value": path})).unwrap();
        let rows = b.request(&mut io, "quickly.query", &json!({"filter": "opened", "query": ""})).unwrap();
        assert!(rows.as_array().unwrap().iter().any(|r| r["value"] == path.as_str() && r["kind"] == "opened"));
        assert_eq!(b.request(&mut io, "quickly.query", &json!({"filter": "bogus", "query": ""})).unwrap(), json!([]));

        std::fs::write(dir.path().join("a.rs"), "fn main() {}\n").unwrap();
        std::fs::create_dir(dir.path().join("z")).unwrap();
        let files = b.request(&mut io, "details.files", &json!({"path": path})).unwrap();
        assert_eq!(files, json!([{"name": "z", "dir": true}, {"name": "a.rs", "dir": false}]));
        let file = dir.path().join("a.rs").display().to_string();
        let read = b.request(&mut io, "file.read", &json!({"path": file})).unwrap();
        assert_eq!((&read["kind"], &read["body"]), (&json!("text"), &json!("fn main() {}\n")));
    }
}
