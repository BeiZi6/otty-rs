//! Details panel: right rail that always follows the focused pane.
//!
//! ⌘⇧R toggles it. Info shows that pane's cwd. Switching tabs re-reads the
//! new focused pane so the panel cannot lag behind.

use crate::model::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailsTab {
    Info,
    Outline,
    Git,
    Files,
}

impl DetailsTab {
    pub fn from_action(action: &str) -> Option<Self> {
        Some(match action {
            "details_tab_info" => Self::Info,
            "details_tab_outline" => Self::Outline,
            "details_tab_git" => Self::Git,
            "details_tab_files" => Self::Files,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Outline => "outline",
            Self::Git => "git",
            Self::Files => "files",
        }
    }
}

/// What Info shows for the focused pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailsInfo {
    pub cwd: String,
}

/// Info for the focused pane of the active tab. Empty cwd when none is known.
pub fn details_info(app: &App) -> DetailsInfo {
    let cwd = app
        .active_window()
        .and_then(|w| w.active_tab())
        .and_then(|t| app.pane(t.focused))
        .and_then(|p| p.cwd.clone())
        .unwrap_or_default();
    DetailsInfo { cwd }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn cmd_shift_r_toggles_the_panel() {
        let mut d = Dispatcher::new();
        assert!(!d.details_open);
        assert!(d.activate("toggle_details_panel"));
        assert!(d.details_open);
        assert!(d.activate("toggle_details_panel"));
        assert!(!d.details_open);
    }

    #[test]
    fn info_shows_cwd_and_follows_the_active_tab() {
        let mut d = Dispatcher::new();
        d.activate("toggle_details_panel");
        d.activate("details_tab_info");
        assert_eq!(d.details_tab, DetailsTab::Info);

        let first = d.app.active_window().unwrap().active_tab().unwrap().focused;
        d.app.pane_mut(first).unwrap().cwd = Some("/work/one".into());
        assert_eq!(details_info(&d.app).cwd, "/work/one");

        assert!(d.activate("new_tab"));
        let second = d.app.active_window().unwrap().active_tab().unwrap().focused;
        assert_ne!(first, second);
        d.app.pane_mut(second).unwrap().cwd = Some("/work/two".into());
        assert_eq!(details_info(&d.app).cwd, "/work/two");

        assert!(d.activate("previous_tab"));
        assert_eq!(
            d.app.active_window().unwrap().active_tab().unwrap().focused,
            first
        );
        assert_eq!(details_info(&d.app).cwd, "/work/one");
    }
}
