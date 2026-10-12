//! What a frontend request *means*, with no stdio, threads or PTY in sight.
//!
//! `main.rs` owns the pipes and the live sessions; this module owns the model
//! and answers the methods of `frontend/electron/PROTOCOL.md` against it,
//! reaching the sessions only through [`Panes`]. That is what lets a test
//! drive it.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use otty_config::theme::{slugify, Mode, Theme};
use otty_config::{bundled_resources, Catalog, Config, KeyKind, SaveScope, Value as Setting};
use otty_core::{expand_path, AgentId, PaneId, TabId, WindowId};
use otty_render::theme::Chrome;
use otty_state::Store;
use otty_term::{find_next, Event, FindHit, Progress};
use otty_ui::ipc::PaneIo;
use otty_ui::mode::InputMode;
use otty_ui::quickly::now_secs;
use otty_ui::recipe::{self, Place, Recipe, Vars};
use otty_ui::tabs::{git_root, TabGroupMode};
use otty_ui::{
    context_rows, filter_palette, palette_entries, pane_hit, query_quickly, shell_menu, Axis,
    Badge, Dispatcher, Node, OpenFilter, Pane, PaneKind, QuicklySources, SplitDirection, Tab,
};
use serde_json::{json, Map, Value};

/// How many closed tabs `tab.reopen` remembers.
const REOPEN_DEPTH: usize = 12;
/// How many agent conversations Open Quickly offers.
const TRANSCRIPTS: usize = 50;
/// How many of a pane's latest commands a recipe saved with its commands keeps.
const RECENT_COMMANDS: usize = 10;
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
    /// The commands OSC 133 marked, as `(grid line, text)`, oldest first.
    fn outline(&self, pane: PaneId) -> Option<Vec<(usize, String)>>;
    /// The TCP ports the pane's processes are listening on.
    fn ports(&self, pane: PaneId) -> Vec<u16>;
    /// Move the viewport: grid line `line` to the top when given, else `delta`
    /// lines back into history.
    fn scroll(&mut self, pane: PaneId, line: Option<usize>, delta: i32);
    /// Type key text (`\n` is Enter) into a pane, once its shell is ready for it.
    fn type_keys(&mut self, pane: PaneId, text: &str);
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
    /// The app is done: `quit` ran, or the last window closed.
    pub quit: bool,
    /// Where on screen a torn-off tab's window should open.
    pub placed: HashMap<WindowId, (i64, i64)>,
    /// Cwds of closed tabs, newest last.
    closed: Vec<Option<String>>,
    /// The tabs as of the last `settle`, so a tab that vanished can be reopened.
    tabs: Vec<(TabId, Option<String>)>,
    /// The focused pane's cwd as of the last `settle`; a new pane starts there.
    last_cwd: Option<String>,
    icons: HashMap<PaneId, &'static str>,
    /// Claude Code's config directory, whose transcripts Open Quickly lists.
    /// `None` in tests.
    pub agent_home: Option<PathBuf>,
    /// The library saved recipes live in.
    pub recipes: PathBuf,
    /// Where `trusted_recipes.json` lives.
    pub support: PathBuf,
    /// Recipe commands a pane has not been given yet, and its replay mode.
    replay: HashMap<PaneId, (VecDeque<String>, String)>,
    /// Panes whose command finished since the last `settle`.
    finished: Vec<PaneId>,
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
            placed: HashMap::new(),
            closed: Vec::new(),
            tabs: Vec::new(),
            last_cwd: None,
            icons: HashMap::new(),
            projects: HashMap::new(),
            agent_home: None,
            recipes: recipe::library_dir(),
            support: otty_core::app_support_dir(),
            replay: HashMap::new(),
            finished: Vec::new(),
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

    /// Every tab of every window, with the window it is in.
    fn all_tabs(&self) -> impl Iterator<Item = (WindowId, &Tab)> {
        self.d.app.windows().iter().flat_map(|w| w.tabs().iter().map(move |t| (w.id, t)))
    }

    pub fn windows(&self) -> Vec<WindowId> {
        self.d.app.windows().iter().map(|w| w.id).collect()
    }

    /// The panes the frontend is showing, and where: each window's active
    /// tab's, or its zoomed one.
    pub fn visible(&self) -> Vec<(WindowId, PaneId)> {
        let shown = |t: &Tab| t.zoomed.map_or_else(|| t.tree.panes(), |z| vec![z]);
        let windows = self.d.app.windows().iter();
        windows.flat_map(|w| w.active_tab().map(shown).unwrap_or_default().into_iter().map(|p| (w.id, p))).collect()
    }

    /// The user closed a window: its tabs and their sessions go with it.
    pub fn close_window(&mut self, window: &str) {
        if let Ok(id) = window.parse() {
            self.d.app.close_window(id);
        }
    }

    /// The active window's snapshot.
    #[cfg(test)]
    pub fn state<P: Panes>(&mut self, io: &P) -> Value {
        let active = self.d.app.active_window().map_or(WindowId::new(0), |w| w.id);
        self.state_of(io, active)
    }

    /// One window's full snapshot. Cheap enough to build every tick and compare.
    pub fn state_of<P: Panes>(&mut self, io: &P, id: WindowId) -> Value {
        let Self { d, projects, icons, .. } = self;
        let app = &d.app;
        let window = app.windows().iter().find(|w| w.id == id);
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
        // ponytail: the tab grouping, the dividers and the input mode are one
        // per app in `Dispatcher`, so every window reports the same; move them
        // onto `Window` when two windows need to differ.
        json!({
            "type": "state",
            "window": id.slug(),
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
            Event::CommandFinished { .. } => self.finished.push(pane),
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
        // A window whose last tab closed or moved away is gone.
        let active = self.d.app.active_window().map(|w| w.id);
        for id in self.windows() {
            if self.d.app.windows().iter().any(|w| w.id == id && w.tabs().is_empty()) {
                self.d.app.close_window(id);
            }
        }
        if let Some(id) = active {
            self.d.app.focus_window(id);
        }
        let windows = self.windows();
        self.placed.retain(|id, _| windows.contains(id));

        // A new pane starts where the user was.
        for id in self.d.app.all_panes() {
            if let Some(p) = self.d.app.pane_mut(id).filter(|p| p.cwd.is_none()) {
                p.cwd.clone_from(&self.last_cwd);
            }
        }
        let app = &self.d.app;
        let cwd_of = |id: PaneId| app.pane(id).and_then(|p| p.cwd.clone());
        self.last_cwd = self.d.focused_pane_id().and_then(cwd_of);

        let tabs: Vec<_> = self.all_tabs().map(|(_, t)| (t.id, cwd_of(t.focused))).collect();
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
        // A pane back at its prompt gets the next of its recipe's commands.
        self.replay.retain(|pane, _| live.iter().any(|(id, _)| id == pane));
        for pane in std::mem::take(&mut self.finished) {
            self.feed(io, pane);
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
        self.quit |= self.d.quit || windows.is_empty();
    }

    /// Answer one `request` from the active window. `Err` is the `error` of an
    /// `ok:false` response.
    pub fn request<P: Panes>(&mut self, io: &mut P, method: &str, params: &Value) -> Answer {
        let answer = self.run(io, method, params);
        self.settle(io);
        answer
    }

    /// [`Backend::request`] from `window`, which becomes the active one.
    pub fn request_in<P: Panes>(&mut self, io: &mut P, window: Option<&str>, method: &str, params: &Value) -> Answer {
        if let Some(id) = window.and_then(|w| w.parse().ok()) {
            self.d.app.focus_window(id);
        }
        self.request(io, method, params)
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

    /// The tab named by `p[key]`, in any window, as `(id, focused pane)`.
    fn tab(&self, p: &Value, key: &str) -> Result<(TabId, PaneId), String> {
        let s = text(p, key)?;
        let id = s.parse::<TabId>().ok();
        self.all_tabs()
            .find(|(_, t)| Some(t.id) == id)
            .map(|(_, t)| (t.id, t.focused))
            .ok_or_else(|| format!("no tab `{s}`"))
    }

    fn transcripts(&self) -> Vec<otty_agent::Transcript> {
        self.agent_home.as_deref().map(|home| otty_agent::claude_transcripts(home, TRANSCRIPTS)).unwrap_or_default()
    }

    /// The command that resumes — or forks — `session` of `agent`.
    fn agent_command(&self, agent: &AgentId, session: &str, fork: bool) -> Result<String, String> {
        // The id is typed into a shell, so it must look like one.
        if session.is_empty() || !session.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
            return Err(format!("`{session}` is not a session id"));
        }
        let overrides: BTreeMap<String, String> = self.config.get_pairs("agent-command").into_iter().collect();
        let command = match fork {
            true => otty_agent::fork_command(agent, session, &overrides),
            false => otty_agent::resume_command(agent, session, &overrides),
        };
        command.ok_or_else(|| format!("{agent} cannot {} a conversation", if fork { "fork" } else { "resume" }))
    }

    /// Run `command` in a new pane in `cwd`: a split of the focused pane
    /// (`split_right`, `split-down`, …), a new window, or else a new tab.
    fn run_in<P: Panes>(&mut self, io: &mut P, place: &str, cwd: Option<String>, command: &str) -> Answer {
        let before = self.d.focused_pane_id();
        let place = place.replace('-', "_");
        let split = place.strip_prefix("split_").and_then(SplitDirection::parse);
        self.d.activate(match (split, place.as_str()) {
            (Some(_), _) => &place,
            (None, "new_window") => "new_window",
            _ => "new_tab",
        });
        let pane = self.d.focused_pane_id().filter(|p| Some(*p) != before).ok_or("no pane to run it in")?;
        if let (Some(p), Some(_)) = (self.d.app.pane_mut(pane), &cwd) {
            p.cwd = cwd;
        }
        // The pane gets its session here, so the command has somewhere to go.
        self.settle(io);
        io.type_keys(pane, &format!("{command}\n"));
        Ok(json!({"pane": pane.slug()}))
    }

    /// Fork the focused pane's agent conversation into a new pane at `place`.
    // ponytail: "the pane's conversation" is the newest transcript for the
    // pane's directory. The hooks report the real session id; use that once
    // the bridge keeps it.
    fn fork_here<P: Panes>(&mut self, io: &mut P, place: &str) -> Answer {
        let cwd = self.cwd(&Value::Null)?;
        let slashes = |path: &str| path.replace('\\', "/").to_lowercase();
        let found = self
            .transcripts()
            .into_iter()
            .find(|t| t.cwd.as_deref().is_some_and(|dir| slashes(dir) == slashes(&cwd)))
            .ok_or("no agent conversation was found for this pane's directory")?;
        let command = self.agent_command(&found.agent, &found.session, true)?;
        self.run_in(io, place, Some(cwd), &command)
    }

    /// What a recipe's `{{…}}` paths stand for, opened from `file` (if any) now.
    fn vars(&self, file: Option<&Path>) -> Vars {
        let cwd = self.d.focused_pane_id().and_then(|id| self.d.app.pane(id)?.cwd.clone());
        Vars {
            current_folder: cwd.unwrap_or_default(),
            home: expand_path("~"),
            recipe_location: file.and_then(Path::parent).map(|dir| dir.display().to_string()).unwrap_or_default(),
        }
    }

    /// Hand `pane` the next of its recipe commands, as far as its mode allows:
    /// `auto` runs them, `ask_once` leaves them at the prompt for one Enter,
    /// `manually` leaves one. A command that hands the terminal to another
    /// shell ends the batch; the rest wait for the pane's next prompt.
    // ponytail: "next prompt" is OSC 133 D, so without shell integration what
    // follows a hand-off (or, in `manually`, the first command) never arrives.
    fn feed<P: Panes>(&mut self, io: &mut P, pane: PaneId) {
        let Some((queue, mode)) = self.replay.get_mut(&pane) else { return };
        let mut batch = Vec::new();
        while let Some(command) = queue.pop_front() {
            let last = *mode == "manually" || recipe::hands_off(&command);
            batch.push(command);
            if last {
                break;
            }
        }
        let keys = match mode.as_str() {
            "auto" => batch.iter().map(|command| format!("{command}\n")).collect(),
            _ => batch.join("; "),
        };
        if queue.is_empty() {
            self.replay.remove(&pane);
        }
        io.type_keys(pane, &keys);
    }

    /// Open the recipe at `path`. One that is neither from the library nor
    /// trusted, and would run commands, is not opened: the asking window gets
    /// a `recipe-trust` event listing them, and asks again with `trust`.
    fn open_recipe<P: Panes>(&mut self, io: &mut P, path: &Path, trust: Option<&str>) -> Answer {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let found = recipe::parse(&bytes)?;
        let saved = path.parent() == Some(self.recipes.as_path());
        let commands = recipe::commands_of(&found);
        let store = self.support.join("trusted_recipes.json");
        if !saved && !commands.is_empty() && !recipe::is_trusted(&store, &bytes) {
            match trust {
                Some("always") => recipe::trust(&store, &bytes).map_err(|e| e.to_string())?,
                Some("once") => {}
                _ => {
                    self.out.push(json!({
                        "type": "recipe-trust",
                        "window": self.d.app.active_window().map(|w| w.id.slug()),
                        "path": path.display().to_string(),
                        "name": found.recipe.name,
                        "commands": commands
                            .iter()
                            .map(|(tab, pane, command)| json!({"tab": tab, "pane": pane, "command": command}))
                            .collect::<Vec<_>>(),
                    }));
                    return Ok(json!({"opened": false}));
                }
            }
        }
        let (key, default) = if saved { ("recipe-replay-saved", "ask_once") } else { ("recipe-replay-file", "manually") };
        let mode = self.config.get_str(key).unwrap_or_else(|| default.to_owned());
        let vars = self.vars(Some(path));
        let window = found.window.unwrap_or_default();
        let made = match found.recipe.scope.as_str() {
            "commands" => vec![(self.pane(&Value::Null)?, found.recipe.commands)],
            "tab" => recipe::apply(&mut self.d, &window, Place::NewTabs, &vars),
            _ => recipe::apply(&mut self.d, &window, Place::NewWindow, &vars),
        };
        // The new panes get their sessions here, so the commands have somewhere to go.
        self.settle(io);
        for (pane, commands) in made {
            if !commands.is_empty() {
                self.replay.insert(pane, (commands.into(), mode.clone()));
                self.feed(io, pane);
            }
        }
        Ok(json!({"opened": true}))
    }

    /// Every window as a recipe document: what [`Backend::restore`] reads back.
    pub fn session(&self) -> String {
        recipe::render(&recipe::capture_session(&self.d.app, self.closed.iter().flatten().cloned().collect()))
    }

    /// Rebuild the windows of a [`Backend::session`] document, the first over
    /// the fresh window this backend started with.
    pub fn restore<P: Panes>(&mut self, io: &mut P, session: &str) -> bool {
        let Ok(session) = recipe::parse(session.as_bytes()) else { return false };
        let vars = self.vars(None);
        let windows = session.windows.iter().filter(|w| w.tabs.iter().any(|t| !t.panes.is_empty()));
        for (n, window) in windows.enumerate() {
            recipe::apply(&mut self.d, window, if n == 0 { Place::Here } else { Place::NewWindow }, &vars);
        }
        if let Some(first) = self.windows().first() {
            self.d.app.focus_window(*first);
        }
        self.closed = session.recipe.closed.into_iter().map(Some).collect();
        self.settle(io);
        true
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
                let id = text(p, "id")?;
                // Fork / Branch in a split, a new tab or a new window.
                if let Some(place) = id.strip_prefix("agent_fork_") {
                    self.fork_here(io, place)?;
                    return Ok(json!({"handled": true}));
                }
                json!({"handled": self.d.activate(id)})
            }

            "tab.new" => self.new_tab(p["cwd"].as_str().map(str::to_owned))?,
            "tab.activate" => {
                self.d.app.reveal_pane(self.tab(p, "tab")?.1);
                Value::Null
            }
            "tab.close" => {
                let (id, pane) = self.tab(p, "tab")?;
                // The tab may be another window's; the asking one stays active.
                let asking = self.d.app.active_window().map(|w| w.id);
                self.d.app.reveal_pane(pane);
                self.d.app.active_window_mut().and_then(|w| w.take_tab(id));
                asking.map(|window| self.d.app.focus_window(window));
                Value::Null
            }
            "tab.move" => {
                let (id, pane) = self.tab(p, "tab")?;
                let here = self.d.app.active_window().map(|w| w.id).ok_or("no window")?;
                let local = self.d.app.window_of(pane) == Some(here);
                let active = self.d.focused_pane_id();
                let mut rest = self.tab_list().iter().filter(|t| t.id != id).map(|t| t.id);
                // No target: the end of this window.
                let at = match p["target"].as_str() {
                    Some(_) => {
                        let (target, _) = self.tab(p, "target")?;
                        let after = p["after"].as_bool().unwrap_or(false) as usize;
                        rest.position(|t| t == target).ok_or("the target tab is in another window")? + after
                    }
                    None => rest.count(),
                };
                self.d.app.move_tab_to_window(id, here, at);
                // A tab dragged in from another window arrives active; one
                // dragged along its own strip leaves the active tab alone.
                if let (true, Some(pane)) = (local, active) {
                    self.d.app.reveal_pane(pane);
                }
                Value::Null
            }
            "tab.detach" => {
                let (_, pane) = self.tab(p, "tab")?;
                let home = self.d.app.window_of(pane);
                // A window's only tab is already a window of its own.
                if self.all_tabs().filter(|(window, _)| Some(*window) == home).count() < 2 {
                    return Ok(Value::Null);
                }
                self.d.app.reveal_pane(pane);
                self.d.activate("move_tab_to_new_window");
                let window = self.d.app.window_of(pane).filter(|w| Some(*w) != home).ok_or("tab could not be moved")?;
                if let (Some(x), Some(y)) = (p["x"].as_i64(), p["y"].as_i64()) {
                    self.placed.insert(window, (x, y));
                }
                json!({"window": window.slug()})
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
            "pane.scroll" => {
                let pane = self.pane(p)?;
                let line = p["line"].as_u64().map(|line| line as usize);
                let delta = p["delta"].as_i64().unwrap_or(0) as i32;
                if line.is_none() && delta == 0 {
                    return Err("missing `line` or `delta`".to_owned());
                }
                io.scroll(pane, line, delta);
                Value::Null
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
                let mut sources = match self.store {
                    Some(_) => QuicklySources::live(opened),
                    None => QuicklySources { opened, ..Default::default() },
                };
                sources.recipes = recipe::list(&self.recipes).into_iter().map(|(name, ..)| name).collect();
                let filter = OpenFilter::from_key(p["filter"].as_str().unwrap_or("all"));
                let query = p["query"].as_str().unwrap_or_default();
                let rows = query_quickly(filter, query, &sources);
                let mut rows: Vec<Value> = rows
                    .iter()
                    .map(|e| {
                        let kind = e.kind.as_str();
                        let name = Path::new(&e.path).file_name().map(|n| n.to_string_lossy().into_owned());
                        match name.filter(|_| !matches!(kind, "ssh" | "agent" | "recipe")) {
                            Some(name) => json!({"title": name, "subtitle": e.path, "kind": kind, "value": e.path}),
                            None => json!({"title": e.title, "subtitle": "", "kind": kind, "value": e.path}),
                        }
                    })
                    .collect();
                // Agent conversations: titled by their first prompt, opened by session id.
                if matches!(filter, Some(OpenFilter::All | OpenFilter::Agents)) {
                    let query = query.trim().to_lowercase();
                    let hit = |text: &str| text.to_lowercase().contains(&query);
                    rows.extend(self.transcripts().iter().filter(|t| hit(&t.title) || t.cwd.as_deref().is_some_and(hit)).map(|t| {
                        json!({"title": t.title, "subtitle": t.cwd, "kind": "agent", "value": format!("{}:{}", t.agent, t.session)})
                    }));
                }
                json!(rows)
            }
            "quickly.open" => {
                let value = text(p, "value")?;
                match text(p, "kind")? {
                    "recipe" => {
                        self.open_recipe(io, &recipe::library_path(&self.recipes, value), None)?;
                    }
                    _ if value.ends_with(".ottyrecipe") => {
                        self.open_recipe(io, Path::new(&expand_path(value)), None)?;
                    }
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
                    // ↩ resumes the conversation where it ran; `fork` starts a second one from it.
                    "agent" => {
                        let (agent, session) = value.split_once(':').ok_or("an agent row is `agent:session`")?;
                        let target = p["target"].as_str().unwrap_or_default();
                        let command = self.agent_command(&AgentId::parse(agent), session, target == "fork")?;
                        let cwd = self.transcripts().into_iter().find(|t| t.session == session).and_then(|t| t.cwd);
                        self.run_in(io, target, cwd, &command)?;
                    }
                    "ssh" => {
                        // The host is typed into a shell, so it must look like one.
                        let ok = |c: char| c.is_ascii_alphanumeric() || "._-@:".contains(c);
                        if value.is_empty() || value.starts_with('-') || !value.chars().all(ok) {
                            return Err(format!("`{value}` is not a host name"));
                        }
                        self.run_in(io, "new_tab", None, &format!("ssh {value}"))?;
                    }
                    _ => return Err("not implemented".to_owned()),
                }
                Value::Null
            }
            "recipe.list" => json!(recipe::list(&self.recipes)
                .iter()
                .map(|(name, path, scope)| json!({"name": name, "path": path.display().to_string(), "scope": scope}))
                .collect::<Vec<_>>()),
            "recipe.open" => {
                let path = match (p["name"].as_str(), p["path"].as_str()) {
                    (Some(name), _) => recipe::library_path(&self.recipes, name),
                    (None, Some(path)) => PathBuf::from(expand_path(path)),
                    _ => return Err("missing `name` or `path`".to_owned()),
                };
                self.open_recipe(io, &path, p["trust"].as_str())?
            }
            "recipe.save" => {
                let name = text(p, "name")?.trim();
                if name.is_empty() {
                    return Err("a recipe needs a name".to_owned());
                }
                let scope = p["scope"].as_str().unwrap_or("window");
                // A pane's latest commands, oldest first, as OSC 133 marked them.
                let recent = |io: &P, pane: PaneId| -> Vec<String> {
                    let all = io.outline(pane).unwrap_or_default();
                    all[all.len().saturating_sub(RECENT_COMMANDS)..].iter().map(|(_, command)| command.clone()).collect()
                };
                let meta = recipe::Meta { name: name.to_owned(), version: 1, scope: scope.to_owned(), ..Default::default() };
                let mut saved = Recipe { recipe: meta, ..Default::default() };
                match scope {
                    "commands" => {
                        saved.recipe.commands = match p["commands"].as_array() {
                            Some(list) => list.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
                            None => recent(io, self.pane(&Value::Null)?),
                        };
                        saved.recipe.commands.retain(|command| !command.trim().is_empty());
                        if saved.recipe.commands.is_empty() {
                            return Err("there are no commands to save".to_owned());
                        }
                    }
                    "tab" | "window" => {
                        let app = &self.d.app;
                        let window = app.active_window().ok_or("no window")?;
                        let mut spec = match scope {
                            "tab" => recipe::WindowSpec { tabs: window.active_tab().map(|t| recipe::capture_tab(app, t)).into_iter().collect() },
                            _ => recipe::capture_window(app, window.id),
                        };
                        let vars = self.vars(None);
                        for pane in spec.tabs.iter_mut().flat_map(|tab| &mut tab.panes) {
                            if p["content"] == "commands" {
                                pane.commands = pane.pane.map(|id| recent(io, id)).unwrap_or_default();
                            }
                            if p["portable"].as_bool().unwrap_or(false) {
                                pane.cwd = pane.cwd.as_deref().map(|cwd| vars.portable(cwd));
                            }
                        }
                        saved.window = Some(spec);
                    }
                    other => return Err(format!("scope `{other}` is not tab, window or commands")),
                }
                // With a path it is an export; without, it goes to the library.
                let path = match p["path"].as_str() {
                    Some(path) => PathBuf::from(expand_path(path)),
                    None => recipe::library_path(&self.recipes, name),
                };
                recipe::write(&path, &saved).map_err(|e| e.to_string())?;
                json!({"path": path.display().to_string()})
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
                    "ports": io.ports(pane),
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

            "details.outline" => {
                let rows = io.outline(self.pane(p)?).ok_or("pane has no live session")?;
                json!(rows
                    .iter()
                    .map(|(line, title)| json!({"line": line, "title": title, "kind": "command"}))
                    .collect::<Vec<_>>())
            }
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
        scrolls: Vec<(Option<usize>, i32)>,
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
        fn outline(&self, _: PaneId) -> Option<Vec<(usize, String)>> {
            Some(vec![(0, "cat /etc/hosts".to_owned())])
        }
        fn ports(&self, _: PaneId) -> Vec<u16> {
            vec![3000]
        }
        fn scroll(&mut self, _: PaneId, line: Option<usize>, delta: i32) {
            self.scrolls.push((line, delta));
        }
        fn type_keys(&mut self, pane: PaneId, text: &str) {
            self.written.push((pane, text.to_owned()));
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
        b.recipes = dir.path().join("recipes");
        b.support = dir.path().join("support");
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
        assert_eq!(keys(&state), ["activeTab", "dividers", "mode", "panes", "tabGroup", "tabs", "type", "window"]);
        assert_eq!((&state["window"], &state["activeTab"]), (&json!("window-1"), &json!("tab-2")));
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
        assert_eq!(b.visible(), [(WindowId::new(1), PaneId::new(2))]);
    }

    #[test]
    fn an_unknown_method_and_an_unbuilt_one_are_errors_not_crashes() {
        let (_dir, mut b, mut io) = backend();
        assert!(b.request(&mut io, "no.such", &json!({})).unwrap_err().contains("no.such"));
        assert_eq!(b.request(&mut io, "recipe.nope", &json!({})).unwrap_err(), "unknown method `recipe.nope`");
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

    /// Each window's tab ids, in window order.
    fn tabs_by_window(b: &mut Backend, io: &Fake) -> Vec<(String, Vec<String>)> {
        b.windows()
            .into_iter()
            .map(|id| {
                let state = b.state_of(io, id);
                let tabs = state["tabs"].as_array().unwrap().iter().map(|t| t["id"].as_str().unwrap().to_owned());
                (state["window"].as_str().unwrap().to_owned(), tabs.collect())
            })
            .collect()
    }

    #[test]
    fn a_tab_moves_between_windows_with_its_sessions_and_an_empty_window_closes() {
        let (_dir, mut b, mut io) = backend();
        b.request(&mut io, "tab.new", &json!({"cwd": "/work"})).unwrap();
        b.request(&mut io, "pane.split", &json!({"direction": "right"})).unwrap();
        b.request(&mut io, "action.run", &json!({"id": "new_window"})).unwrap();
        let shape = |ws: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
            ws.iter().map(|(w, tabs)| (w.to_string(), tabs.iter().map(|t| t.to_string()).collect())).collect()
        };
        assert_eq!(tabs_by_window(&mut b, &io), shape(&[("window-1", &["tab-1", "tab-2"]), ("window-2", &["tab-3"])]));
        // Both windows are on screen, each showing its own active tab's panes.
        let shown = [(WindowId::new(1), PaneId::new(2)), (WindowId::new(1), PaneId::new(3)), (WindowId::new(2), PaneId::new(4))];
        assert_eq!(b.visible(), shown);
        let sessions = io.live.clone();

        // Dropped on window 2's tab: it lands after it, active, sessions untouched.
        let drop = json!({"tab": "tab-2", "target": "tab-3", "after": true});
        b.request_in(&mut io, Some("window-2"), "tab.move", &drop).unwrap();
        assert_eq!(tabs_by_window(&mut b, &io), shape(&[("window-1", &["tab-1"]), ("window-2", &["tab-3", "tab-2"])]));
        let state = b.state_of(&io, WindowId::new(2));
        assert_eq!((&state["activeTab"], &state["tabs"][1]["cwd"]), (&json!("tab-2"), &json!("/work")));
        assert_eq!(state["tabs"][1]["tree"]["first"], json!({"pane": "pane-2"}));
        assert_eq!(io.live, sessions, "no shell was restarted");

        // Dropped on window 1 with no target: the end of it. Window 2 keeps tab-3.
        b.request_in(&mut io, Some("window-1"), "tab.move", &json!({"tab": "tab-2"})).unwrap();
        assert_eq!(tabs_by_window(&mut b, &io), shape(&[("window-1", &["tab-1", "tab-2"]), ("window-2", &["tab-3"])]));
        let cross = json!({"tab": "tab-2", "target": "tab-3"});
        assert!(b.request_in(&mut io, Some("window-1"), "tab.move", &cross).is_err(), "a target must be in the asking window");

        // Torn off: a new window where it was dropped. A window's only tab stays put.
        let torn = b.request_in(&mut io, Some("window-1"), "tab.detach", &json!({"tab": "tab-2", "x": 40, "y": 60})).unwrap();
        assert_eq!(torn, json!({"window": "window-3"}));
        assert_eq!(b.placed.get(&WindowId::new(3)), Some(&(40, 60)));
        assert_eq!(b.request(&mut io, "tab.detach", &json!({"tab": "tab-2"})).unwrap(), Value::Null);
        assert_eq!(b.windows().len(), 3);

        // The last tab leaving a window closes it; closing a window ends its sessions.
        b.request_in(&mut io, Some("window-1"), "tab.move", &json!({"tab": "tab-3"})).unwrap();
        assert_eq!(tabs_by_window(&mut b, &io), shape(&[("window-1", &["tab-1", "tab-3"]), ("window-3", &["tab-2"])]));
        b.close_window("window-3");
        b.settle(&mut io);
        assert_eq!(io.live.iter().map(|(id, _)| id.get()).collect::<Vec<_>>(), [1, 4]);
        assert!(!b.quit);
        b.close_window("window-1");
        b.settle(&mut io);
        assert!(b.quit && io.live.is_empty());
    }

    #[test]
    fn a_window_is_saved_as_a_recipe_listed_and_opened_with_its_commands() {
        let (dir, mut b, mut io) = backend();
        b.request(&mut io, "pane.split", &json!({"direction": "down"})).unwrap();
        let save = json!({"name": "dev", "scope": "window", "content": "commands"});
        let saved = b.request(&mut io, "recipe.save", &save).unwrap();
        let path = dir.path().join("recipes").join("dev.ottyrecipe");
        assert_eq!(saved, json!({"path": path.display().to_string()}));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("split = \"down\"") && text.contains("cat /etc/hosts"), "{text}");
        assert_eq!(b.request(&mut io, "recipe.list", &json!({})).unwrap()[0]["name"], "dev");
        let rows = b.request(&mut io, "quickly.query", &json!({"filter": "recipes", "query": "de"})).unwrap();
        assert_eq!(rows, json!([{"title": "dev", "subtitle": "", "kind": "recipe", "value": "dev"}]));

        // From the library it is trusted, opens a window, and asks once per pane.
        let opened = b.request(&mut io, "quickly.open", &json!({"kind": "recipe", "value": "dev"})).unwrap();
        assert_eq!(opened, Value::Null);
        assert_eq!(b.windows().len(), 2);
        let state = b.state(&io);
        assert_eq!(state["tabs"][0]["tree"]["axis"], "vertical");
        assert_eq!(state["panes"]["pane-3"]["cwd"], "/home");
        let typed: Vec<_> = io.written.iter().map(|(pane, text)| (pane.get(), text.as_str())).collect();
        assert_eq!(typed, [(3, "cat /etc/hosts"), (4, "cat /etc/hosts")], "ask_once leaves them at the prompt");

        assert!(b.request(&mut io, "recipe.save", &json!({"name": " ", "scope": "tab"})).is_err());
        assert!(b.request(&mut io, "recipe.save", &json!({"name": "x", "scope": "galaxy"})).is_err());
        assert!(b.request(&mut io, "recipe.open", &json!({"name": "nope"})).is_err());
    }

    #[test]
    fn a_recipe_file_from_elsewhere_asks_before_it_runs_anything() {
        let (dir, mut b, mut io) = backend();
        let file = dir.path().join("team.ottyrecipe");
        let text = "[recipe]\nname = \"team\"\nversion = 1\nscope = \"tab\"\n\n[[window.tabs]]\n\n[[window.tabs.panes]]\n\
                    cwd = \"{{recipe_location}}\"\ncommands = [\"make setup\", \"ssh web1\", \"uptime\"]\n";
        std::fs::write(&file, text).unwrap();
        let open = |trust: Option<&str>| json!({"path": file.display().to_string(), "trust": trust});

        assert_eq!(b.request(&mut io, "recipe.open", &open(None)).unwrap(), json!({"opened": false}));
        assert_eq!(b.d.app.counts().1, 1, "nothing was opened");
        let asked = b.out.pop().unwrap();
        assert_eq!((&asked["type"], &asked["window"], &asked["name"]), (&json!("recipe-trust"), &json!("window-1"), &json!("team")));
        assert_eq!(asked["commands"][1], json!({"tab": 1, "pane": 1, "command": "ssh web1"}));

        // Run Once: opened, fed one command at a time (`recipe-replay-file` is `manually`).
        assert_eq!(b.request(&mut io, "recipe.open", &open(Some("once"))).unwrap(), json!({"opened": true}));
        let pane = PaneId::new(2);
        assert_eq!(b.state(&io)["panes"]["pane-2"]["cwd"], dir.path().display().to_string());
        assert_eq!(io.written, [(pane, "make setup".to_owned())]);
        b.on_event(pane, Event::CommandFinished { exit_code: Some(0) });
        b.settle(&mut io);
        b.on_event(pane, Event::CommandFinished { exit_code: Some(0) });
        b.settle(&mut io);
        b.on_event(pane, Event::CommandFinished { exit_code: Some(0) });
        b.settle(&mut io);
        let typed: Vec<_> = io.written.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(typed, ["make setup", "ssh web1", "uptime"]);
        assert_eq!(b.request(&mut io, "recipe.open", &open(None)).unwrap(), json!({"opened": false}), "once is once");

        // Always Trust: remembered by hash, and `auto` stops after the hand-off.
        b.request(&mut io, "settings.set", &json!({"key": "recipe-replay-file", "value": "auto"})).unwrap();
        io.written.clear();
        b.request(&mut io, "recipe.open", &open(Some("always"))).unwrap();
        assert_eq!(b.request(&mut io, "recipe.open", &open(None)).unwrap(), json!({"opened": true}), "trusted now");
        assert_eq!(io.written[0].1, "make setup\nssh web1\n", "what follows ssh waits for the prompt");
        std::fs::write(&file, format!("{text}# edited\n")).unwrap();
        assert_eq!(b.request(&mut io, "recipe.open", &open(None)).unwrap(), json!({"opened": false}), "an edit asks again");
    }

    #[test]
    fn a_session_comes_back_with_its_windows_trees_directories_and_closed_tabs() {
        let (_dir, mut b, mut io) = backend();
        b.request(&mut io, "tab.new", &json!({"cwd": "/work"})).unwrap();
        b.request(&mut io, "pane.split", &json!({"direction": "right"})).unwrap();
        b.request(&mut io, "tab.new", &json!({"cwd": "/gone"})).unwrap();
        b.request(&mut io, "tab.close", &json!({"tab": "tab-3"})).unwrap();
        b.request(&mut io, "action.run", &json!({"id": "new_window"})).unwrap();
        let session = b.session();

        let (_dir, mut again, mut io) = backend();
        assert!(again.restore(&mut io, &session));
        let windows = tabs_by_window(&mut again, &io);
        assert_eq!(windows.iter().map(|(_, tabs)| tabs.len()).collect::<Vec<_>>(), [2, 1]);
        let state = again.state(&io);
        assert_eq!(state["window"], "window-1");
        assert_eq!((&state["tabs"][0]["cwd"], &state["tabs"][1]["cwd"]), (&json!("/home"), &json!("/work")));
        assert_eq!(state["tabs"][1]["tree"]["axis"], "horizontal");
        assert_eq!(io.live.len(), 4, "every pane has a shell again");
        // ⇧⌘T still has the tab that was closed before the relaunch.
        again.request(&mut io, "tab.reopen", &json!({})).unwrap();
        let state = again.state(&io);
        assert_eq!(state["tabs"][2]["cwd"], "/gone");
        assert!(!again.restore(&mut io, "not = [toml"));
    }

    #[test]
    fn an_agent_conversation_is_listed_resumed_and_forked() {
        let (dir, mut b, mut io) = backend();
        let project = dir.path().join("claude").join("projects").join("work");
        std::fs::create_dir_all(&project).unwrap();
        let row = r#"{"type":"user","cwd":"/work/api","message":{"content":"fix the login bug"}}"#;
        std::fs::write(project.join("0a1b-2c3d.jsonl"), row).unwrap();
        b.agent_home = Some(dir.path().join("claude"));

        let rows = b.request(&mut io, "quickly.query", &json!({"filter": "agents", "query": "LOGIN"})).unwrap();
        let agent = json!({"title": "fix the login bug", "subtitle": "/work/api", "kind": "agent", "value": "claude:0a1b-2c3d"});
        assert_eq!(rows, json!([agent]));
        assert_eq!(b.request(&mut io, "quickly.query", &json!({"filter": "agents", "query": "nope"})).unwrap(), json!([]));
        assert_eq!(b.request(&mut io, "quickly.query", &json!({"filter": "ssh", "query": ""})).unwrap(), json!([]));

        // ↩ resumes it in a new tab where it ran; Fork starts a second conversation.
        b.request(&mut io, "quickly.open", &json!({"kind": "agent", "value": "claude:0a1b-2c3d"})).unwrap();
        b.request(&mut io, "quickly.open", &json!({"kind": "agent", "value": "claude:0a1b-2c3d", "target": "fork"})).unwrap();
        assert_eq!(io.written, [
            (PaneId::new(2), "claude --resume 0a1b-2c3d\n".to_owned()),
            (PaneId::new(3), "claude --resume 0a1b-2c3d --fork-session\n".to_owned()),
        ]);
        let state = b.state(&io);
        assert_eq!((&state["panes"]["pane-2"]["cwd"], state["tabs"].as_array().unwrap().len()), (&json!("/work/api"), 3));

        // Fork in Right Split, from the pane the agent is in: the pane's directory names the conversation.
        b.request(&mut io, "action.run", &json!({"id": "agent_fork_split_right"})).unwrap();
        assert_eq!(io.written[2], (PaneId::new(4), "claude --resume 0a1b-2c3d --fork-session\n".to_owned()));
        assert_eq!(b.state(&io)["tabs"][2]["tree"]["axis"], "horizontal");
        b.request(&mut io, "tab.activate", &json!({"tab": "tab-1"})).unwrap();
        assert!(b.request(&mut io, "action.run", &json!({"id": "agent_fork_new_tab"})).is_err(), "nothing ran in /home");

        // A session id is typed into a shell; one that could be more than an id is refused.
        let bad = json!({"kind": "agent", "value": "claude:x; rm -rf ~"});
        assert!(b.request(&mut io, "quickly.open", &bad).is_err());
        assert!(b.request(&mut io, "quickly.open", &json!({"kind": "agent", "value": "cursor:abc", "target": "fork"})).is_err());
        assert_eq!(io.written.len(), 3);
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

        // The outline is the pane's marked commands, and a row scrolls to its line.
        let outline = b.request(&mut io, "details.outline", &json!({})).unwrap();
        assert_eq!(outline, json!([{"line": 0, "title": "cat /etc/hosts", "kind": "command"}]));
        b.request(&mut io, "pane.scroll", &json!({"line": 0})).unwrap();
        b.request(&mut io, "pane.scroll", &json!({"pane": "pane-1", "delta": -3})).unwrap();
        assert_eq!(io.scrolls, [(Some(0), 0), (None, -3)]);
        assert!(b.request(&mut io, "pane.scroll", &json!({})).is_err());
        let info = b.request(&mut io, "details.info", &json!({})).unwrap();
        assert_eq!((&info["ports"], &info["processes"]), (&json!([3000]), &json!(["claude"])));
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
        assert_eq!(io.written[1], (PaneId::new(2), "ssh web1\n".to_owned()));
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
