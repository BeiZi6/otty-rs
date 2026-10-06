"""`Session.pty` becomes optional so `Drop` can move it to a reaper thread."""
import pathlib
import re
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\session.rs")
t = F.read_text(encoding="utf-8")
before = t

# 1. The struct field, plus the two new fields.
t = t.replace(
    "pub struct Session {\n    pub pane: PaneId,\n    pty: Pty,",
    "pub struct Session {\n    pub pane: PaneId,\n    /// `None` once the session has been closed, or after `Drop` has handed it to\n    /// the reaper thread.\n    pty: Option<Pty>,",
)
t = t.replace(
    "    reported: (u16, u16),\n    exited: Option<i32>,\n}",
    "    reported: (u16, u16),\n    exited: Option<i32>,\n    closed: bool,\n}",
)
t = t.replace(
    "            reported: (config.cols, config.rows),\n            exited: None,\n        })",
    "            reported: (config.cols, config.rows),\n            exited: None,\n            closed: false,\n        })",
)

# 2. The timeout constant.
t = t.replace(
    "const READ_CHUNK: usize = 64 * 1024;",
    "const READ_CHUNK: usize = 64 * 1024;\n\n/// How long `close` waits for the output pipe to drain before giving up.\n///\n/// Long enough for a normal shell to be reaped, short enough that a wedged child\n/// cannot freeze a pane's close. The wait is bounded on purpose.\nconst CLOSE_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);",
)

# 3. Accessors.
t = t.replace(
    "    /// The terminal, for reading the grid.\n    pub fn terminal(&self) -> &Terminal {",
    "    /// The pty, while the session is live.\n    fn pty(&self) -> Option<&Pty> {\n        self.pty.as_ref()\n    }\n\n    /// The terminal, for reading the grid.\n    pub fn terminal(&self) -> &Terminal {",
)

# 4. Call sites.
SUBS = [
    ("                    self.pty.log_output(&chunk);",
     "                    if let Some(pty) = &mut self.pty {\n                        pty.log_output(&chunk);\n                    }"),
    ("        if !replies.is_empty() {\n            let _ = self.pty.write(&replies);\n        }",
     "        if !replies.is_empty() {\n            if let Some(pty) = &mut self.pty {\n                let _ = pty.write(&replies);\n            }\n        }"),
    ("    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {\n        self.pty.write(bytes)\n    }",
     "    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {\n        match &mut self.pty {\n            Some(pty) => pty.write(bytes),\n            // A closed session is a no-op, not an error: the caller is writing to\n            // a pane that has gone, and the write is not the interesting event.\n            None => Ok(()),\n        }\n    }"),
    ("        self.reported = (cols, rows);\n        let _ = self.pty.resize(cols, rows);\n        true",
     "        self.reported = (cols, rows);\n        if let Some(pty) = &self.pty {\n            let _ = pty.resize(cols, rows);\n        }\n        true"),
    ("    pub fn tty(&self) -> Option<String> {\n        self.pty.tty_name()\n    }",
     "    pub fn tty(&self) -> Option<String> {\n        self.pty()?.tty_name()\n    }"),
    ("    pub fn is_busy(&self) -> bool {\n        self.pty.is_busy()\n    }",
     "    pub fn is_busy(&self) -> bool {\n        self.pty().is_some_and(Pty::is_busy)\n    }"),
    ("    pub fn processes(&self) -> Vec<String> {\n        self.pty.processes()\n    }",
     "    pub fn processes(&self) -> Vec<String> {\n        self.pty().map(Pty::processes).unwrap_or_default()\n    }"),
    ("    pub fn pid(&self) -> Option<u32> {\n        self.pty.pid()\n    }",
     "    pub fn pid(&self) -> Option<u32> {\n        self.pty()?.pid()\n    }"),
    ("    pub fn log_path(&self) -> Option<&std::path::Path> {\n        self.pty.log_path()\n    }",
     "    pub fn log_path(&self) -> Option<&std::path::Path> {\n        self.pty()?.log_path()\n    }"),
    ("        if let Some(status) = self.pty.try_wait() {",
     "        let Some(pty) = &mut self.pty else { return };\n        if let Some(status) = pty.try_wait() {"),
    ("        Ok(Self {\n            pane,\n            pty,",
     "        Ok(Self {\n            pane,\n            pty: Some(pty),"),
]

for old, new in SUBS:
    if old not in t:
        print(f"MISS: {old.strip()[:80]}")
        sys.exit(1)
    t = t.replace(old, new, 1)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched session.rs for Option<Pty>")
