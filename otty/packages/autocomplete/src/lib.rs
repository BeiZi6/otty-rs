//! Autocomplete: a Fig-compatible spec database, an inline ghost-text rule, and
//! the on-device learning that feeds both.
//!
//! The shipped app carries `completions-client.db` — a SQLite file with 711 CLI
//! specs in Fig's JSON format — and this crate reads that same file instead of
//! inventing a format:
//!
//! ```sql
//! CREATE TABLE specs      (name TEXT PRIMARY KEY, spec TEXT NOT NULL,
//!                          source TEXT NOT NULL DEFAULT 'upstream', updated_at TEXT);
//! CREATE TABLE spec_i18n  (name TEXT NOT NULL, lang TEXT NOT NULL, overlay TEXT NOT NULL,
//!                          PRIMARY KEY (name, lang));
//! CREATE TABLE db_meta    (key TEXT PRIMARY KEY, value TEXT NOT NULL);
//! ```
//!
//! `db_meta` answers `schema_version` and `fig_base_date`; `spec_i18n` carries the
//! translated descriptions `autocomplete-description-language` selects.
//!
//! # The inline rule
//!
//! Showing a faded inline preview is a claim about what you meant, so Otty only
//! makes it when one candidate *clearly* leads — never merely the first of
//! several. That is [`Suggestion::inline`]'s whole job, and it is why the
//! function is separated from ranking.

pub mod spec;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

pub use spec::{FigArg, FigOption, FigSpec, FigSubcommand};

/// The schema version this reader understands.
pub const SUPPORTED_SCHEMA: i64 = 2;

/// A failure coming out of the spec database.
#[derive(Debug, thiserror::Error)]
pub enum SpecError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("the completions database says schema version {found}; this build reads {supported}")]
    SchemaVersion { found: i64, supported: i64 },
    #[error("`{name}` holds a spec that is not valid JSON: {source}")]
    BadSpec { name: String, source: serde_json::Error },
}

/// The spec database, opened read-only.
///
/// Read-only on purpose: the file is a build artefact that a future Otty may
/// replace wholesale, and nothing in the UI should be able to corrupt it.
pub struct SpecStore {
    conn: Connection,
}

impl SpecStore {
    /// Open the database at `path`.
    pub fn open(path: &Path) -> Result<Self, SpecError> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let store = Self { conn };
        store.check_schema()?;
        Ok(store)
    }

    /// Open the copy that sits next to the executable, when there is one.
    pub fn open_bundled() -> Option<Result<Self, SpecError>> {
        let path = bundled_path()?;
        Some(Self::open(&path))
    }

    fn check_schema(&self) -> Result<(), SpecError> {
        if let Some(found) = self.meta("schema_version")?.and_then(|v| v.parse().ok()) {
            if found > SUPPORTED_SCHEMA {
                return Err(SpecError::SchemaVersion { found, supported: SUPPORTED_SCHEMA });
            }
        }
        Ok(())
    }

    /// A `db_meta` value.
    pub fn meta(&self, key: &str) -> Result<Option<String>, SpecError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM db_meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// The date the bundled Fig data was cut, for the About panel.
    pub fn fig_base_date(&self) -> Result<Option<String>, SpecError> {
        self.meta("fig_base_date")
    }

    /// Every spec name, sorted.
    pub fn names(&self) -> Result<Vec<String>, SpecError> {
        let mut stmt = self.conn.prepare("SELECT name FROM specs ORDER BY name")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// How many specs are in the database.
    pub fn len(&self) -> Result<usize, SpecError> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM specs", [], |r| r.get::<_, i64>(0))? as usize)
    }

    pub fn is_empty(&self) -> Result<bool, SpecError> {
        Ok(self.len()? == 0)
    }

    /// Load one spec by name, with `lang`'s overlay applied when there is one.
    pub fn spec(&self, name: &str, lang: Option<&str>) -> Result<Option<FigSpec>, SpecError> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT spec FROM specs WHERE name = ?1", [name], |r| r.get(0))
            .optional()?;
        let Some(raw) = raw else { return Ok(None) };
        let mut parsed: FigSpec = serde_json::from_str(&raw)
            .map_err(|source| SpecError::BadSpec { name: name.to_owned(), source })?;
        if let Some(lang) = lang {
            if let Some(overlay) = self.overlay(name, lang)? {
                parsed.apply_overlay(&overlay);
            }
        }
        Ok(Some(parsed))
    }

    /// The translated overlay for a spec, as `autocomplete-description-language`
    /// selects it.
    pub fn overlay(&self, name: &str, lang: &str) -> Result<Option<FigOverlay>, SpecError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT overlay FROM spec_i18n WHERE name = ?1 AND lang = ?2",
                rusqlite::params![name, lang],
                |r| r.get(0),
            )
            .optional()?;
        match raw {
            None => Ok(None),
            Some(raw) => Ok(serde_json::from_str(&raw).ok()),
        }
    }
}

/// Where the bundled database lives, relative to the executable.
pub fn bundled_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for candidate in [
        dir.join("completions-client.db"),
        // macOS `.app` layout.
        dir.join("../Resources/completions-client.db"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// A translated overlay for one spec.
pub type FigOverlay = BTreeMap<String, String>;

/// One candidate in the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The text that would be inserted.
    pub name: String,
    /// `subcommand`, `option`, `file`, `folder`, or `value`.
    pub kind: CandidateKind,
    pub description: Option<String>,
    /// `true` when the spec marks the entry hidden — shown only on an exact
    /// prefix, never in the browsable list.
    pub hidden: bool,
}

/// What a candidate is, which drives the icon and the completion rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Subcommand,
    Option,
    File,
    Folder,
    Value,
}

impl CandidateKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Subcommand => "subcommand",
            Self::Option => "option",
            Self::File => "file",
            Self::Folder => "folder",
            Self::Value => "value",
        }
    }
}

/// The command line being completed, split the way a shell would split it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    /// The tokens before the one under the cursor.
    pub words: Vec<String>,
    /// The token the cursor is inside, which may be empty.
    pub current: String,
    /// Whether the cursor sits in a word that started with a quote.
    pub quoted: bool,
}

impl Buffer {
    /// Split `line` at `cursor` (a byte offset).
    pub fn parse(line: &str, cursor: usize) -> Self {
        let head = &line[..cursor.min(line.len())];
        let mut words = Vec::new();
        let mut cur = String::new();
        let mut quoted = false;
        let mut chars = head.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\'' | '"' => {
                    quoted = !quoted;
                    cur.push(c);
                }
                c if c.is_whitespace() && !quoted => {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                }
                '\\' => {
                    cur.push(c);
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                }
                c => cur.push(c),
            }
        }
        let current = cur.clone();
        // The word being typed is not a finished word.
        if current.is_empty() && head.ends_with(' ') {
            // Nothing.
        } else if !current.is_empty() {
            // Already excluded from `words` by the loop above.
        }
        Self { words, current, quoted }
    }

    /// The command being completed.
    pub fn command(&self) -> Option<&str> {
        self.words.first().map(String::as_str)
    }

    /// Whether the cursor is on the command itself rather than an argument.
    pub fn on_command(&self) -> bool {
        self.words.is_empty()
    }
}

/// What the panel and the ghost text should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The ranked candidate list.
    pub candidates: Vec<Candidate>,
    /// The faded inline preview, when one candidate clearly leads.
    pub inline: Option<String>,
    /// Why there is no inline preview, for `--debug`.
    pub inline_withheld: Option<InlineWithheld>,
}

/// Why the inline preview was withheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineWithheld {
    /// Several candidates matched and none pulled ahead.
    Ambiguous,
    /// The candidates matched on a prefix but no clear winner.
    NoClearLeader,
    /// Nothing matched.
    Nothing,
    /// `autocomplete-inline-suggestion` is off.
    Disabled,
}

/// How the ranking is told what this user actually runs.
#[derive(Debug, Clone, Default)]
pub struct UsageHints {
    /// Commands run in this directory, most-used first.
    pub local_history: Vec<String>,
    /// Commands run anywhere, most-used first.
    pub global_history: Vec<String>,
    /// The most recent command run in this directory, if any.
    pub last_in_directory: Option<String>,
}

impl UsageHints {
    /// Where `name` sits in the ranking, lower being stronger.
    fn rank(&self, name: &str) -> u32 {
        if let Some(i) = self.local_history.iter().position(|c| c == name) {
            return i as u32;
        }
        if let Some(i) = self.global_history.iter().position(|c| c == name) {
            return 100 + i as u32;
        }
        1000
    }
}

/// Produce the suggestion for `buffer`.
///
/// `spec` is the spec for `buffer.command()`, when one exists. Candidates come
/// from three places, in the order they are trusted: the spec's own entries, the
/// user's history in this directory, and the filesystem for arguments the spec
/// declares as paths.
pub fn suggest(
    buffer: &Buffer,
    spec: Option<&FigSpec>,
    hints: &UsageHints,
    inline_enabled: bool,
) -> Suggestion {
    let mut candidates = Vec::new();
    if let Some(spec) = spec {
        let path = spec.resolve(&buffer.words);
        let needle = buffer.current.as_str();
        for sub in path.subcommands {
            // A hidden entry is offered only once it is what is being typed;
            // an empty prefix is the browsable list, where it does not belong.
            if sub.hidden && (needle.is_empty() || !sub.name.starts_with(needle)) {
                continue;
            }
            if matches_prefix(&sub.name, needle) {
                candidates.push(Candidate {
                    name: sub.name.clone(),
                    kind: CandidateKind::Subcommand,
                    description: sub.description.clone(),
                    hidden: sub.hidden,
                });
            }
        }
        for opt in path.options {
            let names = std::iter::once(&opt.name).chain(opt.aliases.iter());
            for n in names {
                if matches_prefix(n, needle) {
                    candidates.push(Candidate {
                        name: n.to_string(),
                        kind: CandidateKind::Option,
                        description: opt.description.clone(),
                        hidden: opt.hidden,
                    });
                }
            }
        }
        for arg in path.args {
            for s in &arg.suggestions {
                if matches_prefix(s, needle) {
                    candidates.push(Candidate {
                        name: s.clone(),
                        kind: CandidateKind::Value,
                        description: None,
                        hidden: false,
                    });
                }
            }
        }
    }

    // The user's own history is a candidate source only for the command itself.
    if buffer.on_command() {
        for cmd in hints.local_history.iter().chain(hints.global_history.iter()) {
            if matches_prefix(cmd, &buffer.current)
                && !candidates.iter().any(|c| c.name == *cmd)
            {
                candidates.push(Candidate {
                    name: cmd.clone(),
                    kind: CandidateKind::Subcommand,
                    description: None,
                    hidden: false,
                });
            }
        }
    }

    candidates.sort_by(|a, b| {
        let (ra, rb) = (hints.rank(&a.name), hints.rank(&b.name));
        ra.cmp(&rb)
            // An exact match outranks a longer word that merely starts the same.
            .then_with(|| a.name.len().cmp(&b.name.len()))
            .then_with(|| a.name.cmp(&b.name))
    });
    candidates.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

    if !inline_enabled {
        return Suggestion {
            candidates,
            inline: None,
            inline_withheld: Some(InlineWithheld::Disabled),
        };
    }

    let (inline, withheld) = inline_for(buffer, &candidates, hints);
    Suggestion { candidates, inline, inline_withheld: withheld }
}

/// Decide the faded preview.
///
/// A preview is offered in exactly three situations, and withheld everywhere
/// else, because guessing wrong is worse than showing nothing:
///
/// * the command line itself leads this directory — the whole line, not a word;
/// * a single candidate matched, so there is no ambiguity;
/// * the candidates matched on a longer shared prefix than the one typed, which
///   is what "an argument you clearly use most" looks like from here.
///
/// "the first of several" is explicitly *not* a preview.
fn inline_for(
    buffer: &Buffer,
    candidates: &[Candidate],
    hints: &UsageHints,
) -> (Option<String>, Option<InlineWithheld>) {
    if candidates.is_empty() {
        return (None, Some(InlineWithheld::Nothing));
    }

    // A whole command line that leads this folder. This is checked against
    // the *entire* typed line, not just the word under the cursor: the rule the
    // product states is "a whole command line that leads this folder (run most
    // and run last)", and `cargo t` in a directory whose last command was
    // `cargo test --all` should preview that line, not the bare subcommand.
    let typed_line = {
        let mut s = buffer.words.join(" ");
        if !buffer.current.is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(&buffer.current);
        }
        s
    };
    if !typed_line.is_empty() {
        if let Some(last) = &hints.last_in_directory {
            if last.starts_with(&typed_line) && *last != typed_line {
                return (Some(last.clone()), None);
            }
        }
    }

    if candidates.len() == 1 {
        let only = &candidates[0];
        return (Some(only.name.clone()), None);
    }

    // A clear leader: the best candidate carries a strictly longer shared
    // prefix with what was typed than the runner-up does.
    let typed = buffer.current.as_str();
    if typed.is_empty() {
        return (None, Some(InlineWithheld::Ambiguous));
    }
    let shared = |s: &str| -> usize {
        s.chars().zip(typed.chars()).take_while(|(a, b)| a == b).count()
    };
    let best = shared(&candidates[0].name);
    let second = shared(&candidates[1].name);
    if best > second && best >= typed.chars().count() {
        return (Some(candidates[0].name.clone()), None);
    }
    (None, Some(InlineWithheld::NoClearLeader))
}

fn matches_prefix(candidate: &str, typed: &str) -> bool {
    if typed.is_empty() {
        return true;
    }
    candidate.to_ascii_lowercase().starts_with(&typed.to_ascii_lowercase())
}

/// The privacy gates that decide what may be recorded.
#[derive(Debug, Clone)]
pub struct LearningPolicy {
    /// `autocomplete-on-device-learning`.
    pub enabled: bool,
    /// `autocomplete-skip-secrets`.
    pub skip_secrets: bool,
    /// `autocomplete-history-ignore` — glob patterns for commands never recorded.
    pub ignore: Vec<String>,
}

impl Default for LearningPolicy {
    fn default() -> Self {
        Self { enabled: true, skip_secrets: false, ignore: Vec::new() }
    }
}

/// Whether a command may be recorded, and why not when it may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnDecision {
    Record,
    /// `autocomplete-on-device-learning` is off.
    LearningOff,
    /// The command matched an `autocomplete-history-ignore` pattern.
    Ignored,
    /// It looks like it carries a credential.
    LooksSecret,
}

impl LearnDecision {
    pub const fn records(self) -> bool {
        matches!(self, Self::Record)
    }

    /// The sentence the Settings panel shows when asked why nothing is learned.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Record => "",
            Self::LearningOff => "on-device learning is off",
            Self::Ignored => "the command matches an ignore pattern",
            Self::LooksSecret => "the command looks like it carries a secret",
        }
    }
}

/// Decide whether `command` may be recorded under `policy`.
pub fn may_learn(command: &str, policy: &LearningPolicy) -> LearnDecision {
    if !policy.enabled {
        return LearnDecision::LearningOff;
    }
    if policy.ignore.iter().any(|p| glob_matches(p, command)) {
        return LearnDecision::Ignored;
    }
    // `skip-secrets` only guards *new* commands — already-stored ones are
    // untouched, and `otty learn` still keeps one deliberately.
    if policy.skip_secrets && looks_secret(command) {
        return LearnDecision::LooksSecret;
    }
    LearnDecision::Record
}

/// Whether a command line looks like it carries a password, token or key.
///
/// A deliberately conservative test: a false positive silently loses a
/// suggestion, which is cheap, while a false negative writes a credential into a
/// plain-text database, which is not.
pub fn looks_secret(command: &str) -> bool {
    /// Flags that introduce a credential. `-p` is *not* here: it means a port
    /// to `docker`, `ssh` and `scp`, so it needs its own test below.
    const FLAGS: &[&str] = &["--password", "--token", "--api-key", "--apikey", "--secret"];
    const NAMES: &[&str] = &[
        "TOKEN", "SECRET", "PASSWORD", "PASSWD", "API_KEY", "APIKEY", "ACCESS_KEY",
        "PRIVATE_KEY", "AUTH_TOKEN", "CREDENTIAL",
    ];

    // Split the way a shell splits, not the way `split_whitespace` does: in
    // `git commit -m 'fix -p handling'` the `-p` is prose inside one argument,
    // and treating it as a flag would refuse to learn an ordinary commit.
    let tokens: Vec<&str> = split_quoted(command);
    for (i, token) in tokens.iter().enumerate() {
        let lower = token.to_ascii_lowercase();

        if FLAGS.contains(&lower.as_str()) {
            return true;
        }
        if let Some((flag, value)) = token.split_once('=') {
            if FLAGS.contains(&flag.to_ascii_lowercase().as_str()) && !value.is_empty() {
                return true;
            }
        }
        if let Some((name, value)) = token.split_once('=') {
            let upper = name.to_ascii_uppercase();
            if NAMES.iter().any(|n| upper.contains(n)) && value.len() >= 6 {
                return true;
            }
        }

        // `Bearer` appears in an authorisation header and nowhere else. It can
        // arrive as its own word (`-H "Authorization: Bearer abc"`) or glued to
        // the header name (`authorization:Bearer`).
        if lower.contains("bearer") {
            return true;
        }

        // `-p` is overloaded: `mysql -p secret` is a password, `docker run -p
        // 8080:80` is a port. Only the first counts, and only when the value it
        // introduces is neither a port mapping nor a bare port number.
        if lower == "-p" {
            if let Some(next) = tokens.get(i + 1) {
                let port_like = next.contains(':')
                    || next.contains('/')
                    || next.chars().all(|c| c.is_ascii_digit() || c == ':');
                if !port_like && !next.is_empty() {
                    return true;
                }
            }
        }
        // The attached spelling has no such ambiguity: `-pSECRET` is never a port.
        if lower.starts_with("-p") && lower.len() > 2 {
            return true;
        }
    }
    false
}

/// Split a command line the way a shell splits it: whitespace separates words
/// except inside single or double quotes, and a backslash escapes the next
/// character. Quotes are stripped, because the caller compares whole words.
fn split_quoted(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == b'\\' && q == b'"' && i + 1 < bytes.len() {
                    i += 1;
                }
            }
            None => {
                if c == b'\'' || c == b'"' {
                    quote = Some(c);
                    if start.is_none() {
                        start = Some(i);
                    }
                } else if c == b'\\' && i + 1 < bytes.len() {
                    i += 1;
                } else if c.is_ascii_whitespace() {
                    if let Some(s) = start.take() {
                        out.push(trim_quotes(&line[s..i]));
                    }
                } else if start.is_none() {
                    start = Some(i);
                }
            }
        }
        i += 1;
    }
    if let Some(s) = start {
        out.push(trim_quotes(&line[s..]));
    }
    out.retain(|t| !t.is_empty());
    out
}

/// Strip one layer of surrounding quotes, so `'secret value'` compares as
/// `secret value`.
fn trim_quotes(s: &str) -> &str {
    let t = s.trim();
    let b = t.as_bytes();
    if b.len() >= 2 {
        let (a, z) = (b[0], b[b.len() - 1]);
        if (a == b'"' && z == b'"') || (a == b'\'' && z == b'\'') {
            return &t[1..t.len() - 1];
        }
    }
    t
}

/// A glob with `*` spanning anything, used by `autocomplete-history-ignore`.
fn glob_matches(pattern: &str, subject: &str) -> bool {
    let (p, s) = (pattern.to_ascii_lowercase(), subject.to_ascii_lowercase());
    wildcard(p.as_bytes(), s.as_bytes())
}

fn wildcard(pat: &[u8], text: &[u8]) -> bool {
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

/// The key that accepts a suggestion (`autocomplete-shortcut`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcceptKey {
    #[default]
    Tab,
    TabOrRightArrow,
    /// Only → accepts; Tab always goes to the shell's own completion.
    RightArrow,
    CtrlSpace,
    Disable,
}

impl AcceptKey {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "tab" => Self::Tab,
            "tab+right-arrow" | "tab+right_arrow" => Self::TabOrRightArrow,
            "right-arrow" | "right_arrow" => Self::RightArrow,
            "ctrl+space" => Self::CtrlSpace,
            "disable" => Self::Disable,
            _ => return None,
        })
    }
}

/// The key that reveals the panel (`autocomplete-show-candidates`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RevealKey {
    Disable,
    #[default]
    Escape,
    Auto,
    OptionEscape,
}

impl RevealKey {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "disable" => Self::Disable,
            "auto" => Self::Auto,
            "escape" => Self::Escape,
            "option-escape" | "option_escape" => Self::OptionEscape,
            _ => return None,
        })
    }
}

/// The language spec descriptions are shown in.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DescriptionLanguage {
    #[default]
    System,
    English,
    Chinese,
}

impl DescriptionLanguage {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "system" => Self::System,
            "english" => Self::English,
            "chinese" => Self::Chinese,
            _ => return None,
        })
    }

    /// The `spec_i18n` language code to ask for, or `None` to use the spec's own
    /// English text.
    pub fn overlay_lang(&self) -> Option<&'static str> {
        match self {
            // A system language is resolved from the environment; anything that
            // is not Chinese falls back to the untranslated spec.
            Self::System => {
                let lang = std::env::var("LANG").unwrap_or_default().to_ascii_lowercase();
                if lang.starts_with("zh") {
                    Some("zh")
                } else {
                    None
                }
            }
            Self::English => None,
            Self::Chinese => Some("zh"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_json() -> &'static str {
        r#"{
          "name": "cargo",
          "description": "Rust's package manager",
          "subcommands": [
            {"name": "build", "description": "Compile the current package"},
            {"name": "test", "description": "Run the tests"},
            {"name": "bench", "description": "Run benchmarks"},
            {"name": "hidden-thing", "description": "not listed", "hidden": true}
          ],
          "options": [
            {"name": "--release", "description": "Build in release mode"},
            {"name": "-r", "description": "Short for --release"}
          ],
          "args": [{"name": "package", "suggestions": ["core", "cli"]}]
        }"#
    }

    fn cargo_spec() -> FigSpec {
        serde_json::from_str(spec_json()).unwrap()
    }

    #[test]
    fn a_buffer_splits_at_the_cursor_and_keeps_the_word_being_typed() {
        let b = Buffer::parse("cargo bu", 8);
        assert_eq!(b.words, ["cargo"]);
        assert_eq!(b.current, "bu");
        assert_eq!(b.command(), Some("cargo"));
        assert!(!b.on_command());

        let b = Buffer::parse("car", 3);
        assert!(b.words.is_empty());
        assert_eq!(b.current, "car");
        assert!(b.on_command());

        let b = Buffer::parse("cargo ", 6);
        assert_eq!(b.words, ["cargo"]);
        assert_eq!(b.current, "");
    }

    #[test]
    fn a_quoted_word_is_not_split() {
        let line = r#"grep "two words" f"#;
        let b = Buffer::parse(line, line.len());
        assert_eq!(b.words[0], "grep");
        // The quoted phrase is one finished word, not two.
        assert_eq!(b.words[1], "\"two words\"");

        // With the cursor inside the quotes, the word is still open.
        let b = Buffer::parse(line, 12);
        assert!(b.current.starts_with('"'), "{:?}", b);
        assert!(!b.current.ends_with('"'));
    }

    #[test]
    fn subcommands_and_options_both_candidate() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo ", 6);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        let names: Vec<&str> = s.candidates.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"build"));
        assert!(names.contains(&"test"));
        assert!(names.contains(&"--release"));
        assert!(names.contains(&"-r"));
        // A hidden entry is not offered on an empty prefix.
        assert!(!names.contains(&"hidden-thing"));
    }

    #[test]
    fn typing_a_prefix_filters_and_a_hidden_entry_appears_when_asked_for() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo te", 8);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert_eq!(s.candidates.len(), 1);
        assert_eq!(s.candidates[0].name, "test");

        // A hidden entry is shown once it is the thing being typed.
        let b = Buffer::parse("cargo hidden", 12);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert_eq!(s.candidates[0].name, "hidden-thing");
    }

    #[test]
    fn matching_is_case_insensitive() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo BU", 8);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert_eq!(s.candidates[0].name, "build");
    }

    #[test]
    fn the_panel_ranks_what_you_run_first() {
        let spec = cargo_spec();
        let hints = UsageHints {
            local_history: vec!["bench".into()],
            ..Default::default()
        };
        let b = Buffer::parse("cargo ", 6);
        let s = suggest(&b, Some(&spec), &hints, true);
        assert_eq!(s.candidates[0].name, "bench", "{:?}", s.candidates);
    }

    #[test]
    fn the_history_supplies_the_command_word_but_not_arguments() {
        let hints = UsageHints {
            local_history: vec!["git".into(), "grep".into()],
            ..Default::default()
        };
        let b = Buffer::parse("g", 1);
        let s = suggest(&b, None, &hints, true);
        let names: Vec<&str> = s.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["git", "grep"]);

        // On an argument the history has nothing to say.
        let b = Buffer::parse("cargo g", 7);
        let s = suggest(&b, None, &hints, true);
        assert!(s.candidates.is_empty());
    }

    #[test]
    fn the_inline_preview_is_shown_for_a_single_match() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo te", 8);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert_eq!(s.inline.as_deref(), Some("test"));
        assert_eq!(s.inline_withheld, None);
    }

    #[test]
    fn the_inline_preview_is_withheld_when_several_candidates_tie() {
        let spec = cargo_spec();
        // `b` matches `build`, `bench` and `--release`/`-r`? No: `b` matches
        // build and bench, which share only the typed prefix, so neither leads.
        let b = Buffer::parse("cargo b", 7);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert!(s.candidates.len() >= 2);
        assert_eq!(s.inline, None, "{:?}", s.candidates);
        assert_eq!(s.inline_withheld, Some(InlineWithheld::NoClearLeader));
    }

    #[test]
    fn the_inline_preview_follows_a_clear_leader() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo --rel", 11);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert_eq!(s.candidates[0].name, "--release");
        assert_eq!(s.inline.as_deref(), Some("--release"));
    }

    #[test]
    fn the_inline_preview_offers_the_line_that_leads_this_directory() {
        let hints = UsageHints {
            last_in_directory: Some("cargo test --all".into()),
            local_history: vec!["cargo test --all".into()],
            ..Default::default()
        };
        let b = Buffer::parse("cargo t", 7);
        let s = suggest(&b, Some(&cargo_spec()), &hints, true);
        assert_eq!(s.inline.as_deref(), Some("cargo test --all"));
    }

    #[test]
    fn the_inline_preview_is_silent_when_nothing_matches() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo zzz", 9);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), true);
        assert!(s.candidates.is_empty());
        assert_eq!(s.inline, None);
        assert_eq!(s.inline_withheld, Some(InlineWithheld::Nothing));
    }

    #[test]
    fn the_inline_preview_respects_its_own_toggle() {
        let spec = cargo_spec();
        let b = Buffer::parse("cargo te", 8);
        let s = suggest(&b, Some(&spec), &UsageHints::default(), false);
        assert_eq!(s.inline, None);
        assert_eq!(s.inline_withheld, Some(InlineWithheld::Disabled));
        // The panel still works.
        assert_eq!(s.candidates.len(), 1);
    }

    #[test]
    fn learning_is_gated_three_ways() {
        let on = LearningPolicy::default();
        assert_eq!(may_learn("cargo test", &on), LearnDecision::Record);

        let off = LearningPolicy { enabled: false, ..Default::default() };
        assert_eq!(may_learn("cargo test", &off), LearnDecision::LearningOff);

        let ignoring = LearningPolicy {
            ignore: vec!["ssh *".into(), "export *TOKEN*".into()],
            ..Default::default()
        };
        assert_eq!(may_learn("ssh web1", &ignoring), LearnDecision::Ignored);
        assert_eq!(may_learn("mkdir x", &ignoring), LearnDecision::Record);

        let secretive = LearningPolicy { skip_secrets: true, ..Default::default() };
        assert_eq!(
            may_learn("curl -H \"Authorization: Bearer abc\"", &secretive),
            LearnDecision::LooksSecret
        );
        // Off by default, so a token-shaped command is still learned.
        assert_eq!(may_learn("curl --token abcdef", &on), LearnDecision::Record);
    }

    #[test]
    fn secret_detection_catches_the_usual_shapes() {
        for s in [
            "mysql -p secret",
            "curl --password hunter2",
            "curl --token=abcdef",
            "export API_KEY=abcdef123",
            "GITHUB_TOKEN=ghp_0123456789",
            "curl -H authorization:Bearer xyz",
        ] {
            assert!(looks_secret(s), "{s} should look secret");
        }
        for s in [
            "cargo test --release",
            "git commit -m 'fix -p handling'",
            "ls -la",
            "docker run -p 8080:80 nginx",
        ] {
            assert!(!looks_secret(s), "{s} should not look secret");
        }
    }

    #[test]
    fn ignore_patterns_are_globs() {
        assert!(glob_matches("ssh *", "ssh web1"));
        assert!(glob_matches("*TOKEN*", "export GITHUB_TOKEN=x"));
        assert!(!glob_matches("ssh *", "scp web1"));
        assert!(glob_matches("git *", "GIT status"));
    }

    #[test]
    fn the_shortcut_enums_parse_their_documented_values() {
        for v in ["tab", "tab+right-arrow", "right-arrow", "ctrl+space", "disable"] {
            assert!(AcceptKey::parse(v).is_some(), "{v}");
        }
        assert_eq!(AcceptKey::parse("tab+right-arrow"), Some(AcceptKey::TabOrRightArrow));
        assert_eq!(AcceptKey::parse("nope"), None);

        for v in ["disable", "auto", "escape", "option-escape"] {
            assert!(RevealKey::parse(v).is_some(), "{v}");
        }
        assert_eq!(RevealKey::default(), RevealKey::Escape);

        for v in ["system", "english", "chinese"] {
            assert!(DescriptionLanguage::parse(v).is_some(), "{v}");
        }
    }

    #[test]
    fn the_description_language_selects_an_overlay_only_for_chinese() {
        assert_eq!(DescriptionLanguage::English.overlay_lang(), None);
        assert_eq!(DescriptionLanguage::Chinese.overlay_lang(), Some("zh"));
        // `system` depends on the environment, so it must not panic and must
        // return either `None` or the Chinese code.
        let got = DescriptionLanguage::System.overlay_lang();
        assert!(got.is_none() || got == Some("zh"));
    }

    #[test]
    fn a_spec_resolves_into_its_subcommand_tree() {
        let spec = cargo_spec();
        // An exact subcommand word navigates.
        let path = spec.resolve(&["cargo".to_owned(), "test".to_owned()]);
        assert_eq!(path.name, "test");
        // A bare command stays at the root.
        assert_eq!(spec.resolve(&["cargo".to_owned()]).name, "cargo");
    }

    #[test]
    fn the_store_reports_a_schema_that_is_too_new() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE db_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO db_meta VALUES ('schema_version', '99');
             CREATE TABLE specs (name TEXT PRIMARY KEY, spec TEXT NOT NULL,
                                 source TEXT NOT NULL DEFAULT 'upstream', updated_at TEXT);
             CREATE TABLE spec_i18n (name TEXT NOT NULL, lang TEXT NOT NULL,
                                     overlay TEXT NOT NULL, PRIMARY KEY (name, lang));",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        drop(conn);
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE db_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO db_meta VALUES ('schema_version', '99');
                 CREATE TABLE specs (name TEXT PRIMARY KEY, spec TEXT NOT NULL,
                                     source TEXT NOT NULL DEFAULT 'upstream', updated_at TEXT);
                 CREATE TABLE spec_i18n (name TEXT NOT NULL, lang TEXT NOT NULL,
                                         overlay TEXT NOT NULL, PRIMARY KEY (name, lang));",
            )
            .unwrap();
        }
        assert!(matches!(
            SpecStore::open(&path),
            Err(SpecError::SchemaVersion { found: 99, .. })
        ));
    }
}
