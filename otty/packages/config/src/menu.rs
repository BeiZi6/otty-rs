//! Application menu bar: every action id is a clickable item.
//!
//! Labels come from the localization catalog when present, else `ACTIONS`.
//! Shortcuts come from the keymap. Items with no default chord still appear.

use crate::actions::ACTIONS;
use crate::catalog::Catalog;
use crate::keybind::Keymap;

/// One clickable row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub action: &'static str,
    pub label: String,
    pub shortcut: Option<String>,
}

/// A top-level menu (`File`, `Edit`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub title: &'static str,
    pub items: Vec<MenuItem>,
}

const SECTIONS: &[(&str, &[&str])] = &[
    (
        "File",
        &[
            "new_window",
            "new_tab",
            "open_settings",
            "new_editor",
            "new_folder_pane",
            "open_recent",
            "open_folder",
            "open_files_quickly",
            "duplicate_tab",
            "close_tab",
            "close_window",
            "save_recipe",
            "save_recipe_as",
            "export_text",
            "export_selected_text",
            "quit",
        ],
    ),
    (
        "Edit",
        &[
            "undo",
            "redo",
            "cut",
            "copy",
            "copy_path",
            "paste",
            "paste_selection",
            "paste_file_base64",
            "paste_shell_escaped",
            "paste_bracketed",
            "paste_continue_composer",
            "select_all",
            "find",
            "find_next",
            "find_previous",
            "use_selection_for_find",
            "insert_file_path",
            "insert_screenshot",
        ],
    ),
    (
        "View",
        &[
            "command_palette",
            "open_quickly",
            "jump_to",
            "jump_to_selection",
            "global_search",
            "search_in_files",
            "toggle_tabs_panel",
            "toggle_details_panel",
            "increase_font_size",
            "decrease_font_size",
            "reset_font_size",
            "split_right",
            "split_left",
            "split_down",
            "split_up",
            "zoom_split",
            "equalize_splits",
            "pin_window",
            "picture_in_picture",
            "toggle_full_screen",
        ],
    ),
    (
        "Shell",
        &[
            "rename_tab",
            "prefix_tab",
            "clear_screen",
            "clear_scrollback_above",
            "reveal_in_finder",
            "toggle_vi_mode",
            "toggle_mark_mode",
            "toggle_hint_mode",
            "toggle_read_only",
            "toggle_editor_vim",
            "send_to_chat",
            "compose_overlay",
            "prompt_queue_add",
            "prompt_queue_clear",
        ],
    ),
    (
        "Window",
        &[
            "minimize",
            "next_tab",
            "previous_tab",
            "show_unread_tab",
            "move_tab_to_new_window",
            "merge_all_windows",
            "focus_next_pane",
            "focus_previous_pane",
            "focus_pane_up",
            "focus_pane_down",
            "focus_pane_left",
            "focus_pane_right",
        ],
    ),
];

fn label_for(action: &'static str, catalog: Option<&Catalog>) -> String {
    if let Some(c) = catalog {
        if let Some(s) = c.action_label(action) {
            return s.to_string();
        }
    }
    ACTIONS
        .iter()
        .find(|(id, _)| *id == action)
        .map(|(_, l)| (*l).to_string())
        .unwrap_or_else(|| action.to_string())
}

fn item(action: &'static str, catalog: Option<&Catalog>, map: &Keymap) -> MenuItem {
    let shortcut = map
        .for_action(action)
        .first()
        .map(|b| b.trigger.to_string());
    MenuItem {
        action,
        label: label_for(action, catalog),
        shortcut,
    }
}

/// File / Edit / View / Shell / Window, then every leftover `ACTIONS` id.
pub fn menu_bar(catalog: Option<&Catalog>, map: &Keymap) -> Vec<Menu> {
    let placed: std::collections::BTreeSet<&str> = SECTIONS
        .iter()
        .flat_map(|(_, ids)| ids.iter().copied())
        .collect();
    let mut menus: Vec<Menu> = SECTIONS
        .iter()
        .map(|(title, ids)| Menu {
            title,
            items: ids
                .iter()
                .filter(|id| ACTIONS.iter().any(|(a, _)| a == *id))
                .map(|id| item(id, catalog, map))
                .collect(),
        })
        .collect();
    let rest: Vec<MenuItem> = ACTIONS
        .iter()
        .filter(|(id, _)| !placed.contains(id))
        .map(|(id, _)| item(id, catalog, map))
        .collect();
    if !rest.is_empty() {
        menus.push(Menu {
            title: "Commands",
            items: rest,
        });
    }
    menus
}

/// Every action id the bar can dispatch, including unbound ones.
pub fn clickable_actions(menus: &[Menu]) -> Vec<&str> {
    menus
        .iter()
        .flat_map(|m| m.items.iter().map(|i| i.action))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, UiLanguage};
    use crate::keybind::Keymap;

    fn resources() -> std::path::PathBuf {
        crate::catalog::bundled_resources()
    }

    #[test]
    fn every_action_is_clickable_including_unbound() {
        let map = Keymap::defaults();
        let menus = menu_bar(None, &map);
        let ids = clickable_actions(&menus);
        assert_eq!(ids.len(), ACTIONS.len());
        for (id, _) in ACTIONS {
            assert!(ids.contains(id), "menu missing {id}");
        }
        let commands = menus.iter().find(|m| m.title == "Commands").unwrap();
        assert!(
            commands.items.iter().any(|i| i.shortcut.is_none()),
            "expected at least one unbound item in Commands"
        );
    }

    #[test]
    fn default_keymap_menu_commands_have_rows() {
        let map = Keymap::defaults();
        let menus = menu_bar(None, &map);
        let ids = clickable_actions(&menus);
        for b in map.iter() {
            assert!(
                ids.contains(&b.action.as_str()),
                "default chord {} action {} missing from menu",
                b.trigger,
                b.action
            );
        }
        let file = menus.iter().find(|m| m.title == "File").unwrap();
        assert!(file.items.iter().any(|i| i.action == "new_tab"));
        assert!(file.items.iter().any(|i| i.action == "quit"));
    }

    #[test]
    fn chinese_catalog_relabels_settings_row() {
        let cat = Catalog::load_for(UiLanguage::Chinese, &resources()).unwrap();
        let menus = menu_bar(Some(&cat), &Keymap::defaults());
        let ids = clickable_actions(&menus);
        assert!(ids.contains(&"open_settings"));
        let settings = menus
            .iter()
            .flat_map(|m| &m.items)
            .find(|i| i.action == "open_settings")
            .unwrap();
        assert_eq!(settings.label, cat.action_label("open_settings").unwrap());
    }
}
