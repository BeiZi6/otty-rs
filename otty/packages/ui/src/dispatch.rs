//! Shared action dispatch for menus, keymap and the palette.
//!
//! Every `ACTIONS` id is accepted. Menu clicks go through [`Dispatcher::activate`];
//! unimplemented actions still count as a successful click (the handler records
//! them) so the menu bar is complete before each GAP-S handler exists.

use otty_config::actions::ACTIONS;
use otty_config::keybind::Keymap;
use otty_config::menu::{menu_bar, Menu};
use otty_config::Catalog;
use otty_core::{PaneId, TabId, WindowId};

use crate::context::ContextHit;
use crate::details::DetailsTab;
use crate::model::App;
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
            "close_tab" => {
                if let Some(tid) = self
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
}
