//! Cell metrics: the advance width, the line box, and the baseline offset that
//! drive the grid arithmetic in `otty-term`.

use otty_core::geometry::{CellHeightAdjust, CellSize};

use crate::catalog::FontCatalog;
use crate::error::FontError;

/// The line height a face falls back to when it publishes no usable vertical
/// metrics — the same 1.2 the schema documents as the Windows default.
pub const DEFAULT_LINE_HEIGHT: f32 = 1.2;

/// Which measurement the cell advance came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvanceBasis {
    /// The face is monospaced and maps `M`: its advance is the cell width.
    Monospace,
    /// The face maps `M` but is not monospaced; the advance is still the grid's
    /// cell width, which is what a terminal does with a proportional face.
    ReferenceGlyph,
    /// `M` is unmapped (a CJK-only or symbol-only face): `OS/2`
    /// `xAvgCharWidth` answered.
    AverageWidth,
    /// `hhea.max_advance` — the last resort, when neither of the above exists.
    MaxAdvance,
}

/// The size knobs a cell is computed with.
///
/// `size` is `font-size`; `line_height` is the Settings multiplier (Compact 1.0,
/// Loose 1.2, or a custom value); `adjust_cell_height` is the parsed
/// `[token].adjust-cell-height` delta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricsOptions {
    /// Em size in logical pixels.
    pub size: f32,
    /// Multiplier applied to the font's natural line height.
    pub line_height: Option<f32>,
    /// Absolute delta or percentage applied after the multiplier.
    pub adjust_cell_height: Option<CellHeightAdjust>,
}

impl MetricsOptions {
    /// At `font-size`, with no line-height multiplier and no adjustment.
    pub const fn new(size: f32) -> Self {
        Self { size, line_height: None, adjust_cell_height: None }
    }

    /// Set the line-height multiplier.
    pub const fn with_line_height(mut self, multiplier: Option<f32>) -> Self {
        self.line_height = multiplier;
        self
    }

    /// Set the parsed `adjust-cell-height` delta.
    pub const fn with_adjust(mut self, adjust: Option<CellHeightAdjust>) -> Self {
        self.adjust_cell_height = adjust;
        self
    }

    /// Parse and set `[token].adjust-cell-height`, which is a *string* in the
    /// theme and the config: `Npx`, bare `N`, `N%`, or `0`.
    pub fn with_adjust_str(mut self, value: Option<&str>) -> Result<Self, FontError> {
        self.adjust_cell_height = match value {
            None => None,
            Some(v) => Some(CellHeightAdjust::parse(v).map_err(|reason| {
                FontError::BadCellHeight { value: v.to_owned(), reason }
            })?),
        };
        Ok(self)
    }
}

/// The measurements of one cell of the terminal grid, at one size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellMetrics {
    /// The em size the metrics were computed at.
    pub size: f32,
    /// Font design units per em, as the face publishes it.
    pub units_per_em: u16,
    /// Cell advance width — the font's own advance for a single cell.
    pub advance: f32,
    /// Which measurement `advance` came from.
    pub basis: AdvanceBasis,
    /// Distance from the baseline up to the top of the font's line box.
    pub ascent: f32,
    /// Distance from the baseline down to the bottom of the line box, as a
    /// positive magnitude whichever sign the face's own table used.
    pub descent: f32,
    /// The font's recommended extra spacing between lines.
    pub line_gap: f32,
    /// The font's own line height, before any multiplier or adjustment:
    /// `ascent + descent + line_gap`.
    pub natural_line_height: f32,
    /// The cell height actually used, after the multiplier and the adjustment.
    pub line_height: f32,
    /// Distance from the top of the cell to the text baseline.
    pub baseline: f32,
    /// Whether the face declares itself monospaced.
    pub monospaced: bool,
}

impl CellMetrics {
    /// The box arithmetic, isolated from any font so it can be reasoned about on
    /// its own.
    ///
    /// The line height is the natural height after the multiplier and then the
    /// `adjust-cell-height` delta — the schema describes the delta as
    /// "add or remove a fixed number of pixels from the *current* cell height",
    /// so it applies after the multiplier. Whatever height is left over after
    /// the natural box is split evenly above and below it, which is what keeps
    /// glyphs vertically centred when the cell is made taller or shorter. A face
    /// that publishes no usable line metrics (a non-positive natural height)
    /// falls back to `1.2 × size`.
    ///
    /// Returns `(line_height, baseline)`.
    pub fn box_from(natural: f32, ascent: f32, options: &MetricsOptions) -> (f32, f32) {
        let natural = if natural > 0.0 { natural } else { options.size * DEFAULT_LINE_HEIGHT };
        let scaled = natural * options.line_height.unwrap_or(1.0);
        let line_height = match options.adjust_cell_height {
            Some(adjust) => adjust.apply(scaled),
            None => scaled,
        };
        let baseline = ascent + (line_height - natural) * 0.5;
        (line_height, baseline)
    }

    /// The grid's cell box. `CellSize::baseline` is "distance from the top of
    /// the cell to the text baseline", which is exactly
    /// [`CellMetrics::baseline`].
    pub const fn to_cell_size(&self) -> CellSize {
        CellSize { width: self.advance, height: self.line_height, baseline: self.baseline }
    }

    /// The same box with the advance and the height snapped to whole pixels.
    ///
    /// The grid-fit paths (`gdi`, `natural`, `natural-symmetric` — see
    /// [`crate::WindowsTextRendering::grid_fits`]) advance by whole pixels, so
    /// the renderer asks for this instead of the fractional box. The baseline is
    /// re-derived from the snapped height so the glyph box stays centred.
    pub fn snapped(&self) -> Self {
        let line_height = self.line_height.round().max(1.0);
        let baseline = self.ascent + (line_height - self.natural_line_height) * 0.5;
        Self {
            advance: self.advance.round().max(1.0),
            line_height,
            baseline: baseline.round(),
            ..*self
        }
    }
}

/// What `swash` reports, reduced to plain scalars so it can leave the
/// `with_face_data` closure.
struct FaceMetrics {
    units_per_em: u16,
    ascent: f32,
    descent: f32,
    leading: f32,
    average_width: f32,
    max_width: f32,
    monospaced: bool,
    reference_advance: Option<f32>,
}

impl FontCatalog {
    /// Measure one face at the options' size.
    ///
    /// The line height is taken from the face's own horizontal line metrics:
    /// `hhea` ascender / descender / lineGap, or the `OS/2` typographic trio
    /// when the face sets `USE_TYPO_METRICS` — whichever `swash` reports — as
    /// `ascent + |descent| + lineGap`. That is the font's natural line height,
    /// which is what the schema says applies when `adjust-cell-height` is unset.
    ///
    /// The advance is the advance width of the glyph `M` at the same size: every
    /// monospaced face defines `M` at the width of every other ASCII cell, which
    /// is the measurement the grid wants. A face that does not map `M` falls
    /// back to `xAvgCharWidth`, then to `hhea.max_advance`, and `basis` records
    /// which answered.
    pub fn metrics(&self, id: fontdb::ID, options: &MetricsOptions) -> Result<CellMetrics, FontError> {
        // A size has to be a real, positive number of points: `<= 0.0` catches
        // zero and negatives, and `is_finite` catches `NaN` and the infinities.
        if options.size <= 0.0 || !options.size.is_finite() {
            return Err(FontError::BadFontSize { size: options.size });
        }
        let entry = self.entry(id).ok_or(FontError::UnknownFace { id })?;

        let raw = self
            .database()
            .with_face_data(id, |data, index| {
                let font = swash::FontRef::from_index(data, index as usize)?;
                let global = font.metrics(&[]).scale(options.size);
                let glyphs = font.glyph_metrics(&[]).scale(options.size);
                // Glyph 0 is `.notdef`, which is what `Charmap::map` answers for
                // a character the face does not have.
                let reference = match font.charmap().map('M') {
                    0 => None,
                    glyph => Some(glyphs.advance_width(glyph)),
                };
                Some(FaceMetrics {
                    units_per_em: global.units_per_em,
                    ascent: global.ascent,
                    descent: global.descent,
                    leading: global.leading,
                    average_width: global.average_width,
                    max_width: global.max_width,
                    monospaced: global.is_monospace,
                    reference_advance: reference,
                })
            })
            .flatten()
            .ok_or_else(|| FontError::NoMetrics { family: entry.family().to_owned() })?;

        // `swash` reports the descender with the sign the face's own table uses:
        // `hhea`'s descender is conventionally negative and swash negates it, so
        // the reported value is already the conventional positive magnitude. The
        // box arithmetic wants the magnitude whichever way a face spelled it, so
        // the sign is normalised away here.
        let descent = raw.descent.abs();
        let natural = raw.ascent + descent + raw.leading;
        let (line_height, baseline) = CellMetrics::box_from(natural, raw.ascent, options);
        let (advance, basis) = match raw.reference_advance {
            Some(advance) if raw.monospaced => (advance, AdvanceBasis::Monospace),
            Some(advance) => (advance, AdvanceBasis::ReferenceGlyph),
            None if raw.average_width > 0.0 => (raw.average_width, AdvanceBasis::AverageWidth),
            None => (raw.max_width, AdvanceBasis::MaxAdvance),
        };
        let natural = if natural > 0.0 { natural } else { options.size * DEFAULT_LINE_HEIGHT };

        Ok(CellMetrics {
            size: options.size,
            units_per_em: raw.units_per_em,
            advance,
            basis,
            ascent: raw.ascent,
            descent,
            line_gap: raw.leading,
            natural_line_height: natural,
            line_height,
            baseline,
            monospaced: raw.monospaced,
        })
    }

    /// The advance width of a single character at `size`.
    ///
    /// `Ok(None)` when the face does not map the character — the renderer then
    /// asks the next family in the chain, or draws the notdef.
    pub fn glyph_advance(
        &self,
        id: fontdb::ID,
        ch: char,
        size: f32,
    ) -> Result<Option<f32>, FontError> {
        if size <= 0.0 || !size.is_finite() {
            return Err(FontError::BadFontSize { size });
        }
        if self.entry(id).is_none() {
            return Err(FontError::UnknownFace { id });
        }
        let advance = self
            .database()
            .with_face_data(id, |data, index| {
                let font = swash::FontRef::from_index(data, index as usize)?;
                let glyph = font.charmap().map(ch);
                if glyph == 0 {
                    return None;
                }
                let glyphs = font.glyph_metrics(&[]).scale(size);
                Some(glyphs.advance_width(glyph))
            })
            .flatten();
        Ok(advance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::DiscoveryOptions;
    use crate::test_support::{user_catalog, JETBRAINS_MONO, SYMBOLS_NERD_FONT};

    const SIZE: f32 = 13.0;

    #[test]
    fn a_face_without_a_reference_glyph_falls_back_to_its_average_width() {
        let (catalog, _dir) = user_catalog();
        let nerd = catalog
            .entries()
            .iter()
            .find(|e| e.family() == SYMBOLS_NERD_FONT)
            .expect("the bundled Nerd Font is in the fixture");
        // Icons only: it maps neither `M` nor any other ASCII letter, so there
        // is no reference glyph to take the cell advance from.
        assert!(catalog.glyph_advance(nerd.id, 'M', SIZE).unwrap().is_none());
        assert!(catalog.glyph_advance(nerd.id, 'i', SIZE).unwrap().is_none());

        let m = catalog.metrics(nerd.id, &MetricsOptions::new(SIZE)).unwrap();
        assert_eq!(m.basis, AdvanceBasis::AverageWidth);
        assert_eq!(m.units_per_em, 2048, "the Nerd Font is a 2048-unit font");
        assert!((m.advance - SIZE).abs() < 1e-3, "its average glyph is one em: {m:?}");
        assert!(m.monospaced);
        assert!(m.line_height > 0.0);
    }

    #[test]
    fn a_proportional_face_takes_the_advance_of_its_reference_glyph() {
        let catalog = FontCatalog::discover(&DiscoveryOptions::system_only());
        let maps_ascii = |entry: &&crate::FontEntry| {
            catalog.glyph_advance(entry.id, 'M', SIZE).ok().flatten().is_some()
                && catalog.glyph_advance(entry.id, 'i', SIZE).ok().flatten().is_some()
        };
        let entry = catalog
            .entries()
            .iter()
            .filter(|e| !e.is_monospace())
            .find(maps_ascii)
            .expect("a machine with system fonts has a proportional Latin face");

        let m = catalog.metrics(entry.id, &MetricsOptions::new(SIZE)).unwrap();
        assert!(!m.monospaced, "{entry:?}");
        assert_eq!(m.basis, AdvanceBasis::ReferenceGlyph, "{entry:?}");

        // The grid still measures one cell from `M` — which in a proportional
        // face is much wider than `i`, and that difference is exactly what a
        // terminal cannot honour per cell.
        let wide = catalog.glyph_advance(entry.id, 'M', SIZE).unwrap().unwrap();
        let narrow = catalog.glyph_advance(entry.id, 'i', SIZE).unwrap().unwrap();
        assert!(wide > narrow, "{}: `M` {wide} vs `i` {narrow}", entry.family());
        assert_eq!(m.advance, wide);
    }

    #[test]
    fn the_box_centres_the_extra_height() {
        let options = MetricsOptions::new(SIZE);
        let (height, baseline) = CellMetrics::box_from(17.0, 13.0, &options);
        assert_eq!((height, baseline), (17.0, 13.0));

        let options = MetricsOptions::new(SIZE).with_line_height(Some(1.2));
        let (height, baseline) = CellMetrics::box_from(20.0, 15.0, &options);
        assert!((height - 24.0).abs() < 1e-4, "{height}");
        // 2 extra points above and below: the baseline moves by half of the slack.
        assert!((baseline - 17.0).abs() < 1e-4, "{baseline}");

        let adjust = MetricsOptions::new(SIZE)
            .with_adjust(Some(CellHeightAdjust::Percent(20.0)));
        let (height, baseline) = CellMetrics::box_from(20.0, 15.0, &adjust);
        assert!((height - 24.0).abs() < 1e-4);
        assert!((baseline - 17.0).abs() < 1e-4);
    }

    #[test]
    fn a_face_without_line_metrics_falls_back_to_twenty_percent_over_size() {
        let options = MetricsOptions::new(10.0);
        let (height, baseline) = CellMetrics::box_from(0.0, 8.0, &options);
        assert!((height - 12.0).abs() < 1e-4);
        assert!((baseline - 8.0).abs() < 1e-4);
    }

    #[test]
    fn adjust_cell_height_parses_and_reports_bad_values() {
        let options = MetricsOptions::new(SIZE)
            .with_adjust_str(Some("-3"))
            .expect("a bare number is a point delta");
        assert_eq!(options.adjust_cell_height, Some(CellHeightAdjust::Points(-3.0)));
        let options = options.with_adjust_str(Some("20%")).expect("percent");
        assert_eq!(options.adjust_cell_height, Some(CellHeightAdjust::Percent(20.0)));
        let options = options.with_adjust_str(None).expect("unset");
        assert_eq!(options.adjust_cell_height, None);

        let error = MetricsOptions::new(SIZE).with_adjust_str(Some("2em")).unwrap_err();
        assert!(matches!(error, FontError::BadCellHeight { .. }), "{error}");
        assert!(error.to_string().contains("2em"), "{error}");
    }

    #[test]
    fn jetbrains_mono_measures_as_a_monospaced_grid_cell() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, crate::FaceStyle::Regular).expect("JBM regular");
        let m = catalog.metrics(id, &MetricsOptions::new(SIZE)).expect("metrics");

        assert!(m.monospaced, "JetBrains Mono declares itself monospaced");
        assert_eq!(m.basis, AdvanceBasis::Monospace);
        assert_eq!(m.units_per_em, 1000, "JBM is a 1000-unit font");
        assert!(m.advance > 0.0, "{m:?}");
        assert!(m.ascent > 0.0, "{m:?}");
        assert!(m.descent > 0.0, "descent is normalised to a positive magnitude");
        assert!(m.line_gap >= 0.0, "{m:?}");

        // The line height is the font's own horizontal metrics, not a constant.
        assert!(
            (m.natural_line_height - (m.ascent + m.descent + m.line_gap)).abs() < 1e-4,
            "{m:?}"
        );
        assert_eq!(m.line_height, m.natural_line_height);
        assert_eq!(m.baseline, m.ascent);

        // Pinned against the font's tables: JetBrains Mono declares `hhea`
        // ascender 1020, descender -300 and lineGap 0 at 1000 units per em, and
        // a 600-unit advance.
        assert!((m.advance - 0.6 * SIZE).abs() < 1e-3, "{m:?}");
        assert!((m.ascent - 13.26).abs() < 1e-3, "{m:?}");
        assert!((m.descent - 3.9).abs() < 1e-3, "{m:?}");
        assert_eq!(m.line_gap, 0.0);
        assert!((m.natural_line_height - 17.16).abs() < 1e-3, "{m:?}");

        // `CellSize::default()` is a rounded reading of exactly this font at
        // exactly this size, which is a useful sanity check on both.
        let default = otty_core::geometry::CellSize::default();
        assert!((m.advance - default.width).abs() < 0.5, "{m:?}");
        assert!((m.line_height - default.height).abs() < 0.5, "{m:?}");
        assert!((m.baseline - default.baseline).abs() < 0.5, "{m:?}");

        // Every ASCII cell of a monospaced face has the same advance.
        for ch in ['M', '0', 'i', ' ', 'W'] {
            let advance = catalog.glyph_advance(id, ch, SIZE).unwrap().expect("mapped");
            assert!((advance - m.advance).abs() < 1e-3, "{ch:?} advances {advance}");
        }
    }

    #[test]
    fn a_cell_height_delta_keeps_the_glyph_box_centred() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, crate::FaceStyle::Regular).expect("JBM regular");
        let plain = catalog.metrics(id, &MetricsOptions::new(SIZE)).unwrap();

        let tall_options = MetricsOptions::new(SIZE).with_adjust_str(Some("20%")).unwrap();
        assert_eq!(tall_options.adjust_cell_height, Some(CellHeightAdjust::Percent(20.0)));
        let tall = catalog.metrics(id, &tall_options).unwrap();
        assert!((tall.line_height - plain.natural_line_height * 1.2).abs() < 1e-3, "{tall:?}");
        assert!(
            (tall.baseline - (plain.ascent + (tall.line_height - plain.natural_line_height) / 2.0))
                .abs()
                < 1e-3,
            "{tall:?}"
        );
        assert!(tall.baseline > plain.baseline, "a taller cell pushes the baseline down");
        // A vertical adjustment leaves the advance alone.
        assert_eq!(tall.advance, plain.advance);
        assert_eq!(tall.natural_line_height, plain.natural_line_height);

        let compact_options = MetricsOptions::new(SIZE).with_adjust_str(Some("-4")).unwrap();
        let compact = catalog.metrics(id, &compact_options).unwrap();
        assert!(
            (compact.line_height - (plain.natural_line_height - 4.0)).abs() < 1e-3,
            "{compact:?}"
        );
        assert!(compact.baseline < plain.baseline);

        // A multiplier and a delta compose: the multiplier scales the font's own
        // height, then the delta is added to the result.
        let both = MetricsOptions::new(SIZE)
            .with_line_height(Some(1.2))
            .with_adjust_str(Some("2px"))
            .unwrap();
        let composed = catalog.metrics(id, &both).unwrap();
        assert!(
            (composed.line_height - (plain.natural_line_height * 1.2 + 2.0)).abs() < 1e-3,
            "{composed:?}"
        );
    }

    #[test]
    fn metrics_scale_linearly_with_the_size() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, crate::FaceStyle::Regular).expect("JBM regular");
        let at13 = catalog.metrics(id, &MetricsOptions::new(13.0)).unwrap();
        let at26 = catalog.metrics(id, &MetricsOptions::new(26.0)).unwrap();
        assert!((at26.advance - at13.advance * 2.0).abs() < 1e-3);
        assert!((at26.line_height - at13.line_height * 2.0).abs() < 1e-3);
        assert!((at26.ascent - at13.ascent * 2.0).abs() < 1e-3);
    }

    #[test]
    fn the_cell_box_drives_the_grid_arithmetic() {
        use otty_core::geometry::{Insets, Size};
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, crate::FaceStyle::Regular).expect("JBM regular");
        let m = catalog.metrics(id, &MetricsOptions::new(SIZE)).unwrap();
        let cell = m.to_cell_size();
        assert_eq!(cell.width, m.advance);
        assert_eq!(cell.height, m.line_height);
        assert_eq!(cell.baseline, m.baseline);

        let (cols, rows) = cell.grid_dimensions(Size::new(800.0, 600.0), Insets::default());
        assert_eq!(cols, (800.0 / m.advance).floor() as u16);
        assert_eq!(rows, (600.0 / m.line_height).floor() as u16);
        assert!(cols > 40 && rows > 20, "{cols}x{rows} is not a plausible grid");
    }

    #[test]
    fn snapping_grid_fits_the_box() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, crate::FaceStyle::Regular).expect("JBM regular");
        let m = catalog.metrics(id, &MetricsOptions::new(SIZE)).unwrap();
        let snapped = m.snapped();
        assert_eq!(snapped.advance, m.advance.round());
        assert_eq!(snapped.line_height, m.line_height.round());
        assert_eq!(snapped.baseline, snapped.baseline.round());
        assert_eq!(snapped.advance, snapped.advance.max(1.0));
        assert_eq!(snapped.units_per_em, m.units_per_em);
    }

    #[test]
    fn bad_input_is_reported_not_guessed() {
        let (catalog, _dir) = user_catalog();
        let id = catalog.find_face(JETBRAINS_MONO, crate::FaceStyle::Regular).expect("JBM regular");
        assert!(matches!(
            catalog.metrics(fontdb::ID::dummy(), &MetricsOptions::new(SIZE)),
            Err(FontError::UnknownFace { .. })
        ));
        assert!(matches!(
            catalog.metrics(id, &MetricsOptions::new(0.0)),
            Err(FontError::BadFontSize { .. })
        ));
        assert!(matches!(
            catalog.glyph_advance(id, 'x', f32::NAN),
            Err(FontError::BadFontSize { .. })
        ));
        assert!(catalog
            .glyph_advance(id, '\u{10FFFD}', SIZE)
            .unwrap()
            .is_none());
    }
}
