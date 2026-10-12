//! Real glyphs, from a real font.
//!
//! [`SwashGlyphs`] is the `GlyphSource` the shell uses: it takes a character and
//! a pixel size and returns coverage, by looking the character up in a face and
//! rasterising its outline.
//!
//! # Why the cache is keyed the way it is
//!
//! Rasterising is the expensive part and a terminal asks for the same few hundred
//! glyphs over and over, so the cache is the whole reason this is fast enough to
//! do on the CPU. The key is `(face, character, pixel size, synthetic flags)` —
//! note **character**, not glyph id: the id is what the face's `cmap` is for, and
//! caching by id would mean two faces that number `'W'` differently collide.
//!
//! The size is quantised to whole pixels, because a terminal picks one font size
//! and keeps it; a cache keyed on a float would miss on every sub-pixel drift and
//! re-rasterise the alphabet.
//!
//! # Synthetic bold and italic
//!
//! Faux-bold is an embolden applied to the outline (swash does it), and faux-italic
//! is a shear. Both are part of the key, because a real bold face and a
//! synthesised one are different bitmaps.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use otty_core::CellSize;

use crate::atlas::GlyphKey;
use crate::raster::{GlyphBitmap, GlyphSource};

/// Where a glyph's ink sits, and how the face is measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphMetrics {
    /// The advance width, which a monospaced grid uses as the cell width.
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    pub units_per_em: f32,
}

/// One face, ready to rasterise.
pub struct Face {
    id: fontdb::ID,
    data: Arc<Vec<u8>>,
    index: u32,
    metrics: GlyphMetrics,
    /// The character a monospaced grid should measure its advance from. `M` is
    /// the conventional choice because it is the widest letter in almost every
    /// font, so a cell sized to it fits the rest.
    reference: char,
}

impl Face {
    /// Load a face from a `fontdb` database.
    pub fn load(db: &fontdb::Database, id: fontdb::ID, size_px: f32) -> Option<Self> {
        let (data, index) = db.with_face_data(id, |data, index| (data.to_vec(), index))?;
        Self::from_data(id, Arc::new(data), index, size_px)
    }

    fn from_data(id: fontdb::ID, data: Arc<Vec<u8>>, index: u32, size_px: f32) -> Option<Self> {
        let font = swash::FontRef::from_index(&data, index as usize)?;
        // `metrics()` gives a proxy; the values live on the materialised form.
        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        Some(Self {
            id,
            data,
            index,
            metrics: GlyphMetrics {
                advance: 0.0,
                ascent: metrics.ascent as f32 * scale,
                descent: metrics.descent as f32 * scale,
                units_per_em: upem,
            },
            reference: 'M',
        })
    }

    pub fn id(&self) -> fontdb::ID {
        self.id
    }

    /// Measure a character's advance at a size, in pixels.
    ///
    /// The glyph's own `hmtx` advance, scaled to `size_px`. Face-wide
    /// `Metrics::average_width` / `max_width` are not the monospaced cell.
    pub fn advance_of(&self, ch: char, size_px: f32) -> f32 {
        let Some(font) = swash::FontRef::from_index(&self.data, self.index as usize) else {
            return 0.0;
        };
        let gid = font.charmap().map(ch);
        font.glyph_metrics(&[]).scale(size_px).advance_width(gid)
    }

    /// The cell a grid should use for this face at this size.
    ///
    /// One width for every glyph, which is what a monospaced grid needs; a
    /// proportional face used as a terminal font gives a ragged grid, which is
    /// why `font-family` names a monospaced family and why `CellMetrics` reports
    /// whether the face really is monospaced.
    ///
    /// The width is [`Face::advance_of`] of the reference glyph (`M`).
    pub fn cell_size(&self, size_px: f32) -> CellSize {
        let advance = self.advance_of(self.reference, size_px);
        // **Add**, not subtract. swash reports `descent` as a positive magnitude,
        // measured on the bundled faces as `ascent 16.32, descent 4.8` at 16 px.
        // Subtracting gave a line height of 11.52 instead of 21.12 — a cell a
        // little over half as tall as it should be, which would clip the descender
        // of every `g` and `y`. The sign is asserted in the tests so a swash
        // version that flips the convention fails loudly rather than silently.
        let line = self.metrics.ascent + self.metrics.descent;
        CellSize {
            width: if advance > 0.0 { advance } else { size_px * 0.6 },
            height: line,
            baseline: self.metrics.ascent,
        }
    }
}

/// System families for what a Latin monospace lacks — CJK first, then symbols —
/// in the order they are tried. Whichever the machine has answers.
const FALLBACK_FAMILIES: &[&str] = &[
    "Microsoft YaHei",
    "PingFang SC",
    "Noto Sans Mono CJK SC",
    "Noto Sans CJK SC",
    "WenQuanYi Micro Hei",
    "Yu Gothic",
    "Malgun Gothic",
    "Segoe UI Symbol",
];

/// The installed [`FALLBACK_FAMILIES`], for [`SwashGlyphs::with_fallbacks`].
///
/// A CJK face is tens of megabytes, so the bytes are read once per process and
/// shared by every pane. The ids are `db`'s, which is the database the primary
/// face came from, so the two cannot collide in a glyph key.
pub fn fallback_faces(db: &fontdb::Database, size_px: f32) -> Vec<Face> {
    type Loaded = Vec<(fontdb::ID, Arc<Vec<u8>>, u32)>;
    static DATA: std::sync::OnceLock<Loaded> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        FALLBACK_FAMILIES
            .iter()
            .filter_map(|name| {
                let families = [fontdb::Family::Name(name)];
                let id = db.query(&fontdb::Query { families: &families, ..Default::default() })?;
                let (data, index) = db.with_face_data(id, |data, index| (data.to_vec(), index))?;
                Some((id, Arc::new(data), index))
            })
            .collect()
    })
    .iter()
    .filter_map(|(id, data, index)| Face::from_data(*id, data.clone(), *index, size_px))
    .collect()
}

/// Rasterises glyphs from a face, with a cache.
pub struct SwashGlyphs {
    face: Face,
    /// Faces asked, in order, for a character `face` does not have.
    fallbacks: Vec<Face>,
    size_px: f32,
    /// The rasterised bitmaps, keyed exactly as the atlas keys its slots.
    cache: RefCell<HashMap<GlyphKey, GlyphBitmap>>,
    context: RefCell<swash::scale::ScaleContext>,
    embolden: bool,
    slant: f32,
}

impl SwashGlyphs {
    pub fn new(face: Face, size_px: f32) -> Self {
        Self {
            face,
            fallbacks: Vec::new(),
            size_px,
            cache: RefCell::new(HashMap::new()),
            context: RefCell::new(swash::scale::ScaleContext::new()),
            embolden: false,
            slant: 0.0,
        }
    }

    pub fn with_fallbacks(mut self, faces: Vec<Face>) -> Self {
        self.fallbacks = faces;
        self
    }

    /// The first face that has `ch`, and its glyph there. Glyph 0 of the
    /// primary face when none does.
    fn pick(&self, ch: char) -> (&Face, u16) {
        std::iter::once(&self.face)
            .chain(&self.fallbacks)
            .find_map(|face| {
                let font = swash::FontRef::from_index(&face.data, face.index as usize)?;
                let gid = u16::from(font.charmap().map(ch));
                (gid != 0).then_some((face, gid))
            })
            .unwrap_or((&self.face, 0))
    }

    /// Synthesise bold for every glyph. Used when the family has no bold face
    /// and `text-bold = synthetic`.
    pub fn with_embolden(mut self, on: bool) -> Self {
        self.embolden = on;
        self
    }

    /// Synthesise an italic slant, in hundredths of a degree.
    pub fn with_slant(mut self, degrees: f32) -> Self {
        self.slant = degrees;
        self
    }

    pub fn face(&self) -> &Face {
        &self.face
    }

    pub fn size_px(&self) -> f32 {
        self.size_px
    }

    /// How many glyphs are cached, so a leak is visible.
    pub fn cached(&self) -> usize {
        self.cache.borrow().len()
    }

    /// The key this source would use for a character.
    pub fn key_for(&self, ch: char) -> GlyphKey {
        let (face, gid) = self.pick(ch);
        GlyphKey::new(
            face.id,
            gid,
            self.size_px,
            self.embolden,
            self.slant,
        )
    }

    fn rasterise(&self, ch: char) -> GlyphBitmap {
        // A character no face contains is not an error; blank keeps this total.
        let (face, glyph_id) = self.pick(ch);
        if glyph_id == 0 {
            return GlyphBitmap::blank();
        }
        let Some(font) = swash::FontRef::from_index(&face.data, face.index as usize) else {
            return GlyphBitmap::blank();
        };

        let mut context = self.context.borrow_mut();
        let mut scaler = context.builder(font).size(self.size_px).build();

        let mut render = swash::scale::Render::new(&[
            swash::scale::Source::Outline,
            // Colour and bitmap faces exist; a terminal wants the outline path so
            // its `font-blending` setting is the only thing deciding the edges.
            swash::scale::Source::ColorOutline(0),
        ]);
        render.format(swash::zeno::Format::Alpha);
        // Sub-pixel positioning is off: a terminal cell is a whole number of
        // pixels and a half-pixel phase would need a second cache axis for no
        // visible gain at text sizes.
        render.offset(swash::zeno::Vector::new(0.0, 0.0));
        if self.embolden {
            // A whole pixel of extra stroke: enough to read as bold at text
            // sizes, and the smallest amount that does not look like a
            // rasterisation artefact. `strength` is in pixels, not a ratio.
            render.embolden(1.0);
        }

        let Some(image) = render.render(&mut scaler, glyph_id) else {
            return GlyphBitmap::blank();
        };
        let placement = image.placement;
        let bitmap = GlyphBitmap {
            width: placement.width,
            height: placement.height,
            left: placement.left,
            top: placement.top,
            coverage: image.data.to_vec(),
        };
        // A shear for faux-italic is applied to the coverage, not the outline:
        // swash has no slant, and resampling the coverage is cheap and good
        // enough for a synthetic face — which is what "synthetic" means.
        if self.slant != 0.0 && !bitmap.is_blank() {
            shear(&bitmap, self.slant)
        } else {
            bitmap
        }
    }
}

impl GlyphSource for SwashGlyphs {
    fn glyph(&self, ch: char, _size_px: f32) -> GlyphBitmap {
        let key = self.key_for(ch);
        if let Some(hit) = self.cache.borrow().get(&key) {
            return hit.clone();
        }
        let bitmap = self.rasterise(ch);
        self.cache.borrow_mut().insert(key, bitmap.clone());
        bitmap
    }
}

/// Shear coverage horizontally, for faux-italic.
///
/// Sample with nearest-neighbour: an interpolation kernel would soften the
/// strokes of a face that has no italic design to soften, and the result reads as
/// blur rather than as a slant.
fn shear(source: &GlyphBitmap, degrees: f32) -> GlyphBitmap {
    let slope = (degrees.to_radians()).tan();
    let extra = (source.height as f32 * slope.abs()) as u32 + 1;
    let width = source.width + extra;
    let mut coverage = vec![0u8; (width * source.height) as usize];
    for y in 0..source.height {
        // A shear pivots around the baseline; this approximates it by pivoting
        // around the bottom of the bitmap.
        let offset = ((source.height - y) as f32 * slope) + extra as f32 / 2.0;
        for x in 0..source.width {
            let v = source.at(x, y);
            if v == 0 {
                continue;
            }
            let nx = x as f32 + offset;
            if nx < 0.0 {
                continue;
            }
            let nx = nx as u32;
            if nx < width {
                coverage[(y * width + nx) as usize] = v;
            }
        }
    }
    GlyphBitmap {
        width,
        height: source.height,
        left: source.left - extra as i32 / 2,
        top: source.top,
        coverage,
    }
}

/// The fonts the repository ships, which is what the tests rasterise with.
///
/// The location itself belongs to `otty-fonts`, which is what the catalog scans;
/// this is the same directory, named once. A second spelling of the path here
/// would be a second thing to move when the bundle moves.
pub fn bundled_fonts_dir() -> std::path::PathBuf {
    otty_fonts::bundled_dir()
}

/// A database holding the bundled fonts, plus the system's when asked.
pub fn bundled_database(system: bool) -> fontdb::Database {
    let mut db = fontdb::Database::new();
    if let Ok(rd) = std::fs::read_dir(bundled_fonts_dir()) {
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for f in files {
            db.load_font_file(&f).ok();
        }
    }
    if system {
        db.load_system_fonts();
    }
    db
}

/// The first face in the database, for a caller that just wants one.
pub fn first_face(db: &fontdb::Database) -> Option<fontdb::ID> {
    db.faces().next().map(|f| f.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Cursor, GridFrame, Line, Palette};
    use crate::raster::GridRasterizer;

    fn loaded() -> (fontdb::Database, Face) {
        let db = bundled_database(false);
        assert!(!db.is_empty(), "the bundled fonts must load");
        let id = first_face(&db).expect("a face");
        let face = Face::load(&db, id, 16.0).expect("the face must load into swash");
        (db, face)
    }

    #[test]
    fn the_bundled_fonts_load_and_a_face_measures() {
        let (_db, face) = loaded();
        assert!(face.metrics.units_per_em > 0.0);
        assert!(face.metrics.ascent > 0.0, "ascent {:?}", face.metrics);
        assert!(
            face.metrics.descent > 0.0,
            "descent {:?} — swash reports a positive magnitude; if that ever flips, \
             `cell_size` must add-to-subtract with it",
            face.metrics.descent
        );
    }

    #[test]
    fn the_cell_size_fits_the_font() {
        let (_db, face) = loaded();
        let cell = face.cell_size(16.0);
        assert!(cell.width > 0.0, "cell width {:?}", cell.width);
        assert!(cell.height > 0.0);
        // The baseline sits inside the cell, or text would be cut off.
        assert!(cell.baseline < cell.height, "baseline {:?} >= height {:?}", cell.baseline, cell.height);
        assert!(cell.baseline > 0.0);
        // A 16 px Latin mono face has an advance around 0.6 em. The previous
        // bound of 4 px let `Metrics::average_width` through, which is what
        // stacked the letters on top of each other.
        assert!((8.0..=14.0).contains(&cell.width), "cell width {:?}", cell.width);
        assert!((10.0..=30.0).contains(&cell.height), "cell height {:?}", cell.height);
    }

    #[test]
    fn a_letter_rasterises_to_ink() {
        let (_db, face) = loaded();
        let glyphs = SwashGlyphs::new(face, 16.0);
        let w = glyphs.glyph('W', 16.0);
        assert!(!w.is_blank(), "W produced no bitmap");
        assert!(w.width > 1 && w.height > 1, "W is {}x{}", w.width, w.height);
        let ink = w.coverage.iter().filter(|c| **c > 0).count();
        assert!(ink > 0, "W produced a bitmap with no covered pixels");
        // A glyph is not a filled box: most of a `W`'s box is empty.
        let total = (w.width * w.height) as usize;
        assert!(ink < total, "W filled its whole box, so it is not an outline render");
    }

    #[test]
    fn a_space_produces_no_ink() {
        let (_db, face) = loaded();
        let glyphs = SwashGlyphs::new(face, 16.0);
        let space = glyphs.glyph(' ', 16.0);
        // A space has an advance and no outline. Either a blank bitmap or an
        // empty one is right; ink is not.
        let ink = space.coverage.iter().filter(|c| **c > 0).count();
        assert_eq!(ink, 0, "a space must have no ink");
    }

    #[test]
    fn different_letters_produce_different_bitmaps() {
        let (_db, face) = loaded();
        let glyphs = SwashGlyphs::new(face, 16.0);
        let i = glyphs.glyph('i', 16.0);
        let w = glyphs.glyph('W', 16.0);
        let ink = |g: &GlyphBitmap| g.coverage.iter().filter(|c| **c > 0).count();
        // `i` is mostly empty; `W` is dense. If they matched, the charmap lookup
        // would be returning one glyph for everything.
        assert!(ink(&i) < ink(&w), "i has {} ink, W has {}", ink(&i), ink(&w));
    }

    #[test]
    fn a_character_the_face_lacks_is_blank_rather_than_an_error() {
        let (_db, face) = loaded();
        let glyphs = SwashGlyphs::new(face, 16.0);
        // A private-use codepoint no text face contains.
        let missing = glyphs.glyph('\u{10FFFD}', 16.0);
        assert_eq!(missing.coverage.iter().filter(|c| **c > 0).count(), 0);
    }

    #[test]
    fn the_cache_hits_on_the_second_request_and_misses_on_a_new_face() {
        let db = bundled_database(false);
        let ids: Vec<fontdb::ID> = db.faces().map(|f| f.id).collect();
        if ids.len() < 2 {
            // The bundle ships several faces, but a machine could have fewer.
            return;
        }
        let a = Face::load(&db, ids[0], 16.0).unwrap();
        let b = Face::load(&db, ids[1], 16.0).unwrap();
        let glyphs = SwashGlyphs::new(a, 16.0);

        assert_eq!(glyphs.cached(), 0);
        let _ = glyphs.glyph('A', 16.0);
        assert_eq!(glyphs.cached(), 1);
        let _ = glyphs.glyph('A', 16.0);
        assert_eq!(glyphs.cached(), 1, "the second request re-rasterised");

        // A different face must not reuse the first face's entry: the cache key
        // carries the face id for exactly this reason.
        let other = SwashGlyphs::new(b, 16.0);
        assert_eq!(other.cached(), 0);
        let _ = other.glyph('A', 16.0);
        assert_eq!(other.cached(), 1);
    }

    #[test]
    fn a_near_miss_size_shares_a_cache_entry() {
        let (_db, face) = loaded();
        // The key quantises the size, so 16.0 and 16.2 are one entry. Without
        // this a sub-pixel layout drift would re-rasterise the alphabet.
        let glyphs = SwashGlyphs::new(face, 16.0);
        let a = glyphs.key_for('A');
        let b = GlyphKey::new(a.face, a.glyph, 16.2, false, 0.0);
        assert_eq!(a, b);
    }

    #[test]
    fn faux_bold_is_a_different_bitmap_from_the_real_one() {
        let (_db, face) = loaded();
        let plain = SwashGlyphs::new(face, 16.0);
        let ink = |g: &GlyphBitmap| g.coverage.iter().filter(|c| **c > 0).count();
        let thin = ink(&plain.glyph('l', 16.0));

        let db = bundled_database(false);
        let id = first_face(&db).unwrap();
        let face2 = Face::load(&db, id, 16.0).unwrap();
        let bold = SwashGlyphs::new(face2, 16.0).with_embolden(true);
        let thick = ink(&bold.glyph('l', 16.0));
        assert!(
            thick > thin,
            "emboldening did not add ink: {thin} -> {thick}"
        );
    }

    #[test]
    fn a_shear_widens_the_bitmap_and_keeps_the_ink() {
        let (_db, face) = loaded();
        let db = bundled_database(false);
        let id = first_face(&db).unwrap();
        let slanted = SwashGlyphs::new(Face::load(&db, id, 16.0).unwrap(), 16.0).with_slant(12.0);
        let upright = SwashGlyphs::new(face, 16.0);
        let a = upright.glyph('H', 16.0);
        let b = slanted.glyph('H', 16.0);
        assert!(b.width > a.width, "a slant must widen the box: {} -> {}", a.width, b.width);
        let ink = |g: &GlyphBitmap| g.coverage.iter().filter(|c| **c > 0).count();
        // Shearing moves ink; losing most of it would mean the resample dropped
        // the strokes.
        assert!(ink(&b) * 2 > ink(&a), "the shear lost ink: {} -> {}", ink(&a), ink(&b));
    }

    #[test]
    fn real_text_lands_in_the_pixel_buffer_at_the_right_colour() {
        // The end-to-end assertion: font file -> bitmap -> composited pixel.
        let (_db, face) = loaded();
        let glyphs = SwashGlyphs::new(face, 16.0);
        let palette = Palette::default();

        let mut frame = GridFrame {
            cols: 8,
            rows: 2,
            cell: otty_core::CellSize::default(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines: vec![Line::blank(palette.background); 2],
            palette,
            revision: 0,
        };
        frame.lines[0] = Line {
            runs: vec![crate::grid::Run {
                text: "H".into(),
                style: crate::grid::CellStyle {
                    fg: 0xFFFFFFFF,
                    bg: palette.background,
                    flags: 0,
                },
            }],
            wrapped: false,
        };

        let mut raster = GridRasterizer::new(128, 64);
        raster.rasterize(&frame, &glyphs);

        // Something in the first cell's band must be lighter than the
        // background, because a glyph was drawn there.
        let cell_w = 128 / 8;
        let cell_h = 64 / 2;
        let mut lightest = 0u8;
        for y in 0..cell_h {
            for x in 0..cell_w {
                lightest = lightest.max((raster.pixel(x, y) >> 24) as u8);
            }
        }
        assert!(lightest > 0x40, "no glyph ink reached the buffer: lightest {lightest:#04X}");

        // And the second cell stays background, so the glyph did not smear.
        let flat = (0..cell_h).all(|y| (0..cell_w).all(|x| raster.pixel(cell_w + x, y) == palette.background));
        assert!(flat, "ink leaked into the second cell");
    }
}
