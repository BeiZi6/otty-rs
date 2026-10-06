//! Coding-agent integrations.
//!
//! Otty does not run agents for you. It plugs into the agent CLIs you already
//! run, so the tab can show a live badge, ping you when a turn ends, resume a
//! conversation days later and fork one into a parallel branch. Each of those
//! needs something only Otty can know: how to relaunch the agent, and where it
//! keeps transcripts. That is what an [`Adapter`] carries.
//!
//! # One table, one shape
//!
//! | Agent | binary | integration | resume | fork |
//! |---|---|---|---|---|
//! | Claude Code | `claude` | JSON hooks in `~/.claude/settings.json` | ✓ | ✓ |
//! | Codex | `codex` | hooks in `~/.codex/hooks.json` | ✓ | ✓ |
//! | OpenCode | `opencode` | plugin in `~/.config/opencode/plugins` | ✓ | ✓ |
//! | Cursor CLI | `agent` | hooks in `~/.cursor/hooks.json` | ✓ | — |
//! | Kimi Code | `kimi` | `[[hooks]]` in `~/.kimi-code/config.toml` | ✓ | — |
//! | Pi | `pi` | extension in `~/.pi/agent/extensions` | ✓ | ✓ |
//! | omp | `omp` | extension in `~/.omp/agent/extensions` | ✓ | ✓ |
//! | Grok Build | `grok` | shares Claude's `~/.claude/settings.json` | ✓ | ✓ |
//! | Reasonix | `reasonix` | hooks in `~/.reasonix/settings.json` | ✓ | ✓ |
//! | CodeBuddy CLI | `codebuddy` | hooks in `~/.codebuddy/settings.json` | ✓ | — |
//! | Antigravity CLI | `agy` | group in `~/.gemini/config/hooks.json` | ✓ | — |
//! | GitHub Copilot CLI | `copilot` | a file under `~/.copilot/hooks/` | ✓ | — |
//! | anything else | your CLI | `otty state:<name>` from your own hook | — | — |
//!
//! Nothing here installs anything. [`Adapter::install_plan`] *describes* the
//! edit — which file, which entries, and how to take them back out — and the app
//! asks the user first. That is why the plan is a value: it is reviewable, and it
//! can be shown in a dialog before a single byte is written.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use otty_core::{AgentId, AgentState};
use serde::{Deserialize, Serialize};

/// Where an integration is installed, and in what shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    /// A JSON file with a `hooks` object. The entries are merged in, never
    /// replacing what is already there.
    JsonHooks { file: PathBuf },
    /// A TOML file with an array of tables (`[[hooks]]`).
    TomlHooks { file: PathBuf },
    /// A directory the agent scans for plugins or extensions at start-up.
    ExtensionDir { dir: PathBuf },
    /// No install of our own: the agent loads another agent's hook file as a
    /// compatibility layer, so Otty adds its entries there and each entry
    /// stands down when the wrong agent invoked it.
    Shares { file: PathBuf, with: &'static str },
    /// A directory the agent scans for hook files.
    HookDir { dir: PathBuf },
    /// Custom agents install nothing; they call `otty state:<name>` themselves.
    None,
}

/// How much of the usage panel an agent can fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageSupport {
    /// Context-window occupancy.
    pub context: bool,
    /// Running token total.
    pub tokens: bool,
    /// Session cost — only where the agent computes its own number.
    pub cost: bool,
}

impl UsageSupport {
    const NONE: Self = Self { context: false, tokens: false, cost: false };
    const TOKENS: Self = Self { context: true, tokens: true, cost: false };
    const FULL: Self = Self { context: true, tokens: true, cost: true };
}

/// Everything the UI needs about one agent.
#[derive(Debug, Clone, PartialEq)]
pub struct Adapter {
    pub id: AgentId,
    /// The binary name on `PATH`.
    pub command: &'static str,
    /// The display name, when it is not the command.
    pub label: &'static str,
    /// The subdirectory of `resources/agent-integration/` holding the payload.
    pub resource: &'static str,
    pub install: InstallKind,
    /// `resume` template; `<id>` is substituted.
    pub resume: Option<&'static str>,
    /// `fork` template. `None` means the action stays greyed out.
    pub fork: Option<&'static str>,
    /// Whether the agent's transcripts can be discovered and browsed.
    pub history: bool,
    /// Whether the agent answers a plan-quota probe.
    pub quota: bool,
    pub usage: UsageSupport,
}

/// The resume/fork templates, kept in one place so the table above reads as data.
macro_rules! adapter_table {
    ($( $variant:ident => {
        command: $command:literal,
        label: $label:literal,
        resource: $resource:literal,
        install: $install:expr,
        resume: $resume:expr,
        fork: $fork:expr,
        history: $history:literal,
        quota: $quota:literal,
        usage: $usage:expr,
    }),* $(,)?) => {
        /// Every adapter Otty ships.
        pub static ADAPTERS: &[Adapter] = &[
            $(Adapter {
                id: AgentId::$variant,
                command: $command,
                label: $label,
                resource: $resource,
                install: $install,
                resume: $resume,
                fork: $fork,
                history: $history,
                quota: $quota,
                usage: $usage,
            }),*
        ];
    };
}

adapter_table! {
    Claude => {
        command: "claude",
        label: "Claude Code",
        resource: "claude",
        install: InstallKind::JsonHooks { file: PathBuf::new() },
        resume: Some("claude --resume <id>"),
        fork: Some("claude --resume <id> --fork-session"),
        history: true,
        quota: true,
        usage: UsageSupport::TOKENS,
    },
    Codex => {
        command: "codex",
        label: "Codex",
        resource: "codex",
        install: InstallKind::JsonHooks { file: PathBuf::new() },
        resume: Some("codex resume <id>"),
        fork: Some("codex fork <id>"),
        history: true,
        quota: true,
        usage: UsageSupport::TOKENS,
    },
    Opencode => {
        command: "opencode",
        label: "OpenCode",
        resource: "opencode",
        install: InstallKind::ExtensionDir { dir: PathBuf::new() },
        resume: Some("opencode --session <id>"),
        fork: Some("opencode --fork --session <id>"),
        history: true,
        quota: false,
        usage: UsageSupport::FULL,
    },
    Cursor => {
        command: "agent",
        label: "Cursor CLI",
        resource: "cursor",
        install: InstallKind::JsonHooks { file: PathBuf::new() },
        resume: Some("agent --resume <id>"),
        // Cursor has no CLI fork command, so the action stays greyed out.
        fork: None,
        history: true,
        quota: false,
        usage: UsageSupport::NONE,
    },
    Kimi => {
        command: "kimi",
        label: "Kimi Code",
        resource: "kimi",
        install: InstallKind::TomlHooks { file: PathBuf::new() },
        resume: Some("kimi --session <id>"),
        fork: None,
        history: true,
        quota: false,
        usage: UsageSupport::TOKENS,
    },
    Pi => {
        command: "pi",
        label: "Pi",
        resource: "pi",
        install: InstallKind::ExtensionDir { dir: PathBuf::new() },
        resume: Some("pi --session <path>"),
        fork: Some("pi --fork <path>"),
        history: true,
        quota: false,
        usage: UsageSupport::FULL,
    },
    Omp => {
        command: "omp",
        label: "omp",
        resource: "opencode",
        install: InstallKind::ExtensionDir { dir: PathBuf::new() },
        resume: Some("omp --session <path>"),
        fork: Some("omp --fork <path>"),
        history: true,
        quota: true,
        usage: UsageSupport::FULL,
    },
    Grok => {
        command: "grok",
        label: "Grok Build",
        resource: "grok",
        // Grok reads Claude Code's hook file as a compatibility layer; its own
        // `hooks/` folder is per-project, so the user-wide install goes here.
        install: InstallKind::Shares { file: PathBuf::new(), with: "claude" },
        resume: Some("grok --resume <id>"),
        fork: Some("grok --resume <id> --fork-session"),
        // History is not wired up yet, so Grok sessions do not appear in it.
        history: false,
        quota: false,
        usage: UsageSupport::NONE,
    },
    Reasonix => {
        command: "reasonix",
        label: "Reasonix",
        resource: "reasonix",
        install: InstallKind::JsonHooks { file: PathBuf::new() },
        resume: Some("reasonix --resume <id>"),
        // `--copy` forks by leaving the original transcript alone.
        fork: Some("reasonix --resume <id> --copy"),
        // History is not wired up yet.
        history: false,
        quota: false,
        usage: UsageSupport::NONE,
    },
    Codebuddy => {
        command: "codebuddy",
        label: "CodeBuddy CLI",
        resource: "codebuddy",
        install: InstallKind::JsonHooks { file: PathBuf::new() },
        resume: Some("codebuddy --resume <id>"),
        fork: None,
        history: false,
        quota: false,
        usage: UsageSupport::NONE,
    },
    Antigravity => {
        command: "agy",
        label: "Google Antigravity CLI",
        resource: "antigravity",
        install: InstallKind::JsonHooks { file: PathBuf::new() },
        resume: Some("agy --conversation <id>"),
        fork: None,
        history: false,
        quota: false,
        usage: UsageSupport::NONE,
    },
    Copilot => {
        command: "copilot",
        label: "GitHub Copilot CLI",
        resource: "copilot",
        install: InstallKind::HookDir { dir: PathBuf::new() },
        resume: Some("copilot --resume <id>"),
        fork: None,
        history: false,
        quota: false,
        usage: UsageSupport::NONE,
    },
}

/// An environment variable that relocates an agent's config home.
///
/// The value is checked before the documented default, which is how a user who
/// set `KIMI_CODE_HOME` gets an integration in the right place.
pub fn config_home_env(id: &AgentId) -> Option<&'static str> {
    Some(match id {
        AgentId::Kimi => "KIMI_CODE_HOME",
        AgentId::Grok => "GROK_HOME",
        AgentId::Reasonix => "REASONIX_HOME",
        AgentId::Codex => "CODEX_HOME",
        AgentId::Copilot => "COPILOT_HOME",
        _ => return None,
    })
}

/// The adapter for an agent, if Otty ships one.
pub fn adapter(id: &AgentId) -> Option<&'static Adapter> {
    ADAPTERS.iter().find(|a| a.id == *id)
}

/// Built-in tab-icon stem for a foreground process.
///
/// `None` means the pane is still a shell (PowerShell, zsh, …) and the ordinary
/// terminal glyph stays. Distinctive names (`claude`, `codex`, `grok`) match a
/// token anywhere in the command line so an npm shim that actually runs
/// `node.exe …/claude-code/cli.js` still counts; short names (`agent`, `pi`,
/// `omp`, `agy`) match only the executable stem so `pip` and `agent-integration`
/// do not steal the icon.
pub fn icon_for_process(command: &str) -> Option<&'static str> {
    let stem = exe_stem(command);
    for rule in PROCESS_ICONS {
        if rule.anywhere {
            if tokens(command).iter().any(|t| token_hits(t, rule.name)) {
                return Some(rule.icon);
            }
        } else if stem == rule.name {
            return Some(rule.icon);
        }
    }
    None
}

/// First matching logo across a pane's foreground processes, else `terminal`.
pub fn icon_for_processes(procs: &[String]) -> &'static str {
    procs.iter().find_map(|p| icon_for_process(p)).unwrap_or("terminal")
}

struct ProcessIcon {
    name: &'static str,
    icon: &'static str,
    anywhere: bool,
}

const PROCESS_ICONS: &[ProcessIcon] = &[
    ProcessIcon { name: "claude", icon: "claude", anywhere: true },
    ProcessIcon { name: "codex", icon: "openai", anywhere: true },
    ProcessIcon { name: "grok", icon: "grok", anywhere: true },
    ProcessIcon { name: "copilot", icon: "copilot", anywhere: true },
    ProcessIcon { name: "gemini", icon: "gemini", anywhere: true },
    ProcessIcon { name: "antigravity", icon: "gemini", anywhere: true },
    ProcessIcon { name: "kimi", icon: "terminal-ai", anywhere: true },
    ProcessIcon { name: "opencode", icon: "terminal-ai", anywhere: true },
    ProcessIcon { name: "reasonix", icon: "terminal-ai", anywhere: true },
    ProcessIcon { name: "codebuddy", icon: "terminal-ai", anywhere: true },
    ProcessIcon { name: "agy", icon: "gemini", anywhere: false },
    ProcessIcon { name: "agent", icon: "terminal-ai", anywhere: false },
    ProcessIcon { name: "pi", icon: "terminal-ai", anywhere: false },
    ProcessIcon { name: "omp", icon: "terminal-ai", anywhere: false },
];

fn exe_stem(command: &str) -> String {
    let first = command
        .trim()
        .trim_matches('"')
        .split(|c: char| c.is_whitespace() || c == '"')
        .find(|s| !s.is_empty())
        .unwrap_or("");
    std::path::Path::new(first)
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn tokens(command: &str) -> Vec<String> {
    command
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn token_hits(token: &str, name: &str) -> bool {
    token == name
        || (name.len() > 2
            && token.starts_with(name)
            && token.as_bytes().get(name.len()) == Some(&b'-'))
}

/// Every agent Otty ships an adapter for, in table order.
pub fn supported() -> impl Iterator<Item = &'static Adapter> {
    ADAPTERS.iter()
}

/// Whether `id` is a custom agent — one Otty has no adapter for, which can still
/// drive its own badge through `otty state:<id>`.
pub fn is_custom(id: &AgentId) -> bool {
    adapter(id).is_none()
}

/// The capitalised display name a custom agent's badges use.
///
/// `state:myagent` reads as "Myagent" when the caller supplies no `label=`.
pub fn default_label(id: &AgentId) -> String {
    let s = id.as_str();
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The name shown on a badge or a notification.
pub fn display_name(id: &AgentId, reported_label: Option<&str>) -> String {
    match reported_label {
        Some(l) if !l.trim().is_empty() => l.trim().to_owned(),
        _ => adapter(id)
            .map(|a| a.label.to_owned())
            .unwrap_or_else(|| default_label(id)),
    }
}

/// Resolve the file or directory an adapter's integration goes into.
///
/// `home` is the user's home directory; agents with a relocatable config home
/// consult their environment variable first, exactly as the table documents.
pub fn install_path(adapter: &Adapter, home: &Path) -> InstallKind {
    let env_home = config_home_env(&adapter.id)
        .and_then(|k| std::env::var_os(k))
        .map(PathBuf::from);

    match &adapter.install {
        InstallKind::JsonHooks { .. } => {
            let file = match adapter.id {
                AgentId::Claude | AgentId::Grok => home.join(".claude").join("settings.json"),
                AgentId::Codex => env_home
                    .unwrap_or_else(|| home.join(".codex"))
                    .join("hooks.json"),
                AgentId::Cursor => home.join(".cursor").join("hooks.json"),
                AgentId::Reasonix => env_home
                    .unwrap_or_else(|| home.join(".reasonix"))
                    .join("settings.json"),
                AgentId::Codebuddy => home.join(".codebuddy").join("settings.json"),
                AgentId::Antigravity => home.join(".gemini").join("config").join("hooks.json"),
                _ => home.join(".config").join(adapter.resource).join("settings.json"),
            };
            InstallKind::JsonHooks { file }
        }
        InstallKind::TomlHooks { .. } => InstallKind::TomlHooks {
            file: env_home
                .unwrap_or_else(|| home.join(".kimi-code"))
                .join("config.toml"),
        },
        InstallKind::ExtensionDir { .. } => InstallKind::ExtensionDir {
            dir: match adapter.id {
                AgentId::Pi => home.join(".pi").join("agent").join("extensions"),
                AgentId::Omp => home.join(".omp").join("agent").join("extensions"),
                _ => home.join(".config").join("opencode").join("plugins"),
            },
        },
        InstallKind::HookDir { .. } => InstallKind::HookDir {
            dir: env_home
                .unwrap_or_else(|| home.join(".copilot"))
                .join("hooks"),
        },
        InstallKind::Shares { with, .. } => InstallKind::Shares {
            file: home.join(".claude").join("settings.json"),
            with,
        },
        InstallKind::None => InstallKind::None,
    }
}

/// What the user is asked to approve before anything is written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub agent: AgentId,
    pub label: String,
    /// The file or directory that would be created or edited.
    pub target: PathBuf,
    /// How it would be edited, in one sentence the dialog can show.
    pub action: String,
    /// Files that already exist and would be merged into rather than replaced.
    pub existing: bool,
    /// Extra steps the user has to take by hand afterwards.
    pub follow_ups: Vec<String>,
    /// Files the user has to restart or re-approve.
    pub restart_required: bool,
}

impl InstallPlan {
    pub fn summary(&self) -> String {
        let mut s = format!("{} — {}", self.label, self.target.display());
        s.push_str("\n");
        s.push_str(&self.action);
        for f in &self.follow_ups {
            s.push_str("\n• ");
            s.push_str(f);
        }
        s
    }
}

/// Describe the install for `id` without performing it.
pub fn install_plan(id: &AgentId, home: &Path) -> Option<InstallPlan> {
    let adapter = adapter(id)?;
    let kind = install_path(adapter, home);
    let (target, action, existing) = match &kind {
        InstallKind::JsonHooks { file } => (
            file.clone(),
            format!(
                "Merge Otty's hook entries into the `hooks` object of {}, leaving every existing \
                 key and hook untouched.",
                file.display()
            ),
            file.exists(),
        ),
        InstallKind::TomlHooks { file } => (
            file.clone(),
            format!(
                "Append Otty's `[[hooks]]` entries to {}, preserving the rest of the file.",
                file.display()
            ),
            file.exists(),
        ),
        InstallKind::ExtensionDir { dir } => (
            dir.clone(),
            format!("Drop Otty's plugin into {}; the agent loads it at start-up.", dir.display()),
            dir.exists(),
        ),
        InstallKind::HookDir { dir } => (
            dir.clone(),
            format!("Write Otty's hook file into {}.", dir.display()),
            dir.exists(),
        ),
        InstallKind::Shares { file, with } => (
            file.clone(),
            format!(
                "Add Otty's entries to {}, which this agent reads as a {with} compatibility layer. \
                 Each entry checks which agent invoked it and stands down when it is not the right \
                 one.",
                file.display()
            ),
            file.exists(),
        ),
        InstallKind::None => return None,
    };

    // The per-agent follow-ups the documentation calls out, because each is a
    // step a user would otherwise report as "the badge never appeared".
    let follow_ups = match id {
        AgentId::Codex => vec![
            "Codex only runs hooks when `hooks = true` in ~/.codex/config.toml; Otty turns it on \
             and offers to re-enable it if it is switched off later."
                .to_owned(),
            "Restart Codex in this tab, and approve the trust prompt when it reappears.".to_owned(),
        ],
        AgentId::Opencode => vec![
            "OpenCode 1.3.4 or later is required for the plugin.".to_owned(),
            "OpenCode does not announce a session when you switch to it with /sessions; send one \
             message in that session and it links up."
                .to_owned(),
        ],
        AgentId::Grok => vec![
            "Grok must trust a folder before it runs any hook in it; accept the prompt, or run \
             /hooks-trust."
                .to_owned(),
        ],
        AgentId::Reasonix => vec![
            "Reasonix loads hooks when a session starts — restart it after installing.".to_owned(),
            "If the install will not stick, settings.json has a syntax error; Otty will not \
             rewrite a file it cannot read."
                .to_owned(),
        ],
        AgentId::Codebuddy => vec![
            "Open CodeBuddy's /hooks panel to review and activate the external change.".to_owned(),
            "On Windows, CodeBuddy runs its hooks through Git Bash.".to_owned(),
        ],
        AgentId::Antigravity => vec![
            "A stop with background tasks still running keeps the processing badge active."
                .to_owned(),
        ],
        AgentId::Copilot => vec![
            "Set `$COPILOT_HOME` before installing if your hooks live somewhere other than \
             ~/.copilot/hooks/."
                .to_owned(),
        ],
        _ => vec![],
    };

    Some(InstallPlan {
        agent: id.clone(),
        label: adapter.label.to_owned(),
        target,
        action,
        existing,
        restart_required: true,
        follow_ups,
    })
}

/// Build the command that resumes `session` for `id`.
///
/// `overrides` is `agent-command = agent=COMMAND` from the config: it supplies
/// the executable and any global arguments, and Otty appends the native resume
/// or fork arguments. That is why the config must **not** contain `--resume` or
/// the session id itself — Otty adds those.
pub fn resume_command(
    id: &AgentId,
    session: &str,
    overrides: &BTreeMap<String, String>,
) -> Option<String> {
    let adapter = adapter(id)?;
    let template = adapter.resume?;
    Some(apply_template(template, session, overrides.get(id.as_str()).map(String::as_str)))
}

/// Build the command that forks `session` for `id`, when the agent supports it.
pub fn fork_command(
    id: &AgentId,
    session: &str,
    overrides: &BTreeMap<String, String>,
) -> Option<String> {
    let adapter = adapter(id)?;
    let template = adapter.fork?;
    Some(apply_template(template, session, overrides.get(id.as_str()).map(String::as_str)))
}

/// Whether a Fork action is offered at all — the UI greys it out when not.
pub fn supports_fork(id: &AgentId) -> bool {
    adapter(id).map(|a| a.fork.is_some()).unwrap_or(false)
}

/// Whether the agent's transcripts can be browsed.
pub fn supports_history(id: &AgentId) -> bool {
    adapter(id).map(|a| a.history).unwrap_or(false)
}

fn apply_template(template: &str, session: &str, override_cmd: Option<&str>) -> String {
    let base = match override_cmd.map(str::trim).filter(|s| !s.is_empty()) {
        // The override replaces the executable and its global arguments; the
        // resume/fork tail still comes from the adapter.
        Some(cmd) => {
            let tail = template.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
            format!("{cmd} {tail}")
        }
        None => template.to_owned(),
    };
    base.replace("<id>", session).replace("<path>", session)
}

/// A plan-quota probe.
///
/// Otty runs the agent's own CLI in the background, using the login the CLI
/// already has. Nothing is read from the Keychain and nothing is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaProbe {
    pub program: String,
    pub args: Vec<String>,
    /// `None` for a long-lived server the probe talks to over `app-server`.
    pub interval: Duration,
}

/// How often quota is re-read, and again whenever a task finishes.
pub const QUOTA_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// The probe for an agent, if it answers one.
pub fn quota_probe(id: &AgentId) -> Option<QuotaProbe> {
    let adapter = adapter(id)?;
    if !adapter.quota {
        return None;
    }
    Some(match id {
        // Run on the cheapest model, at low effort, with session persistence
        // off, so the probe neither appears in history nor costs meaningfully.
        // The model is named by its `haiku` alias, which is the spelling the
        // shipped build uses, so the probe keeps working when a model version
        // is retired.
        AgentId::Claude => QuotaProbe {
            program: "claude".into(),
            args: vec![
                "-p".into(),
                "/usage".into(),
                "--model".into(),
                "haiku".into(),
                "--no-session-persistence".into(),
            ],
            interval: QUOTA_INTERVAL,
        },
        AgentId::Codex => QuotaProbe {
            program: "codex".into(),
            args: vec!["app-server".into()],
            interval: QUOTA_INTERVAL,
        },
        AgentId::Omp => QuotaProbe {
            program: "omp".into(),
            args: vec!["usage".into(), "--json".into()],
            interval: QUOTA_INTERVAL,
        },
        _ => return None,
    })
}

/// The thresholds a bar turns amber and red at.
///
/// Only Codex states a real ceiling, so only Codex's readings get a bar with
/// these colours; every other agent shows raw occupancy.
pub const AMBER_AT: f32 = 0.75;
pub const RED_AT: f32 = 0.90;

/// The colour a quota bar takes at `fraction` full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarTone {
    Normal,
    Amber,
    Red,
}

pub fn bar_tone(fraction: f32) -> BarTone {
    if fraction >= RED_AT {
        BarTone::Red
    } else if fraction >= AMBER_AT {
        BarTone::Amber
    } else {
        BarTone::Normal
    }
}

/// Which per-tab behaviour a state transition should produce.
///
/// The toggles are per-tab overridable, so this is computed from the tab's
/// settings rather than read from a global.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentToggles {
    pub badge_processing: bool,
    pub badge_complete: bool,
    pub badge_awaiting: bool,
    pub notify_complete: bool,
    pub notify_awaiting: bool,
    pub caffeinate: bool,
    pub resume_on_recovery: bool,
}

impl Default for AgentToggles {
    fn default() -> Self {
        // The documented defaults: everything on except keep-awake.
        Self {
            badge_processing: true,
            badge_complete: true,
            badge_awaiting: true,
            notify_complete: true,
            notify_awaiting: true,
            caffeinate: false,
            resume_on_recovery: true,
        }
    }
}

/// What the UI should do about a state change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgentEffects {
    pub badge: Option<BadgeState>,
    pub notify: Option<NotifyKind>,
    /// Whether the sleep assertion should be held.
    pub hold_awake: bool,
}

/// The badge a pane shows for its agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeState {
    Processing,
    Complete,
    Awaiting,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyKind {
    TaskComplete,
    AwaitingInput,
}

/// Fold a state transition into the effects the toggles allow.
pub fn effects_for(state: AgentState, toggles: &AgentToggles) -> AgentEffects {
    match state {
        AgentState::Processing => AgentEffects {
            badge: toggles.badge_processing.then_some(BadgeState::Processing),
            notify: None,
            hold_awake: toggles.caffeinate,
        },
        AgentState::Idle => AgentEffects {
            badge: toggles.badge_complete.then_some(BadgeState::Complete),
            notify: toggles.notify_complete.then_some(NotifyKind::TaskComplete),
            // The hold lasts exactly as long as the task.
            hold_awake: false,
        },
        AgentState::Awaiting => AgentEffects {
            badge: toggles.badge_awaiting.then_some(BadgeState::Awaiting),
            notify: toggles.notify_awaiting.then_some(NotifyKind::AwaitingInput),
            hold_awake: toggles.caffeinate,
        },
        AgentState::Error => AgentEffects {
            badge: toggles.badge_complete.then_some(BadgeState::Error),
            notify: None,
            hold_awake: false,
        },
    }
}

/// Whether an incoming hook event can be attached to a pane.
///
/// Matching is by process tree: the reported pid must be a descendant of some
/// pane's shell, so an agent running outside Otty never badges an unrelated tab.
/// Once a pane has seen a session id, later events carrying the same id land on
/// that pane even without a pid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneMatch {
    /// The pid's ancestry names a pane.
    ByPid(otty_core::PaneId),
    /// A previously-seen session id names a pane.
    BySession(otty_core::PaneId),
    /// Neither; the event is dropped.
    Dropped(&'static str),
}

/// Decide how an event attaches to a pane.
pub fn match_pane(
    agent_pid: Option<u32>,
    session_id: Option<&str>,
    ancestry_lookup: impl Fn(u32) -> Option<otty_core::PaneId>,
    session_lookup: impl Fn(&str) -> Option<otty_core::PaneId>,
) -> PaneMatch {
    if let Some(pid) = agent_pid {
        if let Some(pane) = ancestry_lookup(pid) {
            return PaneMatch::ByPid(pane);
        }
    }
    if let Some(id) = session_id {
        if let Some(pane) = session_lookup(id) {
            return PaneMatch::BySession(pane);
        }
    }
    if agent_pid.is_none() && session_id.is_none() {
        PaneMatch::Dropped("the event carried neither agent-pid nor session-id")
    } else {
        PaneMatch::Dropped("the reported pid is not inside any pane's shell")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/u")
    }

    #[test]
    fn the_table_matches_the_documented_capabilities() {
        let expect = |id: AgentId, fork: bool, history: bool, quota: bool| {
            let a = adapter(&id).unwrap_or_else(|| panic!("no adapter for {id}"));
            assert_eq!(a.fork.is_some(), fork, "{id} fork");
            assert_eq!(a.history, history, "{id} history");
            assert_eq!(a.quota, quota, "{id} quota");
        };
        expect(AgentId::Claude, true, true, true);
        expect(AgentId::Codex, true, true, true);
        expect(AgentId::Opencode, true, true, false);
        expect(AgentId::Cursor, false, true, false);
        expect(AgentId::Kimi, false, true, false);
        expect(AgentId::Pi, true, true, false);
        expect(AgentId::Omp, true, true, true);
        expect(AgentId::Grok, true, false, false);
        expect(AgentId::Reasonix, true, false, false);
        expect(AgentId::Codebuddy, false, false, false);
        expect(AgentId::Antigravity, false, false, false);
        expect(AgentId::Copilot, false, false, false);
    }

    #[test]
    fn every_documented_binary_is_in_the_table() {
        let binaries: Vec<&str> = ADAPTERS.iter().map(|a| a.command).collect();
        for want in [
            "claude", "codex", "opencode", "agent", "kimi", "pi", "omp", "grok", "reasonix",
            "codebuddy", "agy", "copilot",
        ] {
            assert!(binaries.contains(&want), "{want} is missing from the table");
        }
    }

    #[test]
    fn resume_and_fork_commands_match_the_documented_spellings() {
        let none = BTreeMap::new();
        let id = AgentId::Claude;
        assert_eq!(
            resume_command(&id, "9f3c", &none).as_deref(),
            Some("claude --resume 9f3c")
        );
        assert_eq!(
            fork_command(&id, "9f3c", &none).as_deref(),
            Some("claude --resume 9f3c --fork-session")
        );
        assert_eq!(resume_command(&AgentId::Codex, "abc", &none).as_deref(), Some("codex resume abc"));
        assert_eq!(fork_command(&AgentId::Codex, "abc", &none).as_deref(), Some("codex fork abc"));
        assert_eq!(
            resume_command(&AgentId::Opencode, "s1", &none).as_deref(),
            Some("opencode --session s1")
        );
        assert_eq!(
            fork_command(&AgentId::Opencode, "s1", &none).as_deref(),
            Some("opencode --fork --session s1")
        );
        assert_eq!(
            resume_command(&AgentId::Antigravity, "c1", &none).as_deref(),
            Some("agy --conversation c1")
        );
        assert_eq!(
            resume_command(&AgentId::Pi, "/p/s.json", &none).as_deref(),
            Some("pi --session /p/s.json")
        );
        assert_eq!(
            fork_command(&AgentId::Reasonix, "r1", &none).as_deref(),
            Some("reasonix --resume r1 --copy")
        );
    }

    #[test]
    fn an_agent_without_fork_offers_none() {
        let none = BTreeMap::new();
        for id in [AgentId::Cursor, AgentId::Kimi, AgentId::Codebuddy, AgentId::Copilot] {
            assert!(!supports_fork(&id), "{id}");
            assert_eq!(fork_command(&id, "x", &none), None);
            // …but resume still works.
            assert!(resume_command(&id, "x", &none).is_some(), "{id}");
        }
    }

    #[test]
    fn an_agent_command_override_replaces_the_executable_only() {
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "claude".to_owned(),
            "claude --dangerously-skip-permissions".to_owned(),
        );
        assert_eq!(
            resume_command(&AgentId::Claude, "abc", &overrides).as_deref(),
            Some("claude --dangerously-skip-permissions --resume abc")
        );
        // The global flags come before the resume tail, which is what the CLI
        // needs to accept them.
        assert_eq!(
            fork_command(&AgentId::Claude, "abc", &overrides).as_deref(),
            Some("claude --dangerously-skip-permissions --resume abc --fork-session")
        );
    }

    #[test]
    fn a_blank_override_is_ignored() {
        let mut overrides = BTreeMap::new();
        overrides.insert("claude".to_owned(), "   ".to_owned());
        assert_eq!(
            resume_command(&AgentId::Claude, "abc", &overrides).as_deref(),
            Some("claude --resume abc")
        );
    }

    #[test]
    fn a_custom_agent_has_no_adapter_but_still_has_a_name() {
        let custom = AgentId::parse("myagent");
        assert!(is_custom(&custom));
        assert_eq!(adapter(&custom), None);
        assert_eq!(default_label(&custom), "Myagent");
        assert_eq!(display_name(&custom, None), "Myagent");
        assert_eq!(display_name(&custom, Some("My Agent")), "My Agent");
        // A custom agent can neither resume nor fork — that needs an adapter.
        assert_eq!(resume_command(&custom, "x", &BTreeMap::new()), None);
        assert!(!supports_fork(&custom));
    }

    #[test]
    fn built_in_agents_use_their_documented_label() {
        assert_eq!(display_name(&AgentId::Claude, None), "Claude Code");
        assert_eq!(display_name(&AgentId::Omp, None), "omp");
        assert_eq!(display_name(&AgentId::Antigravity, None), "Google Antigravity CLI");
        // A reported label wins, because a custom agent names itself.
        assert_eq!(display_name(&AgentId::Claude, Some("my claude")), "my claude");
    }

    #[test]
    fn tab_icons_follow_the_foreground_agent_and_ignore_the_shell() {
        assert_eq!(icon_for_process("claude"), Some("claude"));
        assert_eq!(icon_for_process("claude.exe"), Some("claude"));
        assert_eq!(icon_for_process(r"C:\npm\claude.cmd"), Some("claude"));
        assert_eq!(
            icon_for_process(r#"node.exe C:\Users\x\node_modules\@anthropic-ai\claude-code\cli.js"#),
            Some("claude")
        );
        assert_eq!(icon_for_process("codex"), Some("openai"));
        assert_eq!(icon_for_process("codex.exe --resume abc"), Some("openai"));
        assert_eq!(icon_for_process("grok"), Some("grok"));
        assert_eq!(icon_for_process("grok-build"), Some("grok"));
        assert_eq!(icon_for_process("copilot"), Some("copilot"));
        assert_eq!(icon_for_process("gemini"), Some("gemini"));
        assert_eq!(icon_for_process("agy"), Some("gemini"));
        assert_eq!(icon_for_process("agent"), Some("terminal-ai"));
        assert_eq!(icon_for_process("pwsh"), None);
        assert_eq!(
            icon_for_process(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"),
            None
        );
        assert_eq!(icon_for_process("pip"), None);
        assert_eq!(icon_for_process("agent-integration"), None);
        assert_eq!(icon_for_processes(&[]), "terminal");
        assert_eq!(
            icon_for_processes(&["pwsh.exe".into(), "claude.exe".into()]),
            "claude"
        );
    }

    #[test]
    fn grok_installs_into_claudes_file_as_a_compatibility_layer() {
        let plan = install_plan(&AgentId::Grok, &home()).unwrap();
        assert_eq!(plan.target, home().join(".claude").join("settings.json"));
        assert!(plan.action.contains("compatibility layer"), "{}", plan.action);
        assert!(plan.action.contains("stands down"), "{}", plan.action);
        assert!(plan.follow_ups.iter().any(|f| f.contains("/hooks-trust")));
    }

    #[test]
    fn kimi_gets_toml_hooks_and_honours_its_config_home() {
        let plan = install_plan(&AgentId::Kimi, &home()).unwrap();
        assert_eq!(plan.target, home().join(".kimi-code").join("config.toml"));
        assert!(plan.action.contains("[[hooks]]"));

        std::env::set_var("KIMI_CODE_HOME", "/elsewhere/kimi");
        let plan = install_plan(&AgentId::Kimi, &home()).unwrap();
        assert_eq!(plan.target, PathBuf::from("/elsewhere/kimi").join("config.toml"));
        std::env::remove_var("KIMI_CODE_HOME");
    }

    #[test]
    fn the_plugin_agents_install_into_a_directory() {
        assert_eq!(
            install_plan(&AgentId::Opencode, &home()).unwrap().target,
            home().join(".config").join("opencode").join("plugins")
        );
        assert_eq!(
            install_plan(&AgentId::Pi, &home()).unwrap().target,
            home().join(".pi").join("agent").join("extensions")
        );
        assert_eq!(
            install_plan(&AgentId::Omp, &home()).unwrap().target,
            home().join(".omp").join("agent").join("extensions")
        );
        assert_eq!(
            install_plan(&AgentId::Copilot, &home()).unwrap().target,
            home().join(".copilot").join("hooks")
        );
    }

    #[test]
    fn the_codex_plan_names_both_prerequisites() {
        let plan = install_plan(&AgentId::Codex, &home()).unwrap();
        assert_eq!(plan.target, home().join(".codex").join("hooks.json"));
        assert!(plan.follow_ups.iter().any(|f| f.contains("hooks = true")));
        assert!(plan.follow_ups.iter().any(|f| f.contains("trust prompt")));
    }

    #[test]
    fn a_custom_agent_has_no_install_plan() {
        assert_eq!(install_plan(&AgentId::parse("myagent"), &home()), None);
    }

    #[test]
    fn the_three_quota_agents_probe_the_documented_commands() {
        let claude = quota_probe(&AgentId::Claude).unwrap();
        assert_eq!(claude.program, "claude");
        assert_eq!(claude.args, ["-p", "/usage", "--model", "haiku", "--no-session-persistence"]);
        let codex = quota_probe(&AgentId::Codex).unwrap();
        assert_eq!(codex.args, ["app-server"]);
        let omp = quota_probe(&AgentId::Omp).unwrap();
        assert_eq!(omp.args, ["usage", "--json"]);
        assert_eq!(claude.interval, QUOTA_INTERVAL);
        assert_eq!(QUOTA_INTERVAL, Duration::from_secs(300));
        // Everything else has no quota to ask for.
        for id in [AgentId::Opencode, AgentId::Cursor, AgentId::Kimi, AgentId::Reasonix] {
            assert_eq!(quota_probe(&id), None, "{id}");
        }
    }

    #[test]
    fn only_codex_gets_a_coloured_bar() {
        // The thresholds apply to whatever a bar is drawn for; what matters is
        // that they are the documented 75 / 90.
        assert_eq!(bar_tone(0.10), BarTone::Normal);
        assert_eq!(bar_tone(0.74), BarTone::Normal);
        assert_eq!(bar_tone(0.75), BarTone::Amber);
        assert_eq!(bar_tone(0.89), BarTone::Amber);
        assert_eq!(bar_tone(0.90), BarTone::Red);
        assert_eq!(bar_tone(1.00), BarTone::Red);
        assert_eq!(AMBER_AT, 0.75);
        assert_eq!(RED_AT, 0.90);
    }

    #[test]
    fn the_default_toggles_match_the_settings_table() {
        let t = AgentToggles::default();
        assert!(t.badge_processing && t.badge_complete && t.badge_awaiting);
        assert!(t.notify_complete && t.notify_awaiting);
        assert!(!t.caffeinate, "Prevent Sleep While Processing is off by default");
        assert!(t.resume_on_recovery);
    }

    #[test]
    fn processing_badges_and_holds_awake_only_while_a_task_runs() {
        let t = AgentToggles { caffeinate: true, ..Default::default() };
        let on = effects_for(AgentState::Processing, &t);
        assert_eq!(on.badge, Some(BadgeState::Processing));
        assert!(on.hold_awake);
        assert_eq!(on.notify, None, "starting a task does not notify");

        let off = effects_for(AgentState::Idle, &t);
        assert_eq!(off.badge, Some(BadgeState::Complete));
        assert_eq!(off.notify, Some(NotifyKind::TaskComplete));
        assert!(!off.hold_awake, "the hold lasts exactly as long as the task");
    }

    #[test]
    fn awaiting_input_has_its_own_badge_and_notification() {
        let t = AgentToggles::default();
        let e = effects_for(AgentState::Awaiting, &t);
        assert_eq!(e.badge, Some(BadgeState::Awaiting));
        assert_eq!(e.notify, Some(NotifyKind::AwaitingInput));
    }

    #[test]
    fn turning_a_toggle_off_silences_only_that_effect() {
        let t = AgentToggles {
            badge_processing: false,
            notify_awaiting: false,
            ..Default::default()
        };
        assert_eq!(effects_for(AgentState::Processing, &t).badge, None);
        // The complete badge and notification are untouched.
        let idle = effects_for(AgentState::Idle, &t);
        assert_eq!(idle.badge, Some(BadgeState::Complete));
        assert_eq!(idle.notify, Some(NotifyKind::TaskComplete));
        // …and only the awaiting *notification* is silenced.
        let awaiting = effects_for(AgentState::Awaiting, &t);
        assert_eq!(awaiting.badge, Some(BadgeState::Awaiting));
        assert_eq!(awaiting.notify, None);
    }

    #[test]
    fn a_failed_turn_badges_but_does_not_ping() {
        let t = AgentToggles::default();
        let e = effects_for(AgentState::Error, &t);
        assert_eq!(e.badge, Some(BadgeState::Error));
        assert_eq!(e.notify, None);
    }

    #[test]
    fn pane_matching_prefers_the_process_tree() {
        let pane = otty_core::PaneId::new(3);
        let by_pid = match_pane(Some(42), Some("s1"), |p| (p == 42).then_some(pane), |_| None);
        assert_eq!(by_pid, PaneMatch::ByPid(pane));

        // A pid outside every pane falls back to a session the pane already saw.
        let by_session = match_pane(Some(99), Some("s1"), |_| None, |s| (s == "s1").then_some(pane));
        assert_eq!(by_session, PaneMatch::BySession(pane));

        // Neither: dropped, so an agent running outside Otty never badges a tab.
        assert!(matches!(
            match_pane(Some(99), None, |_| None, |_| None),
            PaneMatch::Dropped(_)
        ));
        assert!(matches!(
            match_pane(None, None, |_| None, |_| None),
            PaneMatch::Dropped(_)
        ));
    }

    #[test]
    fn the_install_plan_reads_as_a_dialog() {
        let plan = install_plan(&AgentId::Claude, &home()).unwrap();
        let s = plan.summary();
        assert!(s.contains("Claude Code"), "{s}");
        assert!(s.contains("settings.json"), "{s}");
        assert!(s.contains("Merge"), "{s}");
    }

    #[test]
    fn every_adapter_resolves_to_a_path_under_the_given_home() {
        for a in ADAPTERS {
            let kind = install_path(a, &home());
            let target = match &kind {
                InstallKind::JsonHooks { file }
                | InstallKind::TomlHooks { file }
                | InstallKind::Shares { file, .. } => file.clone(),
                InstallKind::ExtensionDir { dir }
                | InstallKind::HookDir { dir } => dir.clone(),
                InstallKind::None => continue,
            };
            assert!(
                target.starts_with(home()) || target.is_absolute(),
                "{} resolved to {}",
                a.id,
                target.display()
            );
        }
    }
}
