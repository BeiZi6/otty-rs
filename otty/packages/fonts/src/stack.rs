//! The type stack: the three roles the product resolves, the family chains they
//! fall down, and the report that says which family actually answered.

use serde::{Deserialize, Serialize};

use crate::catalog::FontCatalog;
use crate::error::FontError;
use crate::faces::FaceStyle;

/// The UI-chrome chain this repository declares, first preference first.
///
/// `Styrene B` then `Styrene A` is the order `docs/TYPOGRAPHY.md` records: B is
/// the text/UI cut, A the display cut, so chrome leans on B and only reaches A
/// before it ever reaches a system font.
pub const STYRENE_UI_STACK: [&str; 2] = ["Styrene B", "Styrene A"];

/// The editorial serif, for prose surfaces (file pane, settings prose).
pub const TIEMPOS_TEXT: &str = "Tiempos Text";

/// The terminal grid's documented default family (`font-family`).
pub const DEFAULT_TERMINAL_FAMILY: &str = "JetBrains Mono";

/// The three typographic roles the product resolves.
///
/// The names here are the CLI spellings (`otty doctor`, `otty font list`), so
/// they are what [`FontRole::parse`] accepts and what a `ResolutionReport`
/// prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontRole {
    /// Titlebar, tab strip, sidebar, Details panel, menu bar, dialogs, Settings.
    #[serde(rename = "ui")]
    UiChrome,
    /// Long-form / editorial prose.
    #[serde(rename = "serif")]
    EditorialSerif,
    /// The terminal grid.
    #[serde(rename = "terminal")]
    TerminalGrid,
}

impl FontRole {
    /// Every role, in the order the Settings panel groups them.
    pub const ALL: [Self; 3] = [Self::UiChrome, Self::EditorialSerif, Self::TerminalGrid];

    /// The CLI spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UiChrome => "ui",
            Self::EditorialSerif => "serif",
            Self::TerminalGrid => "terminal",
        }
    }

    /// Parse the CLI spelling. The long forms are accepted too, so a log line
    /// that spelled one out still reads back.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "ui" | "chrome" | "ui-chrome" => Self::UiChrome,
            "serif" | "editorial" | "editorial-serif" | "text" => Self::EditorialSerif,
            "terminal" | "grid" | "mono" | "terminal-grid" => Self::TerminalGrid,
            _ => return None,
        })
    }
}

impl std::fmt::Display for FontRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A CSS generic family — the last entry of every chain, so a machine without
/// the named fonts still renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenericFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
}

impl GenericFamily {
    /// Every generic, in CSS's order.
    pub const ALL: [Self; 5] =
        [Self::Serif, Self::SansSerif, Self::Monospace, Self::Cursive, Self::Fantasy];

    /// The CSS spelling, which is also what a `ResolutionReport` prints for it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans-serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
        }
    }

    /// Parse a generic. The spaced, unspaced and abbreviated spellings a config
    /// or a theme may carry are accepted; `system-ui` is the platform's UI
    /// family, which is the sans-serif generic here.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "serif" => Self::Serif,
            "sans-serif" | "sans serif" | "sans" | "system-ui" => Self::SansSerif,
            "monospace" | "mono" => Self::Monospace,
            "cursive" => Self::Cursive,
            "fantasy" => Self::Fantasy,
            _ => return None,
        })
    }

    /// The `fontdb` name for this generic.
    pub const fn as_fontdb(self) -> fontdb::Family<'static> {
        match self {
            Self::Serif => fontdb::Family::Serif,
            Self::SansSerif => fontdb::Family::SansSerif,
            Self::Monospace => fontdb::Family::Monospace,
            Self::Cursive => fontdb::Family::Cursive,
            Self::Fantasy => fontdb::Family::Fantasy,
        }
    }
}

/// One entry of a family chain: a named family, or a CSS generic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FamilyRef {
    /// A family name as a theme or the config spells it.
    Named(String),
    /// A CSS generic, resolved by the platform's own cascade.
    Generic(GenericFamily),
}

impl FamilyRef {
    /// A named family.
    pub fn named(name: impl Into<String>) -> Self {
        Self::Named(name.into())
    }

    /// Read one spelling: a generic when it names one, a family name otherwise.
    pub fn parse(spelling: &str) -> Self {
        match GenericFamily::parse(spelling) {
            Some(g) => Self::Generic(g),
            None => Self::Named(spelling.trim().to_owned()),
        }
    }

    /// Read a whole chain, keeping its order and dropping nothing.
    pub fn parse_list<S: AsRef<str>>(spellings: &[S]) -> Vec<Self> {
        spellings.iter().map(|s| Self::parse(s.as_ref())).collect()
    }

    /// How this entry is named in a report: the family, or the generic's CSS
    /// spelling.
    pub fn name(&self) -> &str {
        match self {
            Self::Named(n) => n,
            Self::Generic(g) => g.as_str(),
        }
    }

    /// Whether this entry is a named family rather than a generic.
    pub const fn is_named(&self) -> bool {
        matches!(self, Self::Named(_))
    }

    /// The `fontdb` family to look up.
    pub fn as_fontdb(&self) -> fontdb::Family<'_> {
        match self {
            Self::Named(n) => fontdb::Family::Name(n),
            Self::Generic(g) => g.as_fontdb(),
        }
    }
}

/// The three chains, one per [`FontRole`].
///
/// The chain is data: nothing in this crate looks a family up by hard-coded
/// name at a call site, so the Settings panel, `otty doctor` and a theme can all
/// be answered from the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeStack {
    ui: Vec<FamilyRef>,
    serif: Vec<FamilyRef>,
    terminal: Vec<FamilyRef>,
}

impl Default for TypeStack {
    fn default() -> Self {
        Self::product_default()
    }
}

impl TypeStack {
    /// The stack the product ships, straight out of `docs/TYPOGRAPHY.md`:
    /// `Styrene B → Styrene A → sans-serif`, `Tiempos Text → serif`,
    /// `JetBrains Mono → monospace`.
    pub fn product_default() -> Self {
        Self {
            ui: vec![
                FamilyRef::named(STYRENE_UI_STACK[0]),
                FamilyRef::named(STYRENE_UI_STACK[1]),
                FamilyRef::Generic(GenericFamily::SansSerif),
            ],
            serif: vec![
                FamilyRef::named(TIEMPOS_TEXT),
                FamilyRef::Generic(GenericFamily::Serif),
            ],
            terminal: vec![
                FamilyRef::named(DEFAULT_TERMINAL_FAMILY),
                FamilyRef::Generic(GenericFamily::Monospace),
            ],
        }
    }

    /// The chain for one role.
    pub fn families(&self, role: FontRole) -> &[FamilyRef] {
        match role {
            FontRole::UiChrome => &self.ui,
            FontRole::EditorialSerif => &self.serif,
            FontRole::TerminalGrid => &self.terminal,
        }
    }

    fn families_mut(&mut self, role: FontRole) -> &mut Vec<FamilyRef> {
        match role {
            FontRole::UiChrome => &mut self.ui,
            FontRole::EditorialSerif => &mut self.serif,
            FontRole::TerminalGrid => &mut self.terminal,
        }
    }

    /// Replace one role's chain.
    pub fn set_families(&mut self, role: FontRole, families: Vec<FamilyRef>) {
        *self.families_mut(role) = families;
    }

    /// [`TypeStack::set_families`], chained.
    pub fn with_families(mut self, role: FontRole, families: Vec<FamilyRef>) -> Self {
        self.set_families(role, families);
        self
    }

    /// Replace one role's chain from spellings, as a config key or a theme
    /// writes it: `["JetBrains Mono", "Menlo", "monospace"]`.
    pub fn with_names<S: AsRef<str>>(self, role: FontRole, spellings: &[S]) -> Self {
        self.with_families(role, FamilyRef::parse_list(spellings))
    }

    /// The chain as names, for a message or a saved file.
    pub fn family_names(&self, role: FontRole) -> Vec<String> {
        self.families(role).iter().map(|f| f.name().to_owned()).collect()
    }

    /// A stack read from a theme, with the product default wherever the theme
    /// omits a key.
    ///
    /// `[token].font-ui` and `[token].font-mono` are the two declarations the
    /// theme owns. The config keys that override them — `ui-font-family` and
    /// `font-family` — belong to the caller, which applies them with
    /// [`TypeStack::with_names`].
    pub fn from_theme(theme: &otty_config::theme::Theme) -> Self {
        let mut stack = Self::product_default();
        if let Some(ui) = &theme.token.font_ui {
            stack.set_families(FontRole::UiChrome, FamilyRef::parse_list(ui));
        }
        if let Some(mono) = &theme.token.font_mono {
            stack.set_families(FontRole::TerminalGrid, FamilyRef::parse_list(mono));
        }
        stack
    }
}

/// What one entry of a chain did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    /// A face was found.
    Found(fontdb::ID),
    /// The entry is not installed (or is a generic with nothing behind it).
    NotFound,
}

/// One step of the walk down a chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub family: FamilyRef,
    pub outcome: AttemptOutcome,
}

impl Attempt {
    /// Whether this entry answered.
    pub const fn found(&self) -> bool {
        matches!(self.outcome, AttemptOutcome::Found(_))
    }

    /// The face this entry answered with, when it answered.
    pub const fn face(&self) -> Option<fontdb::ID> {
        match self.outcome {
            AttemptOutcome::Found(id) => Some(id),
            AttemptOutcome::NotFound => None,
        }
    }
}

/// The family that actually answered a role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    /// The face a regular cell of this role uses.
    pub id: fontdb::ID,
    /// The family name (or generic spelling) that answered.
    pub family: String,
    /// Where the file came from.
    pub source: crate::catalog::FontSource,
    /// Position in the chain: 0 means the first preference answered.
    pub index: usize,
}

/// Which family answered, and what had to be missed on the way there.
///
/// Nothing is substituted silently: every chain entry that did not answer is in
/// [`ResolutionReport::missing`], and [`ResolutionReport::lines`] carries the
/// sentences the Settings panel and `otty doctor` print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionReport {
    pub role: FontRole,
    /// Every entry of the chain, as names, in order.
    pub requested: Vec<String>,
    /// The name that answered, or `None` when nothing did.
    pub answered_by: Option<String>,
    /// Chain entries that are not installed, in declaration order.
    pub missing: Vec<String>,
    /// Whether the first preference failed to answer.
    pub substituted: bool,
    /// Human-readable lines, first-most-significant first.
    pub lines: Vec<String>,
}

impl ResolutionReport {
    /// Nothing answered: the role cannot be rendered at all.
    pub fn is_failure(&self) -> bool {
        self.answered_by.is_none()
    }

    /// The first preference is what answered, with nothing missing.
    pub fn is_exact(&self) -> bool {
        !self.substituted && self.missing.is_empty()
    }

    /// The most significant line, or a plain statement that it resolved.
    pub fn summary(&self) -> String {
        if let Some(line) = self.lines.first() {
            return line.clone();
        }
        match &self.answered_by {
            Some(name) => format!("{} resolved by {name}", self.role),
            None => format!("{} did not resolve", self.role),
        }
    }
}

/// The outcome of resolving one role against one stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleResolution {
    pub role: FontRole,
    /// Every step of the walk, up to and including the entry that answered.
    pub attempts: Vec<Attempt>,
    /// The family that answered, if any.
    pub answered: Option<Answered>,
    pub report: ResolutionReport,
}

impl RoleResolution {
    /// The face a regular cell of this role uses.
    pub fn face(&self) -> Option<fontdb::ID> {
        self.answered.as_ref().map(|a| a.id)
    }

    /// Nothing answered.
    pub fn is_failure(&self) -> bool {
        self.report.is_failure()
    }

    /// The answer, or the error a caller that cannot render without one wants.
    ///
    /// A resolution reports rather than fails, because a missing family is
    /// ordinary; this is the point where a caller that has nothing else to fall
    /// back on says so.
    pub fn require(&self) -> Result<&Answered, FontError> {
        self.answered
            .as_ref()
            .ok_or(FontError::RoleUnresolved { role: self.role })
    }
}

impl FontCatalog {
    /// Walk a role's chain and report which family answered.
    ///
    /// The first entry with a face wins, in the order the chain declares. Every
    /// entry that did not answer is recorded — including the ones after the
    /// answer, because a theme that names an uninstalled family has to be
    /// reported even when a later entry saved the render (see
    /// [`crate::ThemeFontResolution::agrees_with_theme`]).
    pub fn resolve_role(&self, stack: &TypeStack, role: FontRole) -> RoleResolution {
        let chain = stack.families(role);
        let mut attempts = Vec::new();
        let mut answered = None;

        for (index, family) in chain.iter().enumerate() {
            let outcome = match self.family_face(family, FaceStyle::Regular) {
                Some(id) => {
                    if answered.is_none() {
                        let entry = self.entry(id);
                        answered = Some(Answered {
                            id,
                            family: family.name().to_owned(),
                            source: entry.map_or(crate::catalog::FontSource::System, |e| e.source),
                            index,
                        });
                    }
                    AttemptOutcome::Found(id)
                }
                None => AttemptOutcome::NotFound,
            };
            attempts.push(Attempt { family: family.clone(), outcome });
            if answered.is_some() {
                break;
            }
        }

        // A named family is "missing" only when the catalog has no face of it at
        // all — a generic is never missing, it is the fallback of last resort.
        let missing: Vec<String> = chain
            .iter()
            .filter(|f| f.is_named() && !self.has_family(f.name()))
            .map(|f| f.name().to_owned())
            .collect();

        let answered_by = answered.as_ref().map(|a| a.family.clone());
        let substituted = answered.as_ref().is_some_and(|a| a.index > 0);
        // The walk stops at the first family that answers, so "falling back to"
        // belongs to the entries the walk stepped *over* — the ones before the
        // answer. An entry after the answer was never asked, and saying the render
        // fell back to the family that in fact answered would name a substitution
        // that did not happen: measured with the shipped bundle answering
        // `JetBrains Mono` first, this reported "Menlo is not installed; falling
        // back to JetBrains Mono".
        let first_after_answer = answered.as_ref().map_or(0, |a| a.index);
        let mut lines = Vec::new();
        match &answered {
            Some(answer) => {
                for (i, name) in missing.iter().enumerate() {
                    let position = chain
                        .iter()
                        .position(|f| f.name() == name)
                        .unwrap_or(usize::MAX);
                    if i == 0 && position < first_after_answer {
                        lines.push(format!(
                            "{name} is not installed; falling back to {}",
                            answer.family
                        ));
                    } else {
                        lines.push(format!("{name} is not installed"));
                    }
                }
                if lines.is_empty() && substituted {
                    lines.push(format!(
                        "{} did not answer; falling back to {}",
                        chain[0].name(),
                        answer.family
                    ));
                }
            }
            None => match missing.first() {
                Some(name) => lines.push(format!("{name} is not installed and no fallback answered")),
                None => lines.push(format!("the {} chain has nothing to resolve", role)),
            },
        }

        let report = ResolutionReport {
            role,
            requested: chain.iter().map(|f| f.name().to_owned()).collect(),
            answered_by,
            missing,
            substituted,
            lines,
        };
        // "Nothing is substituted silently" (docs/TYPOGRAPHY.md) has to hold in the
        // log as well as in the report: a grid that renders in a family nobody
        // named was invisible for exactly as long as this sentence was only
        // computed and dropped. Every resolution that is not exact says so once,
        // at info level, wherever it is called from — the shell, the CLI, or a
        // Settings panel.
        if !report.is_exact() {
            for line in &report.lines {
                log::info!("font role {role}: {line}");
            }
        }
        RoleResolution { role, attempts, answered, report }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{user_catalog, JETBRAINS_MONO, OFFICE_CODE_PRO};

    #[test]
    fn the_product_stack_is_the_documented_one() {
        let stack = TypeStack::product_default();
        assert_eq!(stack.family_names(FontRole::UiChrome), ["Styrene B", "Styrene A", "sans-serif"]);
        assert_eq!(stack.family_names(FontRole::EditorialSerif), ["Tiempos Text", "serif"]);
        assert_eq!(stack.family_names(FontRole::TerminalGrid), ["JetBrains Mono", "monospace"]);
        assert_eq!(TypeStack::default(), stack);
    }

    #[test]
    fn roles_and_generics_round_trip() {
        for role in FontRole::ALL {
            assert_eq!(FontRole::parse(role.as_str()), Some(role));
            assert_eq!(serde_json::to_string(&role).unwrap(), format!("\"{}\"", role.as_str()));
        }
        assert_eq!(FontRole::parse("TERMINAL"), Some(FontRole::TerminalGrid));
        assert_eq!(FontRole::parse("nope"), None);
        for generic in GenericFamily::ALL {
            assert_eq!(GenericFamily::parse(generic.as_str()), Some(generic));
        }
        assert_eq!(GenericFamily::parse("sans serif"), Some(GenericFamily::SansSerif));
        assert_eq!(GenericFamily::parse("system-ui"), Some(GenericFamily::SansSerif));
    }

    #[test]
    fn a_chain_reads_generics_as_generics() {
        let chain = FamilyRef::parse_list(&["JetBrains Mono", "Menlo", "monospace"]);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], FamilyRef::Named("JetBrains Mono".into()));
        assert!(chain[2] == FamilyRef::Generic(GenericFamily::Monospace));
        assert!(!chain[2].is_named());
        assert_eq!(chain[2].name(), "monospace");
        assert_eq!(FamilyRef::parse(" JetBrains Mono ").name(), "JetBrains Mono");

        let stack = TypeStack::product_default().with_names(
            FontRole::TerminalGrid,
            &["Menlo", "monospace"],
        );
        assert_eq!(stack.family_names(FontRole::TerminalGrid), ["Menlo", "monospace"]);
        // The other roles are untouched.
        assert_eq!(stack.family_names(FontRole::UiChrome).len(), 3);
    }

    #[test]
    fn the_first_preference_that_is_installed_answers() {
        let (catalog, _dir) = user_catalog();
        let stack = TypeStack::product_default().with_names(
            FontRole::TerminalGrid,
            &[JETBRAINS_MONO, OFFICE_CODE_PRO, "monospace"],
        );
        let resolved = catalog.resolve_role(&stack, FontRole::TerminalGrid);

        let answer = resolved.require().expect("JetBrains Mono is in the fixture");
        assert_eq!(answer.family, JETBRAINS_MONO);
        assert_eq!(answer.index, 0);
        assert_eq!(answer.source, crate::catalog::FontSource::User);
        assert_eq!(answer.id, catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap());
        assert_eq!(resolved.attempts.len(), 1, "the walk stops at the answer");
        assert_eq!(resolved.attempts[0].face(), Some(answer.id));
        assert!(resolved.report.is_exact());
        assert!(!resolved.report.substituted);
        assert!(resolved.report.missing.is_empty());
        assert!(resolved.report.lines.is_empty());
        assert_eq!(resolved.report.summary(), "terminal resolved by JetBrains Mono");
    }

    /// An entry *after* the answer was never asked, so the report must not claim a
    /// fallback to it — or, as it did before this test, claim a fallback to the
    /// family that in fact answered: with the shipped bundle resolving
    /// `JetBrains Mono` at index 0 and `Menlo` absent later in the chain, the line
    /// read "Menlo is not installed; falling back to JetBrains Mono".
    #[test]
    fn a_family_after_the_answer_is_missing_without_a_fallback_claim() {
        let (catalog, _dir) = user_catalog();
        let stack = TypeStack::product_default().with_names(
            FontRole::TerminalGrid,
            &[JETBRAINS_MONO, "Menlo", "monospace"],
        );
        let resolved = catalog.resolve_role(&stack, FontRole::TerminalGrid);

        assert_eq!(resolved.require().unwrap().family, JETBRAINS_MONO);
        assert!(!resolved.report.substituted, "the first preference answered");
        assert_eq!(resolved.report.missing, ["Menlo"]);
        assert_eq!(resolved.report.lines, ["Menlo is not installed"]);
        assert!(!resolved.report.is_exact(), "a missing entry is still reported");
        assert_eq!(resolved.attempts.len(), 1, "the walk stopped at the answer");
    }

    #[test]
    fn a_missing_family_is_recorded_and_the_next_one_answers() {
        let (catalog, _dir) = user_catalog();
        let stack = TypeStack::product_default().with_names(
            FontRole::UiChrome,
            &[STYRENE_UI_STACK[0], STYRENE_UI_STACK[1], OFFICE_CODE_PRO],
        );
        let resolved = catalog.resolve_role(&stack, FontRole::UiChrome);

        assert!(!resolved.is_failure());
        assert_eq!(resolved.attempts.len(), 3);
        assert!(!resolved.attempts[0].found());
        assert_eq!(resolved.attempts[0].family, FamilyRef::Named("Styrene B".into()));
        assert!(!resolved.attempts[1].found(), "Styrene A is not installed either");
        assert!(resolved.attempts[2].found(), "the third entry answered");
        assert_eq!(resolved.attempts[2].face(), resolved.face());
        assert_eq!(resolved.require().unwrap().family, OFFICE_CODE_PRO);
        assert_eq!(resolved.report.missing, ["Styrene B", "Styrene A"]);
        assert_eq!(
            resolved.report.lines,
            [
                format!("Styrene B is not installed; falling back to {OFFICE_CODE_PRO}"),
                "Styrene A is not installed".to_owned(),
            ]
        );
        assert!(resolved.report.substituted);
        assert!(!resolved.report.is_exact());
    }

    #[test]
    fn a_chain_with_nothing_behind_it_is_a_failure() {
        let (catalog, _dir) = user_catalog();
        let stack = TypeStack::product_default().with_names(
            FontRole::TerminalGrid,
            &["Cascadia Mono", "Menlo"],
        );
        let resolved = catalog.resolve_role(&stack, FontRole::TerminalGrid);

        assert!(resolved.is_failure());
        assert_eq!(resolved.face(), None);
        assert!(resolved.report.summary().contains("Cascadia Mono"));
        assert_eq!(resolved.report.lines, ["Cascadia Mono is not installed and no fallback answered"]);
        let error = resolved.require().unwrap_err();
        assert!(matches!(error, FontError::RoleUnresolved { role: FontRole::TerminalGrid }), "{error}");
        assert_eq!(error.to_string(), "no font answered for the terminal role");

        // And an empty catalog cannot answer anything, generics included.
        let empty = FontCatalog::empty();
        let resolved = empty.resolve_role(&TypeStack::product_default(), FontRole::UiChrome);
        assert!(resolved.is_failure());
    }

    #[test]
    fn a_generic_tail_is_the_last_resort_and_says_so() {
        let (catalog, _dir) = user_catalog();
        let stack = TypeStack::product_default()
            .with_names(FontRole::TerminalGrid, &["Menlo", "monospace"]);
        let resolved = catalog.resolve_role(&stack, FontRole::TerminalGrid);
        let answer = resolved.require().expect("the generic answers");
        assert_eq!(answer.family, "monospace");
        assert_eq!(answer.index, 1, "it answered second");
        assert_eq!(resolved.report.missing, ["Menlo"], "a generic is never missing");
        assert_eq!(
            resolved.report.summary(),
            "Menlo is not installed; falling back to monospace"
        );
        // The face behind the generic is a real one from this catalog.
        assert!(catalog.entry(answer.id).unwrap().is_monospace());
    }
}
