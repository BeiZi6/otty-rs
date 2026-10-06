//! The `otty` command-line tool.
//!
//! Rust's answer to the two rules the CLI reference sets out:
//!
//! * **Running `otty` with no subcommand launches the GUI**, so `otty` and
//!   `otty -e <cmd>` behave like `xterm` / `alacritty` / `ghostty` — that is the
//!   default, not a special case.
//! * **A subcommand that drives the UI needs the app running.** Those ones speak
//!   [`otty_ipc`]; the rest — config, fonts, themes, keybinds, import, export,
//!   features, completions, `jump`, `learn`, `ssh` — work with nothing else
//!   running.
//!
//! Exit codes are part of the interface, so they live in [`output::exit`] rather
//! than being sprinkled through the handlers: a script has to be able to tell
//! "no such pane" (4) from "the app is not running" (3) from "a gate refused
//! this" (13).

mod ipc;
mod local;
mod output;

use clap::{Args, Parser, Subcommand};
use output::{exit, CliError, Format, OutputOptions};

/// Otty — a modern cross-platform terminal.
#[derive(Debug, Parser)]
#[command(
    name = "otty",
    version = otty_core::VERSION,
    about = "Otty — a modern cross-platform terminal",
    long_about = "Run with no subcommand (or with -e) to open the GUI, the way xterm, \
                  alacritty and ghostty do. Subcommands that inspect or drive the UI \
                  require a running Otty.",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The flags every subcommand accepts.
#[derive(Debug, Args, Clone)]
pub struct Global {
    /// Output format: text or json.
    #[arg(long, global = true, value_name = "fmt")]
    pub format: Option<String>,

    /// Shortcut for --format json.
    #[arg(long, global = true)]
    pub json: bool,

    /// Drop table headers from text list output.
    #[arg(long, global = true)]
    pub no_headers: bool,

    /// Suppress non-essential success output.
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Override the runtime control socket path.
    #[arg(long, global = true, value_name = "path")]
    pub socket: Option<std::path::PathBuf>,

    /// Override the config file path.
    #[arg(long, global = true, value_name = "path")]
    pub config_file: Option<std::path::PathBuf>,

    /// IPC timeout in milliseconds.
    #[arg(long, global = true, value_name = "ms", default_value_t = 3000)]
    pub timeout: u64,

    /// Skip destructive-action confirmation prompts.
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
}

impl Global {
    pub fn options(&self) -> Result<OutputOptions, CliError> {
        let format = match (&self.format, self.json) {
            (_, true) => Format::Json,
            (Some(f), false) => Format::parse(f)
                .ok_or_else(|| CliError::usage(format!("unknown output format `{f}`")))?,
            (None, false) => Format::Text,
        };
        Ok(OutputOptions { format, no_headers: self.no_headers, quiet: self.quiet })
    }
}

/// Every subcommand.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open a new Otty window (default shell).
    Open {
        /// Directory to open in.
        path: Option<String>,
        /// Run a command in the new window.
        #[arg(long, value_name = "cmd")]
        command: Option<String>,
        /// Window title.
        #[arg(long, value_name = "title")]
        title: Option<String>,
    },

    /// Run a command in a new window (via /bin/sh -c).
    #[command(name = "-e", hide = true)]
    Exec {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },

    /// Open a file or URL as a read-only pane.
    View {
        target: String,
        #[command(flatten)]
        placement: Placement,
    },

    /// Open a file as an editable pane.
    Edit {
        target: String,
        #[command(flatten)]
        placement: Placement,
    },

    /// Manage configuration.
    #[command(subcommand)]
    Config(ConfigCmd),

    /// Manage fonts.
    #[command(subcommand)]
    Font(FontCmd),

    /// Manage colour themes.
    #[command(subcommand)]
    Theme(ThemeCmd),

    /// List keybindings.
    #[command(subcommand)]
    Keybind(KeybindCmd),

    /// Inspect and manage windows in a running Otty.
    #[command(subcommand)]
    Window(WindowCmd),

    /// Shortcut for `window list`.
    #[command(name = "windows", hide = true)]
    Windows,

    /// Inspect and manage tabs in a running Otty.
    #[command(subcommand)]
    Tab(TabCmd),

    /// Shortcut for `tab list`.
    #[command(name = "tabs", hide = true)]
    Tabs,

    /// Inspect and manage panes in a running Otty.
    #[command(subcommand)]
    Pane(PaneCmd),

    /// Shortcut for `pane list`.
    #[command(name = "panes", hide = true)]
    Panes,

    /// Run a command with a progress badge.
    Watch {
        /// Suppress the system notification on finish.
        #[arg(short, long)]
        quiet: bool,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },

    /// Wait for a code agent to become idle.
    #[command(name = "watch:claude")]
    WatchClaude(WatchAgent),
    /// Wait for a Codex session to become idle.
    #[command(name = "watch:codex")]
    WatchCodex(WatchAgent),
    /// Wait for an OpenCode session to become idle.
    #[command(name = "watch:opencode")]
    WatchOpencode(WatchAgent),
    /// Wait for any agent session to become idle.
    #[command(name = "watch:agent")]
    WatchAgentGeneric(WatchAgent),

    /// Jump to a frecency-ranked directory.
    Jump {
        query: Option<String>,
        /// Resolve only; print the path instead of changing directory.
        #[arg(long)]
        no_cd: bool,
    },

    /// Record a directory visit in the frecency database.
    Learn {
        path: Option<String>,
        /// Print the resulting score instead of recording.
        #[arg(long)]
        score: bool,
    },

    /// Forget a directory or learned command.
    Ignore { path: String },

    /// Import another terminal's configuration, a theme, or a font.
    Import {
        /// `zoxide`, `ghostty`, `kitty`, `alacritty`, `theme`, or a path/URL.
        source: String,
        /// A path, when the source is an external terminal.
        path: Option<String>,
        #[arg(long)]
        activate: bool,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        keep: bool,
    },

    /// Export the configuration to another terminal.
    Export {
        /// `ghostty`, `kitty` or `alacritty`.
        target: String,
        #[arg(short, long, value_name = "path")]
        output: Option<std::path::PathBuf>,
    },

    /// Try a terminal feature sample.
    Features {
        name: Option<String>,
        /// Without this, `<name>` is a shorthand for `try <name>`.
        #[arg(long)]
        list: bool,
    },

    /// SSH integration helpers.
    #[command(subcommand)]
    Ssh(SshCmd),

    /// Print a shell completion script.
    Completions {
        /// bash, zsh, fish, elvish or powershell.
        shell: String,
    },

    /// Report an agent's lifecycle state (used by the bundled hooks).
    #[command(name = "state:claude")]
    StateClaude { args: Vec<String> },
    /// Report a Codex lifecycle state.
    #[command(name = "state:codex")]
    StateCodex { args: Vec<String> },
    /// Report an OpenCode lifecycle state.
    #[command(name = "state:opencode")]
    StateOpencode { args: Vec<String> },
    /// Report any agent's lifecycle state.
    #[command(name = "state:agent")]
    StateGeneric { args: Vec<String> },

    /// Send a raw control message to the running app.
    Ipc {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },

    /// Print version, build hash and a feature summary.
    Version,
}

/// Where a `view`/`edit` pane goes.
#[derive(Debug, Args, Clone, Default)]
pub struct Placement {
    /// Override read-only vs. edit.
    #[arg(long, value_name = "view|edit")]
    pub mode: Option<String>,
    #[arg(long)]
    pub new_tab: bool,
    #[arg(long)]
    pub new_window: bool,
    #[arg(long)]
    pub left: bool,
    #[arg(long)]
    pub right: bool,
    #[arg(long)]
    pub top: bool,
    #[arg(long)]
    pub bottom: bool,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print one key's value.
    Get {
        key: String,
        #[arg(long)]
        show_origin: bool,
        #[arg(long)]
        local: bool,
    },
    /// Set one key.
    Set {
        key: String,
        value: String,
        #[arg(long)]
        reload: bool,
        /// Apply to the running app only; do not persist.
        #[arg(long)]
        transient: bool,
        #[arg(long)]
        local: bool,
    },
    /// Remove a key from both files.
    Unset {
        key: String,
        #[arg(long)]
        local: bool,
    },
    /// Open the config file in $EDITOR.
    Edit {
        #[arg(long)]
        local: bool,
    },
    /// Print the normalised configuration.
    Show,
    /// Print the config file path.
    Path {
        #[arg(long)]
        local: bool,
    },
    /// Check for errors.
    Validate,
    /// Reload the configuration in the running app.
    Reload,
}

#[derive(Debug, Subcommand)]
pub enum FontCmd {
    /// List fonts known to the system and to Otty.
    List {
        #[arg(long, value_name = "name")]
        family: Option<String>,
        #[arg(long)]
        monospace: bool,
        #[arg(long)]
        system: bool,
        #[arg(long)]
        user: bool,
    },
    /// Write a family into `font-family`.
    Apply { family: String },
    /// Copy a font file into `~/.config/otty/fonts/`.
    Import {
        path: std::path::PathBuf,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ThemeCmd {
    /// List themes.
    List {
        #[arg(long, value_name = "dark|light|all", default_value = "all")]
        color: String,
    },
    /// Import a theme from a path or URL.
    Import {
        source: String,
        #[arg(long)]
        activate: bool,
        #[arg(long)]
        overwrite: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum KeybindCmd {
    /// List keybindings.
    List {
        /// Filter by action name (substring).
        #[arg(long, value_name = "name")]
        action: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum WindowCmd {
    Show { selector: Option<String> },
    List,
    New {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        title: Option<String>,
    },
    Close { selector: Option<String> },
    Focus { selector: String },
    Rename { selector: String, title: String },
    /// Print the window's frontmost tab.
    SelectedTab { selector: Option<String> },
}

#[derive(Debug, Subcommand)]
pub enum TabCmd {
    Show { selector: Option<String> },
    List,
    New {
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        profile: Option<String>,
    },
    Close { selector: Option<String> },
    Focus { selector: String },
    Rename { selector: String, title: String },
    Move {
        selector: String,
        #[arg(long, value_name = "window")]
        to_window: Option<String>,
        #[arg(long)]
        position: Option<u32>,
    },
    Badge {
        #[arg(long, value_name = "kind")]
        kind: String,
        #[arg(long, value_name = "selector")]
        tab: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum PaneCmd {
    Show {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
    List,
    Split {
        #[arg(long, value_name = "right|left|up|down")]
        direction: String,
        /// The new pane's share, 10-90.
        #[arg(long, value_name = "percent")]
        size: Option<u8>,
        #[arg(long, value_name = "cmd")]
        command: Option<String>,
        #[arg(long, value_name = "dir")]
        cwd: Option<String>,
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(long)]
        no_focus: bool,
    },
    Close {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
    Focus { selector: String },
    Zoom {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
    Resize {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(long)]
        right: Option<i32>,
        #[arg(long)]
        down: Option<i32>,
        #[arg(long)]
        left: Option<i32>,
        #[arg(long)]
        up: Option<i32>,
    },
    SendKeys {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        keys: Vec<String>,
    },
    SendText {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        text: String,
    },
    Run {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    Exec {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    Wait {
        #[arg(long, value_name = "selector")]
        tab: Option<String>,
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(long, value_name = "secs")]
        timeout_secs: Option<u64>,
    },
    Capture {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
        #[arg(long, value_name = "n")]
        lines: Option<u32>,
        /// Include the escape sequences rather than the plain text.
        #[arg(long)]
        escapes: bool,
    },
    /// Print the pane's tty device path.
    ///
    /// This is what `tty of t` returns in the AppleScript dictionary, and it is
    /// what a script needs to hand the pane to an external tool.
    Tty {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
    /// Print the processes running in the pane.
    Processes {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
    /// Print whether a foreground process other than the shell is running.
    ///
    /// Exits 0 when busy and 4 when idle, so a shell can branch on it without
    /// parsing the output.
    Busy {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
    /// Print the pane's grid size.
    Dimensions {
        #[arg(long, value_name = "selector")]
        pane: Option<String>,
    },
}

/// The flags `otty watch:<agent>` accepts.
#[derive(Debug, Args, Clone)]
pub struct WatchAgent {
    /// The agent's session id.
    pub id: String,
    /// Poll interval in milliseconds.
    #[arg(long, value_name = "n", default_value_t = 5000)]
    pub interval_ms: u64,
    /// Overall timeout; 0 waits forever.
    #[arg(long, value_name = "n", default_value_t = 0)]
    pub timeout_secs: u64,
    /// Give up after this long with no state reported at all; 0 waits forever.
    #[arg(long, value_name = "n", default_value_t = 60)]
    pub unknown_timeout_secs: u64,
    /// Print state transitions to stderr.
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum SshCmd {
    /// Check a remote for a shadowing terminfo entry.
    Terminfo {
        host: String,
        /// Move the shadowing entries aside.
        #[arg(long)]
        clean: bool,
        /// Check a name other than your $TERM.
        #[arg(long, value_name = "name")]
        term: Option<String>,
        /// Skip the confirmation prompt.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Explain what the SSH integration does for a host.
    Doctor {
        host: String,
        /// Diagnose an exact invocation.
        #[arg(last = true, allow_hyphen_values = true)]
        ssh_args: Vec<String>,
    },
    /// Print the wrapper decision for a host.
    Plan { host: String },
}

fn main() {
    let cli = Cli::parse();
    let code = match run(&cli) {
        Ok(code) => code,
        Err(e) => {
            // Errors go to stderr and the process exits with the code the
            // reference documents for that failure.
            eprintln!("otty: {e}");
            e.code()
        }
    };
    std::process::exit(code);
}

fn run(cli: &Cli) -> Result<i32, CliError> {
    let opts = cli.global.options()?;
    let Some(command) = &cli.command else {
        // No subcommand: launch the GUI, exactly as `xterm` would.
        return local::launch_gui(&cli.global, &[]);
    };

    match command {
        // A bare `-e <cmd>` also launches the GUI.
        Command::Exec { argv } => local::launch_gui(&cli.global, argv),
        Command::Open { path, command, title } => local::open(&cli.global, path.as_deref(), command.as_deref(), title.as_deref()),
        Command::View { target, placement } => local::view_or_edit(&cli.global, target, false, placement),
        Command::Edit { target, placement } => local::view_or_edit(&cli.global, target, true, placement),

        Command::Config(cmd) => local::config(&cli.global, cmd, opts),
        Command::Font(cmd) => local::font(&cli.global, cmd, opts),
        Command::Theme(cmd) => local::theme(&cli.global, cmd, opts),
        Command::Keybind(cmd) => local::keybind(&cli.global, cmd, opts),
        Command::Import { source, path, activate, overwrite, keep } => {
            local::import(&cli.global, source, path.as_deref(), *activate, *overwrite, *keep, opts)
        }
        Command::Export { target, output } => local::export(&cli.global, target, output.as_deref(), opts),
        Command::Features { name, list } => local::features(&cli.global, name.as_deref(), *list, opts),
        Command::Completions { shell } => local::completions(shell),
        Command::Jump { query, no_cd } => local::jump(&cli.global, query.as_deref(), *no_cd, opts),
        Command::Learn { path, score } => local::learn(&cli.global, path.as_deref(), *score, opts),
        Command::Ignore { path } => local::ignore(&cli.global, path, opts),
        Command::Ssh(cmd) => local::ssh(&cli.global, cmd, opts),
        Command::Version => local::version(opts),

        Command::StateClaude { args } => ipc::report_state(&cli.global, "claude", args, opts),
        Command::StateCodex { args } => ipc::report_state(&cli.global, "codex", args, opts),
        Command::StateOpencode { args } => ipc::report_state(&cli.global, "opencode", args, opts),
        Command::StateGeneric { args } => ipc::report_state(&cli.global, "custom", args, opts),

        Command::Watch { quiet, argv } => local::watch(&cli.global, argv, *quiet),
        Command::WatchClaude(a) => ipc::watch_agent(&cli.global, "claude", a, opts),
        Command::WatchCodex(a) => ipc::watch_agent(&cli.global, "codex", a, opts),
        Command::WatchOpencode(a) => ipc::watch_agent(&cli.global, "opencode", a, opts),
        Command::WatchAgentGeneric(a) => ipc::watch_agent(&cli.global, "custom", a, opts),

        Command::Windows => ipc::window_list(&cli.global, opts),
        Command::Window(cmd) => ipc::window(&cli.global, cmd, opts),
        Command::Tabs => ipc::tab_list(&cli.global, opts),
        Command::Tab(cmd) => ipc::tab(&cli.global, cmd, opts),
        Command::Panes => ipc::pane_list(&cli.global, opts),
        Command::Pane(cmd) => ipc::pane(&cli.global, cmd, opts),
        Command::Ipc { command } => ipc::raw(&cli.global, command, opts),
    }
}

/// Hand back the exit code a successful run should use.
pub(crate) fn ok() -> Result<i32, CliError> {
    Ok(exit::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_tree_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn no_subcommand_means_launch_the_gui() {
        let cli = Cli::try_parse_from(["otty"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.global.timeout, 3000);
    }

    #[test]
    fn the_documented_global_flags_parse() {
        let cli = Cli::try_parse_from([
            "otty", "--json", "--no-headers", "-q", "--socket", "/tmp/s", "--timeout", "500", "-y",
            "pane", "list",
        ])
        .unwrap();
        assert!(cli.global.json);
        assert!(cli.global.no_headers);
        assert!(cli.global.quiet);
        assert_eq!(cli.global.socket.as_deref(), Some(std::path::Path::new("/tmp/s")));
        assert_eq!(cli.global.timeout, 500);
        assert!(cli.global.yes);
        assert!(matches!(cli.command, Some(Command::Pane(PaneCmd::List))));
    }

    #[test]
    fn json_wins_over_an_explicit_format() {
        let cli = Cli::try_parse_from(["otty", "--format", "text", "--json", "panes"]).unwrap();
        assert_eq!(cli.global.options().unwrap().format, Format::Json);
    }

    #[test]
    fn an_unknown_format_is_a_usage_error() {
        let cli = Cli::try_parse_from(["otty", "--format", "yaml", "panes"]).unwrap();
        let e = cli.global.options().unwrap_err();
        assert_eq!(e.code(), exit::USAGE);
    }

    #[test]
    fn the_plural_aliases_are_shorthands_for_list() {
        for (argv, matches_list) in [
            (vec!["otty", "windows"], true),
            (vec!["otty", "tabs"], true),
            (vec!["otty", "panes"], true),
        ] {
            let cli = Cli::try_parse_from(argv).unwrap();
            assert!(matches_list && cli.command.is_some());
        }
    }

    #[test]
    fn the_colon_forms_parse() {
        for name in ["watch:claude", "watch:codex", "watch:opencode"] {
            let cli = Cli::try_parse_from(["otty", name, "abc123"]).unwrap();
            assert!(cli.command.is_some(), "{name}");
        }
        for name in ["state:claude", "state:codex", "state:opencode"] {
            let cli =
                Cli::try_parse_from(["otty", name, "state=idle", "session-id=x"]).unwrap();
            assert!(cli.command.is_some(), "{name}");
        }
    }

    #[test]
    fn config_prescribes_the_documented_verbs() {
        for argv in [
            vec!["otty", "config", "get", "theme"],
            vec!["otty", "config", "set", "theme", "dracula"],
            vec!["otty", "config", "unset", "font-size"],
            vec!["otty", "config", "edit"],
            vec!["otty", "config", "show"],
            vec!["otty", "config", "path"],
            vec!["otty", "config", "validate"],
            vec!["otty", "config", "reload"],
        ] {
            assert!(Cli::try_parse_from(&argv).is_ok(), "{argv:?}");
        }
    }

    #[test]
    fn config_set_carries_its_three_modifiers() {
        let cli = Cli::try_parse_from([
            "otty", "config", "set", "theme", "dracula", "--reload", "--transient", "--local",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Config(ConfigCmd::Set { reload, transient, local, .. })) => {
                assert!(reload && transient && local);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_long_forms_take_the_documented_flags() {
        let cli = Cli::try_parse_from([
            "otty", "pane", "split", "--direction", "right", "--command", "htop", "--size", "30",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Pane(PaneCmd::Split { direction, size, command, .. })) => {
                assert_eq!(direction, "right");
                assert_eq!(size, Some(30));
                assert_eq!(command.as_deref(), Some("htop"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_trailing_var_arg_keeps_the_commands_own_flags() {
        // `otty pane run -- cmd --flag` must not have `--flag` eaten by us.
        let cli = Cli::try_parse_from(["otty", "pane", "run", "--", "cargo", "test", "--all"])
            .unwrap();
        match cli.command {
            Some(Command::Pane(PaneCmd::Run { argv, .. })) => {
                assert_eq!(argv, ["cargo", "test", "--all"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn send_keys_keeps_key_tokens_intact() {
        let cli =
            Cli::try_parse_from(["otty", "pane", "send-keys", "--pane", "3", "--", "echo hi", "key:Enter"])
                .unwrap();
        match cli.command {
            Some(Command::Pane(PaneCmd::SendKeys { keys, pane })) => {
                assert_eq!(keys, ["echo hi", "key:Enter"]);
                assert_eq!(pane.as_deref(), Some("3"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn watch_agent_defaults_match_the_reference() {
        let cli = Cli::try_parse_from(["otty", "watch:claude", "9f3c"]).unwrap();
        match cli.command {
            Some(Command::WatchClaude(a)) => {
                assert_eq!(a.id, "9f3c");
                assert_eq!(a.interval_ms, 5000);
                assert_eq!(a.timeout_secs, 0);
                assert_eq!(a.unknown_timeout_secs, 60);
                assert!(!a.verbose);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn view_and_edit_share_one_placement_set() {
        for verb in ["view", "edit"] {
            let cli = Cli::try_parse_from(["otty", verb, "./README.md", "--right"]).unwrap();
            assert!(cli.command.is_some(), "{verb}");
        }
        let cli = Cli::try_parse_from(["otty", "view", "https://docs.otty.sh"]).unwrap();
        match cli.command {
            Some(Command::View { target, .. }) => assert_eq!(target, "https://docs.otty.sh"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_remaining_documented_subcommands_parse() {
        for argv in [
            vec!["otty", "font", "list", "--monospace"],
            vec!["otty", "font", "apply", "JetBrains Mono"],
            vec!["otty", "font", "import", "./MyFont.ttf", "--apply"],
            vec!["otty", "theme", "list", "--color", "dark"],
            vec!["otty", "theme", "import", "https://themes.otty.sh/nord.toml", "--activate"],
            vec!["otty", "keybind", "list", "--action", "split"],
            vec!["otty", "window", "list"],
            vec!["otty", "tab", "new", "--command", "cargo watch"],
            vec!["otty", "tab", "badge", "--kind", "running"],
            vec!["otty", "pane", "capture", "--pane", "2", "--lines", "50"],
            vec!["otty", "watch", "make", "deploy"],
            vec!["otty", "watch", "-q", "cargo", "test"],
            vec!["otty", "jump", "otty", "--no-cd"],
            vec!["otty", "learn", "./projects/otty"],
            vec!["otty", "ignore", "./old-project"],
            vec!["otty", "import", "ghostty"],
            vec!["otty", "import", "zoxide"],
            vec!["otty", "export", "ghostty"],
            vec!["otty", "export", "kitty", "-o", "kitty.conf"],
            vec!["otty", "features", "list"],
            vec!["otty", "features", "true-color"],
            vec!["otty", "ssh", "terminfo", "myserver", "--clean"],
            vec!["otty", "ssh", "doctor", "myserver"],
            vec!["otty", "completions", "zsh"],
            vec!["otty", "version"],
            vec!["otty", "ipc", "pane", "list"],
        ] {
            assert!(Cli::try_parse_from(&argv).is_ok(), "{argv:?}");
        }
    }

    #[test]
    fn ssh_doctor_passes_an_exact_invocation_through() {
        let cli = Cli::try_parse_from([
            "otty", "ssh", "doctor", "myserver", "--", "-F", "./config", "-o", "ControlMaster=no",
        ])
        .unwrap();
        match cli.command {
            Some(Command::Ssh(SshCmd::Doctor { host, ssh_args })) => {
                assert_eq!(host, "myserver");
                assert_eq!(ssh_args, ["-F", "./config", "-o", "ControlMaster=no"]);
            }
            other => panic!("{other:?}"),
        }
    }
}
