//! Which face a cell of a given style is drawn with, and the four modes the
//! documentation gives for bold and italic.
//!
//! The whole point of the modes is the difference between them, so the decision
//! is what this module returns: [`FaceChoice::Real`] pins a face, and
//! [`FaceChoice::Synthetic`] says "there is no real one — fake it from this
//! base". The renderer does the faking; this layer never rasterises anything.

use serde::{Deserialize, Serialize};

use crate::catalog::FontCatalog;
use crate::stack::FamilyRef;

/// One of the four faces a family is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FaceStyle {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FaceStyle {
    /// Every style, in the order a family is usually enumerated.
    pub const ALL: [Self; 4] = [Self::Regular, Self::Bold, Self::Italic, Self::BoldItalic];

    /// The style a cell with these SGR flags asks for.
    pub const fn from_flags(bold: bool, italic: bool) -> Self {
        match (bold, italic) {
            (true, true) => Self::BoldItalic,
            (true, false) => Self::Bold,
            (false, true) => Self::Italic,
            (false, false) => Self::Regular,
        }
    }

    /// The style a face's declared weight and slope amount to.
    ///
    /// Bold starts at `usWeightClass` 700 ([`fontdb::Weight::BOLD`]): a
    /// `SemiBold` face is a heavier weight of the same family, not its bold cut,
    /// and treating it as bold would silently change what `text-bold = auto`
    /// picks. A designed italic and an oblique both count as the italic face,
    /// which is what a terminal wants.
    pub const fn from_face(weight: u16, italic: bool) -> Self {
        Self::from_flags(weight >= fontdb::Weight::BOLD.0, italic)
    }

    /// The kebab spelling, which is also the string a `FaceChoice` reports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Bold => "bold",
            Self::Italic => "italic",
            Self::BoldItalic => "bold-italic",
        }
    }

    pub const fn is_bold(self) -> bool {
        matches!(self, Self::Bold | Self::BoldItalic)
    }

    pub const fn is_italic(self) -> bool {
        matches!(self, Self::Italic | Self::BoldItalic)
    }

    pub const fn is_regular(self) -> bool {
        matches!(self, Self::Regular)
    }

    /// The weight a lookup for this style asks `fontdb` for.
    pub const fn weight(self) -> fontdb::Weight {
        if self.is_bold() {
            fontdb::Weight::BOLD
        } else {
            fontdb::Weight::NORMAL
        }
    }

    /// The `fontdb` style a lookup for this style asks for.
    pub const fn db_style(self) -> fontdb::Style {
        if self.is_italic() {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        }
    }

    /// The styles that may stand in for this one, best first.
    ///
    /// Only a bold-italic request has a ladder: a family with a real bold face
    /// and a real italic face but no combined face is the ordinary case, and
    /// either half is a better answer than the regular face.
    pub fn stand_ins(self) -> Vec<Self> {
        match self {
            Self::BoldItalic => vec![Self::BoldItalic, Self::Bold, Self::Italic],
            other => vec![other],
        }
    }
}

impl std::fmt::Display for FaceStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `text-bold` / `text-italic`: how a request for a face the family may not ship
/// is honoured.
///
/// The four modes form a ladder of effort, which is what makes them
/// distinguishable in practice:
///
/// | Mode | Eligible families | When no real face exists |
/// |---|---|---|
/// | `off` | none — the request is ignored | the regular face, at its normal weight and slope |
/// | `primary-only` | the role's own chain | the regular face |
/// | `auto` | the role's chain, then `font-family-fallback-*` | the regular face |
/// | `synthetic` | the same as `auto` | the engine fakes it |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextStyleFallback {
    /// Ignore the request entirely: bold renders at normal weight, italic
    /// upright.
    Off,
    /// Use a real face, borrowing from the fallback fonts when the primary
    /// family does not ship one. The documented default.
    #[default]
    Auto,
    /// Use a real face only from the role's own chain; never borrow.
    PrimaryOnly,
    /// Use a real face when one exists, and fake it when none does.
    Synthetic,
}

impl TextStyleFallback {
    /// Every mode, as the settings schema lists them.
    pub const ALL: [Self; 4] = [Self::Off, Self::Auto, Self::PrimaryOnly, Self::Synthetic];

    /// The config spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Auto => "auto",
            Self::PrimaryOnly => "primary-only",
            Self::Synthetic => "synthetic",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "none" => Self::Off,
            "auto" | "true" => Self::Auto,
            "primary-only" | "primary" => Self::PrimaryOnly,
            "synthetic" | "fake" => Self::Synthetic,
            _ => return None,
        })
    }

    /// Whether the `font-family-fallback-*` chains are eligible. This is the
    /// only axis on which [`TextStyleFallback::Auto`] and
    /// [`TextStyleFallback::PrimaryOnly`] differ, and it is the whole point of
    /// having both.
    pub const fn borrows_from_fallbacks(self) -> bool {
        matches!(self, Self::Auto | Self::Synthetic)
    }

    /// Whether the engine may fake a face that does not exist.
    pub const fn may_synthesise(self) -> bool {
        matches!(self, Self::Synthetic)
    }

    /// What the mode does.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Off => "Ignore the request; render at normal weight, upright.",
            Self::Auto => "Use a real face, borrowing from the fallback fonts.",
            Self::PrimaryOnly => "Use a real face only from the primary chain.",
            Self::Synthetic => "Use a real face, and fake it when there is none.",
        }
    }
}

/// Which axes the engine has to fake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Synthesis {
    /// Thicken the strokes (faux bold).
    pub embolden: bool,
    /// Slant the glyphs (faux italic).
    pub slant: bool,
}

impl Synthesis {
    /// The work needed to pass from `have` off as `want`.
    pub const fn between(have: FaceStyle, want: FaceStyle) -> Self {
        Self {
            embolden: want.is_bold() && !have.is_bold(),
            slant: want.is_italic() && !have.is_italic(),
        }
    }

    /// The work needed to pass the regular face off as `want`.
    pub const fn for_style(want: FaceStyle) -> Self {
        Self::between(FaceStyle::Regular, want)
    }

    /// Whether there is anything to fake at all.
    pub const fn is_nothing(self) -> bool {
        !self.embolden && !self.slant
    }
}

/// What the renderer draws a styled cell with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceChoice {
    /// A real face: rasterise it directly.
    Real(fontdb::ID),
    /// No real face exists; fake the missing axes from `base`.
    Synthetic { base: fontdb::ID, synthesis: Synthesis },
}

/// Where the face that answered came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceOrigin {
    /// The role's own chain (`font-family`, `[token].font-mono`, …).
    Chain(String),
    /// The `font-family-fallback-*` chain.
    Fallback(String),
    /// `font-family-bold` / `font-family-italic` / `font-family-bold-italic`.
    Override(String),
}

impl FaceOrigin {
    /// The family that answered.
    pub fn family(&self) -> &str {
        match self {
            Self::Chain(f) | Self::Fallback(f) | Self::Override(f) => f,
        }
    }

    /// A short word for the origin, for a log line or a `otty doctor` report.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Chain(_) => "the family chain",
            Self::Fallback(_) => "the fallback chain",
            Self::Override(_) => "an explicit override",
        }
    }
}

/// What one style request resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleResolution {
    pub requested: FaceStyle,
    pub mode: TextStyleFallback,
    pub origin: FaceOrigin,
    pub choice: FaceChoice,
    /// Anything worth telling the user: a substituted near face, an ignored
    /// request, a fake.
    pub notes: Vec<String>,
}

impl StyleResolution {
    /// The face a cell is drawn with — the real one, or the base a fake is built
    /// from.
    pub const fn face(&self) -> fontdb::ID {
        match self.choice {
            FaceChoice::Real(id) => id,
            FaceChoice::Synthetic { base, .. } => base,
        }
    }

    /// Whether the engine has to fake anything.
    pub const fn is_synthetic(&self) -> bool {
        matches!(self.choice, FaceChoice::Synthetic { .. })
    }

    /// One line, in the shape `otty doctor` prints.
    pub fn explanation(&self) -> String {
        if let Some(note) = self.notes.first() {
            return note.clone();
        }
        match self.choice {
            FaceChoice::Real(_) => format!(
                "{} cells use the real {} face from {}",
                self.requested,
                self.requested,
                self.origin.kind()
            ),
            FaceChoice::Synthetic { synthesis, .. } => format!(
                "{} cells are faked ({} {}) from the {} face",
                self.requested,
                if synthesis.embolden { "emboldened" } else { "as-is" },
                if synthesis.slant { "slanted" } else { "upright" },
                self.origin.family()
            ),
        }
    }
}

/// One request for a face: the chains to search, the style, and the mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceRequest<'a> {
    /// The role's own chain, in preference order.
    pub families: &'a [FamilyRef],
    /// The `font-family-fallback-*` chain for this style.
    pub fallbacks: &'a [FamilyRef],
    pub style: FaceStyle,
    pub mode: TextStyleFallback,
    /// `font-family-bold`, `font-family-italic` or `font-family-bold-italic`.
    pub override_family: Option<&'a str>,
}

impl<'a> FaceRequest<'a> {
    /// A request for `style` from one chain, with the documented default mode.
    pub const fn new(families: &'a [FamilyRef], style: FaceStyle) -> Self {
        Self { families, fallbacks: &[], style, mode: TextStyleFallback::Auto, override_family: None }
    }

    /// Set the mode.
    pub const fn with_mode(mut self, mode: TextStyleFallback) -> Self {
        self.mode = mode;
        self
    }

    /// Set the per-style fallback chain.
    pub const fn with_fallbacks(mut self, fallbacks: &'a [FamilyRef]) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// Set the per-style family override.
    pub const fn with_override_family(mut self, family: Option<&'a str>) -> Self {
        self.override_family = family;
        self
    }
}

/// One entry of the search order, tagged with the list it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    family: FamilyRef,
    origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Chain,
    Fallback,
    Override,
}

impl FontCatalog {
    /// Decide what a styled cell is drawn with.
    ///
    /// The ladder, in order, and every step of it is recorded in the returned
    /// [`StyleResolution::notes`]:
    ///
    /// 1. `off` ignores the request outright.
    /// 2. A real face of exactly the requested style, from the first eligible
    ///    entry that ships one. Eligibility is the chain, plus the fallback
    ///    chain when the mode borrows, plus an explicit override family in every
    ///    mode — an override is an instruction, not a fallback.
    /// 3. For a bold-italic request only, a *near* face: the bold face, then the
    ///    italic face. A family that ships the exact face wins over one that
    ///    ships a near face, which is what "borrowing from your fallback fonts
    ///    if the primary family doesn't ship one" means for a combined style.
    /// 4. `synthetic` fakes whatever is still missing, on the nearest base it
    ///    found; every other mode renders the regular face.
    ///
    /// Returns `None` only when the catalog cannot produce even a regular face
    /// for the request — a machine with no fonts at all.
    pub fn resolve_style(&self, request: &FaceRequest<'_>) -> Option<StyleResolution> {
        // The base is the one hard requirement: something has to be able to draw
        // a plain cell. The regular search always considers the whole chain,
        // whatever the mode, because a mode decides about *bold* and *italic*,
        // never about whether text renders at all.
        let base = self
            .candidates(request, false, true)
            .into_iter()
            .find_map(|c| self.family_face(&c.family, FaceStyle::Regular).map(|id| (id, c)))?;
        let (base_id, base_candidate) = base;
        let base_origin = base_candidate.origin_of();

        if request.style.is_regular() {
            return Some(StyleResolution {
                requested: request.style,
                mode: request.mode,
                origin: base_origin,
                choice: FaceChoice::Real(base_id),
                notes: Vec::new(),
            });
        }

        if request.mode == TextStyleFallback::Off {
            return Some(StyleResolution {
                requested: request.style,
                mode: request.mode,
                origin: base_origin,
                choice: FaceChoice::Real(base_id),
                notes: vec![format!(
                    "{} = off: {} cells render with the regular face `{}`",
                    request.mode.as_str(),
                    request.style,
                    base_candidate.family.name()
                )],
            });
        }

        let listed = self.candidates(request, true, request.mode.borrows_from_fallbacks());
        let mut near = None;
        for want in request.style.stand_ins() {
            for candidate in &listed {
                if let Some(id) = self.family_face(&candidate.family, want) {
                    if want == request.style {
                        return Some(StyleResolution {
                            requested: request.style,
                            mode: request.mode,
                            origin: candidate.origin_of(),
                            choice: FaceChoice::Real(id),
                            notes: Vec::new(),
                        });
                    }
                    near = Some((id, want, candidate.clone()));
                    break;
                }
            }
            if near.is_some() {
                break;
            }
        }

        if let Some((id, have, candidate)) = near {
            let origin = candidate.origin_of();
            if request.mode.may_synthesise() {
                return Some(StyleResolution {
                    requested: request.style,
                    mode: request.mode,
                    origin,
                    choice: FaceChoice::Synthetic {
                        base: id,
                        synthesis: Synthesis::between(have, request.style),
                    },
                    notes: vec![format!(
                        "`{}` ships no {} face; faking it from its {} face",
                        candidate.family.name(),
                        request.style,
                        have
                    )],
                });
            }
            return Some(StyleResolution {
                requested: request.style,
                mode: request.mode,
                origin,
                choice: FaceChoice::Real(id),
                notes: vec![format!(
                    "`{}` ships no {} face; using its {} face",
                    candidate.family.name(),
                    request.style,
                    have
                )],
            });
        }

        if request.mode.may_synthesise() {
            return Some(StyleResolution {
                requested: request.style,
                mode: request.mode,
                origin: base_origin,
                choice: FaceChoice::Synthetic {
                    base: base_id,
                    synthesis: Synthesis::for_style(request.style),
                },
                notes: vec![format!(
                    "no {} face in the chain; faking it from the regular face `{}`",
                    request.style,
                    base_candidate.family.name()
                )],
            });
        }

        Some(StyleResolution {
            requested: request.style,
            mode: request.mode,
            origin: base_origin,
            choice: FaceChoice::Real(base_id),
            notes: vec![format!(
                "no {} face in the chain; rendering at the regular weight and slope",
                request.style
            )],
        })
    }

    /// The ordered search list for one request.
    fn candidates(
        &self,
        request: &FaceRequest<'_>,
        include_override: bool,
        include_fallbacks: bool,
    ) -> Vec<Candidate> {
        let mut out = Vec::with_capacity(
            request.families.len() + request.fallbacks.len() + 1,
        );
        if include_override {
            if let Some(name) = request.override_family {
                if !name.trim().is_empty() {
                    out.push(Candidate {
                        family: FamilyRef::Named(name.trim().to_owned()),
                        origin: Origin::Override,
                    });
                }
            }
        }
        out.extend(request.families.iter().cloned().map(|family| Candidate {
            family,
            origin: Origin::Chain,
        }));
        if include_fallbacks {
            out.extend(request.fallbacks.iter().cloned().map(|family| Candidate {
                family,
                origin: Origin::Fallback,
            }));
        }
        out
    }
}

impl Candidate {
    fn origin_of(&self) -> FaceOrigin {
        let name = self.family.name().to_owned();
        match self.origin {
            Origin::Chain => FaceOrigin::Chain(name),
            Origin::Fallback => FaceOrigin::Fallback(name),
            Origin::Override => FaceOrigin::Override(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{user_catalog, OFFICE_CODE_PRO, JETBRAINS_MONO};

    fn chain(names: &[&str]) -> Vec<FamilyRef> {
        FamilyRef::parse_list(names)
    }

    #[test]
    fn styles_map_from_flags_and_from_faces() {
        assert_eq!(FaceStyle::from_flags(false, false), FaceStyle::Regular);
        assert_eq!(FaceStyle::from_flags(true, true), FaceStyle::BoldItalic);
        assert_eq!(FaceStyle::from_flags(false, true), FaceStyle::Italic);
        assert_eq!(FaceStyle::from_face(400, false), FaceStyle::Regular);
        assert_eq!(FaceStyle::from_face(700, false), FaceStyle::Bold);
        assert_eq!(FaceStyle::from_face(800, true), FaceStyle::BoldItalic);
        // A SemiBold face is not the bold cut.
        assert_eq!(FaceStyle::from_face(600, false), FaceStyle::Regular);
        assert_eq!(FaceStyle::Bold.weight(), fontdb::Weight::BOLD);
        assert_eq!(FaceStyle::Italic.db_style(), fontdb::Style::Italic);
        assert_eq!(FaceStyle::Regular.stand_ins(), vec![FaceStyle::Regular]);
        assert_eq!(
            FaceStyle::BoldItalic.stand_ins(),
            vec![FaceStyle::BoldItalic, FaceStyle::Bold, FaceStyle::Italic]
        );
    }

    #[test]
    fn modes_parse_and_round_trip() {
        for mode in TextStyleFallback::ALL {
            assert_eq!(TextStyleFallback::parse(mode.as_str()), Some(mode));
            assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{}\"", mode.as_str()));
        }
        assert_eq!(TextStyleFallback::default(), TextStyleFallback::Auto);
        assert_eq!(TextStyleFallback::parse("Primary-Only"), Some(TextStyleFallback::PrimaryOnly));
        assert_eq!(TextStyleFallback::parse("nope"), None);
        assert!(TextStyleFallback::Auto.borrows_from_fallbacks());
        assert!(!TextStyleFallback::PrimaryOnly.borrows_from_fallbacks());
        assert!(TextStyleFallback::Synthetic.may_synthesise());
        assert!(!TextStyleFallback::Auto.may_synthesise());
    }

    #[test]
    fn off_ignores_the_request_entirely() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[OFFICE_CODE_PRO]);
        let regular = catalog.find_face(OFFICE_CODE_PRO, FaceStyle::Regular).unwrap();
        let resolved = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_mode(TextStyleFallback::Off),
            )
            .expect("a base face");
        assert_eq!(resolved.choice, FaceChoice::Real(regular));
        assert!(!resolved.is_synthetic());
        assert!(resolved.notes[0].contains("off"), "{:?}", resolved.notes);
    }

    #[test]
    fn primary_only_never_borrows_and_auto_does() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO]);
        let fallbacks = chain(&[OFFICE_CODE_PRO]);
        let jbm_regular = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let ocp_bold = catalog.find_face(OFFICE_CODE_PRO, FaceStyle::Bold).unwrap();

        let primary = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_fallbacks(&fallbacks)
                    .with_mode(TextStyleFallback::PrimaryOnly),
            )
            .expect("a base face");
        assert_eq!(primary.choice, FaceChoice::Real(jbm_regular));
        assert_eq!(primary.origin, FaceOrigin::Chain(JETBRAINS_MONO.to_owned()));

        let auto = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_fallbacks(&fallbacks)
                    .with_mode(TextStyleFallback::Auto),
            )
            .expect("a base face");
        assert_eq!(auto.choice, FaceChoice::Real(ocp_bold));
        assert_eq!(auto.origin, FaceOrigin::Fallback(OFFICE_CODE_PRO.to_owned()));
    }

    #[test]
    fn a_real_face_wins_over_faking_it() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO]);
        let fallbacks = chain(&[OFFICE_CODE_PRO]);
        let ocp_bold = catalog.find_face(OFFICE_CODE_PRO, FaceStyle::Bold).unwrap();
        let resolved = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_fallbacks(&fallbacks)
                    .with_mode(TextStyleFallback::Synthetic),
            )
            .unwrap();
        assert_eq!(resolved.choice, FaceChoice::Real(ocp_bold));
        assert!(!resolved.is_synthetic());
    }

    #[test]
    fn synthetic_fakes_it_when_nothing_real_exists() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO]);
        let jbm_regular = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let resolved = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_mode(TextStyleFallback::Synthetic),
            )
            .unwrap();
        assert_eq!(
            resolved.choice,
            FaceChoice::Synthetic {
                base: jbm_regular,
                synthesis: Synthesis { embolden: true, slant: false }
            }
        );
        assert!(resolved.is_synthetic());
        assert_eq!(resolved.face(), jbm_regular);
        assert!(resolved.explanation().contains("faking"), "{}", resolved.explanation());
    }

    #[test]
    fn auto_renders_plain_when_there_is_no_real_face_to_use() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO]);
        let jbm_regular = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let jbm_italic = catalog.find_face(JETBRAINS_MONO, FaceStyle::Italic).unwrap();

        // Nothing in the chain ships a bold face, and auto never fakes one.
        let bold = catalog.resolve_style(&FaceRequest::new(&families, FaceStyle::Bold)).unwrap();
        assert_eq!(bold.choice, FaceChoice::Real(jbm_regular));
        assert!(!bold.is_synthetic());
        assert!(bold.notes[0].contains("no bold face"), "{:?}", bold.notes);

        // The italic face does exist, so the request answers with it and needs
        // no note at all.
        let italic = catalog
            .resolve_style(&FaceRequest::new(&families, FaceStyle::Italic))
            .unwrap();
        assert_eq!(italic.choice, FaceChoice::Real(jbm_italic));
        assert!(italic.notes.is_empty(), "{:?}", italic.notes);
        assert!(italic.explanation().contains("real italic face"), "{}", italic.explanation());
    }

    #[test]
    fn a_near_face_beats_the_regular_one_and_is_recorded() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO]);
        let jbm_italic = catalog.find_face(JETBRAINS_MONO, FaceStyle::Italic).unwrap();

        // JetBrains Mono ships an italic and no bold, so the italic is the
        // nearest face to a bold-italic request.
        let auto = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::BoldItalic)
                    .with_mode(TextStyleFallback::Auto),
            )
            .unwrap();
        assert_eq!(auto.choice, FaceChoice::Real(jbm_italic));
        assert_eq!(auto.origin, FaceOrigin::Chain(JETBRAINS_MONO.to_owned()));

        let synthetic = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::BoldItalic)
                    .with_mode(TextStyleFallback::Synthetic),
            )
            .unwrap();
        assert_eq!(
            synthetic.choice,
            FaceChoice::Synthetic {
                base: jbm_italic,
                // The italic half is already there; only the weight is faked.
                synthesis: Synthesis { embolden: true, slant: false }
            }
        );
    }

    #[test]
    fn an_exact_face_in_a_later_family_beats_a_near_face_in_an_earlier_one() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO, OFFICE_CODE_PRO]);
        let ocp_bold_italic = catalog.find_face(OFFICE_CODE_PRO, FaceStyle::BoldItalic).unwrap();
        let resolved = catalog
            .resolve_style(&FaceRequest::new(&families, FaceStyle::BoldItalic))
            .unwrap();
        assert_eq!(resolved.choice, FaceChoice::Real(ocp_bold_italic));
        assert_eq!(resolved.origin, FaceOrigin::Chain(OFFICE_CODE_PRO.to_owned()));
    }

    #[test]
    fn an_override_family_is_an_instruction_in_every_mode_but_off() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[JETBRAINS_MONO]);
        let ocp_bold = catalog.find_face(OFFICE_CODE_PRO, FaceStyle::Bold).unwrap();

        for mode in [
            TextStyleFallback::Auto,
            TextStyleFallback::PrimaryOnly,
            TextStyleFallback::Synthetic,
        ] {
            let resolved = catalog
                .resolve_style(
                    &FaceRequest::new(&families, FaceStyle::Bold)
                        .with_mode(mode)
                        .with_override_family(Some("Office Code Pro")),
                )
                .unwrap();
            assert_eq!(resolved.choice, FaceChoice::Real(ocp_bold), "{mode:?}");
            assert_eq!(resolved.origin, FaceOrigin::Override(OFFICE_CODE_PRO.to_owned()));
        }

        let jbm_regular = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let off = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_mode(TextStyleFallback::Off)
                    .with_override_family(Some("Office Code Pro")),
            )
            .unwrap();
        assert_eq!(off.choice, FaceChoice::Real(jbm_regular));
    }

    #[test]
    fn a_blank_override_is_ignored() {
        let (catalog, _dir) = user_catalog();
        let families = chain(&[OFFICE_CODE_PRO]);
        let ocp_bold = catalog.find_face(OFFICE_CODE_PRO, FaceStyle::Bold).unwrap();
        let resolved = catalog
            .resolve_style(
                &FaceRequest::new(&families, FaceStyle::Bold)
                    .with_override_family(Some("   ")),
            )
            .unwrap();
        assert_eq!(resolved.choice, FaceChoice::Real(ocp_bold));
        assert_eq!(resolved.origin, FaceOrigin::Chain(OFFICE_CODE_PRO.to_owned()));
    }

    #[test]
    fn an_empty_catalog_resolves_nothing() {
        let catalog = FontCatalog::empty();
        let families = chain(&[JETBRAINS_MONO]);
        assert!(catalog
            .resolve_style(&FaceRequest::new(&families, FaceStyle::Regular))
            .is_none());
        assert!(catalog
            .resolve_style(&FaceRequest::new(&families, FaceStyle::Bold))
            .is_none());
    }

    #[test]
    fn synthesis_describes_the_missing_axes() {
        assert_eq!(
            Synthesis::for_style(FaceStyle::BoldItalic),
            Synthesis { embolden: true, slant: true }
        );
        assert_eq!(
            Synthesis::between(FaceStyle::Italic, FaceStyle::BoldItalic),
            Synthesis { embolden: true, slant: false }
        );
        assert_eq!(
            Synthesis::between(FaceStyle::Bold, FaceStyle::BoldItalic),
            Synthesis { embolden: false, slant: true }
        );
        assert!(Synthesis::for_style(FaceStyle::Regular).is_nothing());
    }
}
