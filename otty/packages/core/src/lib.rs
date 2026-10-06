//! Shared vocabulary for the whole Otty workspace.
//!
//! Everything that more than one package needs to agree on lives here: colours,
//! identifiers, geometry, and the terminal-style key/escape vocabulary that the
//! config layer parses and the UI layer consumes.

pub mod color;
pub mod geometry;
pub mod id;
pub mod key;

pub use color::{Color, ColorError, NamedColor};
pub use geometry::{CellSize, Insets, Point, Rect, Size};
pub use id::{AgentId, AgentState, PaneId, ParseIdError, SessionId, TabId, WindowId};
pub use key::{Chord, ChordParseError, Key, KeyEvent, KeyEventKind, Modifiers, NamedKey, Trigger};

/// Where Otty stores per-user state. Mirrors the directory layout the shipped
/// build uses: `$XDG_CONFIG_HOME/otty` on POSIX, `%APPDATA%\otty` on Windows,
/// `~/Library/Application Support/otty` for the macOS-only app-support tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
    Other,
}

impl Platform {
    pub const fn host() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }

    /// The default `$TERM` for this platform's Otty build.
    pub const fn default_term(self) -> &'static str {
        "xterm-otty"
    }
}

/// Path of the user configuration directory (`~/.config/otty`).
pub fn config_dir() -> std::path::PathBuf {
    platform_config_dir()
}

fn platform_config_dir() -> std::path::PathBuf {
    if let Some(x) = std::env::var_os("OTTY_CONFIG_DIR") {
        return x.into();
    }
    if cfg!(target_os = "macos") {
        // On macOS Otty follows the XDG layout too, so a dotfile repo synced
        // between macOS and Linux keeps working.
        if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
            return std::path::PathBuf::from(x).join("otty");
        }
        if let Some(h) = dirs_home() {
            return h.join(".config").join("otty");
        }
    } else if cfg!(target_os = "windows") {
        if let Some(x) = std::env::var_os("APPDATA") {
            return std::path::PathBuf::from(x).join("otty");
        }
    } else {
        if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
            return std::path::PathBuf::from(x).join("otty");
        }
        if let Some(h) = dirs_home() {
            return h.join(".config").join("otty");
        }
    }
    std::path::PathBuf::from(".otty")
}

/// Platform application-support directory, used for caches and the session
/// log database (`<app-support>/otty`).
pub fn app_support_dir() -> std::path::PathBuf {
    if let Some(x) = std::env::var_os("OTTY_APP_SUPPORT_DIR") {
        return x.into();
    }
    if cfg!(target_os = "macos") {
        if let Some(h) = dirs_home() {
            return h.join("Library").join("Application Support").join("otty");
        }
    } else if cfg!(target_os = "windows") {
        if let Some(x) = std::env::var_os("LOCALAPPDATA") {
            return std::path::PathBuf::from(x).join("otty");
        }
    } else if let Some(x) = std::env::var_os("XDG_STATE_HOME") {
        return std::path::PathBuf::from(x).join("otty");
    } else if let Some(h) = dirs_home() {
        return h.join(".local").join("state").join("otty");
    }
    std::path::PathBuf::from(".otty-state")
}

/// `$HOME` (POSIX) or `%USERPROFILE%` (Windows).
pub fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(Into::into)
}

/// Expand a leading `~` and any `$VAR` / `${VAR}` occurrences.
pub fn expand_path(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            '~' if i == 0 || bytes[i - 1] == '=' || bytes[i - 1] == ':' => {
                if let Some(h) = dirs_home() {
                    out.push_str(&h.to_string_lossy());
                    i += 1;
                } else {
                    out.push('~');
                    i += 1;
                }
            }
            '$' => {
                let mut j = i + 1;
                let braced = bytes.get(j) == Some(&'{');
                if braced {
                    j += 1;
                }
                let start = j;
                while j < bytes.len()
                    && (bytes[j].is_ascii_alphanumeric() || bytes[j] == '_')
                {
                    j += 1;
                }
                if j > start {
                    let name: String = bytes[start..j].iter().collect();
                    if braced && bytes.get(j) == Some(&'}') {
                        j += 1;
                    }
                    match std::env::var(&name) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => {
                            out.push('$');
                            out.extend(&bytes[i + 1..j]);
                        }
                    }
                    i = j;
                } else {
                    out.push('$');
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// `Otty` version string, with the build hash when the build script supplied one.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git revision this build came from, or `unknown` for a plain `cargo build`.
pub const GIT_HASH: &str = match option_env!("OTTY_GIT_HASH") {
    Some(h) => h,
    None => "unknown",
};
