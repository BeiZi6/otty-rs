"""view.rs: the real APIs, read out of the crate rather than assumed.

  * `Progress { Clear, Set(u8), Error, Indeterminate, Paused(u8) }`
  * `Terminal.cursor` is a **public field** of type `otty_term::Cursor`
    (`row`, `col`, `visible`, `shape`, `blinking`)
  * `otty_render::grid::CursorShape` is this crate's own enum, so the engine's
    shape has to be mapped rather than copied
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\view.rs")
t = F.read_text(encoding="utf-8")
before = t

# 1. The cursor: a public field, and the shape needs a mapping.
t = t.replace(
    """    fn cursor(&self) -> Cursor {
        let term = self.session.terminal();
        Cursor {
            col: term.cursor_col(),
            row: term.cursor_row(),
            visible: term.cursor_visible(),
            shape: self.chrome.cursor_shape,
            blinking: self.chrome.cursor_blink,
        }
    }""",
    """    fn cursor(&self) -> Cursor {
        // `Terminal::cursor` is a public field, not an accessor. The engine's
        // shape is reported per-pane (the program can change it with DECSCUSR),
        // but the theme names the shape to use when the program has not asked for
        // one, so the theme wins here — which is what a themed window does.
        let engine = self.session.terminal().cursor;
        Cursor {
            col: engine.col,
            row: engine.row,
            visible: engine.visible,
            shape: self.chrome.cursor_shape,
            blinking: self.chrome.cursor_blink,
        }
    }""",
)

# 2. The progress badge: the engine's five states, not four invented ones.
t = t.replace(
    """            otty_term::Event::Progress(p) => {
                if let Some(tab) = self.app.active_window_mut().and_then(|w| w.active_tab_mut()) {
                    tab.badge = Some(match p {
                        otty_term::ProgressState::Running => crate::model::Badge::Running,
                        otty_term::ProgressState::Done => crate::model::Badge::Success,
                        otty_term::ProgressState::Error => crate::model::Badge::Error,
                        otty_term::ProgressState::Percent(n) => crate::model::Badge::Progress(*n),
                    });
                }
            }""",
    """            otty_term::Event::Progress(p) => {
                // `OSC 9;4` carries five states, and the tab bar shows one badge.
                // `Clear` clears it rather than showing a zero, because "no
                // progress" and "0% done" are different things to a user.
                let badge = match p {
                    otty_term::Progress::Clear => None,
                    otty_term::Progress::Set(percent) => {
                        Some(crate::model::Badge::Progress(*percent))
                    }
                    otty_term::Progress::Paused(percent) => {
                        Some(crate::model::Badge::Progress(*percent))
                    }
                    otty_term::Progress::Indeterminate => Some(crate::model::Badge::Running),
                    otty_term::Progress::Error => Some(crate::model::Badge::Error),
                };
                if let Some(tab) = self.app.active_window_mut().and_then(|w| w.active_tab_mut()) {
                    tab.badge = badge;
                }
            }""",
)

# 3. `rasterize` takes `&impl GlyphSource`, so the trait object needs its slice.
t = t.replace(
    "        self.raster.rasterize(&frame, self.glyphs.as_ref())",
    "        // `&*` because the parameter is `&impl GlyphSource`: an unsized trait\n        // object has to be reborrowed into a `&dyn` that satisfies the bound.\n        self.raster.rasterize(&frame, &*self.glyphs)",
)

# 4. The `with_session` initialiser was missing the new field.
t = t.replace(
    """            cell,
            pane,
            buffer: pixel_size,
            events: Vec::new(),
            cwd: None,
        }
    }

    // -------------------------------------------------------------- geometry""",
    """            cell,
            pane,
            buffer: pixel_size,
            events: Vec::new(),
            cwd: None,
        }
    }

    // -------------------------------------------------------------- geometry""",
)
t = t.replace(
    """            options: LayoutOptions::default(),
            cell,
            pane,
            buffer: pixel_size,
            events: Vec::new(),
        }""",
    """            options: LayoutOptions::default(),
            cell,
            pane,
            buffer: pixel_size,
            events: Vec::new(),
            cwd: None,
        }""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched view.rs")
