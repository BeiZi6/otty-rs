//! Visual reorder for RTL. The grid stays logical so copy/find are unchanged.

use unicode_bidi::BidiInfo;

/// Reorder `logical` for display when `enabled`. Identity when disabled.
pub fn visual_order(logical: &str, enabled: bool) -> String {
    if !enabled || logical.is_empty() {
        return logical.to_owned();
    }
    let info = BidiInfo::new(logical, None);
    let Some(para) = info.paragraphs.first() else {
        return logical.to_owned();
    };
    info.reorder_line(para, para.range.clone()).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtl_line_is_reordered_when_enabled_and_left_alone_when_not() {
        let heb = "אב";
        let vis = visual_order(heb, true);
        assert_ne!(vis, heb, "Hebrew must reverse for reading order");
        assert_eq!(visual_order(heb, false), heb);
        assert_eq!(visual_order("ab", true), "ab");
    }

    #[test]
    fn csi_8_l_turns_off_implicit_reorder() {
        use crate::perform::Parser;
        use crate::screen::{TermConfig, Terminal};
        let mut t = Terminal::new(20, 2, TermConfig::default());
        let mut p = Parser::new();
        assert!(t.bidi_implicit);
        p.advance(&mut t, b"\x1b[8l");
        assert!(!t.bidi_implicit, "BDSM mode 8 reset disables implicit bidi");
        p.advance(&mut t, b"\x1b[8h");
        assert!(t.bidi_implicit);
    }
}
