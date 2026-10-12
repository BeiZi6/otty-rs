//! Right-click menu: which action ids a hit enables.
//!
//! Copy when there is a selection, Copy Path when the click covers a path,
//! Close Tab when the hit is a tab. Shell binds; this decides.

/// What the pointer landed on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextHit {
    pub selection: Option<String>,
    pub path: Option<String>,
    pub on_tab: bool,
}

/// One enabled context-menu row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    pub action: &'static str,
    pub label: &'static str,
}

/// Enabled items for this hit. Disabled actions are omitted, not greyed.
pub fn context_items(hit: &ContextHit) -> Vec<ContextItem> {
    let mut items = Vec::new();
    if hit
        .selection
        .as_deref()
        .is_some_and(|s| !s.is_empty())
    {
        items.push(ContextItem {
            action: "copy",
            label: "Copy",
        });
    }
    if hit.path.as_deref().is_some_and(|s| !s.is_empty()) {
        items.push(ContextItem {
            action: "copy_path",
            label: "Copy Path",
        });
    }
    if hit.on_tab {
        items.push(ContextItem {
            action: "close_tab",
            label: "Close Tab",
        });
    }
    items
}

/// Build a pane hit from a screen line, a column, and any current selection.
pub fn pane_hit(line: &str, col: usize, selection: Option<&str>) -> ContextHit {
    ContextHit {
        selection: selection
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        path: otty_term::path_at(line, col),
        on_tab: false,
    }
}

pub fn tab_hit() -> ContextHit {
    ContextHit {
        on_tab: true,
        ..ContextHit::default()
    }
}

/// Rows the frontend feeds a context menu: `(label, action)`.
pub fn context_rows(hit: &ContextHit) -> Vec<(String, String)> {
    context_items(hit)
        .into_iter()
        .map(|i| (i.label.to_string(), i.action.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn selection_offers_copy_and_copy_fills_the_clipboard() {
        let hit = pane_hit("hello world", 0, Some("hello"));
        let items = context_items(&hit);
        assert_eq!(items.iter().map(|i| i.action).collect::<Vec<_>>(), ["copy"]);
        let mut d = Dispatcher::new();
        d.hit = hit;
        assert!(d.activate("copy"));
        assert_eq!(d.clipboard.as_deref(), Some("hello"));
    }

    #[test]
    fn path_offers_copy_path_and_copy_path_fills_the_clipboard() {
        let hit = pane_hit("see /Users/me/src/lib.rs", 8, None);
        assert_eq!(hit.path.as_deref(), Some("/Users/me/src/lib.rs"));
        let items = context_items(&hit);
        assert_eq!(
            items.iter().map(|i| i.action).collect::<Vec<_>>(),
            ["copy_path"]
        );
        let mut d = Dispatcher::new();
        d.hit = hit;
        assert!(d.activate("copy_path"));
        assert_eq!(d.clipboard.as_deref(), Some("/Users/me/src/lib.rs"));
    }

    #[test]
    fn tab_offers_close_tab_and_close_tab_closes() {
        let items = context_items(&tab_hit());
        assert_eq!(
            items.iter().map(|i| i.action).collect::<Vec<_>>(),
            ["close_tab"]
        );
        let mut d = Dispatcher::new();
        assert!(d.activate("new_tab"));
        assert_eq!(d.app.counts().1, 2);
        assert!(d.close_tab_at(1));
        assert_eq!(d.app.counts().1, 1);
    }

    #[test]
    fn empty_pane_hit_has_no_copy_rows() {
        let items = context_items(&pane_hit("hello", 1, None));
        assert!(items.is_empty());
    }
}
