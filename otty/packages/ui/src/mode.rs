//! Hint / Vi / Mark: keys stay in the shell, not the PTY.

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Shell,
    Hint,
    Vi,
    Mark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintLabel {
    pub tag: String,
    pub url: String,
}

/// Label URLs in screen text with `a`, `s`, `d`, …
pub fn hint_labels(lines: &[String]) -> Vec<HintLabel> {
    let re = Regex::new(r"https?://[^\s]+").expect("url regex");
    let alphabet = b"asdfghjkl";
    let mut out = Vec::new();
    for line in lines {
        for m in re.find_iter(line) {
            let tag = alphabet
                .get(out.len())
                .copied()
                .map(|c| (c as char).to_string())
                .unwrap_or_else(|| out.len().to_string());
            let url = m.as_str().trim_end_matches(['.', ',', ';', ')']).to_owned();
            out.push(HintLabel { tag, url });
        }
    }
    out
}

pub fn open_hint<'a>(labels: &'a [HintLabel], tag: &str) -> Option<&'a str> {
    labels
        .iter()
        .find(|h| h.tag == tag)
        .map(|h| h.url.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViState {
    pub line: usize,
    pub total: usize,
    pub visual_from: Option<usize>,
}

impl ViState {
    pub fn new(total: usize) -> Self {
        Self {
            line: total.saturating_sub(1),
            total,
            visual_from: None,
        }
    }

    /// Handle a vi key. Returns `true` if the key must not reach the shell.
    pub fn handle(&mut self, key: &str) -> bool {
        match key {
            "g" | "gg" => {
                self.line = 0;
                true
            }
            "j" => {
                if self.line + 1 < self.total {
                    self.line += 1;
                }
                true
            }
            "k" => {
                self.line = self.line.saturating_sub(1);
                true
            }
            "y" | "enter" | "\n" => true,
            _ => true,
        }
    }

    pub fn yank_range(&self) -> (usize, usize) {
        match self.visual_from {
            Some(from) => (from.min(self.line), from.max(self.line)),
            None => (self.line, self.line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn hint_labels_a_url_and_vi_gg_y_swallows_j() {
        let labels = hint_labels(&["see https://docs.otty.sh/x".into()]);
        assert_eq!(labels[0].tag, "a");
        assert_eq!(open_hint(&labels, "a"), Some("https://docs.otty.sh/x"));

        let mut d = Dispatcher::new();
        assert!(d.activate("toggle_hint_mode"));
        assert_eq!(d.mode, InputMode::Hint);
        d.hint_labels = labels;
        assert_eq!(d.open_hint("a").as_deref(), Some("https://docs.otty.sh/x"));

        assert!(d.activate("toggle_vi_mode"));
        assert_eq!(d.mode, InputMode::Vi);
        d.vi = ViState::new(10);
        d.vi.line = 9;
        assert!(d.handle_mode_key("gg"), "gg swallowed");
        assert_eq!(d.vi.line, 0, "gg goes to top");
        d.hit.selection = Some("copied".into());
        assert!(d.handle_mode_key("y"));
        assert_eq!(d.clipboard.as_deref(), Some("copied"));
        assert_eq!(d.mode, InputMode::Shell, "y exits vi");
        d.activate("toggle_vi_mode");
        assert!(d.handle_mode_key("j"));
        assert_eq!(d.mode, InputMode::Vi, "j never leaves vi and never reaches the shell");
    }
}
