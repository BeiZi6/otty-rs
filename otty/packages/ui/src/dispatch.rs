//! Shared action dispatch for menus, keymap and the palette.
//!
//! Every `ACTIONS` id is accepted. Menu clicks go through [`Dispatcher::activate`];
//! unimplemented actions still count as a successful click (the handler records
//! them) so the menu bar is complete before each GAP-S handler exists.

use otty_config::actions::ACTIONS;
use otty_config::keybind::Keymap;
use otty_config::menu::{menu_bar, Menu};
use otty_config::Catalog;
use otty_core::{AgentState, PaneId, TabId, WindowId};

use crate::context::ContextHit;
use crate::details::DetailsTab;
use crate::drop::DropZone;
use crate::hotkey::{apply_global_hotkey, GlobalHotkey, HotkeyEffect};
use crate::mode::{hint_labels, open_hint, InputMode, ViState};
use crate::model::{App, PaneKind};
use crate::tabs::TabGroupMode;
use crate::pane_view::load_view;
use crate::quickly::{open_directory, OpenFilter};
use otty_state::Store;

/// The thing the shell, keymap and palette call.
#[derive(Debug)]
pub struct Dispatcher {
    pub app: App,
    next_window: u64,
    next_tab: u64,
    next_pane: u64,
    /// Last action id that was activated, including ones with no handler yet.
    pub last: Option<String>,
    pub quit: bool,
    /// What the last right-click (or test) landed on.
    pub hit: ContextHit,
    /// Last Copy / Copy Path payload.
    pub clipboard: Option<String>,
    pub settings_open: bool,
    pub palette_open: bool,
    pub quickly_open: bool,
    pub quickly_filter: Option<OpenFilter>,
    pub last_opened: Option<String>,
    pub opened: Vec<String>,
    pub details_open: bool,
    pub details_tab: DetailsTab,
    pub quit_after_last_window_closed: bool,
    pub prefix_input: Option<String>,
    pub tab_group: TabGroupMode,
    pub dividers: Vec<usize>,
    pub find_open: bool,
    pub find_query: String,
    pub find_hit: Option<otty_term::FindHit>,
    pub find_lines: Vec<String>,
    pub mode: InputMode,
    pub hint_labels: Vec<crate::mode::HintLabel>,
    pub vi: ViState,
    pub last_opened_url: Option<String>,
    pub composer_open: bool,
    pub last_sent: Option<String>,
    pub pinned: bool,
    pub pip: Option<PaneId>,
    pub raised: bool,
    pub hidden: bool,
}

impl Dispatcher {
    pub fn new() -> Self {
        let mut app = App::new();
        app.add_window(WindowId::new(1), TabId::new(1), PaneId::new(1));
        Self {
            app,
            next_window: 2,
            next_tab: 2,
            next_pane: 2,
            last: None,
            quit: false,
            hit: ContextHit::default(),
            clipboard: None,
            settings_open: false,
            palette_open: false,
            quickly_open: false,
            quickly_filter: Some(OpenFilter::All),
            last_opened: None,
            opened: Vec::new(),
            details_open: false,
            details_tab: DetailsTab::Info,
            quit_after_last_window_closed: false,
            prefix_input: None,
            tab_group: TabGroupMode::None,
            dividers: Vec::new(),
            find_open: false,
            find_query: String::new(),
            find_hit: None,
            find_lines: Vec::new(),
            mode: InputMode::Shell,
            hint_labels: Vec::new(),
            vi: ViState::new(0),
            last_opened_url: None,
            composer_open: false,
            last_sent: None,
            pinned: false,
            pip: None,
            raised: false,
            hidden: false,
        }
    }

    /// True when `action` is a known id. Unknown ids are rejected.
    pub fn activate(&mut self, action: &str) -> bool {
        if !ACTIONS.iter().any(|(id, _)| *id == action) {
            return false;
        }
        self.last = Some(action.to_string());
        match action {
            "copy" => {
                if let Some(s) = self.hit.selection.as_ref().filter(|s| !s.is_empty()) {
                    self.clipboard = Some(s.clone());
                }
            }
            "copy_path" => {
                if let Some(p) = self.hit.path.as_ref().filter(|s| !s.is_empty()) {
                    self.clipboard = Some(p.clone());
                }
            }
            "open_settings" => self.settings_open = true,
            "command_palette" => self.palette_open = true,
            "open_quickly" => {
                self.quickly_open = true;
                self.quickly_filter = Some(OpenFilter::All);
            }
            "open_files_quickly" => {
                self.quickly_open = true;
                self.quickly_filter = Some(OpenFilter::Files);
            }
            "open_recent" => {
                self.quickly_open = true;
                self.quickly_filter = Some(OpenFilter::Recent);
            }
            "open_agent" => {
                self.quickly_open = true;
                self.quickly_filter = Some(OpenFilter::Agents);
            }
            "open_folder" => {
                self.quickly_open = true;
                self.quickly_filter = Some(OpenFilter::Folders);
            }
            "toggle_details_panel" => self.details_open = !self.details_open,
            "details_tab_info" => self.details_tab = DetailsTab::Info,
            "details_tab_outline" => self.details_tab = DetailsTab::Outline,
            "details_tab_git" => self.details_tab = DetailsTab::Git,
            "details_tab_files" => self.details_tab = DetailsTab::Files,
            "next_tab" => self.cycle_tab(1),
            "previous_tab" => self.cycle_tab(-1),
            "quit" => self.quit = true,
            "new_tab" => {
                if let Some(wid) = self.app.active_window().map(|w| w.id) {
                    let tab = TabId::new(self.next_tab);
                    let pane = PaneId::new(self.next_pane);
                    self.next_tab += 1;
                    self.next_pane += 1;
                    self.app.add_tab(wid, tab, pane);
                }
            }
            "new_window" => {
                let w = WindowId::new(self.next_window);
                let t = TabId::new(self.next_tab);
                let p = PaneId::new(self.next_pane);
                self.next_window += 1;
                self.next_tab += 1;
                self.next_pane += 1;
                self.app.add_window(w, t, p);
            }
            "move_tab_to_new_window" => {
                if let Some(tid) = self
                    .app
                    .active_window()
                    .and_then(|w| w.active_tab().map(|t| t.id))
                {
                    let w = WindowId::new(self.next_window);
                    self.next_window += 1;
                    let _ = self.app.move_tab_to_new_window(tid, w);
                }
            }
            "merge_all_windows" => {
                let _ = self.app.merge_all_windows();
            }
            "tabs_group_none" => self.tab_group = TabGroupMode::None,
            "tabs_group_project" => self.tab_group = TabGroupMode::Project,
            "tabs_group_date" => self.tab_group = TabGroupMode::Date,
            "tabs_insert_divider" => {
                if let Some(i) = self.app.active_window().map(|w| w.active_index()) {
                    if !self.dividers.contains(&i) {
                        self.dividers.push(i);
                    }
                }
            }
            "tabs_remove_dividers" => self.dividers.clear(),
            "find" => self.find_open = true,
            "use_selection_for_find" => {
                if let Some(s) = self.hit.selection.clone().filter(|s| !s.is_empty()) {
                    self.find_query = s;
                    self.find_open = true;
                }
            }
            "toggle_hint_mode" => {
                self.mode = if self.mode == InputMode::Hint {
                    InputMode::Shell
                } else {
                    InputMode::Hint
                };
                if self.mode == InputMode::Hint {
                    self.hint_labels = hint_labels(&self.find_lines);
                }
            }
            "pin_window" => self.pinned = !self.pinned,
            "picture_in_picture" => {
                if let Some(p) = self.focused_pane() {
                    let w = WindowId::new(self.next_window);
                    let t = TabId::new(self.next_tab);
                    let np = PaneId::new(self.next_pane);
                    self.next_window += 1;
                    self.next_tab += 1;
                    self.next_pane += 1;
                    self.app.add_window(w, t, np);
                    self.pip = Some(p);
                }
            }
            "picture_in_picture_follow" => {
                self.pip = self.focused_pane();
            }
            "close_picture_in_picture" => {
                if let Some(id) = self
                    .app
                    .windows()
                    .last()
                    .filter(|_| self.pip.is_some())
                    .map(|w| w.id)
                {
                    let _ = self.app.close_window(id);
                }
                self.pip = None;
            }
            "compose_overlay" => self.composer_open = !self.composer_open,
            "prompt_queue_clear" => {}
            "toggle_vi_mode" => {
                self.mode = if self.mode == InputMode::Vi {
                    InputMode::Shell
                } else {
                    InputMode::Vi
                };
                if self.mode == InputMode::Vi {
                    self.vi = ViState::new(self.find_lines.len().max(1));
                }
            }
            "find_next" => {
                self.find_hit =
                    otty_term::find_next(&self.find_lines, &self.find_query, self.find_hit);
            }
            "prefix_tab" => {
                if let Some(p) = self.prefix_input.clone() {
                    if let Some(t) = self
                        .app
                        .active_window_mut()
                        .and_then(|w| w.active_tab_mut())
                    {
                        t.prefix = Some(p);
                    }
                }
            }
            "close_window" => {
                if let Some(id) = self.app.active_window().map(|w| w.id) {
                    let remaining = self.app.close_window(id);
                    if !remaining && self.quit_after_last_window_closed {
                        self.quit = true;
                    }
                }
            }
            "close_tab" => {
                let multi = self
                    .app
                    .active_window()
                    .and_then(|w| w.active_tab())
                    .is_some_and(|t| t.tree.len() > 1);
                if multi {
                    if let Some(p) = self.focused_pane() {
                        let _ = self.app.close_pane(p);
                    }
                } else if let Some(tid) = self
                    .app
                    .active_window()
                    .and_then(|w| w.active_tab().map(|t| t.id))
                {
                    let _ = self.app.active_window_mut().map(|w| w.close_tab(tid));
                }
            }
            "split_right" => self.split(crate::model::SplitDirection::Right),
            "split_left" => self.split(crate::model::SplitDirection::Left),
            "split_down" => self.split(crate::model::SplitDirection::Down),
            "split_up" => self.split(crate::model::SplitDirection::Up),
            "zoom_split" => {
                if let Some(p) = self.focused_pane() {
                    self.app.toggle_zoom(p);
                }
            }
            _ => {}
        }
        true
    }

    /// Run a palette row. `keep_open` is ⌘↩: execute and leave the palette up.
    pub fn run_palette(&mut self, action: &str, keep_open: bool) -> bool {
        let ok = self.activate(action);
        self.palette_open = keep_open;
        ok
    }

    /// Switch Open Quickly's filter. Unknown keys leave `quickly_filter` as
    /// `None` so [`crate::query_quickly`] returns an empty list.
    pub fn set_quickly_filter(&mut self, key: &str) {
        self.quickly_filter = OpenFilter::from_key(key);
    }

    /// Open an existing directory from Open Quickly. Records frecency when a
    /// store is given and closes the overlay.
    pub fn run_quickly(&mut self, path: &str, store: Option<&Store>, now: i64) -> bool {
        match open_directory(path, store, now) {
            Ok(p) => {
                self.last_opened = Some(p.clone());
                if !self.opened.iter().any(|o| o == &p) {
                    self.opened.push(p);
                }
                self.quickly_open = false;
                true
            }
            Err(_) => false,
        }
    }

    /// Open a path using the default frecency store.
    pub fn run_quickly_path(&mut self, path: &str) -> bool {
        let store = Store::open_default().ok();
        self.run_quickly(path, store.as_ref(), crate::quickly::now_secs())
    }

    /// Focus the tab at `index` in the active window.
    pub fn select_tab_at(&mut self, index: usize) -> bool {
        self.last = Some("select_tab".into());
        let Some(w) = self.app.active_window_mut() else { return false };
        let Some(id) = w.tabs().get(index).map(|t| t.id) else { return false };
        w.focus_tab(id)
    }

    /// Close the tab at `index` in the active window. Used by the tab context menu.
    pub fn close_tab_at(&mut self, index: usize) -> bool {
        self.last = Some("close_tab".into());
        let Some(w) = self.app.active_window_mut() else { return false };
        let Some(id) = w.tabs().get(index).map(|t| t.id) else { return false };
        w.close_tab(id)
    }

    /// Enter in Composer: send now, or queue while the agent is processing.
    pub fn send_composer(&mut self, text: &str, store: Option<&Store>, now: i64) -> bool {
        let pane = self.focused_pane();
        let busy = pane
            .and_then(|id| self.app.pane(id))
            .and_then(|p| p.agent.as_ref())
            .is_some_and(|(_, s)| *s == AgentState::Processing);
        if busy {
            if let (Some(store), Some(id)) = (store, pane) {
                let _ = store.enqueue_prompt(id.get() as i64, text, now);
            }
            return true;
        }
        self.last_sent = Some(text.to_owned());
        self.composer_open = false;
        true
    }

    /// Drain one queued prompt when the agent is idle.
    pub fn flush_queue(&mut self, store: &Store) -> Option<String> {
        let pane = self.focused_pane()?;
        let idle = self
            .app
            .pane(pane)
            .and_then(|p| p.agent.as_ref())
            .is_none_or(|(_, s)| *s == AgentState::Idle);
        if !idle {
            return None;
        }
        let text = store.dequeue_prompt(pane.get() as i64).ok().flatten()?;
        self.last_sent = Some(text.clone());
        Some(text)
    }

    pub fn apply_hotkey(&mut self, kind: GlobalHotkey, otty_front: bool) {
        match apply_global_hotkey(kind, otty_front) {
            HotkeyEffect::Raise => {
                self.raised = true;
                self.hidden = false;
                if self.app.windows().is_empty() {
                    let w = WindowId::new(self.next_window);
                    let t = TabId::new(self.next_tab);
                    let p = PaneId::new(self.next_pane);
                    self.next_window += 1;
                    self.next_tab += 1;
                    self.next_pane += 1;
                    self.app.add_window(w, t, p);
                }
            }
            HotkeyEffect::Hide => {
                self.hidden = true;
                self.raised = false;
            }
            HotkeyEffect::None => {}
        }
    }

    /// Drop a folder onto an edge: new terminal pane with that cwd.
    pub fn drop_path(&mut self, path: &str, zone: DropZone) -> bool {
        match zone {
            DropZone::Pane => self.open_view(path),
            DropZone::Top | DropZone::Bottom | DropZone::Left | DropZone::Right => {
                let Some(wid) = self.app.active_window().map(|w| w.id) else {
                    return false;
                };
                let tab = TabId::new(self.next_tab);
                let pane = PaneId::new(self.next_pane);
                self.next_tab += 1;
                self.next_pane += 1;
                if !self.app.add_tab(wid, tab, pane) {
                    return false;
                }
                if let Some(p) = self.app.pane_mut(pane) {
                    p.cwd = Some(path.to_owned());
                    p.kind = PaneKind::Terminal;
                }
                true
            }
        }
    }

    /// Open `path` in a new tab as a file, folder or URL pane.
    pub fn open_view(&mut self, path: &str) -> bool {
        let Ok(kind) = load_view(path) else { return false };
        let Some(wid) = self.app.active_window().map(|w| w.id) else { return false };
        let tab = TabId::new(self.next_tab);
        let pane = PaneId::new(self.next_pane);
        self.next_tab += 1;
        self.next_pane += 1;
        if !self.app.add_tab(wid, tab, pane) {
            return false;
        }
        if let Some(p) = self.app.pane_mut(pane) {
            p.kind = kind;
            p.custom_title = Some(path.trim().to_owned());
        }
        true
    }

    pub fn focused_pane_id(&self) -> Option<PaneId> {
        self.focused_pane()
    }

    pub fn open_hint(&mut self, tag: &str) -> Option<String> {
        let url = open_hint(&self.hint_labels, tag).map(str::to_string)?;
        self.last_opened_url = Some(url.clone());
        self.mode = InputMode::Shell;
        Some(url)
    }

    /// True when the key is consumed by Hint/Vi and must not go to the PTY.
    pub fn handle_mode_key(&mut self, key: &str) -> bool {
        match self.mode {
            InputMode::Hint => {
                if let Some(url) = open_hint(&self.hint_labels, key) {
                    self.last_opened_url = Some(url.to_string());
                    self.mode = InputMode::Shell;
                }
                true
            }
            InputMode::Vi => {
                let swallowed = self.vi.handle(key);
                if key == "y" || key == "enter" || key == "\n" {
                    if let Some(s) = self.hit.selection.clone() {
                        self.clipboard = Some(s);
                    }
                    self.mode = InputMode::Shell;
                }
                swallowed
            }
            InputMode::Mark => true,
            InputMode::Shell => false,
        }
    }

    /// Clear Badge — the titlebar dropdown action (not an ACTIONS id).
    pub fn clear_badge(&mut self) {
        if let Some(t) = self
            .app
            .active_window_mut()
            .and_then(|w| w.active_tab_mut())
        {
            t.badge = None;
        }
    }

    fn cycle_tab(&mut self, delta: isize) {
        let Some(w) = self.app.active_window_mut() else { return };
        let n = w.tabs().len() as isize;
        if n == 0 {
            return;
        }
        let i = (w.active_index() as isize + delta).rem_euclid(n) as usize;
        let id = w.tabs()[i].id;
        w.focus_tab(id);
    }

    fn focused_pane(&self) -> Option<PaneId> {
        self.app
            .active_window()
            .and_then(|w| w.active_tab().map(|t| t.focused))
    }

    fn split(&mut self, dir: crate::model::SplitDirection) {
        let Some(target) = self.focused_pane() else { return };
        let pane = PaneId::new(self.next_pane);
        self.next_pane += 1;
        if !self.app.split_pane(target, pane, dir, 50) {
            self.next_pane -= 1;
        }
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// The File/Edit/View/Shell/Window(+Commands) tree the shell binds.
pub fn shell_menu(catalog: Option<&Catalog>, map: &Keymap) -> Vec<Menu> {
    menu_bar(catalog, map)
}

/// Flattened (title, action) rows per top-level menu, for Slint models.
pub fn shell_menu_models(
    catalog: Option<&Catalog>,
    map: &Keymap,
) -> Vec<(&'static str, Vec<(String, String)>)> {
    shell_menu(catalog, map)
        .into_iter()
        .map(|m| {
            let rows = m
                .items
                .into_iter()
                .map(|i| {
                    let title = match i.shortcut {
                        Some(s) => format!("{}\t{s}", i.label),
                        None => i.label,
                    };
                    (title, i.action.to_string())
                })
                .collect();
            (m.title, rows)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_config::actions::ACTIONS;
    use otty_config::keybind::Keymap;
    use otty_config::menu::clickable_actions;

    #[test]
    fn every_action_id_is_clickable_including_unbound() {
        let mut d = Dispatcher::new();
        let map = Keymap::defaults();
        let menus = shell_menu(None, &map);
        let ids = clickable_actions(&menus);
        assert_eq!(ids.len(), ACTIONS.len());
        let mut unbound = 0;
        for (id, _) in ACTIONS {
            assert!(d.activate(id), "menu click rejected {id}");
            assert_eq!(d.last.as_deref(), Some(*id));
            if map.for_action(id).is_empty() {
                unbound += 1;
            }
        }
        assert!(unbound > 0, "expected unbound actions in ACTIONS");
        assert!(d.activate("quit"));
        assert!(d.quit);
        assert!(!d.activate("not_an_action"));
    }

    #[test]
    fn default_keymap_menu_commands_dispatch() {
        let mut d = Dispatcher::new();
        let map = Keymap::defaults();
        for b in map.iter() {
            assert!(
                d.activate(&b.action),
                "default chord {} action {} not clickable",
                b.trigger,
                b.action
            );
        }
        let mut d = Dispatcher::new();
        assert!(d.activate("new_tab"));
        assert_eq!(d.app.counts(), (1, 2, 2));
    }

    #[test]
    fn shell_menu_models_cover_the_five_menus_and_commands() {
        let models = shell_menu_models(None, &Keymap::defaults());
        let titles: Vec<_> = models.iter().map(|(t, _)| *t).collect();
        assert_eq!(
            titles,
            ["File", "Edit", "View", "Shell", "Window", "Commands"]
        );
        let file = &models[0].1;
        assert!(file.iter().any(|(_, a)| a == "new_tab"));
        assert!(file.iter().any(|(_, a)| a == "quit"));
        let n: usize = models.iter().map(|(_, rows)| rows.len()).sum();
        assert_eq!(n, ACTIONS.len());
        assert!(models[5].1.iter().any(|(title, _)| !title.contains('\t')));
    }

    #[test]
    fn open_settings_opens_the_panel() {
        let mut d = Dispatcher::new();
        assert!(!d.settings_open);
        assert!(d.activate("open_settings"));
        assert!(d.settings_open);
    }

    #[test]
    fn split_right_then_down_close_middle_zoom_toggles_back() {
        let mut d = Dispatcher::new();
        assert!(d.activate("split_right"));
        assert!(d.activate("split_down"));
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(tab.tree.len(), 3, "left then down is three panes");
        let panes = tab.tree.panes();
        let middle = panes[1];
        assert!(d.app.focus_pane(middle));
        assert!(d.activate("close_tab"));
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(tab.tree.len(), 2);
        assert!(tab.tree.contains(tab.focused), "tree stays connected");

        assert!(d.activate("zoom_split"));
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(tab.zoomed, Some(tab.focused));
        assert!(d.activate("zoom_split"));
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(tab.zoomed, None, "zoom toggles back");
    }

    #[test]
    fn tear_tab_merge_and_last_window_respects_quit_flag() {
        let mut d = Dispatcher::new();
        assert!(d.activate("new_tab"));
        assert_eq!(d.app.counts().0, 1);
        assert!(d.activate("move_tab_to_new_window"));
        assert_eq!(d.app.counts().0, 2, "tab moved into a new window");
        assert!(d.activate("merge_all_windows"));
        assert_eq!(d.app.counts().0, 1, "merge pulls tabs back");
        assert_eq!(d.app.active_window().unwrap().tabs().len(), 2);

        d.quit_after_last_window_closed = false;
        assert!(d.activate("close_window"));
        assert!(!d.quit, "flag off: last window does not quit");
        assert_eq!(d.app.counts().0, 0);

        let mut d = Dispatcher::new();
        d.quit_after_last_window_closed = true;
        assert!(d.activate("close_window"));
        assert!(d.quit, "flag on: last window quits");
    }

    #[test]
    fn prefix_and_clear_badge() {
        let mut d = Dispatcher::new();
        if let Some(t) = d.app.active_window_mut().and_then(|w| w.active_tab_mut()) {
            t.badge = Some(crate::model::Badge::Error);
        }
        d.prefix_input = Some("api".into());
        assert!(d.activate("prefix_tab"));
        d.clear_badge();
        let t = d.app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(t.prefix.as_deref(), Some("api"));
        assert_eq!(t.badge, None);
    }

    #[test]
    fn find_next_and_selection_fills_query() {
        let mut d = Dispatcher::new();
        d.find_lines = vec!["alpha foo".into(), "foo baz".into()];
        d.find_query = "foo".into();
        assert!(d.activate("find"));
        assert!(d.find_open);
        assert!(d.activate("find_next"));
        assert_eq!(d.find_hit, Some(otty_term::FindHit { line: 0, col: 6 }));
        assert!(d.activate("find_next"));
        assert_eq!(d.find_hit, Some(otty_term::FindHit { line: 1, col: 0 }));

        d.hit.selection = Some("bar".into());
        assert!(d.activate("use_selection_for_find"));
        assert_eq!(d.find_query, "bar");
    }

    #[test]
    fn composer_sends_or_queues_while_busy() {
        let mut d = Dispatcher::new();
        assert!(d.activate("compose_overlay"));
        assert!(d.composer_open);
        assert!(d.send_composer("hello", None, 0));
        assert_eq!(d.last_sent.as_deref(), Some("hello"));
        assert!(!d.composer_open);

        let store = Store::in_memory().unwrap();
        let pane = d.focused_pane_id().unwrap();
        d.app.pane_mut(pane).unwrap().agent = Some(("claude".into(), AgentState::Processing));
        d.activate("compose_overlay");
        assert!(d.send_composer("later", Some(&store), 1));
        assert_eq!(d.last_sent.as_deref(), Some("hello"), "busy: queued, not sent");
        d.app.pane_mut(pane).unwrap().agent = Some(("claude".into(), AgentState::Idle));
        assert_eq!(d.flush_queue(&store).as_deref(), Some("later"));
        assert_eq!(d.last_sent.as_deref(), Some("later"));
    }

    #[test]
    fn pin_floats_and_pip_is_its_own_window() {
        let mut d = Dispatcher::new();
        assert!(d.activate("pin_window"));
        assert!(d.pinned);
        assert!(d.activate("pin_window"));
        assert!(!d.pinned);

        assert_eq!(d.app.counts().0, 1);
        assert!(d.activate("picture_in_picture"));
        assert_eq!(d.app.counts().0, 2, "PiP is a second window");
        assert!(d.pip.is_some());
        assert!(d.activate("close_picture_in_picture"));
        assert!(d.pip.is_none());
        assert_eq!(d.app.counts().0, 1);
    }
}
