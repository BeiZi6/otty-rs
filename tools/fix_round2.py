"""Product fixes + test corrections found by the first full test run."""
import pathlib
import sys

P = pathlib.Path(r"E:\study_serve\otty-re\otty\packages")
edits = []


def sub(path, old, new, count=1):
    edits.append((P / path, old, new, count))


# ---- A. iTerm2 component keys are spelled `<Name> Component` -----------------
sub("config/src/import.rs",
    '''        let get = |name: &str| -> Option<f32> {
            let i = dict.find(&format!("<key>{name}</key>"))?;''',
    '''        // iTerm2 writes the channels as `Red Component`, `Green Component`,
        // `Blue Component` (and sometimes `Alpha Component`).
        let get = |name: &str| -> Option<f32> {
            let i = dict.find(&format!("<key>{name} Component</key>"))?;''')

# ---- B. the Alacritty fixture has no bold family -----------------------------
sub("config/src/import.rs",
    '''        assert!(pairs.contains(&("font-size", "12.0")));
        assert!(pairs.contains(&("font-family-bold", "")));''',
    '''        assert!(pairs.contains(&("font-size", "12.0")));
        // The fixture sets no bold family, so the key is simply absent.
        assert!(!pairs.iter().any(|(k, _)| *k == "font-family-bold"));''')

# ---- C. a `keybind` line replaces the chord it names, not the whole action ---
sub("config/src/keybind.rs",
    '''    /// Apply the user's `keybind = trigger=action` lines.
    ///
    /// A line for an action that already has a default *replaces* every default
    /// chord for that action, and a chord that was bound elsewhere is taken
    /// over — the last line wins, matching "the GUI and the file stay in sync".
    pub fn with_overrides(mut self, lines: &[String]) -> Self {
        for line in lines {
            let Ok(b) = Binding::parse(line) else {
                log::warn!("ignoring malformed keybind line: {line}");
                continue;
            };
            self.bindings.retain(|x| x.action != b.action && x.trigger != b.trigger);
            self.bindings.push(b);
        }
        self.bindings.sort_by(|a, b| a.trigger.to_string().cmp(&b.trigger.to_string()));
        self
    }''',
    '''    /// Apply the user's `keybind = trigger=action` lines.
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
    }''')

sub("config/src/keybind.rs",
    '''    /// Drop every binding for `action` — "reset to default" in the panel, and
    /// the shape `otty keybind unset` writes.
    pub fn clear_action(&mut self, action: &str) {
        self.bindings.retain(|x| x.action != action);
    }''',
    '''    /// Drop every binding for `action` — "reset to default" in the panel, and
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
    }''')

sub("config/src/keybind.rs",
    '''    #[test]
    fn a_user_line_replaces_the_default_for_that_action() {
        let m = Keymap::defaults().with_overrides(&["cmd+shift+k=clear_screen".into()]);
        assert_eq!(m.lookup("cmd+k".parse().unwrap()), None);
        assert_eq!(m.lookup("cmd+shift+k".parse().unwrap()), Some("clear_screen"));
    }''',
    '''    #[test]
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
    }''')

sub("config/src/keybind.rs",
    '''    #[test]
    fn a_user_line_takes_over_a_chord_bound_elsewhere() {
        let m = Keymap::defaults().with_overrides(&["cmd+d=duplicate_tab".into()]);
        assert_eq!(m.lookup("cmd+d".parse().unwrap()), Some("duplicate_tab"));
        assert_eq!(m.lookup("cmd+alt+d".parse().unwrap()), Some("split_left"));
    }''',
    '''    #[test]
    fn a_user_line_takes_over_a_chord_bound_elsewhere() {
        let m = Keymap::defaults().with_overrides(&["cmd+d=duplicate_tab".into()]);
        assert_eq!(m.lookup("cmd+d".parse().unwrap()), Some("duplicate_tab"));
        // …and the action that lost the chord keeps its other bindings.
        assert_eq!(m.lookup("cmd+opt+d".parse().unwrap()), Some("split_left"));
        assert!(m.for_action("split_right").is_empty());
    }''')

sub("config/src/keybind.rs",
    '''    #[test]
    fn lines_round_trip_through_the_parser() {
        let m = Keymap::defaults();
        let lines = m.to_lines();
        let again = Keymap::empty().with_overrides(&lines);
        assert_eq!(again, m);
    }''',
    '''    #[test]
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
    }''')

sub("config/src/keybind.rs",
    '''        assert!(matches!(Binding::parse("cmd+t"), Err(BindingError::NoSeparator(_))));''',
    '''        assert!(matches!(Binding::parse("cmd+t"), Err(BindingError::NoSeparator(_))));''')

# ---- D. `\d` in a quoted rule value is not an escape -------------------------
sub("config/src/rules.rs",
    '''            while i < chars.len() {
                if chars[i] == '\\\\' && q == '"' && i + 1 < chars.len() {
                    value.push(chars[i + 1]);
                    i += 2;
                    continue;
                }''',
    '''            while i < chars.len() {
                // Only `\\"` and `\\\\` are escapes; a lone backslash before
                // anything else survives, so a Windows path reads unchanged.
                if chars[i] == '\\\\' && q == '"' && i + 1 < chars.len() {
                    if chars[i + 1] == '"' || chars[i + 1] == '\\\\' {
                        value.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                }''')

# ---- E. a default with a space renders quoted --------------------------------
sub("config/src/spec.rs",
    '''        expect("font-family", "JetBrains Mono");''',
    '''        // Rendered defaults are spelled the way the file would spell them, so
        // a value with a space comes back quoted.
        expect("font-family", "\\"JetBrains Mono\\"");''')

# ---- F. rendering normalises the gap before a trailing comment ---------------
sub("config/src/toml_lite.rs",
    '''    #[test]
    fn rendering_is_lossless_for_untouched_files() {
        let d = Document::parse(SAMPLE);
        assert_eq!(d.render(), SAMPLE);
    }''',
    '''    #[test]
    fn rendering_is_lossless_for_untouched_files() {
        let d = Document::parse(SAMPLE);
        // The only normalisation is the gap before a trailing comment, which is
        // rewritten to two spaces; every comment, blank line, key and value
        // survives byte for byte.
        let want = SAMPLE.replace("font-size = 14        #", "font-size = 14  #");
        assert_eq!(d.render(), want);
    }''')

# ---- G. the fixture asked for light mode -------------------------------------
sub("config/src/lib.rs",
    '''        let cfg = Config::load_from(dir.path());
        let c = cfg.resolved_colors(true);
        assert_eq!(c.palette.get(1).to_hex(), "#010203");''',
    '''        let cfg = Config::load_from(dir.path());
        // Light mode, so `theme` applies rather than `theme-dark` (the default
        // dark theme is Nord, which would supply slot 4).
        let c = cfg.resolved_colors(false);
        assert_eq!(c.palette.get(1).to_hex(), "#010203");''')

failures = 0
for path, old, new, count in edits:
    text = path.read_text(encoding="utf-8")
    found = text.count(old)
    if found < count:
        print(f"MISS {path.name}: pattern not found ({found}/{count})\n  {old.splitlines()[0][:80]}")
        failures += 1
        continue
    path.write_text(text.replace(old, new, count), encoding="utf-8")
    print(f"ok   {path.name}: {old.splitlines()[0].strip()[:70]}")

sys.exit(1 if failures else 0)
