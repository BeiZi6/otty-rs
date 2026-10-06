//! Ligatures: the level, the letter/digit suppression rule, and the shaping that
//! implements it.
//!
//! The rule is the part that has to be exact, so it is a pure function over a
//! run of characters — [`LigatureSettings::boundary_allowed`] — and shaping is
//! driven by it: a run is split at the boundaries the rule forbids, and each
//! piece is shaped on its own, so a `calt` table can never join characters the
//! user asked to keep apart.

use std::ops::Range;

use serde::{Deserialize, Serialize};
use swash::Setting;

use crate::catalog::FontCatalog;
use crate::error::FontError;

/// `font-ligatures` (alias `ligatures`): the OpenType ligature level.
///
/// Note on the default: the Settings page describes `off` as the default, while
/// `spec/config-schema.json` documents `dlig` as the key's default. The schema is
/// the machine-readable specification, so it is the one this enum follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LigatureLevel {
    /// No ligatures at all.
    Off,
    /// The standard set plus contextual alternates — `=>`, `!=`, `>=`.
    Calt,
    /// Everything in `calt`, plus discretionary ligatures — the schema's
    /// documented default for the key.
    #[default]
    Dlig,
}

impl LigatureLevel {
    /// Every level, weakest first.
    pub const ALL: [Self; 3] = [Self::Off, Self::Calt, Self::Dlig];

    /// The config spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Calt => "calt",
            Self::Dlig => "dlig",
        }
    }

    /// Parse a level. `none` is accepted because the terminal-features page
    /// spells the disabled level that way.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "false" => Self::Off,
            "calt" => Self::Calt,
            "dlig" | "true" => Self::Dlig,
            _ => return None,
        })
    }

    /// What the level does, as the Settings panel words it.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Off => "No ligatures; glyphs render individually.",
            Self::Calt => "Standard ligatures and contextual alternates.",
            Self::Dlig => "Contextual alternates plus discretionary ligatures.",
        }
    }

    /// Whether the level permits any ligature at all.
    pub const fn permits_ligatures(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// The OpenType feature settings this level asks the shaper for.
    ///
    /// `liga` and `clig` are the standard and contextual sets, `calt` is the
    /// contextual-alternates feature programming fonts put `=>` and `!=` in, and
    /// `dlig` is the discretionary set. An explicit `0` is what the level uses to
    /// turn one off, because a face may enable a feature by default.
    pub fn features(self) -> [Setting<u16>; 4] {
        let (liga, clig, calt, dlig) = match self {
            Self::Off => (0, 0, 0, 0),
            Self::Calt => (1, 1, 1, 0),
            Self::Dlig => (1, 1, 1, 1),
        };
        [
            feature("liga", liga),
            feature("clig", clig),
            feature("calt", calt),
            feature("dlig", dlig),
        ]
    }
}

fn feature(name: &str, value: u16) -> Setting<u16> {
    Setting { tag: swash::tag_from_str_lossy(name), value }
}

/// Whether a character counts as "alphabet" for the suppression rule.
///
/// Letters, digits and CJK ideographs — everything Unicode classifies as
/// alphanumeric, which is the "runs of letters/digits/CJK" the schema describes.
pub fn is_alphabet(c: char) -> bool {
    c.is_alphanumeric()
}

/// `font-ligatures` together with `font-ligatures-alphabet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LigatureSettings {
    pub level: LigatureLevel,
    /// `font-ligatures-alphabet`: also form ligatures inside runs of
    /// letters/digits/CJK. Off by default, which keeps `fi`/`fl` from collapsing
    /// cells.
    pub alphabet: bool,
}

impl LigatureSettings {
    /// A level and the alphabet setting.
    pub const fn new(level: LigatureLevel, alphabet: bool) -> Self {
        Self { level, alphabet }
    }

    /// No ligatures.
    pub const fn off() -> Self {
        Self::new(LigatureLevel::Off, false)
    }

    /// The level's OpenType features.
    pub fn features(self) -> [Setting<u16>; 4] {
        self.level.features()
    }

    /// Whether a ligature may form across the boundary between `run[boundary]`
    /// and `run[boundary + 1]`.
    ///
    /// By default ligatures fire only on symbol runs, so `=>` and `!=` join while
    /// `fi` does not. A boundary is permitted when:
    ///
    /// * the level permits ligatures at all, **and**
    /// * the boundary exists and neither side is blank or a control character,
    ///   **and**
    /// * `font-ligatures-alphabet` is set, or at least one side is a symbol.
    ///
    /// The last clause is the documented rule read per boundary: a boundary
    /// inside a run of letters and digits is suppressed, a boundary that touches
    /// a symbol is left to the font.
    pub fn boundary_allowed(self, run: &[char], boundary: usize) -> bool {
        if !self.level.permits_ligatures() {
            return false;
        }
        if boundary + 1 >= run.len() {
            return false;
        }
        let left = run[boundary];
        let right = run[boundary + 1];
        if left.is_whitespace() || right.is_whitespace() || left.is_control() || right.is_control() {
            return false;
        }
        self.alphabet || !is_alphabet(left) || !is_alphabet(right)
    }

    /// Whether any ligature may form inside this run at all.
    pub fn run_allowed(self, run: &[char]) -> bool {
        (0..run.len().saturating_sub(1)).any(|b| self.boundary_allowed(run, b))
    }

    /// The substrings to shape separately, as byte ranges into `text`.
    ///
    /// A run is cut at every boundary the rule forbids, because `calt` and
    /// `dlig` are contextual: the only way to keep a table from joining `fi` is
    /// to keep the `f` and the `i` out of one shaping run.
    ///
    /// With the level off the whole run stays one piece: the features are
    /// disabled, so nothing can ligate anyway, and splitting would only break
    /// clustering for marks with nothing to show for it.
    pub fn shaping_pieces(self, text: &str) -> Vec<Range<usize>> {
        let bounds = byte_bounds(text);
        if text.is_empty() {
            return Vec::new();
        }
        if !self.level.permits_ligatures() {
            // Nothing can ligate once the features are off, so the run stays
            // whole: cutting it up would only break mark clustering.
            return std::iter::once(0..text.len()).collect();
        }
        let chars: Vec<char> = text.chars().collect();
        let mut pieces = Vec::new();
        let mut start = 0usize;
        for boundary in 0..chars.len().saturating_sub(1) {
            if !self.boundary_allowed(&chars, boundary) {
                pieces.push(bounds[start]..bounds[boundary + 1]);
                start = boundary + 1;
            }
        }
        pieces.push(bounds[start]..text.len());
        pieces
    }

    /// Whether the text becomes more than one shaping piece.
    pub fn splits(self, text: &str) -> bool {
        self.shaping_pieces(text).len() > 1
    }
}

/// The byte offset of every character, plus the end of the string.
fn byte_bounds(text: &str) -> Vec<usize> {
    let mut bounds: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    bounds.push(text.len());
    bounds
}

/// One glyph out of the shaper.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ShapedGlyph {
    pub id: u16,
    pub advance: f32,
    pub x: f32,
    pub y: f32,
    /// Whether the shaper marked this glyph as a mark attached to the previous
    /// one — a combining accent, which is drawn but does not advance the cell.
    pub mark: bool,
}

/// One shaped cluster: the glyphs, and the source text they came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedCluster {
    /// Byte range of the source text this cluster covers.
    pub source: Range<usize>,
    pub glyphs: Vec<ShapedGlyph>,
    /// The source range of each ligature component; empty unless the cluster is
    /// a ligature.
    pub components: Vec<Range<usize>>,
}

impl Default for ShapedCluster {
    fn default() -> Self {
        Self { source: 0..0, glyphs: Vec::new(), components: Vec::new() }
    }
}

impl ShapedCluster {
    /// Whether the shaper joined several source clusters into this one — which
    /// is exactly what a ligature is.
    pub fn is_ligature(&self) -> bool {
        !self.components.is_empty()
    }

    /// Whether this cluster drew nothing (a control character).
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// The cluster's advance.
    pub fn advance(&self) -> f32 {
        self.glyphs.iter().map(|g| g.advance).sum()
    }
}

/// The result of shaping one run of text with one face.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShapedRun {
    /// Every glyph, in drawing order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Every cluster, in source order.
    pub clusters: Vec<ShapedCluster>,
    /// The run's total advance.
    pub advance: f32,
}

impl ShapedRun {
    /// How many glyphs the run draws.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// How many clusters the shaper joined from more than one source cluster.
    pub fn ligature_count(&self) -> usize {
        self.clusters.iter().filter(|c| c.is_ligature()).count()
    }

    /// Whether no ligature formed anywhere in the run.
    pub fn is_ligature_free(&self) -> bool {
        self.ligature_count() == 0
    }
}

impl FontCatalog {
    /// Shape a run of text with one face, one size and one ligature setting.
    ///
    /// The whole run is shaped as a single piece, so the font's contextual tables
    /// see it whole. Use [`FontCatalog::shape_ruled`] when the letter/digit
    /// suppression rule has to be applied.
    pub fn shape(
        &self,
        id: fontdb::ID,
        text: &str,
        size: f32,
        settings: LigatureSettings,
    ) -> Result<ShapedRun, FontError> {
        let features = settings.features();
        self.shape_piece(id, text, size, &features)
    }

    /// Shape a run with the letter/digit rule applied.
    ///
    /// The text is cut into the pieces [`LigatureSettings::shaping_pieces`]
    /// returns and each is shaped on its own, so a ligature can only form across
    /// a boundary the rule allowed. Cluster source ranges are reported against
    /// the whole `text`, not against the piece.
    pub fn shape_ruled(
        &self,
        id: fontdb::ID,
        text: &str,
        size: f32,
        settings: LigatureSettings,
    ) -> Result<ShapedRun, FontError> {
        let features = settings.features();
        let mut run = ShapedRun::default();
        for piece in settings.shaping_pieces(text) {
            let slice = text.get(piece.clone()).unwrap_or_default();
            let mut shaped = self.shape_piece(id, slice, size, &features)?;
            for cluster in &mut shaped.clusters {
                cluster.source =
                    cluster.source.start + piece.start..cluster.source.end + piece.start;
                for component in &mut cluster.components {
                    *component = component.start + piece.start..component.end + piece.start;
                }
            }
            run.advance += shaped.advance;
            run.glyphs.extend(shaped.glyphs);
            run.clusters.extend(shaped.clusters);
        }
        Ok(run)
    }

    /// Shape one piece of text with explicit OpenType feature settings.
    fn shape_piece(
        &self,
        id: fontdb::ID,
        text: &str,
        size: f32,
        features: &[Setting<u16>],
    ) -> Result<ShapedRun, FontError> {
        let entry = self.entry(id).ok_or(FontError::UnknownFace { id })?;
        let run = self
            .database()
            .with_face_data(id, |data, index| {
                let font = swash::FontRef::from_index(data, index as usize)?;
                let mut context = swash::shape::ShapeContext::new();
                let mut shaper = context
                    .builder(font)
                    .size(size)
                    .features(features.iter().copied())
                    .build();
                shaper.add_str(text);
                let mut run = ShapedRun::default();
                shaper.shape_with(|cluster| {
                    let glyphs: Vec<ShapedGlyph> = cluster
                        .glyphs
                        .iter()
                        .map(|g| ShapedGlyph {
                            id: g.id,
                            advance: g.advance,
                            x: g.x,
                            y: g.y,
                            mark: g.info.is_mark(),
                        })
                        .collect();
                    run.advance += cluster.advance();
                    run.glyphs.extend(glyphs.iter().copied());
                    run.clusters.push(ShapedCluster {
                        source: cluster.source.to_range(),
                        glyphs,
                        components: cluster.components.iter().map(|c| c.to_range()).collect(),
                    });
                });
                Some(run)
            })
            .flatten()
            .ok_or_else(|| FontError::NoMetrics { family: entry.family().to_owned() })?;
        Ok(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{user_catalog, JETBRAINS_MONO};
    use crate::FaceStyle;

    const SIZE: f32 = 13.0;

    fn run(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn levels_parse_and_round_trip() {
        for level in LigatureLevel::ALL {
            assert_eq!(LigatureLevel::parse(level.as_str()), Some(level));
            assert_eq!(serde_json::to_string(&level).unwrap(), format!("\"{}\"", level.as_str()));
        }
        assert_eq!(LigatureLevel::default(), LigatureLevel::Dlig);
        assert_eq!(LigatureLevel::parse("none"), Some(LigatureLevel::Off));
        assert_eq!(LigatureLevel::parse("nope"), None);
        assert!(!LigatureLevel::Off.permits_ligatures());
        assert!(LigatureLevel::Calt.permits_ligatures());
    }

    #[test]
    fn the_features_match_the_level() {
        let off = LigatureLevel::Off.features();
        assert!(off.iter().all(|s| s.value == 0), "{off:?}");
        let calt = LigatureLevel::Calt.features();
        let named = |settings: [Setting<u16>; 4], name: &str| {
            settings
                .iter()
                .find(|s| s.tag == swash::tag_from_str_lossy(name))
                .expect("feature is set")
                .value
        };
        assert_eq!(named(calt, "calt"), 1);
        assert_eq!(named(calt, "liga"), 1);
        assert_eq!(named(calt, "dlig"), 0);
        let dlig = LigatureLevel::Dlig.features();
        assert_eq!(named(dlig, "dlig"), 1);
        // The settings carry the level's features through.
        assert_eq!(LigatureSettings::new(LigatureLevel::Calt, true).features(), calt);
    }

    #[test]
    fn symbols_ligate_by_default_and_letters_do_not() {
        let settings = LigatureSettings::new(LigatureLevel::Calt, false);
        for symbol in ["=>", "!=", ">=", "->", "::"] {
            assert!(settings.boundary_allowed(&run(symbol), 0), "{symbol}");
            assert!(settings.run_allowed(&run(symbol)), "{symbol}");
            assert!(!settings.splits(symbol), "{symbol}");
        }
        for letters in ["fi", "fl", "ffi"] {
            let chars = run(letters);
            for boundary in 0..chars.len() - 1 {
                assert!(!settings.boundary_allowed(&chars, boundary), "{letters}");
            }
            assert!(!settings.run_allowed(&chars), "{letters}");
            assert!(settings.splits(letters), "{letters}");
        }
    }

    #[test]
    fn the_alphabet_setting_is_what_allows_letter_ligatures() {
        let plain = LigatureSettings::new(LigatureLevel::Calt, false);
        let alphabet = LigatureSettings::new(LigatureLevel::Calt, true);
        assert!(!plain.boundary_allowed(&run("fi"), 0));
        assert!(alphabet.boundary_allowed(&run("fi"), 0));
        assert_eq!(alphabet.shaping_pieces("field"), vec![0..5]);
        assert_eq!(plain.shaping_pieces("field"), vec![0..1, 1..2, 2..3, 3..4, 4..5]);

        // Letters, digits and CJK all count as alphabet.
        assert!(is_alphabet('a'));
        assert!(is_alphabet('9'));
        assert!(is_alphabet('漢'));
        assert!(!is_alphabet('='));
        assert!(!is_alphabet('_'));
    }

    #[test]
    fn off_permits_no_boundary_and_does_not_split() {
        let settings = LigatureSettings::off();
        assert!(!settings.boundary_allowed(&run("=>"), 0));
        assert!(!settings.run_allowed(&run("=>")));
        // Nothing can ligate once the features are off, so the run stays whole —
        // splitting it would only break mark clustering.
        assert_eq!(settings.shaping_pieces("fie=>ld"), vec![0..7]);
    }

    #[test]
    fn blank_and_impossible_boundaries_are_not_permitted() {
        let settings = LigatureSettings::new(LigatureLevel::Dlig, true);
        assert!(!settings.boundary_allowed(&run("= ="), 1), "no ligature across a space");
        assert!(!settings.boundary_allowed(&run("a"), 0), "there is no boundary");
        assert!(!settings.boundary_allowed(&run("="), 5), "out of range");
        assert!(!settings.boundary_allowed(&[], 0));
        assert!(!settings.boundary_allowed(&run("f\ni"), 1), "no ligature across a newline");
        assert!(settings.shaping_pieces("").is_empty());
    }

    #[test]
    fn pieces_cut_exactly_at_the_forbidden_boundaries() {
        let settings = LigatureSettings::new(LigatureLevel::Calt, false);
        // `ab=>cd`: the letter pairs are cut, the symbol run is not.
        assert_eq!(settings.shaping_pieces("ab=>cd"), vec![0..1, 1..5, 5..6]);
        assert_eq!(settings.shaping_pieces("=>"), vec![0..2]);
        // Multi-byte characters keep their byte offsets: the cut lands after the
        // second `漢`, not inside it.
        assert_eq!(settings.shaping_pieces("漢漢=漢"), vec![0..3, 3..10]);
        let covered: usize = settings.shaping_pieces("ab=>cd").iter().map(|r| r.len()).sum();
        assert_eq!(covered, "ab=>cd".len());
    }

    #[test]
    fn shaping_a_symbol_run_builds_a_run_that_covers_its_source() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let settings = LigatureSettings::new(LigatureLevel::Calt, false);
        let shaped = catalog.shape(id, "=>!=", SIZE, settings).unwrap();
        assert!(!shaped.clusters.is_empty());
        assert_eq!(shaped.clusters[0].source.start, 0);
        assert_eq!(shaped.clusters.last().unwrap().source.end, 4);
        assert!(shaped.advance > 0.0);
        // Every glyph is accounted for in a cluster.
        let clustered: usize = shaped.clusters.iter().map(|c| c.glyphs.len()).sum();
        assert_eq!(clustered, shaped.glyph_count());
    }

    #[test]
    fn turning_ligatures_off_removes_every_ligature_the_font_would_form() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let text = "fi != => fl -> ffi";
        let on = catalog
            .shape(id, text, SIZE, LigatureSettings::new(LigatureLevel::Dlig, true))
            .unwrap();
        let off = catalog.shape(id, text, SIZE, LigatureSettings::off()).unwrap();
        assert!(
            off.is_ligature_free(),
            "with the level off the shaper must not join anything: {:?}",
            off.clusters.iter().filter(|c| c.is_ligature()).collect::<Vec<_>>()
        );
        assert!(
            off.ligature_count() <= on.ligature_count(),
            "disabling the features cannot add ligatures"
        );
        assert!(!off.clusters.is_empty());
    }

    #[test]
    fn no_ligature_ever_crosses_a_forbidden_boundary() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let settings = LigatureSettings::new(LigatureLevel::Calt, false);
        for text in ["fi", "field", "x=>y", "a!=b", "=>", "fi=>fl"] {
            let chars: Vec<char> = text.chars().collect();
            let bounds = byte_bounds(text);
            let char_of = |byte: usize| bounds.iter().position(|b| *b == byte).unwrap_or(0);

            let ruled = catalog.shape_ruled(id, text, SIZE, settings).unwrap();
            // Every cluster's source is inside the text.
            for cluster in &ruled.clusters {
                assert!(cluster.source.end <= text.len(), "{text}: {:?}", cluster.source);
            }
            // A ligature may only span boundaries the rule allowed.
            for cluster in ruled.clusters.iter().filter(|c| c.is_ligature()) {
                let first = char_of(cluster.source.start);
                let last = char_of(cluster.source.end);
                for boundary in first..last.saturating_sub(1) {
                    assert!(
                        settings.boundary_allowed(&chars, boundary),
                        "{text}: a ligature spans the forbidden boundary {boundary}"
                    );
                }
            }
        }
    }

    #[test]
    fn splitting_can_only_remove_ligatures_never_add_them() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).unwrap();
        let text = "a=>b fi c!=d";
        let whole = catalog
            .shape(id, text, SIZE, LigatureSettings::new(LigatureLevel::Dlig, false))
            .unwrap();
        let ruled = catalog
            .shape_ruled(id, text, SIZE, LigatureSettings::new(LigatureLevel::Dlig, false))
            .unwrap();
        assert!(
            ruled.ligature_count() <= whole.ligature_count(),
            "cutting the run up cannot create joins: {} vs {}",
            ruled.ligature_count(),
            whole.ligature_count()
        );
        // The ruled run still describes the whole text.
        assert_eq!(ruled.clusters.first().unwrap().source.start, 0);
        assert_eq!(ruled.clusters.last().unwrap().source.end, text.len());
        assert_eq!(
            ruled.clusters.iter().map(|c| c.source.len()).sum::<usize>(),
            text.len()
        );
    }

    #[test]
    fn shaping_reports_an_unknown_face() {
        let (catalog, _dir) = user_catalog();
        let settings = LigatureSettings::default();
        assert!(matches!(
            catalog.shape(fontdb::ID::dummy(), "x", SIZE, settings),
            Err(FontError::UnknownFace { .. })
        ));
    }
}
