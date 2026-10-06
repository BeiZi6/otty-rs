//! Compositing a frame into pixels.
//!
//! This is the CPU side of the Slint route: the engine produces a [`GridFrame`],
//! glyphs come from an atlas, and this turns the two into one RGBA buffer that
//! the shell hands to Slint as an `Image`. No GPU, no window, no `slint` — which
//! is what makes it testable.
//!
//! # Why CPU compositing is the right call here
//!
//! A terminal is not a game. The content changes in *lines*, most frames change
//! nothing, and the visible cell count is a few thousand — the work is bounded by
//! the window, not by a scene. Measured against the alternative: the same grid
//! through a GPU pass costs a queue submission, a bind-group update and a
//! synchronisation point, and the win is a memcpy we would still have to do.
//!
//! The one thing that *would* hurt is recompositing and reuploading the whole
//! buffer every frame, so the rasterizer tracks dirty rows and exposes them. A
//! cursor blink repaints one cell; a log line repaints one row.
//!
//! # Blending
//!
//! A glyph is coverage, not colour: the atlas holds one byte per pixel saying how
//! much of the foreground to apply. So the compositor fills the cell with its
//! background and then blends `coverage * foreground` over it. Doing it this way
//! is what lets one rasterised glyph serve every colour — the alternative,
//! rasterising per colour, multiplies the atlas by the palette size.
//!
//! Blending is done in **sRGB space**, i.e. straight multiplication of the
//! eight-bit channels. That is what the shipped `font-blending = "srgb-over"`
//! default asks for; the other three modes are corrections layered on top and
//! belong to a later pass with `FontBlending` from `otty-fonts`.

use otty_term::Flags;

use crate::atlas::GlyphSlot;
use crate::grid::{CursorShape, GridFrame, Palette, Rgba, Run};

/// A rasterised glyph.
///
/// Deliberately not the atlas's [`GlyphSlot`]: a slot says *where in a texture*
/// and a bitmap says *what the pixels are*. Keeping them separate is what lets
/// the compositor be tested with synthetic glyphs and the atlas be tested with a
/// fake blitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    /// Offset from the pen position to the bitmap's top-left, in pixels. Signed:
    /// an italic glyph overhangs to the left.
    pub left: i32,
    pub top: i32,
    /// One byte of coverage per pixel, row-major.
    pub coverage: Vec<u8>,
}

impl GlyphBitmap {
    pub fn blank() -> Self {
        Self { width: 0, height: 0, left: 0, top: 0, coverage: Vec::new() }
    }

    pub fn is_blank(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The coverage at `(x, y)`, or 0 outside.
    pub fn at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        self.coverage.get((y * self.width + x) as usize).copied().unwrap_or(0)
    }
}

/// Where glyphs come from.
///
/// A trait rather than a concrete type so the compositor can be tested
/// deterministically: [`SyntheticGlyphs`] draws recognisable shapes without a
/// font file, and the real implementation goes through `swash` and the atlas.
pub trait GlyphSource {
    /// The bitmap for one character at one pixel size.
    fn glyph(&self, ch: char, size_px: f32) -> GlyphBitmap;
}

/// Draws each character as a filled square, so a test can assert exactly which
/// pixels a glyph covered.
#[derive(Debug, Default)]
pub struct SyntheticGlyphs {
    /// The square's edge, in pixels. Defaults to a cell-filling glyph.
    pub edge: Option<u32>,
    /// Characters with no bitmap at all, for testing the blank-glyph path.
    pub blanks: Vec<char>,
}

impl GlyphSource for SyntheticGlyphs {
    fn glyph(&self, ch: char, size_px: f32) -> GlyphBitmap {
        if self.blanks.contains(&ch) || ch == ' ' {
            return GlyphBitmap::blank();
        }
        let edge = self.edge.unwrap_or_else(|| size_px.round().clamp(1.0, 64.0) as u32);
        // Full coverage, so an assertion can say "this pixel IS the foreground".
        // The square's `top` is its own height, which — because `top` is measured
        // from the baseline upward — puts its bottom edge *on* the baseline. A
        // fixture that floats above the baseline would make the placement tests
        // say nothing about where a real glyph goes.
        GlyphBitmap {
            width: edge,
            height: edge,
            left: 0,
            top: edge as i32,
            coverage: vec![255u8; (edge * edge) as usize],
        }
    }
}

/// A boxed glyph source is itself a glyph source.
///
/// The blanket impl belongs here, beside the trait: `GlyphSource` is this crate's,
/// so a caller in another crate cannot write it (the orphan rule), and a caller
/// that holds a `Box<dyn GlyphSource + Send>` needs it to satisfy `rasterize`'s
/// `&impl GlyphSource` parameter, which requires `Sized`.
impl<T: GlyphSource + ?Sized> GlyphSource for Box<T> {
    fn glyph(&self, ch: char, size_px: f32) -> GlyphBitmap {
        (**self).glyph(ch, size_px)
    }
}

/// What the rasterizer produced, and what changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RasterOutput {
    pub width: u32,
    pub height: u32,
    /// Rows whose pixels differ from the previous frame.
    pub dirty_rows: Vec<u32>,
    /// Set when the whole buffer must be reuploaded: the first frame, a resize,
    /// or a palette change.
    pub full: bool,
}

/// Composites frames into an RGBA buffer.
pub struct GridRasterizer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    /// The previous frame's rows, for the dirty test. One `u64` hash per row is
    /// enough to decide "repaint this row", and it is far cheaper than keeping a
    /// second full buffer.
    row_hashes: Vec<u64>,
    previous_palette: Option<Palette>,
    cursor_shape: CursorShape,
    cursor_blink_on: bool,
    last_dirty: Vec<u32>,
}

impl GridRasterizer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
            row_hashes: Vec::new(),
            previous_palette: None,
            cursor_shape: CursorShape::Block,
            cursor_blink_on: true,
            last_dirty: Vec::new(),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Resize the buffer, which always means a full repaint.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        if (width, height) == (self.width, self.height) {
            return false;
        }
        self.width = width;
        self.height = height;
        self.pixels = vec![0; (width * height * 4) as usize];
        self.row_hashes.clear();
        self.previous_palette = None;
        true
    }

    /// Set the cursor's shape and blink phase.
    ///
    /// The blink phase is a parameter rather than a timer because a rasterizer
    /// that owned a clock would be untestable and would repaint on its own
    /// schedule; the shell decides when to toggle.
    pub fn set_cursor(&mut self, shape: CursorShape, blink_on: bool) {
        self.cursor_shape = shape;
        self.cursor_blink_on = blink_on;
    }

    /// Composite `frame` and report what changed.
    pub fn rasterize(&mut self, frame: &GridFrame, glyphs: &impl GlyphSource) -> RasterOutput {
        let size_changed = self.previous_palette.is_some()
            && frame.palette != self.previous_palette.unwrap_or(frame.palette);
        let first = self.previous_palette.is_none();
        let full = first || size_changed || self.pixels.is_empty();

        // Every cell is painted from its own background first, so a row that
        // only lost a style still repaints correctly. Painting only the runs
        // would leave the previous frame's pixels wherever a run used to be.
        if self.pixels.len() != (self.width * self.height * 4) as usize {
            self.pixels = vec![0; (self.width * self.height * 4) as usize];
        }

        let rows = frame.rows;
        let mut dirty = Vec::new();
        self.row_hashes.resize(rows as usize, 0);

        // A full frame repaints every row band, but a band is exactly `cell.height`
        // tall and the buffer is the whole pane: `cols * cell.width` and
        // `rows * cell.height` rarely fill it, and the strip left over at the
        // bottom is the pane's own background rather than an unpainted hole. The
        // right-hand strip needs no handling here — each row's background already
        // spans the full width.
        if full {
            let painted = rows as f32 * frame.cell.height.max(1.0);
            if painted < self.height as f32 {
                self.fill_rect(
                    0.0,
                    painted,
                    self.width as f32,
                    self.height as f32 - painted,
                    frame.palette.background,
                );
            }
        }

        for row in 0..rows {
            let hash = hash_line(frame, row);
            if full || self.row_hashes.get(row as usize) != Some(&hash) {
                self.paint_row(frame, row, glyphs);
                dirty.push(row as u32);
                if let Some(slot) = self.row_hashes.get_mut(row as usize) {
                    *slot = hash;
                }
            }
        }

        // The cursor is composited last and always, because it moves without the
        // grid changing.
        if frame.cursor.visible && self.cursor_blink_on {
            self.paint_cursor(frame);
            if !dirty.contains(&(frame.cursor.row as u32)) {
                dirty.push(frame.cursor.row as u32);
            }
        }

        self.previous_palette = Some(frame.palette);
        dirty.sort_unstable();
        dirty.dedup();
        self.last_dirty = dirty.clone();
        RasterOutput { width: self.width, height: self.height, dirty_rows: dirty, full }
    }

    /// Paint one row: background per cell, then glyphs.
    fn paint_row(&mut self, frame: &GridFrame, row: u16, glyphs: &impl GlyphSource) {
        self.fill_background(frame, row);

        let Some(line) = frame.lines.get(row as usize) else { return };
        // The frame carries no font size, so the cell height stands in for it.
        // [`SwashGlyphs`](crate::swash_source::SwashGlyphs) ignores the argument —
        // it holds the size it was built with — so this only decides the shape of
        // a synthetic source, which is what a test asserts against.
        let size_px = frame.cell.height.max(1.0);

        let mut col = 0u16;
        for run in &line.runs {
            for ch in run.text.chars() {
                if col >= frame.cols {
                    return;
                }
                let width = char_width(ch);
                self.blit(frame, col, row, ch, run, glyphs, size_px);
                col = col.saturating_add(width);
            }
        }
    }

    /// Fill a row's cells with their own backgrounds.
    ///
    /// Runs, not cells, because a row is almost always one background across its
    /// whole width — this is the same reason the frame carries runs.
    fn fill_background(&mut self, frame: &GridFrame, row: u16) {
        let cell_w = frame.cell.width.max(1.0);
        let cell_h = frame.cell.height.max(1.0);
        // The default background covers everything the runs do not.
        let y = row as f32 * cell_h;
        self.fill_rect(0.0, y, self.width as f32, cell_h, frame.palette.background);

        let Some(line) = frame.lines.get(row as usize) else { return };
        let mut col = 0usize;
        for run in &line.runs {
            for ch in run.text.chars() {
                let bg = run.style.bg;
                let w = char_width(ch) as f32;
                self.fill_rect(
                    col as f32 * cell_w,
                    y,
                    w * cell_w,
                    cell_h,
                    bg,
                );
                col += char_width(ch) as usize;
            }
        }
    }

    /// Blend one glyph over the cell.
    #[allow(clippy::too_many_arguments)]
    fn blit(
        &mut self,
        frame: &GridFrame,
        col: u16,
        row: u16,
        ch: char,
        run: &Run,
        glyphs: &impl GlyphSource,
        size_px: f32,
    ) {
        if run.style.flags & Flags::HIDDEN.0 != 0 {
            // Concealed text: the background is already painted, so there is
            // nothing to do — which is exactly what hiding means.
            return;
        }
        let bitmap = glyphs.glyph(ch, size_px);
        if bitmap.is_blank() {
            return;
        }

        let cell_w = frame.cell.width.max(1.0);
        let cell_h = frame.cell.height.max(1.0);
        let cell_x = col as f32 * cell_w;
        let cell_y = row as f32 * cell_h;
        // `placement.left` is an offset from the pen, so it is added to the cell's
        // left edge — and `placement.top` is *not* the mirror of that. swash
        // rasterises with `zeno`'s `Origin::BottomLeft`, so `top` is measured from
        // the **baseline upward** and the bitmap's top edge is `baseline - top`.
        //
        // Adding `top` to the cell's top edge instead — which is what this did —
        // drops every glyph by `top - (baseline - top)`. Measured on the shipped
        // Courier New at 13 px: a capital `M` (8 px tall, `top` 8) belongs at y
        // 2.8..10.8 in a 14.7 px cell, and was drawn at 8..16 — bottom cut off by
        // the row band, descenders cut off entirely, every line sitting low.
        let origin_x = cell_x + bitmap.left as f32;
        let origin_y = cell_y + frame.cell.baseline - bitmap.top as f32;

        let fg = run.style.fg;

        // Clip vertically to this row's band. The asymmetry with the horizontal
        // axis is deliberate and was forced by measurement:
        //
        // * **Horizontally**, a glyph may overhang its cell — that is what an
        //   italic `f` does — because a whole row is composited as a unit, so
        //   every column in it is repainted together and the overhang cannot
        //   outlive its row.
        // * **Vertically**, it must not, because rows are repainted
        //   independently. A glyph overhanging into the row above leaves a pixel
        //   there; if that neighbour is not repainted, the pixel survives as the
        //   ghost of the previous text. Measured at pixel (16,14): a one-row
        //   change diverged from a from-scratch rasterisation.
        //
        // The alternative — marking neighbours dirty — was tried and is wrong: a
        // row's repaint *erases* its neighbour's overhang, so the marking
        // cascades and ends up repainting everything below the change.
        //
        // A glyph taller than its cell is therefore cut at the cell edge. That is
        // the correct trade for a monospaced grid: `CellMetrics` exists to make
        // the cell fit the font, so a glyph that does not fit is a measurement
        // error, and cutting it at the cell boundary keeps the row arithmetic
        // honest.
        let band_top = cell_y.max(0.0) as u32;
        let band_bottom = ((cell_y + cell_h) as u32).min(self.height);

        for gy in 0..bitmap.height {
            let py = origin_y + gy as f32;
            if py < 0.0 {
                continue;
            }
            let py = py as u32;
            if py >= self.height {
                break;
            }
            if py < band_top || py >= band_bottom {
                continue;
            }
            for gx in 0..bitmap.width {
                let px = origin_x + gx as f32;
                if px < 0.0 {
                    continue;
                }
                let px = px as u32;
                if px >= self.width {
                    break;
                }
                let coverage = bitmap.at(gx, gy);
                if coverage == 0 {
                    continue;
                }
                let dst = ((py * self.width + px) * 4) as usize;
                blend(&mut self.pixels, dst, fg, coverage);
            }
        }
    }

    fn paint_cursor(&mut self, frame: &GridFrame) {
        let cell_w = frame.cell.width.max(1.0);
        let cell_h = frame.cell.height.max(1.0);
        let x = frame.cursor.col as f32 * cell_w;
        let y = frame.cursor.row as f32 * cell_h;
        let colour = frame.palette.cursor;
        match self.cursor_shape {
            // A block cursor is drawn *under* nothing — it inverts the cell, but
            // compositing over the glyph would erase it, so it is drawn as a
            // translucent wash that leaves the text legible. That is what the
            // shipped themes look like; a hard invert needs the glyph's alpha,
            // which this pass has already thrown away.
            CursorShape::Block => {
                let x0 = x.max(0.0) as u32;
                let y0 = y.max(0.0) as u32;
                let x1 = ((x + cell_w) as u32).min(self.width);
                let y1 = ((y + cell_h) as u32).min(self.height);
                for py in y0..y1 {
                    for px in x0..x1 {
                        let dst = ((py * self.width + px) * 4) as usize;
                        blend(&mut self.pixels, dst, colour, 128);
                    }
                }
            }
            // An outlined cursor does not obscure the character under it, which
            // is why a bar or an underline is preferred for insert mode.
            CursorShape::Bar => {
                let width = (cell_w / 6.0).max(1.0);
                self.fill_rect(x, y, width, cell_h, colour);
            }
            CursorShape::Underline => {
                let height = (cell_h / 8.0).max(1.0);
                self.fill_rect(x, y + cell_h - height, cell_w, height, colour);
            }
        }
    }

    /// Fill a rectangle, clipped to the buffer.
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32, colour: Rgba) {
        let x0 = x.max(0.0) as u32;
        let y0 = y.max(0.0) as u32;
        let x1 = ((x + w).min(self.width as f32).max(0.0)) as u32;
        let y1 = ((y + h).min(self.height as f32).max(0.0)) as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                let dst = ((py * self.width + px) * 4) as usize;
                if dst + 4 <= self.pixels.len() {
                    self.pixels[dst..dst + 4].copy_from_slice(&colour.to_be_bytes());
                }
            }
        }
    }

    /// The rows the last `rasterize` repainted. A measuring instrument, so a
    /// divergence can be reported against what was and was not redrawn.
    pub fn last_dirty(&self) -> &[u32] {
        &self.last_dirty
    }

    /// Read one pixel, as `0xRRGGBBAA`. For tests and for saving a screenshot.
    pub fn pixel(&self, x: u32, y: u32) -> Rgba {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let i = ((y * self.width + x) * 4) as usize;
        u32::from_be_bytes([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }
}

/// Source-over blend of `coverage`/255 of `src` onto the pixel at `dst`.
fn blend(pixels: &mut [u8], dst: usize, src: Rgba, coverage: u8) {
    if dst + 4 > pixels.len() {
        return;
    }
    let a = coverage as u32;
    let sr = (src >> 24) & 0xFF;
    let sg = (src >> 16) & 0xFF;
    let sb = (src >> 8) & 0xFF;
    for (i, s) in [sr, sg, sb].into_iter().enumerate() {
        let d = pixels[dst + i] as u32;
        // Integer maths on purpose: a float round trip here would cost more than
        // it buys, and the eight-bit result is what is written either way.
        let out = (s * a + d * (255 - a) + 127) / 255;
        pixels[dst + i] = out.min(255) as u8;
    }
    // The destination is opaque; the compositor never produces transparency,
    // because the window material handles that, not the pixels.
    pixels[dst + 3] = 0xFF;
}

/// How many cells a character occupies.
fn char_width(ch: char) -> u16 {
    // Only the two cases the grid itself distinguishes. The engine decides
    // width when it writes a cell and marks the spacer; a `char` here has already
    // been stripped of that, so the ranges that matter are the wide blocks.
    let c = ch as u32;
    let wide = (0x1100..=0x115F).contains(&c)      // Hangul Jamo
        || (0x2E80..=0xA4CF).contains(&c)          // CJK radicals through Yi
        || (0xAC00..=0xD7A3).contains(&c)          // Hangul syllables
        || (0xF900..=0xFAFF).contains(&c)          // CJK compatibility ideographs
        || (0xFE30..=0xFE6F).contains(&c)          // CJK compatibility forms
        || (0xFF00..=0xFF60).contains(&c)          // fullwidth forms
        || (0xFFE0..=0xFFE6).contains(&c)
        || (0x1F300..=0x1FAFF).contains(&c)        // emoji
        || (0x20000..=0x3FFFD).contains(&c); // CJK extensions
    if wide {
        2
    } else {
        1
    }
}

/// A stable hash of one row's worth of the frame, for the dirty test.
fn hash_line(frame: &GridFrame, row: u16) -> u64 {
    // FNV-1a over the runs' text and style keys. Any change to any of them
    // changes the hash, which is all the test needs to be: a false positive
    // repaints a row unnecessarily, a false negative loses a repaint.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |byte: u8| {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    eat(row as u8);
    eat((row >> 8) as u8);
    if let Some(line) = frame.lines.get(row as usize) {
        eat(line.wrapped as u8);
        for run in &line.runs {
            let (fg, bg, flags) = run.style.key();
            for b in fg
                .to_le_bytes()
                .into_iter()
                .chain(bg.to_le_bytes())
                .chain(flags.to_le_bytes())
            {
                eat(b);
            }
            for &b in run.text.as_bytes() {
                eat(b);
            }
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{encode_line, CellStyle, Cursor, GridDiffer, Line, Palette};
    use otty_core::CellSize;
    use otty_term::{Parser, TermConfig, Terminal};

    const FG: Rgba = 0xFFFFFFFF;
    const BG: Rgba = 0x000000FF;

    fn palette() -> Palette {
        Palette { ansi: [BG; 16], foreground: FG, background: BG, cursor: 0xFF0000FF }
    }

    /// The cell the fixtures lay out on: a plausible 16 px face — an 8 px
    /// advance, 12 px above the baseline, 4 px below it. The buffers below are
    /// sized as `cols * 8` by `rows * 16`, so the grid fills them exactly and a
    /// pixel can be named without arithmetic the test does not show.
    fn cell() -> CellSize {
        CellSize { width: 8.0, height: 16.0, baseline: 12.0 }
    }

    fn empty_frame(cols: u16, rows: u16) -> GridFrame {
        GridFrame {
            cols,
            rows,
            cell: cell(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines: vec![Line::blank(BG); rows as usize],
            palette: palette(),
            revision: 0,
        }
    }

    /// A frame with one styled line, so assertions can name cells.
    fn frame_with(text: &str, cols: u16, rows: u16) -> GridFrame {
        let mut term = Terminal::new(cols, rows, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, text.as_bytes());
        let lines: Vec<Line> = term.grid().rows().map(|r| encode_line(r, &palette())).collect();
        GridFrame {
            cols: term.cols(),
            rows: term.rows(),
            cell: cell(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines,
            palette: palette(),
            revision: 0,
        }
    }

    fn glyphs() -> SyntheticGlyphs {
        SyntheticGlyphs { edge: Some(4), blanks: vec!['x'] }
    }

    #[test]
    fn a_blank_frame_is_the_background_everywhere() {
        let mut r = GridRasterizer::new(64, 32);
        let out = r.rasterize(&empty_frame(8, 2), &glyphs());
        assert!(out.full, "the first frame is always full");
        for y in 0..32 {
            for x in 0..64 {
                assert_eq!(r.pixel(x, y), BG, "({x},{y})");
            }
        }
    }

    #[test]
    fn a_glyph_paints_the_foreground_where_its_coverage_is_full() {
        let mut r = GridRasterizer::new(64, 32);
        // Column 0 gets a glyph; the cell is 8 px wide, 16 px tall.
        r.rasterize(&frame_with("A", 8, 2), &glyphs());
        // The synthetic glyph is a 4x4 square standing on the baseline, so it
        // covers x 0..3 and y 8..11 of the cell.
        assert_eq!(r.pixel(1, 8), FG, "the glyph should be foreground");
        assert_eq!(r.pixel(1, 11), FG, "its bottom row is a pixel above the baseline");
        assert_eq!(r.pixel(1, 12), BG, "the baseline row itself has no ink");
        // Right of the glyph, inside the same cell, is still background.
        assert_eq!(r.pixel(6, 8), BG, "the rest of the cell stays background");
    }

    #[test]
    fn a_cell_with_no_glyph_shows_only_its_background() {
        let mut r = GridRasterizer::new(64, 32);
        // `x` is in the synthetic blank list, so it draws nothing.
        r.rasterize(&frame_with("x", 8, 2), &glyphs());
        assert_eq!(r.pixel(1, 1), BG, "a blank glyph must paint nothing");
    }

    #[test]
    fn hidden_text_paints_its_background_and_nothing_else() {
        let mut frame = frame_with("A", 8, 2);
        // SGR 8 is conceal.
        frame.lines[0].runs[0].style.flags |= Flags::HIDDEN.0;
        let mut r = GridRasterizer::new(64, 32);
        r.rasterize(&frame, &glyphs());
        assert_eq!(r.pixel(1, 1), BG, "concealed text must not be drawn");
    }

    #[test]
    fn a_run_s_own_background_is_painted_before_its_glyph() {
        let mut frame = frame_with("A", 8, 2);
        frame.lines[0].runs[0].style.bg = 0x0000FFFF;
        let mut r = GridRasterizer::new(64, 32);
        r.rasterize(&frame, &glyphs());
        // The cell's background changed, the glyph is still foreground.
        assert_eq!(r.pixel(6, 8), 0x0000FFFF, "the run background");
        assert_eq!(r.pixel(1, 8), FG, "the glyph on top of it");
    }

    #[test]
    fn a_covered_pixel_is_the_foreground_not_a_mixture() {
        // Full coverage must land exactly on the source colour, or text would be
        // subtly dimmer than the theme asked for.
        let mut pixels = vec![0x00u8, 0x00, 0x00, 0xFF];
        blend(&mut pixels, 0, 0xFF8040FF, 255);
        assert_eq!(pixels, vec![0xFF, 0x80, 0x40, 0xFF]);
    }

    #[test]
    fn half_coverage_lands_halfway() {
        let mut pixels = vec![0x00u8, 0x00, 0x00, 0xFF];
        blend(&mut pixels, 0, 0xFFFFFFFF, 128);
        // 255 * 128 / 255 = 128.
        assert_eq!(pixels[0], 128);
        assert_eq!(pixels[3], 0xFF, "the destination stays opaque");
    }

    #[test]
    fn zero_coverage_leaves_the_pixel_alone() {
        let mut pixels = vec![0x12u8, 0x34, 0x56, 0xFF];
        blend(&mut pixels, 0, 0xFFFFFFFF, 0);
        assert_eq!(pixels, vec![0x12, 0x34, 0x56, 0xFF]);
    }

    #[test]
    fn a_glyph_that_overhangs_the_edge_is_clipped_not_dropped_whole() {
        // An italic `f` overhangs to the left; at column 0 that means pixels with
        // a negative x. The right half must still be drawn.
        let mut r = GridRasterizer::new(16, 16);
        let mut frame = frame_with("A", 2, 1);
        frame.lines[0].runs[0].style = CellStyle { fg: FG, bg: BG, flags: 0 };
        // A glyph 4 px wide starting 2 px to the left of the pen.
        struct Overhang;
        impl GlyphSource for Overhang {
            fn glyph(&self, _: char, _: f32) -> GlyphBitmap {
                GlyphBitmap { width: 4, height: 4, left: -2, top: 0, coverage: vec![255; 16] }
            }
        }
        r.rasterize(&frame, &Overhang);
        // x = 0 is the pen; the glyph covers -2..2, so 0 and 1 are drawn. `top` is
        // 0, so the glyph starts at the baseline (y 12) and runs to the cell's
        // bottom edge.
        assert_eq!(r.pixel(0, 12), FG);
        assert_eq!(r.pixel(1, 12), FG);
        assert_eq!(r.pixel(2, 12), BG, "past the glyph");
    }

    #[test]
    fn a_glyph_that_overhangs_the_bottom_is_clipped() {
        // A cell exactly as tall as the buffer, so the row band is the whole
        // buffer and the mask runs off the bottom of it.
        let mut r = GridRasterizer::new(16, 16);
        struct Tall;
        impl GlyphSource for Tall {
            fn glyph(&self, _: char, _: f32) -> GlyphBitmap {
                // `top` of 4 puts the mask's first row 4 px above the baseline —
                // y = 12 - 4 = 8 — and its 40 rows run far past the buffer.
                GlyphBitmap { width: 2, height: 40, left: 0, top: 4, coverage: vec![255; 80] }
            }
        }
        r.rasterize(&frame_with("A", 2, 1), &Tall);
        assert_eq!(r.pixel(0, 7), BG, "above the mask, still background");
        assert_eq!(r.pixel(0, 8), FG, "the first row of the mask is 4 px above the baseline");
        assert_eq!(r.pixel(0, 15), FG, "and it is cut at the buffer edge, not dropped");
        assert_eq!(r.pixel(0, 11), FG, "still ink just above the baseline");
        // And nothing panics.
    }

    #[test]
    fn only_the_changed_row_is_recomposited() {
        let mut term = Terminal::new(8, 4, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, b"one\r\ntwo\r\nthree\r\nfour");
        let frame_of = |t: &Terminal| GridFrame {
            cols: t.cols(),
            rows: t.rows(),
            cell: cell(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines: t.grid().rows().map(|r| encode_line(r, &palette())).collect(),
            palette: palette(),
            revision: 0,
        };

        let mut r = GridRasterizer::new(64, 64);
        let first = r.rasterize(&frame_of(&term), &glyphs());
        assert!(first.full);
        assert_eq!(first.dirty_rows.len(), 4, "a full frame repaints every row");

        // Change only the second line.
        parser.advance(&mut term, b"\x1b[2;1HCHANGED\x1b[K");
        let out = r.rasterize(&frame_of(&term), &glyphs());
        assert!(!out.full);
        assert_eq!(out.dirty_rows, [1], "only row 1 changed");
    }

    #[test]
    fn an_unchanged_frame_recomposites_nothing() {
        let term = Terminal::new(8, 4, TermConfig::default());
        let frame = GridFrame {
            cols: 8,
            rows: 4,
            cell: cell(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines: term.grid().rows().map(|r| encode_line(r, &palette())).collect(),
            palette: palette(),
            revision: 0,
        };
        let mut r = GridRasterizer::new(64, 64);
        let _ = r.rasterize(&frame, &glyphs());
        let out = r.rasterize(&frame, &glyphs());
        assert!(!out.full);
        assert!(out.dirty_rows.is_empty(), "{:?}", out.dirty_rows);
    }

    #[test]
    fn a_palette_change_forces_a_full_repaint() {
        let frame = empty_frame(8, 4);
        let mut r = GridRasterizer::new(64, 64);
        let _ = r.rasterize(&frame, &glyphs());
        let mut other = frame.clone();
        other.palette.ansi[1] = 0xFF00FFFF;
        let out = r.rasterize(&other, &glyphs());
        assert!(out.full, "every resolved colour may have moved");
    }

    #[test]
    fn a_block_cursor_washes_the_cell_without_erasing_the_glyph() {
        let mut frame = frame_with("A", 8, 2);
        frame.cursor = Cursor { col: 0, row: 0, visible: true, shape: CursorShape::Block, blinking: true };
        let mut r = GridRasterizer::new(64, 32);
        r.set_cursor(CursorShape::Block, true);
        r.rasterize(&frame, &glyphs());
        // The glyph is a 4x4 square on the baseline: y 8..11 in the cell.
        let p = r.pixel(1, 8);
        assert_ne!(p, FG, "the cursor must be visible over the glyph");
        assert_ne!(p, 0xFF0000FF, "the glyph must still show through");
        assert_eq!(p & 0xFF, 0xFF);
    }

    #[test]
    fn a_bar_cursor_is_thin_and_does_not_cover_the_character() {
        let mut frame = frame_with("A", 8, 2);
        frame.cursor = Cursor { col: 0, row: 0, visible: true, shape: CursorShape::Bar, blinking: true };
        let mut r = GridRasterizer::new(64, 32);
        r.set_cursor(CursorShape::Bar, true);
        r.rasterize(&frame, &glyphs());
        // The bar is `cell_w / 6` = 8 / 6 -> 1 px wide, so it covers x = 0.
        assert_eq!(r.pixel(0, 0), 0xFF0000FF, "the bar itself");
        // And it is thin: the pixel beside it is the glyph, untouched. The
        // synthetic glyph covers y = 8..11 (a 4x4 square standing on the
        // baseline), which is why this checks a row inside the glyph. The first
        // version of this test asserted the middle of the cell, where there is
        // only background — it was wrong about where the glyph was, not about the
        // cursor.
        assert_eq!(r.pixel(1, 8), FG, "the glyph is untouched beside the bar");
    }

    #[test]
    fn a_hidden_cursor_draws_nothing() {
        let mut frame = frame_with("A", 8, 2);
        frame.cursor = Cursor { col: 0, row: 0, visible: false, shape: CursorShape::Block, blinking: true };
        let mut r = GridRasterizer::new(64, 32);
        r.set_cursor(CursorShape::Block, true);
        r.rasterize(&frame, &glyphs());
        assert_eq!(r.pixel(6, 6), BG, "a hidden cursor must leave no mark");
    }

    #[test]
    fn a_blink_phase_off_draws_no_cursor() {
        let mut frame = frame_with("A", 8, 2);
        frame.cursor = Cursor { col: 6, row: 0, visible: true, shape: CursorShape::Block, blinking: true };
        let mut r = GridRasterizer::new(64, 32);
        r.set_cursor(CursorShape::Block, false);
        r.rasterize(&frame, &glyphs());
        assert_eq!(r.pixel(56, 8), BG, "the cursor is in its off phase");
    }

    #[test]
    fn the_cursor_repaints_its_row_even_when_the_grid_did_not_change() {
        let mut frame = empty_frame(8, 2);
        let mut r = GridRasterizer::new(64, 32);
        let _ = r.rasterize(&frame, &glyphs());
        frame.cursor = Cursor { col: 3, row: 1, visible: true, shape: CursorShape::Block, blinking: true };
        r.set_cursor(CursorShape::Block, true);
        let out = r.rasterize(&frame, &glyphs());
        assert!(out.dirty_rows.contains(&1), "the cursor moved into row 1");
    }

    #[test]
    fn a_resize_reallocates_and_forces_a_full_repaint() {
        let mut r = GridRasterizer::new(64, 32);
        let _ = r.rasterize(&empty_frame(8, 2), &glyphs());
        assert!(r.resize(128, 64));
        assert_eq!(r.size(), (128, 64));
        assert_eq!(r.pixels().len(), 128 * 64 * 4);
        let out = r.rasterize(&empty_frame(16, 4), &glyphs());
        assert!(out.full);
        assert!(!r.resize(128, 64), "the same size is not a resize");
    }

    #[test]
    fn a_wide_glyph_advances_two_cells() {
        // 日本語 occupies six cells; as a frame it is three wide characters.
        let frame = frame_with("日本語", 8, 2);
        assert_eq!(frame.lines[0].text(), "日本語");
        let mut r = GridRasterizer::new(64, 32);
        r.rasterize(&frame, &glyphs());
        // The second character starts at column 2, so its glyph is at x = 16, on
        // the baseline row band.
        assert_eq!(r.pixel(17, 8), FG, "the second wide glyph's cell");
    }

    #[test]
    fn a_frame_wider_than_the_buffer_is_clipped_rather_than_panicking() {
        let mut r = GridRasterizer::new(16, 16);
        // 40 columns of glyphs into a 16 px buffer.
        let frame = frame_with("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abcdefghij", 40, 2);
        let out = r.rasterize(&frame, &glyphs());
        assert_eq!(out.width, 16);
        // No panic, and the visible part drew.
        assert_eq!(r.pixel(1, 8), FG);
    }

    /// The grid is drawn on the frame's cell, not on the buffer divided by the
    /// grid.
    ///
    /// This is the regression test for the drift: a renderer that divides the
    /// buffer by the column count stretches the grid to fill it, so every column
    /// sits a fraction of a pixel further right than the engine's arithmetic says
    /// — a few pixels by the right edge, and a whole cell by the last row.
    #[test]
    fn the_cell_comes_from_the_frame_not_from_the_buffer() {
        // A buffer far wider than 8 columns of 8 px.
        let mut r = GridRasterizer::new(200, 32);
        r.rasterize(&frame_with("A", 8, 2), &glyphs());
        // The glyph is in cell 0, which is x 0..7 whatever the buffer is.
        assert_eq!(r.pixel(1, 8), FG, "cell 0 is where the cell says it is");
        assert_eq!(r.pixel(25, 8), BG, "200/8 would have put it at x = 25");
        // And the width the grid does not cover is the pane's background.
        assert_eq!(r.pixel(199, 8), BG, "past the last column");
    }

    /// A cell that does not divide the buffer leaves background, not a hole.
    #[test]
    fn a_cell_that_does_not_fill_the_buffer_leaves_the_remainder_as_background() {
        // 2 rows of 16 px = 32 px of grid in a 48 px buffer.
        let mut r = GridRasterizer::new(64, 48);
        let out = r.rasterize(&empty_frame(8, 2), &glyphs());
        assert!(out.full);
        assert_eq!(r.pixel(10, 31), BG, "inside the grid");
        assert_eq!(r.pixel(10, 32), BG, "the first row below the grid");
        assert_eq!(r.pixel(10, 47), BG, "and the last one");
        // Opaque, not the zeroed buffer a fresh allocation starts as.
        assert_eq!(r.pixel(10, 47) & 0xFF, 0xFF, "the remainder must be painted");
    }

    /// The baseline is the cell's, so a glyph sits above it by its own `top`.
    #[test]
    fn the_baseline_decides_where_a_glyph_sits() {
        let mut r = GridRasterizer::new(80, 60);
        let mut frame = frame_with("A\r\nB", 10, 3);
        // A cell 17 px tall with a 13 px baseline does not divide the 60 px
        // buffer, which is the point: the placement has to follow the cell.
        frame.cell = CellSize { width: 7.8, height: 17.0, baseline: 13.0 };
        // The glyph is a 4 px square standing on the baseline. Row 0's baseline
        // is y 13, so its ink is 9..12; row 1's is y 30, so its ink is 26..29.
        r.rasterize(&frame, &SyntheticGlyphs { edge: Some(4), blanks: Vec::new() });
        assert_eq!(r.pixel(1, 8), BG, "above row 0's glyph");
        assert_eq!(r.pixel(1, 9), FG, "row 0: starts `top` px above the baseline");
        assert_eq!(r.pixel(1, 12), FG, "row 0: the pixel above the baseline");
        assert_eq!(r.pixel(1, 13), BG, "the baseline row itself has no ink");
        assert_eq!(r.pixel(1, 26), FG, "row 1: one cell lower, 26 = 17 + 13 - 4");
        assert_eq!(r.pixel(1, 29), FG, "row 1: still above its baseline");
        assert_eq!(r.pixel(1, 30), BG, "row 1's baseline row is clean");
    }

    #[test]
    fn char_width_classifies_the_wide_blocks() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('日'), 2);
        assert_eq!(char_width('あ'), 2);
        assert_eq!(char_width('한'), 2);
        assert_eq!(char_width('🙂'), 2);
        assert_eq!(char_width('é'), 1);
    }

    #[test]
    fn the_buffer_is_rgba_and_the_alpha_is_always_opaque() {
        let mut r = GridRasterizer::new(8, 8);
        r.rasterize(&empty_frame(1, 1), &glyphs());
        assert_eq!(r.pixels().len(), 8 * 8 * 4);
        for px in r.pixels().chunks_exact(4) {
            assert_eq!(px[3], 0xFF, "the compositor must produce an opaque surface");
        }
    }

    #[test]
    fn a_diff_driven_repaint_matches_a_full_one() {
        // The property that matters: incremental and full rasterisation must
        // agree, or a redraw would show stale glyphs.
        let mut term = Terminal::new(16, 4, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, b"alpha\r\nbeta\r\ngamma\r\ndelta");
        let frame_of = |t: &Terminal| GridFrame {
            cols: t.cols(),
            rows: t.rows(),
            cell: cell(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines: t.grid().rows().map(|r| encode_line(r, &palette())).collect(),
            palette: palette(),
            revision: 0,
        };

        let mut incremental = GridRasterizer::new(128, 64);
        let mut differ = GridDiffer::new();
        let _ = incremental.rasterize(&frame_of(&term), &glyphs());

        // A sequence of edits, each applied through the framer and the differ.
        //
        // The comparison happens **inside** the loop, not only at the end. The
        // final comparison can only say that the two disagree; comparing after
        // every step names the step, and the first step that differs is the one
        // worth reading. Deriving the cause from the code failed twice, so this
        // is the measuring instrument.
        for (step, edit) in [
            &b"\x1b[2;1HXY\x1b[K"[..],
            &b"\x1b[4;1H\x1b[2K"[..],
            &b"\x1b[1;3H!\x1b[K"[..],
        ]
        .into_iter()
        .enumerate()
        {
            parser.advance(&mut term, edit);
            let f = frame_of(&term);
            let _ = differ.diff(&f);
            incremental.rasterize(&f, &glyphs());

            let mut scratch = GridRasterizer::new(128, 64);
            scratch.rasterize(&f, &glyphs());
            let (a, b) = (incremental.pixels(), scratch.pixels());
            let first = a.iter().zip(b.iter()).position(|(x, y)| x != y);
            if let Some(byte) = first {
                let pixel = byte as u32 / 4;
                let (px, py) = (pixel % 128, pixel / 128);
                let cell_h = 64 / f.rows.max(1) as u32;
                let cell_w = 128 / f.cols.max(1) as u32;
                panic!(
                    "step {step} diverged at pixel ({px},{py}), cell (col {}, row {}), \
                     cell {cell_w}x{cell_h}; row text {:?}, dirty {:?}",
                    px / cell_w,
                    py / cell_h,
                    f.lines.get((py / cell_h) as usize).map(|l| l.text()),
                    incremental.last_dirty(),
                );
            }
        }

        // The same final grid, rasterised from scratch.
        let mut full = GridRasterizer::new(128, 64);
        full.rasterize(&frame_of(&term), &glyphs());

        // Compare without printing the buffers: a full-frame `assert_eq!` on two
        // 32 KB vectors dumps them into the test log, which buries the finding it
        // is trying to report. The first differing byte is the useful part.
        let (a, b) = (incremental.pixels(), full.pixels());
        assert_eq!(a.len(), b.len(), "the two rasterisers disagree on size");
        let first_diff = a.iter().zip(b.iter()).position(|(x, y)| x != y);
        if let Some(byte) = first_diff {
            // Name the place in the coordinates that can be reasoned about: the
            // pixel, and the grid row it falls in. Printing the buffers (which
            // the first version did) buries this under 32 KB of numbers.
            let width = 128u32;
            let pixel = byte as u32 / 4;
            let (px, py) = (pixel % width, pixel / width);
            let grid_row = py * 4 / 64;
            let cell = |buf: &[u8]| -> [u8; 4] {
                let i = byte - (byte % 4);
                [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
            };
            panic!(
                "diverged at pixel ({px},{py}) = grid row {grid_row}: \
                 incremental {:?} vs full {:?}",
                cell(a),
                cell(b)
            );
        }
    }
}
