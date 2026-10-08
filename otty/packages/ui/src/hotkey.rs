//! Global hotkeys: handled before the terminal sees the chord.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalHotkey {
    Show,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyEffect {
    Raise,
    Hide,
    None,
}

/// What the chord does when another app may be in front.
///
/// Show: raise if Otty is not frontmost; do nothing if it already is.
/// Toggle: raise from the background, hide when Otty is already frontmost.
pub fn apply_global_hotkey(kind: GlobalHotkey, otty_front: bool) -> HotkeyEffect {
    match kind {
        GlobalHotkey::Show => {
            if otty_front {
                HotkeyEffect::None
            } else {
                HotkeyEffect::Raise
            }
        }
        GlobalHotkey::Toggle => {
            if otty_front {
                HotkeyEffect::Hide
            } else {
                HotkeyEffect::Raise
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn chord_raises_otty_when_another_app_is_front() {
        assert_eq!(
            apply_global_hotkey(GlobalHotkey::Show, false),
            HotkeyEffect::Raise
        );
        assert_eq!(
            apply_global_hotkey(GlobalHotkey::Show, true),
            HotkeyEffect::None
        );
        assert_eq!(
            apply_global_hotkey(GlobalHotkey::Toggle, true),
            HotkeyEffect::Hide
        );

        let mut d = Dispatcher::new();
        d.apply_hotkey(GlobalHotkey::Show, false);
        assert!(d.raised, "background Show must bring Otty forward");
        d.raised = false;
        d.apply_hotkey(GlobalHotkey::Show, true);
        assert!(!d.raised, "already front: Show is a no-op");
    }
}
