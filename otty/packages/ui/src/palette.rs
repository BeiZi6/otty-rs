//! Command Palette: every `ACTIONS` id, including unbound ones.
//!
//! Filter matches action id, label, and the current chord (`cmd+t` finds
//! `new_tab`). Execute goes through [`crate::Dispatcher`]; `keep_open` is ⌘↩.

use otty_config::actions::ACTIONS;
use otty_config::keybind::{group_of, Keymap};
use otty_config::Catalog;

/// Where a palette row applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteScope {
    Pane,
    Window,
    App,
}

impl PaletteScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pane => "pane",
            Self::Window => "window",
            Self::App => "app",
        }
    }

    fn from_group(group: &str) -> Self {
        match group {
            "pane" | "edit" | "view" | "text_editing" | "scroll" => Self::Pane,
            "tabs" | "window" => Self::Window,
            _ => Self::App,
        }
    }
}

/// One searchable row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub action: &'static str,
    pub label: String,
    pub shortcut: Option<String>,
    pub scope: PaletteScope,
}

/// The 152 `ACTIONS` rows, labels from the catalog when present.
pub fn palette_entries(catalog: Option<&Catalog>, map: &Keymap) -> Vec<PaletteEntry> {
    ACTIONS
        .iter()
        .map(|(id, label)| {
            let shortcut = map
                .for_action(id)
                .first()
                .map(|b| b.trigger.to_string());
            let label = catalog
                .and_then(|c| c.action_label(id).map(str::to_string))
                .unwrap_or_else(|| (*label).to_string());
            PaletteEntry {
                action: id,
                label,
                shortcut,
                scope: PaletteScope::from_group(group_of(id)),
            }
        })
        .collect()
}

/// Filter by action id, label, or chord. Empty query keeps every row.
pub fn filter_palette<'a>(entries: &'a [PaletteEntry], query: &str) -> Vec<&'a PaletteEntry> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return entries.iter().collect();
    }
    entries
        .iter()
        .filter(|e| {
            e.action.to_ascii_lowercase().contains(&q)
                || e.label.to_ascii_lowercase().contains(&q)
                || e.shortcut
                    .as_deref()
                    .is_some_and(|s| s.to_ascii_lowercase().contains(&q))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;
    use otty_config::keybind::Keymap;

    fn map() -> Keymap {
        Keymap::defaults()
    }

    #[test]
    fn every_action_is_in_the_palette_including_unbound() {
        let map = map();
        let entries = palette_entries(None, &map);
        assert_eq!(entries.len(), ACTIONS.len());
        let mut unbound = 0;
        for (id, _) in ACTIONS {
            assert!(
                entries.iter().any(|e| e.action == *id),
                "palette missing {id}"
            );
            if map.for_action(id).is_empty() {
                unbound += 1;
            }
        }
        assert!(unbound > 0, "expected unbound ACTIONS in the palette");
        assert!(filter_palette(&entries, "")
            .iter()
            .any(|e| e.shortcut.is_none()));
    }

    #[test]
    fn query_finds_each_action_by_id_and_cmd_t_finds_the_chord() {
        let map = map();
        let entries = palette_entries(None, &map);
        for (id, _) in ACTIONS {
            let hits = filter_palette(&entries, id);
            assert!(
                hits.iter().any(|e| e.action == *id),
                "query `{id}` did not find itself"
            );
        }
        let by_chord = filter_palette(&entries, "cmd+t");
        assert!(
            by_chord.iter().any(|e| e.action == "new_tab"),
            "cmd+t should find new_tab"
        );
    }

    #[test]
    fn enter_closes_palette_cmd_enter_keeps_it_open() {
        let mut d = Dispatcher::new();
        assert!(d.activate("command_palette"));
        assert!(d.palette_open);
        d.run_palette("new_tab", false);
        assert!(!d.palette_open);
        assert_eq!(d.last.as_deref(), Some("new_tab"));

        d.activate("command_palette");
        d.run_palette("copy", true);
        assert!(d.palette_open, "⌘↩ must keep the palette open");
        assert_eq!(d.last.as_deref(), Some("copy"));
    }
}
