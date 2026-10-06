//! The keymap.
//!
//! `keybind = trigger=action` lines in `config.toml` layer on top of a built-in
//! default map. Every action is re-bindable and many ship unbound; a user line
//! for an action *replaces* the default rather than adding to it, which is what
//! "the two stay in sync" means for the Settings panel (it rewrites the file
//! from the same map).
//!
//! The action ids and labels come from [`crate::actions`], generated from the
//! shipped app's localization catalog.

use std::collections::BTreeMap;

use otty_core::{Chord, Trigger};

/// One `trigger -> action` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub trigger: Trigger,
    pub action: String,
}

impl Binding {
    /// Parse the `trigger=action` spelling used in `config.toml`.
    pub fn parse(line: &str) -> Result<Self, BindingError> {
        let (lhs, rhs) = line
            .split_once('=')
            .ok_or_else(|| BindingError::NoSeparator(line.to_owned()))?;
        let trigger: Trigger = lhs
            .trim()
            .parse()
            .map_err(|e| BindingError::BadTrigger(lhs.trim().to_owned(), e))?;
        let action = rhs.trim();
        if action.is_empty() {
            return Err(BindingError::NoAction(line.to_owned()));
        }
        if !crate::actions::ACTIONS.iter().any(|(id, _)| *id == action) {
            return Err(BindingError::UnknownAction(action.to_owned()));
        }
        Ok(Self { trigger, action: action.to_owned() })
    }

    pub fn render(&self) -> String {
        format!("{}={}", self.trigger, self.action)
    }
}

/// The bindings that ship with Otty.
///
/// Transcribed from the Keybindings Reference. Where the reference lists two
/// chords for one action they appear as two entries.
pub static DEFAULT_BINDINGS: &[(&str, &str)] = &[
    // General
    ("cmd+shift+p", "command_palette"),
    ("cmd+shift+o", "open_quickly"),
    ("cmd+j", "jump_to"),
    ("cmd+comma", "open_settings"),
    // Window
    ("cmd+n", "new_window"),
    ("cmd+shift+w", "close_window"),
    ("cmd+m", "minimize"),
    ("cmd+ctrl+f", "toggle_full_screen"),
    // Tab
    ("cmd+t", "new_tab"),
    ("cmd+w", "close_tab"),
    ("cmd+shift+t", "reopen_closed_tab"),
    ("cmd+shift+left_bracket", "previous_tab"),
    ("cmd+shift+right_bracket", "next_tab"),
    ("cmd+1", "goto_tab_1"),
    ("cmd+2", "goto_tab_2"),
    ("cmd+3", "goto_tab_3"),
    ("cmd+4", "goto_tab_4"),
    ("cmd+5", "goto_tab_5"),
    ("cmd+6", "goto_tab_6"),
    ("cmd+7", "goto_tab_7"),
    ("cmd+8", "goto_tab_8"),
    ("cmd+9", "goto_tab_9"),
    ("cmd+shift+l", "toggle_tabs_panel"),
    ("cmd+shift+r", "toggle_details_panel"),
    ("cmd+shift+u", "show_unread_tab"),
    // Pane
    ("cmd+d", "split_right"),
    ("cmd+opt+d", "split_left"),
    ("cmd+shift+d", "split_down"),
    ("cmd+opt+shift+d", "split_up"),
    ("cmd+shift+enter", "zoom_split"),
    ("cmd+ctrl+equal", "equalize_splits"),
    ("cmd+right_bracket", "focus_next_pane"),
    ("cmd+left_bracket", "focus_previous_pane"),
    ("cmd+ctrl+up", "focus_pane_up"),
    ("cmd+ctrl+down", "focus_pane_down"),
    ("cmd+ctrl+left", "focus_pane_left"),
    ("cmd+ctrl+right", "focus_pane_right"),
    ("cmd+ctrl+shift+up", "move_divider_up"),
    ("cmd+ctrl+shift+down", "move_divider_down"),
    ("cmd+ctrl+shift+left", "move_divider_left"),
    ("cmd+ctrl+shift+right", "move_divider_right"),
    // Clipboard and selection
    ("cmd+c", "copy"),
    ("cmd+x", "cut"),
    ("cmd+v", "paste"),
    ("cmd+a", "select_all"),
    ("cmd+z", "undo"),
    ("cmd+shift+z", "redo"),
    ("cmd+y", "redo"),
    // Find and search
    ("cmd+f", "find"),
    ("cmd+g", "find_next"),
    ("cmd+shift+g", "find_previous"),
    ("cmd+shift+f", "global_search"),
    // Scrolling
    ("cmd+page_up", "scroll_page_up"),
    ("cmd+page_down", "scroll_page_down"),
    ("cmd+opt+up", "scroll_up"),
    ("cmd+opt+down", "scroll_down"),
    ("cmd+opt+page_up", "scroll_up"),
    ("cmd+opt+page_down", "scroll_down"),
    ("cmd+home", "scroll_to_top"),
    ("cmd+end", "scroll_to_bottom"),
    // Text editing
    ("cmd+left", "cursor_line_start"),
    ("cmd+right", "cursor_line_end"),
    ("opt+left", "cursor_word_left"),
    ("opt+right", "cursor_word_right"),
    ("opt+backspace", "delete_word_left"),
    ("opt+forward_delete", "delete_word_right"),
    ("cmd+backspace", "delete_to_line_start"),
    ("cmd+forward_delete", "delete_to_line_end"),
    // View
    ("cmd+equal", "increase_font_size"),
    ("cmd+minus", "decrease_font_size"),
    ("cmd+0", "reset_font_size"),
    ("cmd+k", "clear_screen"),
    // Composer and Vi mode
    ("cmd+shift+e", "compose_overlay"),
    ("cmd+shift+m", "prompt_queue_add"),
    ("ctrl+shift+space", "toggle_vi_mode"),
    // Recipes
    ("cmd+s", "save_recipe"),
    ("cmd+shift+s", "save_recipe_as"),
];

/// The grouped display bucket for an action id.
///
/// The Settings panel groups actions; the grouping is not part of the file
/// format, so it is derived here from the id namespace.
pub fn group_of(action: &str) -> &'static str {
    if action.starts_with("goto_tab_") || action.starts_with("tabs_") {
        return "tabs";
    }
    if action.starts_with("details_tab_") {
        return "pane";
    }
    if action.starts_with("agent_") {
        return "agents";
    }
    if action.starts_with("split_") || action.starts_with("focus_") || action.starts_with("move_divider_")
    {
        return "pane";
    }
    if action.starts_with("scroll_") {
        return "scroll";
    }
    if action.starts_with("cursor_") || action.starts_with("delete_") {
        return "text_editing";
    }
    if action.starts_with("paste_") || action.starts_with("hint_") {
        return if action.starts_with("paste_") { "edit" } else { "view" };
    }
    match action {
        "new_window" | "close_window" | "minimize" | "toggle_full_screen" => "window",
        "new_tab" | "close_tab" | "reopen_closed_tab" | "next_tab" | "previous_tab"
        | "duplicate_tab" | "rename_tab" | "prefix_tab" | "move_tab_to_new_window"
        | "show_unread_tab" | "toggle_tabs_panel" => "tabs",
        "split_right" | "split_left" | "split_down" | "split_up" | "zoom_split"
        | "equalize_splits" | "switch_pane_to_folder" | "switch_pane_to_terminal"
        | "toggle_details_panel" => "pane",
        "copy" | "cut" | "paste" | "select_all" | "undo" | "redo" => "edit",
        "increase_font_size" | "decrease_font_size" | "reset_font_size" | "clear_screen"
        | "find" | "find_next" | "find_previous" | "global_search" | "use_selection_for_find"
        | "search_in_files" | "toggle_vi_mode" | "toggle_vi_mode_help" | "toggle_mark_mode"
        | "toggle_hint_mode" | "toggle_read_only" | "compose_overlay" | "prompt_queue_add"
        | "prompt_queue_clear" => "view",
        "save_recipe" | "save_recipe_as" => "recipe",
        _ => "general",
    }
}

/// The label shown for an action, and its id.
pub fn action_label(action: &str) -> Option<&'static str> {
    crate::actions::ACTIONS.iter().find(|(id, _)| *id == action).map(|(_, l)| *l)
}

/// A resolved keymap: defaults plus the user's `keybind` lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Keymap {
    /// The built-in map.
    pub fn defaults() -> Self {
        let mut map = Self::empty();
        for (trigger, action) in DEFAULT_BINDINGS {
            if let Ok(t) = trigger.parse::<Trigger>() {
                map.bindings.push(Binding { trigger: t, action: (*action).to_owned() });
            }
        }
        map.bindings.sort_by(|a, b| a.trigger.to_string().cmp(&b.trigger.to_string()));
        map
    }

    /// An empty map, for `otty keybind list --reset` previews and tests.
    pub fn empty() -> Self {
        Self { bindings: Vec::new() }
    }

    /// Apply the user's `keybind = trigger=action` lines.
    ///
    /// A line *rebinds the chord it names*: it replaces whatever that chord was
    /// bound to, and leaves the action's other chords alone. That is what makes
    /// a map round-trip through its own text form, and what lets
    /// `cmd+shift+k=clear_screen` be an additional way to clear the screen
    /// rather than a silent removal of `cmd+k`.
    ///
    /// To remove a binding outright, use [`Keymap::clear_action`] or
    /// [`Keymap::clear_trigger`] — the Settings panel writes the resulting map.
    pub fn with_overrides(mut self, lines: &[String]) -> Self {
        for line in lines {
            let Ok(b) = Binding::parse(line) else {
                log::warn!("ignoring malformed keybind line: {line}");
                continue;
            };
            self.bindings.retain(|x| x.trigger != b.trigger);
            self.bindings.push(b);
        }
        self.bindings.sort_by(|a, b| a.trigger.to_string().cmp(&b.trigger.to_string()));
        self
    }

    /// Insert or replace one binding.
    pub fn set(&mut self, trigger: Trigger, action: &str) {
        self.bindings.retain(|x| x.action != action && x.trigger != trigger);
        self.bindings.push(Binding { trigger, action: action.to_owned() });
    }

    /// Drop every binding for `action` — "reset to default" in the panel, and
    /// the shape `otty keybind unset` writes.
    pub fn clear_action(&mut self, action: &str) {
        self.bindings.retain(|x| x.action != action);
    }

    /// Drop whatever is bound to `trigger`, leaving `action` unbound.
    pub fn clear_trigger(&mut self, trigger: &Trigger) {
        self.bindings.retain(|x| &x.trigger != trigger);
    }

    /// Only the bindings that differ from the built-in map — what the Settings
    /// panel writes back into `config.toml`, so the file carries the user's
    /// intent rather than a copy of the defaults.
    pub fn to_overrides(&self) -> Vec<String> {
        let defaults = Self::defaults();
        self.bindings
            .iter()
            .filter(|b| !defaults.bindings.iter().any(|d| d == *b))
            .map(Binding::render)
            .collect()
    }

    /// The action a chord invokes, if any.
    pub fn lookup(&self, chord: Chord) -> Option<&str> {
        let want = chord.normalized();
        self.bindings
            .iter()
            .find(|b| b.trigger.len() == 1 && b.trigger.first().map(Chord::normalized) == Some(want))
            .map(|b| b.action.as_str())
    }

    /// Every binding for an action, in chord order.
    pub fn for_action(&self, action: &str) -> Vec<&Binding> {
        self.bindings.iter().filter(|b| b.action == action).collect()
    }

    /// Every binding whose action id or label contains `needle`.
    pub fn search(&self, needle: &str) -> Vec<&Binding> {
        let n = needle.to_ascii_lowercase();
        self.bindings
            .iter()
            .filter(|b| {
                b.action.to_ascii_lowercase().contains(&n)
                    || b.trigger.to_string().contains(&n)
                    || action_label(&b.action)
                        .is_some_and(|l| l.to_ascii_lowercase().contains(&n))
            })
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Binding> {
        self.bindings.iter()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Chords bound to more than one action — what the Settings panel flags as
    /// a conflict.
    pub fn conflicts(&self) -> BTreeMap<String, Vec<&str>> {
        let mut seen: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for b in &self.bindings {
            seen.entry(b.trigger.to_string()).or_default().push(b.action.as_str());
        }
        seen.retain(|_, v| v.len() > 1);
        seen
    }

    /// The `keybind` lines that reproduce this map — what a GUI edit writes
    /// back into `config.toml`.
    pub fn to_lines(&self) -> Vec<String> {
        self.bindings.iter().map(Binding::render).collect()
    }
}

/// A `keybind` line that could not be understood.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    #[error("`{0}` is not a `trigger=action` binding")]
    NoSeparator(String),
    #[error("`{0}` is not a key chord: {1}")]
    BadTrigger(String, otty_core::ChordParseError),
    #[error("`{0}` has no action on the right-hand side")]
    NoAction(String),
    #[error("`{0}` is not an action Otty knows")]
    UnknownAction(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_default_binding_parses_and_names_a_real_action() {
        for (trigger, action) in DEFAULT_BINDINGS {
            let t: Trigger = trigger.parse().unwrap_or_else(|e| panic!("{trigger}: {e}"));
            assert_eq!(t.to_string(), *trigger, "default trigger is not canonical");
            assert!(
                crate::actions::ACTIONS.iter().any(|(id, _)| id == action),
                "default binding names unknown action `{action}`"
            );
        }
    }

    #[test]
    fn the_documented_defaults_are_present() {
        let m = Keymap::defaults();
        let expect = |trigger: &str, action: &str| {
            let c: Chord = trigger.parse().unwrap();
            assert_eq!(m.lookup(c), Some(action), "{trigger} should map to {action}");
        };
        expect("cmd+shift+p", "command_palette");
        expect("cmd+shift+o", "open_quickly");
        expect("cmd+j", "jump_to");
        expect("cmd+comma", "open_settings");
        expect("cmd+n", "new_window");
        expect("cmd+ctrl+f", "toggle_full_screen");
        expect("cmd+t", "new_tab");
        expect("cmd+w", "close_tab");
        expect("cmd+shift+t", "reopen_closed_tab");
        expect("cmd+shift+left_bracket", "previous_tab");
        expect("cmd+shift+right_bracket", "next_tab");
        expect("cmd+9", "goto_tab_9");
        expect("cmd+shift+l", "toggle_tabs_panel");
        expect("cmd+shift+r", "toggle_details_panel");
        expect("cmd+d", "split_right");
        expect("cmd+opt+d", "split_left");
        expect("cmd+shift+d", "split_down");
        expect("cmd+opt+shift+d", "split_up");
        expect("cmd+shift+enter", "zoom_split");
        expect("cmd+ctrl+equal", "equalize_splits");
        expect("cmd+right_bracket", "focus_next_pane");
        expect("cmd+left_bracket", "focus_previous_pane");
        expect("cmd+ctrl+up", "focus_pane_up");
        expect("cmd+ctrl+shift+right", "move_divider_right");
        expect("cmd+c", "copy");
        expect("cmd+v", "paste");
        expect("cmd+f", "find");
        expect("cmd+shift+f", "global_search");
        expect("cmd+opt+up", "scroll_up");
        expect("cmd+home", "scroll_to_top");
        expect("opt+backspace", "delete_word_left");
        expect("cmd+backspace", "delete_to_line_start");
        expect("cmd+equal", "increase_font_size");
        expect("cmd+minus", "decrease_font_size");
        expect("cmd+0", "reset_font_size");
        expect("cmd+k", "clear_screen");
        expect("cmd+shift+e", "compose_overlay");
        expect("cmd+shift+m", "prompt_queue_add");
        expect("ctrl+shift+space", "toggle_vi_mode");
        expect("cmd+s", "save_recipe");
        expect("cmd+shift+s", "save_recipe_as");
    }

    #[test]
    fn two_actions_share_one_chord_only_where_documented() {
        let m = Keymap::defaults();
        let conflicts = m.conflicts();
        // `redo` is documented as ⌘⇧Z *and* ⌘Y — different chords, so no clash.
        assert!(conflicts.is_empty(), "unexpected default conflicts: {conflicts:?}");
        assert_eq!(m.for_action("redo").len(), 2);
    }

    #[test]
    fn a_user_line_adds_a_chord_without_disturbing_the_others() {
        let m = Keymap::defaults().with_overrides(&["cmd+shift+k=clear_screen".into()]);
        assert_eq!(m.lookup("cmd+shift+k".parse().unwrap()), Some("clear_screen"));
        // The default chord still works: the line rebinds a chord, it does not
        // delete the action's other bindings.
        assert_eq!(m.lookup("cmd+k".parse().unwrap()), Some("clear_screen"));
        assert_eq!(m.for_action("clear_screen").len(), 2);

        // Rebinding an existing chord does take it over.
        let m = Keymap::defaults().with_overrides(&["cmd+k=clear_scrollback_above".into()]);
        assert_eq!(m.lookup("cmd+k".parse().unwrap()), Some("clear_scrollback_above"));
    }

    #[test]
    fn a_user_line_takes_over_a_chord_bound_elsewhere() {
        let m = Keymap::defaults().with_overrides(&["cmd+d=duplicate_tab".into()]);
        assert_eq!(m.lookup("cmd+d".parse().unwrap()), Some("duplicate_tab"));
        assert_eq!(m.lookup("cmd+opt+d".parse().unwrap()), Some("split_left"));
    }

    #[test]
    fn sequences_are_stored_but_do_not_shadow_single_chords() {
        let m = Keymap::defaults().with_overrides(&["cmd+k>cmd+t=new_tab".into()]);
        assert_eq!(m.lookup("cmd+k".parse().unwrap()), Some("clear_screen"));
        assert!(m.iter().any(|b| b.trigger.is_sequence()));
    }

    #[test]
    fn malformed_and_unknown_lines_are_rejected() {
        assert!(matches!(Binding::parse("cmd+t"), Err(BindingError::NoSeparator(_))));
        assert!(matches!(Binding::parse("=new_tab"), Err(BindingError::BadTrigger(..))));
        assert!(matches!(
            Binding::parse("cmd+t=no_such_action"),
            Err(BindingError::UnknownAction(_))
        ));
        // …and a bad line is skipped rather than poisoning the map.
        let m = Keymap::defaults().with_overrides(&["garbage=also_garbage".into()]);
        assert_eq!(m.len(), Keymap::defaults().len());
    }

    #[test]
    fn lines_round_trip_through_the_parser() {
        let m = Keymap::defaults();
        let lines = m.to_lines();
        let again = Keymap::empty().with_overrides(&lines);
        assert_eq!(again, m);
    }

    #[test]
    fn the_panel_writes_only_what_the_user_changed() {
        let m = Keymap::defaults();
        // Nothing changed, so nothing is written.
        assert!(m.to_overrides().is_empty());

        // One chord added, one rebound: exactly two lines come back.
        let m = Keymap::defaults().with_overrides(&[
            "cmd+shift+k=clear_screen".into(),
            "cmd+shift+y=show_scrollback_history".into(),
        ]);
        let lines = m.to_overrides();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().any(|l| l == "cmd+shift+k=clear_screen"));
        assert!(lines.iter().any(|l| l == "cmd+shift+y=show_scrollback_history"));

        // And reloading those lines reproduces the map that wrote them.
        let again = Keymap::defaults().with_overrides(&lines);
        assert_eq!(again, m);
    }

    #[test]
    fn every_action_falls_into_a_group() {
        for (id, _) in crate::actions::ACTIONS {
            let g = group_of(id);
            assert!(
                [
                    "general", "window", "tabs", "pane", "agents", "edit", "text_editing",
                    "scroll", "view", "recipe"
                ]
                .contains(&g),
                "{id} -> {g}"
            );
        }
    }

    #[test]
    fn searching_matches_ids_and_labels() {
        let m = Keymap::defaults();
        assert!(!m.search("split").is_empty());
        assert!(!m.search("Open Quickly").is_empty());
    }
}
