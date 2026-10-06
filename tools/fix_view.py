"""view.rs: fix the structural mistakes before asking the compiler about APIs.

Three things here are wrong on their face and worth fixing without a build:
  * `self.cwd` was used but never declared, and the `WorkingDirectory` newtype
    plus `cwd_slot` were scaffolding around that mistake;
  * `pty_config` used a `.pipe(...)` that Rust does not have;
  * `encode_line` lives in `otty_render::grid`, not `otty_term`.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\view.rs")
t = F.read_text(encoding="utf-8")
before = t

# 1. The field itself.
t = t.replace(
    """    /// The events the engine produced on the last pump, for badges and titles.
    events: Vec<otty_term::Event>,
}""",
    """    /// The events the engine produced on the last pump, for badges and titles.
    events: Vec<otty_term::Event>,
    /// The directory the shell last reported over OSC 7, when it reports one.
    cwd: Option<PathBuf>,
}""",
)
t = t.replace(
    """            buffer: pixel_size,
            events: Vec::new(),
        }
    }

    /// A view over an already-running session""",
    """            buffer: pixel_size,
            events: Vec::new(),
            cwd: None,
        }
    }

    /// A view over an already-running session""",
)
t = t.replace(
    """            buffer: pixel_size,
            events: Vec::new(),
        }
    }

    // -------------------------------------------------------------- geometry""",
    """            buffer: pixel_size,
            events: Vec::new(),
            cwd: None,
        }
    }

    // -------------------------------------------------------------- geometry""",
)

# The scaffolding around the missing field goes away.
t = t.replace(
    """/// The working directory the shell reported, if it has reported one.
///
/// A field rather than a local so `working_directory()` can hand it out.
impl TerminalView {
    fn cwd_slot(&self) -> &Option<PathBuf> {
        &self.cwd
    }
}

// The field is declared here rather than in the struct literal above so the
// constructor cannot forget it — `Option::default()` is `None`, which is exactly
// "the shell has not said yet".
impl Default for WorkingDirectory {
    fn default() -> Self {
        Self(None)
    }
}

/// A newtype so `TerminalView`'s field list stays readable.
#[derive(Debug, Default)]
pub struct WorkingDirectory(pub Option<PathBuf>);

""",
    "",
)

# 2. `encode_line` is the frame layer's, not the engine's.
t = t.replace(
    "use otty_term::{encode_line, TermConfig};",
    "use otty_term::TermConfig;",
)
t = t.replace(
    "use otty_render::grid::{CellStyle, Cursor, GridFrame, Line, Palette, Run};",
    "use otty_render::grid::{encode_line, Cursor, GridFrame, Line, Palette};",
)

# 3. No `pipe` in Rust. Build the config once, then adjust it.
t = t.replace(
    """pub fn pty_config(config: &Config, cols: u16, rows: u16) -> PtyConfig {
    let (program, args) = crate::session::test_shell_config(cols, rows).pipe(|mut c| {
        c.program = config
            .get_str("command")
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or(c.program);
        c.args = config.get_list("command-args");
        (c.program.clone(), c.args.clone())
    });
    let mut cfg = crate::session::test_shell_config(cols, rows);
    cfg.program = program;
    cfg.args = args;
    cfg.cwd = config""",
    """pub fn pty_config(config: &Config, cols: u16, rows: u16) -> PtyConfig {
    // `test_shell_config` supplies the per-platform default shell, which is what
    // a user who has configured nothing gets. Everything below overrides it from
    // the configuration, in the order the docs give.
    let mut cfg = crate::session::test_shell_config(cols, rows);
    if let Some(program) = config.get_str("command").filter(|s| !s.is_empty()) {
        cfg.program = program;
    } else if let Ok(shell) = std::env::var("SHELL") {
        cfg.program = shell;
    }
    cfg.args = config.get_list("command-args");
    cfg.cwd = config""",
)

# 4. The pane's tab title is set through the model, so the unused local goes.
t = t.replace(
    """            otty_term::Event::Title(title) => {
                if let Some(tab) = self.app.active_window_mut().and_then(|w| w.active_tab_mut()) {
                    let _ = tab;
                }
                if let Some(pane) = self.app.pane_mut(self.pane) {
                    pane.shell_title = Some(title.clone());
                }
            }
            otty_term::Event::WorkingDirectory(path) => {
                if let Some(pane) = self.app.pane_mut(self.pane) {
                    let _ = pane;
                }
                self.cwd = Some(PathBuf::from(path));
            }""",
    """            otty_term::Event::Title(title) => {
                if let Some(pane) = self.app.pane_mut(self.pane) {
                    // The shell's own title, which a user-set title beats.
                    pane.shell_title = Some(title.clone());
                }
            }
            otty_term::Event::WorkingDirectory(path) => {
                self.cwd = Some(PathBuf::from(path));
            }""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched view.rs")
