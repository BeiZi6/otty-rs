//! The config key registry: metadata for every key Otty recognises.
//!
//! The data itself lives in the generated [`crate::keys`] module, produced from
//! `spec/config-schema.json` by `tools/gen_keys.py`. This module owns the types
//! and the lookups so the generated file stays pure data.
//!
//! The registry is the single source of truth for:
//!
//! * which keys exist — the counterpart of `Config::known_keys()` in the shipped
//!   build, which is what generated the editor syntax files;
//! * how a value is parsed (see [`KeyKind`]);
//! * what a key resets to;
//! * which enum literals a key accepts, for validation and completion;
//! * the legacy spellings that are rewritten on write (`shell` → `command`,
//!   `font-bold` → `text-bold`).

use crate::value::KeyKind;
use otty_core::{Color, Platform};

/// How a list-valued key is spelled in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyle {
    /// One line per entry, in order — `env = FOO=bar`.
    Repeated,
    /// A single line holding the whole list — `font-family-fallback = ["a", "b"]`.
    Inline,
}

/// A key's default, in the type the key actually uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Default {
    /// The docs say `(none)` / `(auto)` / `(theme)` / `(built-in …)`: the
    /// effective value is derived at runtime, not from a constant.
    Derived,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
    Color(&'static str),
    List(&'static [&'static str]),
}

impl Default {
    /// The default as a config [`Value`](crate::value::Value), when one exists.
    pub fn value(self) -> Option<crate::value::Value> {
        use crate::value::Value;
        Some(match self {
            Self::Derived => return None,
            Self::Bool(b) => Value::Bool(b),
            Self::Int(i) => Value::Int(i),
            Self::Float(f) => Value::Float(f),
            Self::Str(s) => Value::Str(s.to_owned()),
            Self::Color(s) => Value::Color(Color::parse(s).ok()?),
            Self::List(l) => Value::List(l.iter().map(|s| (*s).to_owned()).collect()),
        })
    }

    /// The default in the spelling the config file would use.
    pub fn rendered(self) -> Option<String> {
        self.value().map(|v| v.render())
    }
}

/// Everything the loader, the Settings UI, the CLI and the syntax highlighter
/// need to know about one config key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeySpec {
    /// Canonical key name.
    pub key: &'static str,
    /// The reference page's section heading, used to group keys in the UI.
    pub section: &'static str,
    pub kind: KeyKind,
    pub default: Default,
    /// Verbatim default text when [`KeySpec::default`] is [`Default::Derived`].
    pub default_note: &'static str,
    /// For enums, the type name — several keys share one roster.
    pub enum_type: &'static str,
    /// The complete set of accepted literals, for enums only.
    pub enum_values: &'static [&'static str],
    /// Other spellings accepted on read and rewritten on write.
    pub aliases: &'static [&'static str],
    /// `all`, `macos`, `windows`, `linux` or `windows-linux`.
    pub platform: &'static str,
    /// `Some(..)` for list-valued keys.
    pub list_style: Option<ListStyle>,
    /// The shape of a structured entry, e.g. `KEY=VALUE`, `Name|bundle`.
    pub value_shape: &'static str,
    pub doc: &'static str,
}

impl KeySpec {
    /// Whether this key is meaningful on the running platform.
    pub fn applies_here(&self) -> bool {
        match self.platform {
            "all" => true,
            "macos" => cfg!(target_os = "macos"),
            "windows" => cfg!(target_os = "windows"),
            "linux" => cfg!(target_os = "linux"),
            "windows-linux" => cfg!(any(target_os = "windows", target_os = "linux")),
            _ => true,
        }
    }

    pub fn platform_enum(&self) -> Platform {
        match self.platform {
            "macos" => Platform::Macos,
            "windows" => Platform::Windows,
            "linux" => Platform::Linux,
            _ => Platform::host(),
        }
    }

    /// Whether `value` is an accepted literal for this key, when it is an enum.
    pub fn accepts_literal(&self, value: &str) -> bool {
        if self.kind != KeyKind::Enum || self.enum_values.is_empty() {
            return true;
        }
        self.enum_values.iter().any(|v| v.eq_ignore_ascii_case(value.trim()))
    }
}

/// Every key Otty recognises, in documentation order.
pub fn all() -> &'static [KeySpec] {
    crate::keys::KEYS
}

/// Look a key up by its canonical name or any of its aliases.
pub fn lookup(name: &str) -> Option<&'static KeySpec> {
    let n = name.trim();
    crate::keys::KEYS
        .iter()
        .find(|k| k.key == n || k.aliases.contains(&n))
}

/// The canonical spelling of `name`, following aliases.
pub fn canonical(name: &str) -> Option<&'static str> {
    lookup(name).map(|k| k.key)
}

/// Whether the key is one Otty knows. Unknown keys are ignored, not rejected.
pub fn is_known(name: &str) -> bool {
    lookup(name).is_some()
}

/// Every distinct enum roster in the registry, for validation messages.
pub fn enum_roster(enum_type: &str) -> &'static [&'static str] {
    crate::keys::KEYS
        .iter()
        .find(|k| k.enum_type == enum_type && !k.enum_values.is_empty())
        .map(|k| k.enum_values)
        .unwrap_or(&[])
}

/// The keys whose value is a list, split by spelling.
pub fn list_keys(style: ListStyle) -> impl Iterator<Item = &'static KeySpec> {
    all().iter().filter(move |k| k.list_style == Some(style))
}

/// How many keys the registry holds — the counterpart of the "236 keys" figure
/// the shipped build's generated syntax file encodes.
pub fn count() -> usize {
    all().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_non_empty_and_deduplicated() {
        assert!(count() >= 230, "registry holds only {} keys", count());
        let mut names: Vec<&str> = all().iter().map(|k| k.key).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate key in the registry");
    }

    #[test]
    fn aliases_resolve_to_their_canonical_key() {
        let mut broken = Vec::new();
        for spec in all() {
            for alias in spec.aliases {
                if canonical(alias) != Some(spec.key) {
                    broken.push(format!(
                        "alias `{alias}` of `{}` resolves to {:?}",
                        spec.key,
                        canonical(alias)
                    ));
                }
                // An alias must never collide with a real key, or the real key
                // could never be written.
                if all().iter().any(|k| k.key == *alias) {
                    broken.push(format!("alias `{alias}` shadows a real key"));
                }
            }
        }
        assert!(broken.is_empty(), "{}", broken.join("\n"));
    }

    #[test]
    fn documented_defaults_are_present() {
        let expect = |k: &str, want: &str| {
            let spec = lookup(k).unwrap_or_else(|| panic!("{k} is not registered"));
            assert_eq!(
                spec.default.rendered().as_deref(),
                Some(want),
                "{k} default"
            );
        };
        // Rendered defaults are spelled the way the file would spell them, so
        // a value with a space comes back quoted.
        expect("font-family", "\"JetBrains Mono\"");
        expect("font-size", "13.0");
        expect("scrollback-lines", "10000");
        expect("cursor-style", "block");
        expect("cursor-style-blink", "default_off");
        expect("font-blending", "srgb-over");
        expect("font-ligatures", "dlig");
        expect("text-bold", "auto");
        expect("text-underline", "true");
        expect("text-blink", "false");
        expect("background-opacity", "1.0");
        expect("faint-opacity", "0.5");
        expect("minimum-contrast", "1.0");
        expect("theme", "Paper");
        expect("theme-dark", "Nord");
        expect("auto-theme-dark-mode", "true");
        expect("window-layout", "sidebar-left");
        expect("language", "system");
        expect("on-launch", "restore_session");
        expect("new-tab-position", "auto");
        expect("osc-color-report-format", "16-bit");
        expect("session-log-mode", "redacted");
        expect("session-log-size-mb", "5");
        expect("clipboard-read", "ask");
        expect("clipboard-write", "allow");
        expect("right-click-action", "context-menu");
        expect("link-open-with", "browser");
        expect("file-open-with", "default-app");
        expect("details-panel-width", "220");
        expect("open-quickly-folders-limit", "12");
        expect("editor-tab-size", "4");
        expect("recipe-replay-saved", "ask_once");
        expect("recipe-replay-file", "manually");
        expect("window-size", "remember");
        expect("term", "auto");
    }

    #[test]
    fn documented_enum_rosters_are_complete() {
        let expect = |k: &str, want: &[&str]| {
            let spec = lookup(k).unwrap_or_else(|| panic!("{k} is not registered"));
            for v in want {
                assert!(spec.accepts_literal(v), "{k} rejects documented value `{v}`");
            }
        };
        expect("cursor-style", &["block", "bar", "underline", "block_hollow"]);
        expect(
            "cursor-style-blink",
            &["default_off", "default_on", "always_off", "always_on"],
        );
        expect("text-bold", &["off", "auto", "primary-only", "synthetic"]);
        expect("text-italic", &["off", "auto", "primary-only", "synthetic"]);
        expect("font-ligatures", &["off", "calt", "dlig"]);
        expect("font-blending", &["srgb-over", "macos-like", "linear", "perceptual"]);
        expect("window-layout", &["sidebar-left", "tabs-top", "tabs-bottom"]);
        expect("window-size", &["remember", "frame", "grid"]);
        expect("language", &["system", "english", "chinese"]);
        expect("on-launch", &["new_window", "restore_session"]);
        expect("new-tab-position", &["end", "auto", "after-current"]);
        expect("confirm-close-tab", &["always", "process", "never"]);
        expect("confirm-close-window", &["always", "process", "multiple_tabs", "never"]);
        expect("clipboard-read", &["ask", "allow", "deny"]);
        expect("clipboard-write", &["ask", "allow", "deny"]);
        expect(
            "right-click-action",
            &["context-menu", "copy", "paste", "copy-or-paste", "ignore"],
        );
        expect("link-open-with", &["browser", "otty"]);
        expect("file-open-with", &["default-app", "otty"]);
        expect("folder-open-with", &["default-app", "otty"]);
        expect("link-schemes", &["all", "custom"]);
        expect("macos-option-as-alt", &["true", "false", "left", "right"]);
        expect("osc-color-report-format", &["none", "8-bit", "16-bit"]);
        expect("session-log-mode", &["plain", "redacted"]);
        expect("session-restore-processes", &["none", "whitelist", "all"]);
        expect("recipe-replay-saved", &["auto", "ask_once", "manually"]);
        expect("recipe-replay-file", &["auto", "ask_once", "manually"]);
        expect("autocomplete-shortcut", &["tab", "tab+right-arrow", "right-arrow", "ctrl+space", "disable"]);
        expect("autocomplete-show-candidates", &["disable", "auto", "escape", "option-escape"]);
        expect("notification-foreground", &["off", "always", "tab-unfocused"]);
        expect("quick-terminal-cwd", &["last-used", "current-pane"]);
        expect("tab-icon-placement", &["combined", "separate"]);
        expect(
            "terminal-scroll-past-end",
            &["disabled", "last-line-with-content", "last-line-in-middle", "cursor-line"],
        );
        expect(
            "terminal-scroll-past-first-line",
            &["disabled", "same-as-last-line", "first-line-with-content", "first-line-in-middle"],
        );
        expect("output-scroll-animation", &["auto", "off", "smooth"]);
        expect("cursor-animation", &["off", "smooth"]);
        expect(
            "widen-ambiguous",
            &[
                "enclosed-alphanumerics",
                "number-forms",
                "math-operators",
                "misc-technical",
                "misc-symbols",
                "dingbats",
                "arrows",
                "geometric-shapes",
            ],
        );
        expect("auto-hide-tab-bar", &["always", "default", "auto"]);
        expect("auto-hide-tabs-panel", &["always", "default", "auto"]);
        expect("autocomplete-description-language", &["system", "english", "chinese"]);
    }

    #[test]
    fn list_spellings_match_the_config_file_format_page() {
        let repeated = [
            "keybind", "env", "cli-alias", "hint-pattern", "agent-command", "agent-quota-off",
            "open-with-app", "open-with-command", "profile", "tab-rule", "details-view",
            "autocomplete-history-ignore",
        ];
        for k in repeated {
            let spec = lookup(k).unwrap_or_else(|| panic!("{k} is not registered"));
            assert_eq!(spec.list_style, Some(ListStyle::Repeated), "{k} list style");
        }
        let inline = [
            "font-family-fallback", "font-family-fallback-bold", "font-family-fallback-italic",
            "font-family-fallback-bold-italic", "widen-ambiguous", "link-scheme-allowlist",
            "shell-integration-disabled-shells", "notification-sound",
            "session-restore-process-allowlist", "progress-bar-commands",
        ];
        for k in inline {
            let spec = lookup(k).unwrap_or_else(|| panic!("{k} is not registered"));
            assert_eq!(spec.list_style, Some(ListStyle::Inline), "{k} list style");
        }
        assert_eq!(ListStyle::Repeated, ListStyle::Repeated);
    }

    #[test]
    fn unknown_keys_are_simply_unknown() {
        assert!(!is_known("no-such-key"));
        assert!(lookup("no-such-key").is_none());
        assert!(is_known("font-size"));
        // Aliases count as known.
        assert!(is_known("shell"));
        assert!(is_known("ligatures"));
    }
}
