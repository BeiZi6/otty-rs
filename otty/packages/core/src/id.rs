//! Identifiers.
//!
//! Every window, tab, pane and agent session has a stable id that survives
//! across the CLI (`otty pane show --pane "$OTTY_PANE_ID"`), the IPC layer, the
//! AppleScript bridge and session restore.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl $name {
            /// Wrap a raw counter value.
            pub const fn new(v: u64) -> Self {
                Self(v)
            }

            /// The raw counter value.
            pub const fn get(self) -> u64 {
                self.0
            }

            /// The spelling used in `--pane`/`--tab` selectors and `$OTTY_*` env
            /// variables, e.g. `pane-7`.
            pub fn slug(self) -> String {
                format!("{}-{}", $prefix, self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = ParseIdError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let t = s.strip_prefix(concat!($prefix, "-")).unwrap_or(s);
                t.parse::<u64>().map(Self).map_err(|_| ParseIdError)
            }
        }
    };
}

id_type!(
    /// Identifies a top-level window.
    WindowId,
    "window"
);
id_type!(
    /// Identifies a tab inside a window.
    TabId,
    "tab"
);
id_type!(
    /// Identifies a pane (a single terminal / file / web surface).
    PaneId,
    "pane"
);
id_type!(
    /// Identifies a persisted agent conversation.
    SessionId,
    "session"
);

/// Failed to parse an id selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("not a valid Otty id")]
pub struct ParseIdError;

/// A supported coding-agent integration.
///
/// The set matches the adapters Otty ships (`Resources/agent-integration/`) plus
/// the generic escape hatch: `state:<agent>` accepts any name, which is how a
/// third-party agent integrates itself without a built-in adapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentId {
    Claude,
    Codex,
    Opencode,
    Cursor,
    Copilot,
    Grok,
    Kimi,
    Pi,
    Gemini,
    Antigravity,
    Codebuddy,
    Reasonix,
    Omp,
    /// An integration Otty has no built-in adapter for.
    Custom(String),
}

impl AgentId {
    /// The identifier used in config (`agent-command = claude=…`), hook scripts
    /// and `otty watch:<agent>`.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Opencode => "opencode",
            Self::Cursor => "cursor",
            Self::Copilot => "copilot",
            Self::Grok => "grok",
            Self::Kimi => "kimi",
            Self::Pi => "pi",
            Self::Gemini => "gemini",
            Self::Antigravity => "antigravity",
            Self::Codebuddy => "codebuddy",
            Self::Reasonix => "reasonix",
            Self::Omp => "omp",
            Self::Custom(s) => s,
        }
    }

    /// The agents Otty ships a first-class adapter for.
    pub const BUILTIN: &'static [&'static str] = &[
        "claude", "codex", "opencode", "cursor", "copilot", "grok", "kimi", "pi", "gemini",
        "antigravity", "codebuddy", "reasonix", "omp",
    ];

    /// Parse an agent name, falling back to [`AgentId::Custom`].
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "opencode" => Self::Opencode,
            "cursor" => Self::Cursor,
            "copilot" => Self::Copilot,
            "grok" => Self::Grok,
            "kimi" => Self::Kimi,
            "pi" => Self::Pi,
            "gemini" => Self::Gemini,
            "antigravity" => Self::Antigravity,
            "codebuddy" => Self::Codebuddy,
            "reasonix" => Self::Reasonix,
            "omp" => Self::Omp,
            other => Self::Custom(other.to_owned()),
        }
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The lifecycle state an agent reports through its hooks.
///
/// Follows `otty state:<agent> state=<v>` — see the recovered `otty-hook.sh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    /// The agent is mid-turn.
    Processing,
    /// The turn finished; the agent is waiting for the next prompt.
    Idle,
    /// The agent is blocked on an approval or a question.
    Awaiting,
    /// The turn failed. `error-kind` names why (`rate_limit`, …).
    Error,
}

impl AgentState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Processing => "processing",
            Self::Idle => "idle",
            Self::Awaiting => "awaiting",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "processing" => Some(Self::Processing),
            "idle" => Some(Self::Idle),
            "awaiting" => Some(Self::Awaiting),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_selectors() {
        let p = PaneId::new(7);
        assert_eq!(p.to_string(), "pane-7");
        assert_eq!("pane-7".parse::<PaneId>().unwrap(), p);
        assert_eq!("7".parse::<PaneId>().unwrap(), p);
        assert!("pane-x".parse::<PaneId>().is_err());
    }

    #[test]
    fn unknown_agents_become_custom() {
        assert_eq!(AgentId::parse("Claude"), AgentId::Claude);
        assert_eq!(AgentId::parse("aider"), AgentId::Custom("aider".into()));
        assert_eq!(AgentId::parse("aider").as_str(), "aider");
    }

    #[test]
    fn every_builtin_agent_parses_to_itself() {
        for name in AgentId::BUILTIN {
            assert_eq!(AgentId::parse(name).as_str(), *name);
        }
    }
}
