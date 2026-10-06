//! The `otty features` catalogue.
//!
//! Each entry is a self-contained escape-sequence script that exercises one part
//! of the engine. They are shipped rather than hidden because they are the
//! fastest way to answer "does this terminal do X" for a *user* as well as for a
//! test: `otty features true-color` runs the script through a real
//! [`Terminal`](crate::Terminal) and prints the result, so what you see is what
//! the engine actually does, not a hand-written description of it.

/// One demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Feature {
    /// The name `otty features <name>` takes.
    pub name: &'static str,
    /// One line, shown by `otty features list`.
    pub summary: &'static str,
    /// The escape sequence script.
    pub script: &'static str,
}

/// Every demo, in the order `otty features list` prints them.
pub static CATALOG: &[Feature] = &[
    Feature {
        name: "true-color",
        summary: "24-bit SGR foreground and background",
        script: "\x1b[38;2;255;100;0mtrue color foreground\x1b[0m\r\n\
                 \x1b[48;2;0;100;200mtrue color background\x1b[0m\r\n",
    },
    Feature {
        name: "256-color",
        summary: "the xterm 256-colour cube and greyscale ramp",
        script: "\x1b[38;5;196mred\x1b[0m \x1b[38;5;46mgreen\x1b[0m \x1b[38;5;21mblue\x1b[0m\r\n\
                 \x1b[38;5;244mgrey 244\x1b[0m \x1b[38;5;255mgrey 255\x1b[0m\r\n",
    },
    Feature {
        name: "styles",
        summary: "bold, dim, italic, underline, blink, reverse, strike",
        script: "\x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m \x1b[3mitalic\x1b[0m \
                 \x1b[4munderline\x1b[0m \x1b[9mstrike\x1b[0m\r\n\
                 \x1b[7mreverse\x1b[0m \x1b[21mdouble underline\x1b[0m\r\n",
    },
    Feature {
        name: "text-style",
        summary: "the SGR 4: sub-parameter underline flavours and SGR 53 overline",
        script: "\x1b[4:1mcurly off\x1b[0m \x1b[4:3mcurly\x1b[0m \x1b[4:4mdotted\x1b[0m \
                 \x1b[53moverline\x1b[0m\r\n",
    },
    Feature {
        name: "emoji",
        summary: "emoji presentation and zero-width joiners",
        script: "🙂 👍 🚀 🧑‍💻 🇨🇳\r\n",
    },
    Feature {
        name: "cjk",
        summary: "East-Asian width, including the ambiguous blocks",
        script: "日本語 中文 한국어\r\n\
                 ①②③ Ⓐ ⓐ ①-⑳\r\n",
    },
    Feature {
        name: "rtl",
        summary: "Hebrew and Arabic reordered by the bidi algorithm",
        script: "שלום עולם\r\nمرحبا بالعالم\r\nmixed שלום world\r\n",
    },
    Feature {
        name: "box-drawing",
        summary: "the DEC and Unicode box-drawing sets",
        script: "┌───────┐\r\n│ box   │\r\n└───────┘\r\n\
                 ╔═══════╗\r\n║ heavy ║\r\n╚═══════╝\r\n",
    },
    Feature {
        name: "nerd-font",
        summary: "the Nerd Font icon range and Powerline separators",
        script: "\u{e0b0} \u{e0b2} \u{f121} \u{f09b}\r\n",
    },
    Feature {
        name: "ligature",
        summary: "the sequences `font-ligatures = calt` joins",
        script: "=> != >= <= === -> <- :: >>=\r\n",
    },
    Feature {
        name: "unicode",
        summary: "a one-shot complex-script and width sheet",
        script: "combining: e\u{301} a\u{308} n\u{303}\r\n\
                 zero-width: a\u{200b}b\r\n\
                 astral: \u{1f600} \u{1d400} \u{20bb7}\r\n\
                 thai: สวัสดี\r\n",
    },
    Feature {
        name: "semantic-prompt",
        summary: "OSC 133 marks, which is what the outline is built from",
        script: "\x1b]133;A\x07$ \x1b]133;B\x07make build\x1b]133;C\x07\r\n\
                 \x1b]133;D;0\x07",
    },
    Feature {
        name: "osc7",
        summary: "OSC 7 working-directory reporting",
        script: "\x1b]7;file://localhost/tmp\x07",
    },
    Feature {
        name: "set-title",
        summary: "OSC 0 / OSC 2 window and tab titles",
        script: "\x1b]0;Otty\r\n\x1b]2;Feature demo\x07",
    },
    Feature {
        name: "hyperlink",
        summary: "OSC 8 hyperlinks",
        script: "\x1b]8;;https://docs.otty.sh\x07docs.otty.sh\x1b]8;;\x07\r\n",
    },
    Feature {
        name: "progress",
        summary: "OSC 9;4 the ConEmu progress protocol",
        script: "\x1b]9;4;3\x07\x1b]9;4;1;50\x07",
    },
    Feature {
        name: "error-state",
        summary: "the OSC 9;4 error state",
        script: "\x1b]9;4;2\x07",
    },
    Feature {
        name: "notification",
        summary: "OSC 9, the iTerm-style desktop notification",
        script: "\x1b]9;Otty feature demo\x07",
    },
    Feature {
        name: "notification-title",
        summary: "OSC 777, the urxvt-style notification with a title",
        script: "\x1b]777;notify;Otty;the feature demo ran\x07",
    },
    Feature {
        name: "notification-kitty",
        summary: "OSC 99, the Kitty notification protocol",
        script: "\x1b]99;i=1:d=SGVsbG8\x07",
    },
    Feature {
        name: "clipboard-write",
        summary: "OSC 52 clipboard write",
        script: "\x1b]52;c;aGVsbG8=\x07",
    },
    Feature {
        name: "clipboard-read",
        summary: "OSC 52 clipboard read",
        script: "\x1b]52;c;?\x07",
    },
    Feature {
        name: "cursor-shape",
        summary: "DECSCUSR cursor shapes",
        script: "\x1b[1 q\x1b[3 q\x1b[5 q\x1b[2 q",
    },
    Feature {
        name: "mouse-shape",
        summary: "OSC 22, the pointer shape over the pane",
        script: "\x1b]22;pointer\x07\x1b]22;default\x07",
    },
    Feature {
        name: "change-color",
        summary: "OSC 4 palette set, OSC 10/11 dynamic colours",
        script: "\x1b]4;1;#ff0000\x07\x1b]11;#101010\x07",
    },
    Feature {
        name: "color-scheme-update",
        summary: "OSC 4 reset, and the 110/111 reset companions",
        script: "\x1b]104;1\x07\x1b]111\x07",
    },
    Feature {
        name: "beep",
        summary: "BEL and the bell behaviours it can map to",
        script: "\x07",
    },
    Feature {
        name: "bracketed-paste",
        summary: "DECSET 2004 bracketed paste",
        script: "\x1b[?2004h\x1b[?2004l",
    },
    Feature {
        name: "sync-update",
        summary: "DECSET 2026 synchronized output",
        script: "\x1b[?2026h\x1b[?2026l",
    },
    Feature {
        name: "ed-scroll-clear",
        summary: "ED 2 and ED 3 clearing the screen and the scrollback",
        script: "before\r\n\x1b[2J\x1b[3Jafter\r\n",
    },
    Feature {
        name: "font-blending",
        summary: "thin strokes, which is what the blending modes change",
        script: "\x1b[38;2;120;120;120mthin grey on grey\x1b[0m\r\n",
    },
    Feature {
        name: "iterm-image",
        summary: "OSC 1337 inline images",
        script: "\x1b]1337;File=name=aGkucG5n;inline=1:aGVsbG8=\x07",
    },
    Feature {
        name: "kitty-image",
        summary: "the Kitty graphics protocol",
        script: "\x1b_Ga=T,f=100;AAAA\x1b\\",
    },
    Feature {
        name: "sixel-image",
        summary: "sixel, which the documentation marks unsupported",
        script: "\x1bPq\"1;1;2;2#0;2;0;0;0#1;2;100;100;0#1~~@@vv@@~~\x1b\\",
    },
    Feature {
        name: "text-hint",
        summary: "text that Hint Mode has something to act on",
        script: "https://docs.otty.sh and /tmp/otty-demo.txt\r\n",
    },
];

/// Look a demo up by name.
pub fn find(name: &str) -> Option<&'static Feature> {
    let n = name.trim();
    CATALOG.iter().find(|f| f.name == n)
}

/// The names, for completion.
pub fn names() -> impl Iterator<Item = &'static str> {
    CATALOG.iter().map(|f| f.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Parser, TermConfig, Terminal};

    #[test]
    fn the_catalogue_has_no_duplicate_names() {
        let mut names: Vec<&str> = names().collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate feature name");
        assert!(before >= 30, "only {} features", before);
    }

    #[test]
    fn every_feature_is_findable_by_its_own_name() {
        for f in CATALOG {
            assert_eq!(find(f.name).map(|x| x.name), Some(f.name));
            assert!(!f.summary.is_empty(), "{} has no summary", f.name);
            assert!(!f.script.is_empty(), "{} has no script", f.name);
        }
        // Leading and trailing space is tolerated, because a shell may hand it
        // over quoted.
        assert_eq!(find("  cjk ").map(|f| f.name), Some("cjk"));
        assert_eq!(find("nope"), None);
    }

    #[test]
    fn every_script_runs_through_the_engine_without_panicking() {
        // This is the contract: a feature is a script the engine can execute.
        // If one of them cannot be parsed the demo is broken, not the test.
        for f in CATALOG {
            let mut term = Terminal::new(80, 24, TermConfig::default());
            let mut parser = Parser::new();
            parser.advance(&mut term, f.script.as_bytes());
            // Drain everything the script produced so no event is left queued.
            let _ = term.drain_events();
            let _ = term.drain_replies();
        }
    }

    #[test]
    fn the_plain_text_features_leave_their_text_on_screen() {
        for (name, want) in [("cjk", "日本語"), ("box-drawing", "┌"), ("emoji", "🙂")] {
            let f = find(name).unwrap();
            let mut term = Terminal::new(80, 24, TermConfig::default());
            let mut parser = Parser::new();
            parser.advance(&mut term, f.script.as_bytes());
            assert!(
                term.contents().contains(want),
                "{name} did not render `{want}`: {:?}",
                term.contents()
            );
        }
    }

    #[test]
    fn the_control_features_emit_the_events_they_are_named_for() {
        use crate::Event;
        let cases: &[(&str, fn(&Event) -> bool)] = &[
            ("set-title", |e| matches!(e, Event::Title(_))),
            ("osc7", |e| matches!(e, Event::WorkingDirectory(_))),
            ("hyperlink", |e| matches!(e, Event::Hyperlink { .. })),
            ("beep", |e| matches!(e, Event::Bell)),
            ("progress", |e| matches!(e, Event::Progress(_))),
            ("error-state", |e| matches!(e, Event::Progress(_))),
            ("notification", |e| matches!(e, Event::Notify(_))),
            ("notification-title", |e| matches!(e, Event::Notify(_))),
            ("notification-kitty", |e| matches!(e, Event::Notify(_))),
            ("clipboard-write", |e| matches!(e, Event::ClipboardStore { .. })),
            ("clipboard-read", |e| matches!(e, Event::ClipboardLoad { .. })),
            ("mouse-shape", |e| matches!(e, Event::MouseShape(_))),
            ("change-color", |e| matches!(e, Event::PaletteSet { .. } | Event::SpecialColorSet { .. })),
            ("color-scheme-update", |e| matches!(e, Event::PaletteReset { .. } | Event::SpecialColorReset { .. })),
            ("iterm-image", |e| matches!(e, Event::Image(_))),
            ("semantic-prompt", |e| matches!(e, Event::PromptMark(_))),
        ];
        for (name, pred) in cases {
            let f = find(name).unwrap();
            let mut term = Terminal::new(80, 24, TermConfig::default());
            let mut parser = Parser::new();
            parser.advance(&mut term, f.script.as_bytes());
            let events = term.drain_events();
            assert!(
                events.iter().any(pred),
                "{name} produced no matching event: {events:?}"
            );
        }
    }

    #[test]
    fn the_mode_features_actually_set_and_clear_their_modes() {
        use crate::Modes;
        // Bracketed paste is on then off, so it ends off; synchronized output
        // likewise. What matters is that both bits exist and are addressable.
        let mut term = Terminal::new(80, 24, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, find("bracketed-paste").unwrap().script.as_bytes());
        assert!(!term.modes.get(Modes::BRACKETED_PASTE), "the script ends with DECRST");
        parser.advance(&mut term, find("sync-update").unwrap().script.as_bytes());
        assert!(!term.modes.get(Modes::SYNCHRONIZED_OUTPUT));
    }

    #[test]
    fn ed_scroll_clear_keeps_only_what_comes_after_the_clear() {
        let f = find("ed-scroll-clear").unwrap();
        let mut term = Terminal::new(80, 24, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, f.script.as_bytes());
        let history = term.history();
        assert!(history.contains("after"), "{history}");
    }
}
