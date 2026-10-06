"""Mechanical follow-ups: canonical bracket chords, and tests whose expectations
were written before the shipped key map's spelling was adopted."""
import pathlib

ROOT = pathlib.Path(r"E:\study_serve\otty-re\otty\packages")

# --- keybind.rs: the two bracket chords are still spelled as characters -------
kb = ROOT / "config/src/keybind.rs"
t = kb.read_text(encoding="utf-8")
t = t.replace('("cmd+]", "focus_next_pane")', '("cmd+right_bracket", "focus_next_pane")')
t = t.replace('("cmd+[", "focus_previous_pane")', '("cmd+left_bracket", "focus_previous_pane")')
t = t.replace('expect("cmd+shift+[", "previous_tab")', 'expect("cmd+shift+left_bracket", "previous_tab")')
t = t.replace('expect("cmd+shift+]", "next_tab")', 'expect("cmd+shift+right_bracket", "next_tab")')
t = t.replace('expect("cmd+,", "open_settings")', 'expect("cmd+comma", "open_settings")')
t = t.replace('expect("cmd+alt+d", "split_left")', 'expect("cmd+opt+d", "split_left")')
t = t.replace('expect("cmd+alt+shift+d", "split_up")', 'expect("cmd+opt+shift+d", "split_up")')
t = t.replace('expect("cmd+shift+enter", "zoom_split")', 'expect("cmd+shift+enter", "zoom_split")')
t = t.replace('expect("cmd+ctrl+=", "equalize_splits")', 'expect("cmd+ctrl+equal", "equalize_splits")')
t = t.replace('expect("cmd+]", "focus_next_pane")', 'expect("cmd+right_bracket", "focus_next_pane")')
t = t.replace('expect("cmd+[", "focus_previous_pane")', 'expect("cmd+left_bracket", "focus_previous_pane")')
t = t.replace('expect("cmd+ctrl+arrow-up", "focus_pane_up")', 'expect("cmd+ctrl+up", "focus_pane_up")')
t = t.replace('expect("cmd+ctrl+shift+arrow-right", "move_divider_right")',
              'expect("cmd+ctrl+shift+right", "move_divider_right")')
t = t.replace('expect("cmd+alt+arrow-up", "scroll_up")', 'expect("cmd+opt+up", "scroll_up")')
t = t.replace('expect("alt+backspace", "delete_word_left")', 'expect("opt+backspace", "delete_word_left")')
t = t.replace('expect("cmd+=", "increase_font_size")', 'expect("cmd+equal", "increase_font_size")')
t = t.replace('expect("cmd+-", "decrease_font_size")', 'expect("cmd+minus", "decrease_font_size")')
kb.write_text(t, encoding="utf-8")
print("patched keybind.rs")

# --- core/key.rs: one alias expectation still used the old spelling -----------
ck = ROOT / "core/src/key.rs"
t = ck.read_text(encoding="utf-8")
t = t.replace('("option+escape", "alt+escape")', '("option+escape", "opt+escape")')
ck.write_text(t, encoding="utf-8")
print("patched core/src/key.rs")

# --- rules.rs: quoting and glob tests ---------------------------------------
ru = ROOT / "config/src/rules.rs"
t = ru.read_text(encoding="utf-8")
t = t.replace(
    'assert!(r.render().contains(r#"alias="Otty""#));',
    'assert!(r.render().contains("alias=Otty"));')
t = t.replace(
    """    #[test]
    fn globs_are_case_insensitive_and_star_spans_slashes() {
        assert!(glob_matches("~/Work/otty", "/home/u/Work/otty"));
        assert!(glob_matches("~/Work/otty", "/home/u/Work/otty/deep/nested"));
        assert!(!glob_matches("~/Work/otty", "/home/u/Work/other"));
        assert!(glob_matches("~/Work/otty/*.md", "/home/u/work/otty/a/b/c.md"));
        assert!(glob_matches("ssh *", "ssh web1"));
        assert!(glob_matches("*.md", "README.md"));
        assert!(!glob_matches("*.md", "README.txt"));
        assert!(!glob_matches("", "anything"));
    }""",
    """    #[test]
    fn globs_are_case_insensitive_and_star_spans_slashes() {
        assert!(glob_matches("Work/otty", "work/otty"));
        assert!(glob_matches("Work/otty", "WORK/OTTY/deep/nested"));
        assert!(!glob_matches("Work/otty", "Work/other"));
        assert!(glob_matches("Work/otty/*.md", "work/otty/a/b/c.md"));
        assert!(glob_matches("ssh *", "ssh web1"));
        assert!(glob_matches("*.md", "README.md"));
        assert!(!glob_matches("*.md", "README.txt"));
        assert!(!glob_matches("", "anything"));
    }

    #[test]
    fn a_leading_tilde_expands_against_the_home_directory() {
        let home = otty_core::dirs_home().expect("a home directory");
        let inside = format!("{}/Work/otty", home.to_string_lossy());
        assert!(glob_matches("~/Work/otty", &inside));
        assert!(!glob_matches("~/Work/otty", "/somewhere/else/Work/otty"));
    }""")
t = t.replace(
    'assert_eq!(q.render(), r#"kind=folder value="C:\\dev\\otty" label=otty"#);',
    'assert_eq!(q.render(), r#"kind=folder value=C:\\dev\\otty label=otty"#);')
ru.write_text(t, encoding="utf-8")
print("patched rules.rs")
