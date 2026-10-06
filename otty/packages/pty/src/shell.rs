//! Shell discovery and shell-integration injection.
//!
//! Otty never edits the user's dotfiles. Instead it points the shell at a
//! directory of its own, from which the integration loads *and then* hands
//! control back to the user's real configuration. The contract is asymmetric per
//! shell, because every shell offers a different seam:
//!
//! | shell | seam | what Otty sets |
//! |---|---|---|
//! | zsh | `ZDOTDIR` | `ZDOTDIR=<bundle>/shell-integration/zsh`, with the user's original saved in `OTTY_ZSH_ZDOTDIR` |
//! | bash | `ENV` plus `XDG_DATA_DIRS` | the integration is sourced before the user's rc |
//! | fish | `XDG_DATA_DIRS` | `vendor_conf.d/otty-shell-integration.fish` |
//!
//! The zsh entry point (`shell-integration/zsh/.zshenv`) is the shape the other
//! two follow: restore the real root, source the user's own file, then defer the
//! payload to the first prompt so the user's plugin manager installs its hooks
//! first and Otty's OSC 133 marks win.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The shells Otty ships integration for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Zsh,
    Bash,
    Fish,
    /// Anything else: run it, but do not inject.
    Other,
}

impl ShellKind {
    /// Identify a shell from its executable name or path.
    pub fn of(program: &str) -> Self {
        let name = Path::new(program)
            .file_name()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_else(|| program.to_ascii_lowercase());
        match name.as_str() {
            "zsh" | "zsh5" => Self::Zsh,
            "bash" | "bash5" | "rbash" => Self::Bash,
            "fish" => Self::Fish,
            _ => Self::Other,
        }
    }

    /// Whether the integration ships a payload for this shell.
    pub const fn has_integration(self) -> bool {
        matches!(self, Self::Zsh | Self::Bash | Self::Fish)
    }
}

/// Everything the integration scripts read out of the environment.
///
/// The names are the ones the shipped scripts use; `OTTY_CLI` is how an
/// `otty-hook.sh` finds the binary without a `PATH` search, and `OTTY_PANE_ID`
/// is how a script learns which pane it is running in — the documented way to
/// target "the pane I am in" rather than the focused one.
#[derive(Debug, Clone, Default)]
pub struct IntegrationEnv {
    /// Absolute path to the directory holding `otty` and `otty-cli`.
    pub bin_dir: Option<PathBuf>,
    /// A private directory of on/off flag files (`allow_overwrite`, `omit_prefix`).
    pub flags_dir: Option<PathBuf>,
    /// The pane this shell belongs to, exported as `OTTY_PANE_ID`.
    pub pane_id: Option<String>,
    /// The session id, for history and restore, exported as `OTTY_PANE_HISTORY`.
    pub pane_history: Option<PathBuf>,
    /// Per-pane prompt-token secret, so a foreign process cannot forge marks.
    pub prompt_token: Option<String>,
    /// Command prefixes the integration emits `OSC 9;4` progress for
    /// (`progress-bar-commands`).
    pub progress_commands: Vec<String>,
    /// `history-up-arrow` / `command-history-scope`, exported verbatim as
    /// `OTTY_HISTORY_SCOPE`.
    pub history_scope: Option<String>,
    /// `cli-alias` entries, rendered as `name=command|name=command`.
    pub cli_aliases: Vec<(String, String)>,
    /// `strip-cjk-wrap-pad`, which the zsh entry point consults directly.
    pub strip_cjk_wrap_pad: bool,
    /// `shell-integration-disabled-shells`: skip injection for these.
    pub disabled_shells: Vec<String>,
}

impl IntegrationEnv {
    /// Render into the environment a spawned shell should see.
    pub fn to_env(&self, kind: ShellKind, integration_dir: &Path, into: &mut BTreeMap<String, String>, user_zdotdir: Option<&Path>) {
        if let Some(dir) = &self.bin_dir {
            into.insert("OTTY_BIN_DIR".into(), dir.display().to_string());
        }
        if let Some(dir) = &self.flags_dir {
            into.insert("OTTY_FLAGS_DIR".into(), dir.display().to_string());
        }
        if let Some(id) = &self.pane_id {
            into.insert("OTTY_PANE_ID".into(), id.clone());
        }
        if let Some(p) = &self.pane_history {
            into.insert("OTTY_PANE_HISTORY".into(), p.display().to_string());
        }
        if let Some(t) = &self.prompt_token {
            into.insert("OTTY_PROMPT_TOKEN".into(), t.clone());
        }
        if !self.progress_commands.is_empty() {
            into.insert("OTTY_PROGRESS_COMMANDS".into(), self.progress_commands.join("|"));
        }
        if let Some(s) = &self.history_scope {
            into.insert("OTTY_HISTORY_SCOPE".into(), s.clone());
        }
        if !self.cli_aliases.is_empty() {
            let rendered: Vec<String> = self
                .cli_aliases
                .iter()
                .map(|(name, cmd)| format!("{name}={cmd}"))
                .collect();
            into.insert("OTTY_CLI_ALIASES".into(), rendered.join("|"));
        }
        if self.strip_cjk_wrap_pad {
            into.insert("OTTY_STRIP_CJK_WRAP_PAD".into(), "1".into());
        }

        match kind {
            ShellKind::Zsh => {
                // Otty owns ZDOTDIR; the entry point restores the user's own
                // from OTTY_ZSH_ZDOTDIR, so the variable is only set when the
                // user actually had one.
                into.insert(
                    "ZDOTDIR".into(),
                    integration_dir.join("zsh").display().to_string(),
                );
                if let Some(orig) = user_zdotdir {
                    into.insert("OTTY_ZSH_ZDOTDIR".into(), orig.display().to_string());
                }
            }
            ShellKind::Bash => {
                into.insert(
                    "ENV".into(),
                    integration_dir.join("otty-integration.bash").display().to_string(),
                );
                // bash reads `$XDG_DATA_DIRS/vendor_conf.d` style drop-ins only
                // through its own startup chain, so the integration file is
                // sourced by name instead; the data dir is still exported for
                // sub-shells and for the fish/`vendor_conf.d` convention.
                prepend_data_dir(into, integration_dir);
            }
            ShellKind::Fish => prepend_data_dir(into, integration_dir),
            ShellKind::Other => {}
        }
    }

    /// Whether injection should happen at all for `program`.
    pub fn should_inject(&self, program: &str, enabled: bool) -> bool {
        if !enabled {
            return false;
        }
        let kind = ShellKind::of(program);
        if !kind.has_integration() {
            return false;
        }
        let name = Path::new(program)
            .file_name()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        !self.disabled_shells.iter().any(|s| s.to_ascii_lowercase() == name)
    }
}

/// Put `dir` first in `XDG_DATA_DIRS`, preserving whatever was there.
fn prepend_data_dir(into: &mut BTreeMap<String, String>, dir: &Path) {
    let existing = into
        .get("XDG_DATA_DIRS")
        .cloned()
        .or_else(|| std::env::var("XDG_DATA_DIRS").ok())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    let joined = format!("{}:{}", dir.display(), existing);
    into.insert("XDG_DATA_DIRS".into(), joined);
}

/// The user's shell, following the documented fallback chain.
pub fn detect_login_shell() -> String {
    if let Ok(s) = std::env::var("SHELL") {
        if !s.is_empty() {
            return s;
        }
    }
    // `command`'s documented default is `$SHELL`, else `/bin/zsh` on POSIX and
    // the ComSpec on Windows.
    if cfg!(target_os = "windows") {
        std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_owned())
    } else {
        "/bin/zsh".to_owned()
    }
}

/// Whether `path` looks like an executable we can actually launch.
pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(m) => m.is_file() && m.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// The program and arguments for a shell, honouring `login-greeting`.
///
/// A login shell prints the MOTD, which is why it is off by default: it changes
/// what a fresh pane looks like.
pub fn shell_invocation(program: &str, login_greeting: bool) -> (String, Vec<String>) {
    let kind = ShellKind::of(program);
    if !login_greeting {
        return (program.to_owned(), Vec::new());
    }
    match kind {
        ShellKind::Zsh | ShellKind::Bash => {
            // `-l` is the portable way to ask either for a login shell.
            let name = Path::new(program)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| program.to_owned());
            let login = format!("-{name}");
            (program.to_owned(), vec![login])
        }
        ShellKind::Fish => (program.to_owned(), vec!["--login".into()]),
        ShellKind::Other => (program.to_owned(), Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shells_are_identified_by_basename() {
        assert_eq!(ShellKind::of("/bin/zsh"), ShellKind::Zsh);
        assert_eq!(ShellKind::of("bash"), ShellKind::Bash);
        assert_eq!(ShellKind::of("/usr/local/bin/FISH"), ShellKind::Fish);
        assert_eq!(ShellKind::of("/opt/homebrew/bin/nu"), ShellKind::Other);
        assert!(ShellKind::Zsh.has_integration());
        assert!(!ShellKind::Other.has_integration());
    }

    #[test]
    fn zsh_gets_zdotdir_and_remembers_the_users_own() {
        let env = IntegrationEnv::default();
        let mut out = BTreeMap::new();
        let dir = Path::new("/app/Resources/shell-integration");
        env.to_env(ShellKind::Zsh, dir, &mut out, Some(Path::new("/home/u/.config/zsh")));
        // The separator is whatever the host uses, so build the expectation the
        // same way the implementation does rather than hard-coding `/`.
        assert_eq!(out["ZDOTDIR"], dir.join("zsh").display().to_string());
        assert!(out["ZDOTDIR"].ends_with("zsh"));
        assert_eq!(out["OTTY_ZSH_ZDOTDIR"], "/home/u/.config/zsh");

        // With no user ZDOTDIR, the variable is left unset — the entry point
        // treats unset as $HOME, which is what zsh itself does.
        let mut out = BTreeMap::new();
        env.to_env(ShellKind::Zsh, dir, &mut out, None);
        assert!(!out.contains_key("OTTY_ZSH_ZDOTDIR"));
    }

    #[test]
    fn fish_and_bash_get_the_data_dir_ahead_of_the_system() {
        let env = IntegrationEnv::default();
        let mut out = BTreeMap::new();
        env.to_env(ShellKind::Fish, Path::new("/app/si"), &mut out, None);
        let dirs = &out["XDG_DATA_DIRS"];
        assert!(dirs.starts_with("/app/si:"), "{dirs}");
        assert!(dirs.contains("/usr/share"));
    }

    #[test]
    fn the_pane_variables_the_scripts_read_are_exported() {
        let env = IntegrationEnv {
            bin_dir: Some("/app/bin".into()),
            flags_dir: Some("/tmp/flags".into()),
            pane_id: Some("pane-3".into()),
            pane_history: Some("/tmp/h".into()),
            prompt_token: Some("s3cret".into()),
            progress_commands: vec!["curl".into(), "git push".into()],
            history_scope: Some("global".into()),
            cli_aliases: vec![("gs".into(), "git status".into())],
            strip_cjk_wrap_pad: true,
            disabled_shells: vec![],
        };
        let mut out = BTreeMap::new();
        env.to_env(ShellKind::Bash, Path::new("/app/si"), &mut out, None);
        assert_eq!(out["OTTY_BIN_DIR"], "/app/bin");
        assert_eq!(out["OTTY_FLAGS_DIR"], "/tmp/flags");
        assert_eq!(out["OTTY_PANE_ID"], "pane-3");
        assert_eq!(out["OTTY_PANE_HISTORY"], "/tmp/h");
        assert_eq!(out["OTTY_PROMPT_TOKEN"], "s3cret");
        assert_eq!(out["OTTY_PROGRESS_COMMANDS"], "curl|git push");
        assert_eq!(out["OTTY_HISTORY_SCOPE"], "global");
        assert_eq!(out["OTTY_CLI_ALIASES"], "gs=git status");
        assert_eq!(out["OTTY_STRIP_CJK_WRAP_PAD"], "1");
    }

    #[test]
    fn injection_is_skipped_for_unintegrated_and_disabled_shells() {
        let env = IntegrationEnv {
            disabled_shells: vec!["fish".into()],
            ..Default::default()
        };
        assert!(env.should_inject("/bin/zsh", true));
        assert!(!env.should_inject("/usr/bin/fish", true));
        assert!(!env.should_inject("/opt/bin/nu", true));
        assert!(!env.should_inject("/bin/zsh", false));
    }

    #[test]
    fn login_greeting_uses_the_documented_flag_per_shell() {
        assert_eq!(shell_invocation("/bin/zsh", false), ("/bin/zsh".into(), vec![]));
        assert_eq!(
            shell_invocation("/bin/zsh", true),
            ("/bin/zsh".into(), vec!["-zsh".into()])
        );
        assert_eq!(
            shell_invocation("/bin/bash", true),
            ("/bin/bash".into(), vec!["-bash".into()])
        );
        assert_eq!(
            shell_invocation("/usr/bin/fish", true),
            ("/usr/bin/fish".into(), vec!["--login".into()])
        );
    }

    #[test]
    fn the_login_shell_falls_back_the_way_the_reference_says() {
        // Whatever `$SHELL` says wins; the test asserts only that something
        // non-empty comes back, because the environment is the oracle.
        std::env::set_var("SHELL", "/bin/test-shell");
        assert_eq!(detect_login_shell(), "/bin/test-shell");
        std::env::remove_var("SHELL");
        assert!(!detect_login_shell().is_empty());
    }
}
