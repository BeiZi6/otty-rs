//! Configuration values and the lenient value grammar.
//!
//! Otty's config is a flat `key = value` file. It is a valid *subset* of TOML,
//! but it is read by a hand-written reader, not a TOML parser, because:
//!
//! * quotes around simple strings are optional (`theme = Dracula`);
//! * `on` / `off` are booleans alongside `true` / `false`;
//! * a bare `KEY=VALUE` is a valid value for the pair-typed keys (`env`,
//!   `cli-alias`, `agent-command`, `open-with-app`, `open-with-command`);
//! * unknown keys are silently ignored so a config from a newer build still
//!   loads on an older one.

use std::fmt;

use otty_core::Color;

/// What kind of value a key holds. The registry in [`crate::keys`] maps every
/// known key to one of these, and it drives both parsing and re-rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Bool,
    Int,
    Float,
    /// A bare string: an enum literal, a font family, a path, a free word.
    Str,
    /// `#rgb` / `#rrggbb` / `#rrggbbaa` / a CSS colour name / `transparent`.
    Color,
    /// An enum literal; parsed as a string, validated against the registry.
    Enum,
    /// An inline array: `["a", "b"]`.
    List,
    /// A repeated key, one entry per line, each entry free-form text.
    Repeated,
    /// A repeated key whose entries are `LHS=RHS` or `LHS|RHS` pairs.
    Pairs,
    /// A repeated key whose entry is a set of `field="value"` pairs
    /// (`tab-rule`, `details-view`, `quick-access`).
    Rule,
}

impl KeyKind {
    /// Whether the key may appear more than once in the file.
    pub const fn is_repeatable(self) -> bool {
        matches!(self, Self::Repeated | Self::Pairs | Self::Rule)
    }
}

/// A parsed configuration value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    /// Any text value, including enum literals.
    Str(String),
    Color(Color),
    /// An inline array; entries keep their source order.
    List(Vec<String>),
}

impl Value {
    /// Parse `raw` according to `kind`.
    ///
    /// The reader is lenient on purpose: a quoted string and a bare one parse
    /// the same, `on`/`off` are booleans, and a colour written as a name is as
    /// good as one written as hex. Only a structurally impossible value is an
    /// error — e.g. `font-size = wide`.
    pub fn parse(raw: &str, kind: KeyKind) -> Result<Self, ValueError> {
        let s = raw.trim();
        match kind {
            KeyKind::Bool => match s.to_ascii_lowercase().as_str() {
                "true" | "on" | "yes" | "1" => Ok(Self::Bool(true)),
                "false" | "off" | "no" | "0" => Ok(Self::Bool(false)),
                _ => Err(ValueError::NotBool(s.to_owned())),
            },
            KeyKind::Int => s
                .parse::<i64>()
                .map(Self::Int)
                .map_err(|_| ValueError::NotInt(s.to_owned())),
            KeyKind::Float => parse_float(s).map(Self::Float),
            KeyKind::Color => Color::parse(&unquote(s))
                .map(Self::Color)
                .map_err(|e| ValueError::NotColor(s.to_owned(), e)),
            KeyKind::List => parse_inline_array(s).map(Self::List),
            // Enum literals, paths, font families and free text all land here;
            // validation against the enum roster is a separate, advisory step.
            KeyKind::Str | KeyKind::Enum | KeyKind::Repeated | KeyKind::Pairs | KeyKind::Rule => {
                Ok(Self::Str(unquote(s)))
            }
        }
    }

    /// Parse without knowing the key: the best-effort form used for values that
    /// arrive from `otty config set` or from an external config importer.
    pub fn parse_auto(raw: &str) -> Self {
        let s = raw.trim();
        if let Ok(b) = Self::parse(s, KeyKind::Bool) {
            return b;
        }
        if let Ok(i) = s.parse::<i64>() {
            return Self::Int(i);
        }
        if let Ok(f) = parse_float(s) {
            return Self::Float(f);
        }
        if s.starts_with('[') && s.ends_with(']') {
            if let Ok(l) = parse_inline_array(s) {
                return Self::List(l);
            }
        }
        Self::Str(unquote(s))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<Color> {
        match self {
            Self::Color(c) => Some(*c),
            Self::Str(s) => Color::parse(s).ok(),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Self::List(l) => Some(l),
            _ => None,
        }
    }

    /// Render back into config spelling, ready to be written to the file.
    pub fn render(&self) -> String {
        match self {
            Self::Bool(b) => b.to_string(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => {
                if f.fract() == 0.0 {
                    format!("{f:.1}")
                } else {
                    let mut s = format!("{f}");
                    if s.len() > 8 {
                        s = format!("{f:.4}");
                        while s.ends_with('0') {
                            s.pop();
                        }
                        if s.ends_with('.') {
                            s.push('0');
                        }
                    }
                    s
                }
            }
            Self::Color(c) => c.to_hex(),
            Self::List(items) => {
                let inner: Vec<String> = items.iter().map(|i| quote_if_needed(i)).collect();
                format!("[{}]", inner.join(", "))
            }
            Self::Str(s) => quote_if_needed(s),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Str(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Str(v)
    }
}

impl From<Color> for Value {
    fn from(v: Color) -> Self {
        Self::Color(v)
    }
}

fn parse_float(s: &str) -> Result<f64, ValueError> {
    // Accept `0.92`, `1`, `-1.5`, `.5`, and a trailing `%`-free form. Reject
    // bare words so `font-size = wide` is a real error rather than zero.
    let t = s.trim();
    if t.is_empty() {
        return Err(ValueError::NotFloat(s.to_owned()));
    }
    let numeric = t.strip_prefix('+').unwrap_or(t);
    let ok = numeric
        .chars()
        .enumerate()
        .all(|(i, c)| c.is_ascii_digit() || c == '.' || (c == '-' && i == 0));
    if !ok || numeric == "." || numeric == "-" {
        return Err(ValueError::NotFloat(s.to_owned()));
    }
    t.parse::<f64>().map_err(|_| ValueError::NotFloat(s.to_owned()))
}

/// Strip one layer of matching `"` or `'` quotes.
pub fn unquote(s: &str) -> String {
    let t = s.trim();
    let bytes = t.as_bytes();
    if bytes.len() >= 2 {
        let (a, b) = (bytes[0], bytes[bytes.len() - 1]);
        if (a == b'"' && b == b'"') || (a == b'\'' && b == b'\'') {
            let inner = &t[1..t.len() - 1];
            return if a == b'"' {
                unescape_double(inner)
            } else {
                // Single quotes are literal, exactly as the theme format says.
                inner.to_owned()
            };
        }
    }
    t.to_owned()
}

fn unescape_double(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Wrap in double quotes when the value would otherwise not read back the same.
pub fn quote_if_needed(s: &str) -> String {
    let needs = s.is_empty()
        || s.contains(|c: char| c.is_whitespace())
        || s.contains(['#', '"', '\'', '\\', '[', ']', ',', '='])
        || s.starts_with(['+', '-'])
        || s.chars().any(|c| c.is_control());
    if needs {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.to_owned()
    }
}

/// Parse `["a", "b", 'c']`, with or without quotes per element.
///
/// The brackets are required — this is the *inline array* spelling from the
/// config-file-format page (`font-family-fallback = ["A", "B"]`), not a
/// comma-separated bare list. `[]` is the empty list.
pub fn parse_inline_array(s: &str) -> Result<Vec<String>, ValueError> {
    let t = s.trim();
    let inner = t
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .ok_or_else(|| ValueError::NotArray(t.to_owned()))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push('\\');
                        cur.push(n);
                    }
                } else if c == q {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == ',' {
                    out.push(unquote(cur.trim()));
                    cur.clear();
                } else {
                    cur.push(c);
                }
            }
        }
    }
    if quote.is_some() {
        return Err(ValueError::Unterminated);
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(unquote(cur.trim()));
    }
    Ok(out)
}

/// Split a `KEY=VALUE` entry on its *first* `=`, leaving the rest intact — the
/// rule the config-file-format page states for `env`.
pub fn split_first_eq(s: &str) -> Option<(&str, &str)> {
    s.split_once('=')
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, _)| !k.is_empty())
}

/// Split a `LEFT|RIGHT` entry on its first `|`, used by `open-with-app`,
/// `open-with-command` and `open-with-app`-style settings.
pub fn split_first_pipe(s: &str) -> Option<(&str, &str)> {
    s.split_once('|')
        .map(|(k, v)| (k.trim(), v.trim()))
        .filter(|(k, _)| !k.is_empty())
}

/// A value that could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    #[error("expected a boolean, found `{0}`")]
    NotBool(String),
    #[error("expected an integer, found `{0}`")]
    NotInt(String),
    #[error("expected a number, found `{0}`")]
    NotFloat(String),
    #[error("expected a colour, found `{0}` ({1})")]
    NotColor(String, otty_core::ColorError),
    #[error("expected an inline array `[...]`, found `{0}`")]
    NotArray(String),
    #[error("unterminated quote inside an inline array")]
    Unterminated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn booleans_accept_on_and_off() {
        assert_eq!(Value::parse("on", KeyKind::Bool).unwrap(), Value::Bool(true));
        assert_eq!(Value::parse("OFF", KeyKind::Bool).unwrap(), Value::Bool(false));
        assert!(Value::parse("maybe", KeyKind::Bool).is_err());
    }

    #[test]
    fn floats_want_a_number_not_a_word() {
        assert_eq!(Value::parse("13.0", KeyKind::Float).unwrap(), Value::Float(13.0));
        assert_eq!(Value::parse("-0.5", KeyKind::Float).unwrap(), Value::Float(-0.5));
        assert!(Value::parse("wide", KeyKind::Float).is_err());
    }

    #[test]
    fn quoting_is_symmetric_and_optional() {
        assert_eq!(Value::parse("Dracula", KeyKind::Str).unwrap(), Value::Str("Dracula".into()));
        assert_eq!(
            Value::parse("\"JetBrains Mono\"", KeyKind::Str).unwrap(),
            Value::Str("JetBrains Mono".into())
        );
        assert_eq!(Value::parse("'literal \\n'", KeyKind::Str).unwrap(), Value::Str("literal \\n".into()));
        assert_eq!(
            Value::parse("\"escaped \\\" quote\"", KeyKind::Str).unwrap(),
            Value::Str("escaped \" quote".into())
        );
    }

    #[test]
    fn inline_arrays_parse_and_round_trip() {
        let v = Value::parse(r#"["PingFang SC", "Noto Color Emoji"]"#, KeyKind::List).unwrap();
        assert_eq!(v.as_list().unwrap(), ["PingFang SC", "Noto Color Emoji"]);
        assert_eq!(v.render(), r#"["PingFang SC", "Noto Color Emoji"]"#);

        assert_eq!(Value::parse("[]", KeyKind::List).unwrap(), Value::List(vec![]));
        assert_eq!(Value::parse("[a, b]", KeyKind::List).unwrap(), Value::List(vec!["a".into(), "b".into()]));
        // A bare comma-separated list is NOT the inline-array spelling.
        assert!(Value::parse("a, b", KeyKind::List).is_err());
    }

    #[test]
    fn pair_entries_split_only_on_the_first_separator() {
        assert_eq!(split_first_eq("NO_PROXY=localhost,127.0.0.1"),
                   Some(("NO_PROXY", "localhost,127.0.0.1")));
        assert_eq!(split_first_eq("claude=claude --dangerously-skip-permissions"),
                   Some(("claude", "claude --dangerously-skip-permissions")));
        assert_eq!(split_first_eq("novalue"), None);
        assert_eq!(split_first_pipe("VS Code|com.microsoft.VSCode"),
                   Some(("VS Code", "com.microsoft.VSCode")));
        assert_eq!(split_first_pipe("yazi|yazi {}"), Some(("yazi", "yazi {}")));
    }

    #[test]
    fn numbers_render_as_the_docs_write_them() {
        assert_eq!(Value::Float(13.0).render(), "13.0");
        assert_eq!(Value::Float(0.92).render(), "0.92");
        assert_eq!(Value::Int(10000).render(), "10000");
        assert_eq!(Value::Bool(true).render(), "true");
    }

    #[test]
    fn auto_parsing_picks_the_tightest_fit() {
        assert_eq!(Value::parse_auto("true"), Value::Bool(true));
        assert_eq!(Value::parse_auto("42"), Value::Int(42));
        assert_eq!(Value::parse_auto("1.5"), Value::Float(1.5));
        assert_eq!(Value::parse_auto(r#"["a"]"#), Value::List(vec!["a".into()]));
        assert_eq!(Value::parse_auto("Dracula"), Value::Str("Dracula".into()));
        assert_eq!(Value::parse_auto("off"), Value::Bool(false));
    }

    #[test]
    fn rendering_quotes_only_when_required() {
        assert_eq!(Value::Str("Dracula".into()).render(), "Dracula");
        assert_eq!(Value::Str("JetBrains Mono".into()).render(), "\"JetBrains Mono\"");
        assert_eq!(Value::Str("#abc".into()).render(), "\"#abc\"");
        assert_eq!(Value::Str("-1".into()).render(), "\"-1\"");
        assert_eq!(Value::Color(otty_core::Color::rgb(1, 2, 3)).render(), "#010203");
    }

    #[test]
    fn repeatable_kinds_are_declared() {
        assert!(KeyKind::Pairs.is_repeatable());
        assert!(KeyKind::Rule.is_repeatable());
        assert!(KeyKind::Repeated.is_repeatable());
        assert!(!KeyKind::Str.is_repeatable());
        assert!(!KeyKind::List.is_repeatable());
    }
}
