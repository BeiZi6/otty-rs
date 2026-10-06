//! The rule-shaped config values: `tab-rule`, `details-view` and `quick-access`.
//!
//! All three share one grammar — a single line of `field="value"` pairs — so
//! they share one parser ([`parse_field_pairs`]) and one serializer
//! ([`render_field_pairs`]), which is also what keeps the Settings panel's
//! round-trip byte-stable.
//!
//! Matching rules, from the configuration reference:
//!
//! * matching is case-insensitive;
//! * `~` expands to the home directory;
//! * `*` spans `/`, so one wildcard covers a whole tree;
//! * a pattern with no `*` also matches everything *below* it, so
//!   `~/Work/otty` covers the whole project;
//! * each of `alias` / `icon` / `title` resolves independently — the first rule
//!   that matches and fills that field wins.

use std::path::Path;

use otty_core::{expand_path, Color};

/// Parse `path="~/Work/otty" alias="Otty" icon="terminal"` into pairs.
///
/// Values may be single- or double-quoted; an unquoted value runs to the next
/// whitespace. Escapes inside double quotes are honoured (`\"`).
pub fn parse_field_pairs(line: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_')
        {
            i += 1;
        }
        if i == start {
            break;
        }
        let key: String = chars[start..i].iter().collect();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '=' {
            // A bare word with no value: ignore it, the next iteration resumes.
            continue;
        }
        i += 1; // `=`
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
            let q = chars[i];
            i += 1;
            while i < chars.len() {
                // Only `\"` and `\\` are escapes; a lone backslash before
                // anything else survives, so a Windows path reads unchanged.
                if chars[i] == '\\' && q == '"' && i + 1 < chars.len() {
                    if chars[i + 1] == '"' || chars[i + 1] == '\\' {
                        value.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                }
                if chars[i] == q {
                    i += 1;
                    break;
                }
                value.push(chars[i]);
                i += 1;
            }
        } else {
            while i < chars.len() && !chars[i].is_whitespace() {
                value.push(chars[i]);
                i += 1;
            }
        }
        out.push((key, value));
    }
    out
}

/// Serialize pairs back into the one-line spelling, quoting where needed.
pub fn render_field_pairs(pairs: &[(String, String)]) -> String {
    let mut parts = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        let quoted = if v.contains(' ') || v.contains('"') {
            format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
        } else if v.contains('\'') {
            format!("\"{}\"", v.replace('"', "\\\""))
        } else {
            v.clone()
        };
        parts.push(format!("{k}={quoted}"));
    }
    parts.join(" ")
}

/// Glob matcher with Otty's rules: case-insensitive, `*` spans `/`, and a
/// pattern with no `*` also matches everything beneath it.
pub fn glob_matches(pattern: &str, subject: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    let p = expand_path(pattern);
    let (p, s) = (p.to_lowercase(), subject.to_lowercase());
    let p = p.trim_end_matches('/');
    let s_trim = s.trim_end_matches('/');

    if !p.contains('*') && !p.contains('?') {
        return s_trim == p || s_trim.starts_with(&format!("{p}/"));
    }
    glob_wildcard(p.as_bytes(), s.as_bytes())
}

/// Backtracking wildcard match where `*` matches any run (including `/`).
fn glob_wildcard(pat: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(sp) = star {
            p = sp + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

/// What a `tab-rule` is matched against.
#[derive(Debug, Clone, Default)]
pub struct MatchContext {
    /// The pane's working directory.
    pub path: Option<String>,
    /// The foreground command line.
    pub command: Option<String>,
    /// The coding agent bound to the pane.
    pub agent: Option<String>,
    /// The SSH host of a remote session.
    pub host: Option<String>,
    /// A file / folder / URL pane's name. Never set for a terminal pane.
    pub file: Option<String>,
    /// The shell's `$USER`.
    pub user: Option<String>,
    /// The repository's current branch.
    pub branch: Option<String>,
    /// Whatever the program set with OSC 0/1/2.
    pub title: Option<String>,
    /// The shell's basename.
    pub shell: Option<String>,
    /// The pane's index within its window.
    pub index: Option<usize>,
}

impl MatchContext {
    /// Build the context for a pane's working directory.
    pub fn in_dir(dir: impl AsRef<Path>) -> Self {
        Self {
            path: Some(dir.as_ref().to_string_lossy().into_owned()),
            ..Default::default()
        }
    }
}

/// One `tab-rule` line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TabRule {
    pub path: Option<String>,
    pub command: Option<String>,
    pub agent: Option<String>,
    pub host: Option<String>,
    pub file: Option<String>,
    pub alias: Option<String>,
    pub icon: Option<String>,
    /// An ANSI colour name resolved against the active palette, or `#rrggbb`.
    pub icon_color: Option<String>,
    pub title: Option<String>,
}

impl TabRule {
    pub fn parse(line: &str) -> Result<Self, RuleError> {
        let pairs = parse_field_pairs(line);
        if pairs.is_empty() {
            return Err(RuleError::Empty);
        }
        let mut r = Self::default();
        for (k, v) in pairs {
            match k.as_str() {
                "path" => r.path = Some(v),
                "command" => r.command = Some(v),
                "agent" => r.agent = Some(v),
                "host" => r.host = Some(v),
                "file" => r.file = Some(v),
                "alias" => r.alias = Some(v),
                "icon" => r.icon = Some(v),
                "icon-color" => r.icon_color = Some(v),
                "title" => r.title = Some(v),
                other => return Err(RuleError::UnknownField(other.to_owned())),
            }
        }
        Ok(r)
    }

    pub fn render(&self) -> String {
        let mut pairs: Vec<(String, String)> = Vec::new();
        let mut push = |k: &str, v: &Option<String>| {
            if let Some(v) = v {
                pairs.push((k.to_owned(), v.clone()));
            }
        };
        push("path", &self.path);
        push("command", &self.command);
        push("agent", &self.agent);
        push("host", &self.host);
        push("file", &self.file);
        push("alias", &self.alias);
        push("icon", &self.icon);
        push("icon-color", &self.icon_color);
        push("title", &self.title);
        render_field_pairs(&pairs)
    }

    /// Every condition the rule states must hold. An omitted condition is
    /// "don't care".
    pub fn matches(&self, ctx: &MatchContext) -> bool {
        let check = |pat: &Option<String>, subject: &Option<String>| match (pat, subject) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(p), Some(s)) => glob_matches(p, s),
        };
        check(&self.path, &ctx.path)
            && check(&self.command, &ctx.command)
            && check(&self.agent, &ctx.agent)
            && check(&self.host, &ctx.host)
            && check(&self.file, &ctx.file)
    }

    /// Resolve this rule's icon colour against a palette.
    pub fn resolve_icon_color(&self, palette: &crate::palette::Palette) -> Option<Color> {
        let raw = self.icon_color.as_deref()?;
        if raw.is_empty() {
            return None;
        }
        if let Some(idx) = ansi_index(raw) {
            return Some(palette.get(idx));
        }
        Color::parse(raw).ok()
    }
}

/// Map an ANSI colour name (`red`, `bright-blue`, …) to its palette index.
pub fn ansi_index(name: &str) -> Option<u8> {
    let n = name.trim().to_ascii_lowercase().replace('_', "-");
    let (bright, base) = match n.strip_prefix("bright-") {
        Some(rest) => (true, rest),
        None => (false, n.as_str()),
    };
    let slot = match base {
        "black" => 0,
        "red" => 1,
        "green" => 2,
        "yellow" => 3,
        "blue" => 4,
        "magenta" | "purple" => 5,
        "cyan" => 6,
        "white" => 7,
        _ => return None,
    };
    Some(if bright { slot + 8 } else { slot })
}

/// The title variables a `tab-rule` may interpolate.
pub const TITLE_VARS: &[&str] = &[
    "alias", "cwd", "folder", "user", "host", "agent", "branch", "command", "title", "shell",
    "file", "index",
];

/// Expand a `title` template against a context.
///
/// A variable with no value expands to nothing, and a separator left stranded
/// by it is dropped — `${folder} · ${branch}` reads `otty` outside a repository,
/// not `otty ·`. Chains are written `|`-separated with the first non-empty
/// winning: `${title|folder}`; a single-quoted last link ends the chain in
/// literal text: `${title|file|folder|'Shell'}`.
pub fn expand_title(template: &str, ctx: &MatchContext, alias: Option<&str>) -> String {
    let mut out = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
            let mut j = i + 2;
            let mut depth = 1;
            while j < chars.len() {
                if chars[j] == '{' {
                    depth += 1;
                } else if chars[j] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            if j >= chars.len() {
                out.push(chars[i]);
                i += 1;
                continue;
            }
            let expr: String = chars[i + 2..j].iter().collect();
            out.push_str(&resolve_chain(&expr, ctx, alias));
            i = j + 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    tidy_separators(&out)
}

fn resolve_chain(expr: &str, ctx: &MatchContext, alias: Option<&str>) -> String {
    for link in split_chain(expr) {
        let link = link.trim();
        if link.len() >= 2 && link.starts_with('\'') && link.ends_with('\'') {
            return link[1..link.len() - 1].to_owned();
        }
        if let Some(v) = title_var(link, ctx, alias) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    String::new()
}

/// Split a chain on `|`, leaving quoted segments intact.
fn split_chain(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    for c in expr.chars() {
        match c {
            '\'' => {
                quote = !quote;
                cur.push(c);
            }
            '|' if !quote => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn title_var(name: &str, ctx: &MatchContext, alias: Option<&str>) -> Option<String> {
    Some(match name {
        "alias" => alias?.to_owned(),
        "cwd" => ctx.path.clone()?,
        "folder" => Path::new(ctx.path.as_deref()?)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())?,
        "user" => ctx.user.clone()?,
        "host" => ctx.host.clone()?,
        "agent" => ctx.agent.clone()?,
        "branch" => ctx.branch.clone()?,
        "command" => ctx.command.clone()?,
        "title" => ctx.title.clone()?,
        "shell" => ctx
            .shell
            .as_deref()
            .map(|s| Path::new(s).file_name().map_or_else(|| s.to_owned(), |f| f.to_string_lossy().into_owned()))?,
        "file" => ctx.file.clone()?,
        "index" => ctx.index?.to_string(),
        _ => return None,
    })
}

/// Drop separators left stranded when every variable around them was empty.
fn tidy_separators(s: &str) -> String {
    let mut out = s.trim().to_owned();
    while out.ends_with(" ·") || out.ends_with(" -") || out.ends_with(" |") || out.ends_with(" —") {
        let cut = out.rfind(' ').unwrap_or(0);
        out.truncate(cut);
        out = out.trim_end().to_owned();
    }
    while out.starts_with("· ") || out.starts_with("- ") || out.starts_with("| ") {
        out = out[2..].to_owned();
    }
    out
}

/// What kind of view a Details-Panel entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewKind {
    #[default]
    Builtin,
    /// A program running in an embedded terminal.
    Tui,
    /// A URL loaded in a web pane.
    Web,
}

impl ViewKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "builtin" => Self::Builtin,
            "tui" => Self::Tui,
            "web" => Self::Web,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Tui => "tui",
            Self::Web => "web",
        }
    }
}

/// One `details-view` line.
#[derive(Debug, Clone, PartialEq)]
pub struct DetailsView {
    /// Required. A built-in name, or your own — which also names the view's
    /// folder under `~/.config/otty/views/`.
    pub id: String,
    pub kind: ViewKind,
    pub title: Option<String>,
    pub icon: Option<String>,
    /// A whole command line, run the way a prompt would run it.
    pub command: Option<String>,
    pub url: Option<String>,
    /// Ask the site for its mobile page. On by default.
    pub mobile: Option<bool>,
    /// Where a tui view runs. Empty means `~/.config/otty/views/<id>`.
    pub cwd: Option<String>,
    pub enabled: Option<bool>,
}

impl DetailsView {
    /// The four built-ins, in the order they are appended when `details-view`
    /// is absent or partial.
    pub const BUILTINS: &'static [&'static str] = &["info", "outline", "git", "files"];

    pub fn parse(line: &str) -> Result<Self, RuleError> {
        let pairs = parse_field_pairs(line);
        if pairs.is_empty() {
            return Err(RuleError::Empty);
        }
        let mut id = None;
        let mut kind = None;
        let mut title = None;
        let mut icon = None;
        let mut command = None;
        let mut url = None;
        let mut mobile = None;
        let mut cwd = None;
        let mut enabled = None;
        for (k, v) in pairs {
            match k.as_str() {
                "id" => id = Some(v),
                "kind" => kind = ViewKind::parse(&v),
                "title" => title = Some(v),
                "icon" => icon = Some(v),
                "command" => command = Some(v),
                "url" => url = Some(v),
                "mobile" => mobile = parse_bool(&v),
                "cwd" => cwd = Some(v),
                "enabled" => enabled = parse_bool(&v),
                other => return Err(RuleError::UnknownField(other.to_owned())),
            }
        }
        let id = id.ok_or(RuleError::MissingField("id"))?;
        // `kind` is inferred from `url` / `command` when omitted.
        let kind = kind.unwrap_or(if url.is_some() {
            ViewKind::Web
        } else if command.is_some() {
            ViewKind::Tui
        } else {
            ViewKind::Builtin
        });
        Ok(Self { id, kind, title, icon, command, url, mobile, cwd, enabled })
    }

    pub fn render(&self) -> String {
        let mut pairs: Vec<(String, String)> = vec![("id".into(), self.id.clone())];
        if self.kind != ViewKind::Builtin
            || self.command.is_some()
            || self.url.is_some()
        {
            pairs.push(("kind".into(), self.kind.as_str().to_owned()));
        }
        if let Some(t) = &self.title {
            pairs.push(("title".into(), t.clone()));
        }
        if let Some(i) = &self.icon {
            pairs.push(("icon".into(), i.clone()));
        }
        if let Some(c) = &self.command {
            pairs.push(("command".into(), c.clone()));
        }
        if let Some(u) = &self.url {
            pairs.push(("url".into(), u.clone()));
        }
        if let Some(m) = self.mobile {
            pairs.push(("mobile".into(), m.to_string()));
        }
        if let Some(c) = &self.cwd {
            pairs.push(("cwd".into(), c.clone()));
        }
        if let Some(e) = self.enabled {
            pairs.push(("enabled".into(), e.to_string()));
        }
        render_field_pairs(&pairs)
    }

    /// Where a tui view runs when `cwd` is empty.
    pub fn working_dir(&self) -> std::path::PathBuf {
        match &self.cwd {
            Some(c) if !c.is_empty() => expand_path(c).into(),
            _ => crate::Config::views_dir().join(&self.id),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    /// The full ordered view list: the user's entries, then any built-in they
    /// did not mention, so a built-in added in a later release still shows up.
    pub fn resolve_all(configured: &[DetailsView]) -> Vec<DetailsView> {
        let mut out = configured.to_vec();
        for b in Self::BUILTINS {
            if !out.iter().any(|v| v.id == *b) {
                out.push(DetailsView {
                    id: (*b).to_owned(),
                    kind: ViewKind::Builtin,
                    title: None,
                    icon: None,
                    command: None,
                    url: None,
                    mobile: None,
                    cwd: None,
                    enabled: None,
                });
            }
        }
        out
    }
}

/// One `quick-access` line: a pin for the `+` menu, the tray menu and the
/// Windows Jump List.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickAccess {
    /// `folder`, `recipe`, `ssh` or `profile`.
    pub kind: QuickKind,
    /// The path, recipe id, ssh host or profile id.
    pub value: String,
    pub label: String,
}

/// What a `quick-access` pin points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickKind {
    Folder,
    Recipe,
    Ssh,
    Profile,
}

impl QuickKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "folder" => Self::Folder,
            "recipe" => Self::Recipe,
            "ssh" => Self::Ssh,
            "profile" => Self::Profile,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Recipe => "recipe",
            Self::Ssh => "ssh",
            Self::Profile => "profile",
        }
    }
}

impl QuickAccess {
    pub fn parse(line: &str) -> Result<Self, RuleError> {
        let pairs = parse_field_pairs(line);
        let mut kind = None;
        let mut value = None;
        let mut label = None;
        for (k, v) in pairs {
            match k.as_str() {
                "kind" => kind = QuickKind::parse(&v),
                "value" => value = Some(v),
                "label" => label = Some(v),
                other => return Err(RuleError::UnknownField(other.to_owned())),
            }
        }
        let kind = kind.ok_or(RuleError::MissingField("kind"))?;
        let value = value.ok_or(RuleError::MissingField("value"))?;
        let label = label.unwrap_or_else(|| {
            Path::new(&value)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| value.clone())
        });
        Ok(Self { kind, value, label })
    }

    pub fn render(&self) -> String {
        render_field_pairs(&[
            ("kind".into(), self.kind.as_str().to_owned()),
            ("value".into(), self.value.clone()),
            ("label".into(), self.label.clone()),
        ])
    }
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// A rule line that could not be understood.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    #[error("the rule is empty")]
    Empty,
    #[error("`{0}` is not a field of this rule")]
    UnknownField(String),
    #[error("the rule is missing the required field `{0}`")]
    MissingField(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_pairs_parse_all_the_documented_examples() {
        let r = TabRule::parse(r#"path="~/Work/otty" alias="Otty" icon="terminal" title="${alias} · ${branch}""#).unwrap();
        assert_eq!(r.path.as_deref(), Some("~/Work/otty"));
        assert_eq!(r.alias.as_deref(), Some("Otty"));
        assert_eq!(r.icon.as_deref(), Some("terminal"));
        assert_eq!(r.title.as_deref(), Some("${alias} · ${branch}"));

        let r = TabRule::parse(r#"agent="claude" icon="claude" icon-color="magenta""#).unwrap();
        assert_eq!(r.agent.as_deref(), Some("claude"));
        assert_eq!(r.icon_color.as_deref(), Some("magenta"));

        let r = TabRule::parse(r#"command="ssh *" icon="server" icon-color="cyan" title="${user}@${host}""#).unwrap();
        assert_eq!(r.command.as_deref(), Some("ssh *"));

        let r = TabRule::parse(r#"file="*.md" icon="markdown" icon-color="blue""#).unwrap();
        assert_eq!(r.file.as_deref(), Some("*.md"));
    }

    #[test]
    fn rules_round_trip_through_their_own_spelling() {
        let line = r#"path="~/Work/otty" alias="Otty" title="${alias} · ${branch}""#;
        let r = TabRule::parse(line).unwrap();
        let again = TabRule::parse(&r.render()).unwrap();
        assert_eq!(r, again);
        assert!(r.render().contains("alias=Otty"));
    }

    #[test]
    fn globs_are_case_insensitive_and_star_spans_slashes() {
        assert!(glob_matches("Work/otty", "work/otty"));
        assert!(glob_matches("Work/otty", "WORK/OTTY/deep/nested"));
        assert!(!glob_matches("Work/otty", "Work/other"));
        assert!(glob_matches("Work/otty/*.md", "work/otty/a/b/c.md"));
        assert!(glob_matches("ssh *", "ssh web1"));
        assert!(glob_matches("*.md", "README.md"));
        assert!(!glob_matches("*.md", "README.txt"));
        assert!(!glob_matches("", "anything"));
    }

    #[test]
    fn a_leading_tilde_expands_against_the_home_directory() {
        let home = otty_core::dirs_home().expect("a home directory");
        let inside = format!("{}/Work/otty", home.to_string_lossy());
        assert!(glob_matches("~/Work/otty", &inside));
        assert!(!glob_matches("~/Work/otty", "/somewhere/else/Work/otty"));
    }

    #[test]
    fn a_rule_with_several_conditions_ands_them() {
        let r = TabRule::parse(r#"command="ssh *" host="web*""#).unwrap();
        let ctx = MatchContext { command: Some("ssh web1".into()), host: Some("web1".into()), ..Default::default() };
        assert!(r.matches(&ctx));
        let ctx = MatchContext { command: Some("ssh web1".into()), host: Some("db1".into()), ..Default::default() };
        assert!(!r.matches(&ctx));
        // An omitted condition is "don't care".
        let r = TabRule::parse(r#"icon="terminal""#).unwrap();
        assert!(r.matches(&MatchContext::default()));
    }

    #[test]
    fn a_file_rule_never_matches_a_terminal_pane() {
        let r = TabRule::parse(r#"file="*.md""#).unwrap();
        assert!(!r.matches(&MatchContext::default()));
        assert!(r.matches(&MatchContext { file: Some("notes.md".into()), ..Default::default() }));
    }

    #[test]
    fn title_templates_drop_stranded_separators() {
        let ctx = MatchContext {
            path: Some("/home/u/Work/otty".into()),
            ..Default::default()
        };
        assert_eq!(expand_title("${folder} · ${branch}", &ctx, None), "otty");
        assert_eq!(expand_title("${folder} · ${branch}", &MatchContext {
            branch: Some("main".into()),
            ..ctx.clone()
        }, None), "otty · main");
        assert_eq!(expand_title("${alias} · ${branch}", &ctx, Some("Otty")), "Otty");
    }

    #[test]
    fn title_chains_take_the_first_non_empty_link() {
        let ctx = MatchContext { path: Some("/home/u/Work/typora".into()), ..Default::default() };
        assert_eq!(expand_title("${alias}: ${title|folder}", &ctx, Some("Typora")), "Typora: typora");

        let empty = MatchContext::default();
        assert_eq!(expand_title("${title|file|folder|'Shell'}", &empty, None), "Shell");
        assert_eq!(expand_title("${title|file|folder|'Shell'}", &ctx, None), "typora");
    }

    #[test]
    fn every_documented_title_variable_resolves() {
        let ctx = MatchContext {
            path: Some("/home/u/Work/otty".into()),
            command: Some("cargo test".into()),
            agent: Some("claude".into()),
            host: Some("web1".into()),
            file: Some("main.rs".into()),
            user: Some("hitxy".into()),
            branch: Some("main".into()),
            title: Some("building".into()),
            shell: Some("/bin/zsh".into()),
            index: Some(3),
        };
        for v in TITLE_VARS {
            let got = expand_title(&format!("${{{v}}}"), &ctx, Some("Otty"));
            assert!(!got.is_empty(), "${v} did not resolve");
        }
        assert_eq!(expand_title("${shell}", &ctx, None), "zsh");
        assert_eq!(expand_title("${index}", &ctx, None), "3");
        assert_eq!(expand_title("${alias}", &ctx, Some("Otty")), "Otty");
    }

    #[test]
    fn ansi_colour_names_resolve_against_the_palette() {
        let p = crate::palette::Palette::default();
        let r = TabRule::parse(r#"icon="x" icon-color="bright-blue""#).unwrap();
        assert_eq!(r.resolve_icon_color(&p), Some(p.get(12)));
        // A literal `#` inside the value needs a longer raw-string delimiter.
        let r = TabRule::parse(r##"icon="x" icon-color="#ff0000""##).unwrap();
        assert_eq!(r.resolve_icon_color(&p), Some(Color::rgb(255, 0, 0)));
        let r = TabRule::parse(r#"icon="x""#).unwrap();
        assert_eq!(r.resolve_icon_color(&p), None);
    }

    #[test]
    fn details_views_infer_their_kind_and_append_the_builtins() {
        let line = r#"id="lazygit" kind="tui" title="Git" command="lazygit -p \"${cwd}\"""#;
        let v = DetailsView::parse(line).unwrap();
        assert_eq!(v.kind, ViewKind::Tui);
        assert_eq!(v.command.as_deref(), Some(r#"lazygit -p "${cwd}""#));
        assert_eq!(DetailsView::parse(&v.render()).unwrap(), v);

        let v = DetailsView::parse(r#"id="grafana" url="http://localhost:3000""#).unwrap();
        assert_eq!(v.kind, ViewKind::Web);

        let v = DetailsView::parse(r#"id="docker" command="lazydocker""#).unwrap();
        assert_eq!(v.kind, ViewKind::Tui);

        let v = DetailsView::parse(r#"id="info""#).unwrap();
        assert_eq!(v.kind, ViewKind::Builtin);

        // Listing one built-in still yields all four, in order.
        let all = DetailsView::resolve_all(&[v]);
        let ids: Vec<&str> = all.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["info", "outline", "git", "files"]);
    }

    #[test]
    fn a_view_without_an_id_is_rejected() {
        let e = DetailsView::parse(r#"kind="tui""#).unwrap_err();
        assert_eq!(e, RuleError::MissingField("id"));
    }

    #[test]
    fn quick_access_entries_default_their_label_to_the_value() {
        let q = QuickAccess::parse(r#"kind="folder" value="C:\dev\otty""#).unwrap();
        assert_eq!(q.kind, QuickKind::Folder);
        assert_eq!(q.label, "otty");
        let q = QuickAccess::parse(r#"kind="folder" value="C:\dev\otty" label="otty""#).unwrap();
        assert_eq!(q.render(), r#"kind=folder value=C:\dev\otty label=otty"#);
        assert_eq!(QuickAccess::parse(&q.render()).unwrap(), q);
        assert!(matches!(
            QuickAccess::parse(r#"kind="nope" value="x""#),
            Err(RuleError::MissingField("kind"))
        ));
    }
}
