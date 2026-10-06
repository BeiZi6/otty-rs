//! The grid as a frame the shell can draw.
//!
//! This is the contract between the Rust engine and whatever paints the screen —
//! a canvas in a webview, a Slint scene, or the wgpu pass in this crate. It is
//! deliberately **serialisable and renderer-agnostic**: the engine's job is to say
//! "row 3, columns 10–14, bold, red on default", and the shell's job is to turn
//! that into pixels. Neither needs to know the other's internals.
//!
//! # Why runs, not cells
//!
//! A terminal line is almost always one style across its whole width. Sending
//! 120 cells per line means 120 objects across any IPC boundary, and 4 800 for a
//! 40-line grid, of which maybe a dozen differ. [`encode_line`] merges adjacent
//! cells that share a style into a [`Run`], which turns the common case into one
//! or two objects per line and makes a JSON frame small enough to send every
//! frame without thinking about it.
//!
//! # Why line granularity for the diff
//!
//! Terminal output rewrites lines: a prompt, a progress bar stepping, a log
//! appending. [`GridDiffer`] therefore compares line by line and sends only the
//! lines whose runs changed. Cell-granular diffs would be smaller in theory and
//! are not worth it in practice, because the unit that changes *is* the line and
//! comparing runs is exact — two lines are equal exactly when their runs are.

use serde::{Deserialize, Serialize};

use otty_core::CellSize;
use otty_term::{ColorSpec, Flags, Row};

/// A colour, `0xRRGGBBAA`.
pub type Rgba = u32;

/// The theme's colours, resolved.
///
/// `otty-render` does not depend on the theme's internals: a theme produces a
/// palette, and everything below this line only sees numbers. That keeps the
/// frame layer testable without a theme file and keeps a theme change from
/// rippling into the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    /// ANSI 0–15, in the order the terminal numbers them.
    pub ansi: [Rgba; 16],
    pub foreground: Rgba,
    pub background: Rgba,
    /// The colour a cursor is drawn in when the cell under it is default.
    pub cursor: Rgba,
}

impl Default for Palette {
    /// The xterm defaults, so a frame can be built with no theme at all.
    fn default() -> Self {
        Self {
            ansi: [
                0x000000FF, 0xCD0000FF, 0x00CD00FF, 0xCDCD00FF, 0x0000EEFF, 0xCD00CDFF,
                0x00CDCDFF, 0xE5E5E5FF, 0x7F7F7FFF, 0xFF0000FF, 0x00FF00FF, 0xFFFF00FF,
                0x5C5CFFFF, 0xFF00FFFF, 0x00FFFFFF, 0xFFFFFFFF,
            ],
            foreground: 0xD0D0D0FF,
            background: 0x101010FF,
            cursor: 0xD0D0D0FF,
        }
    }
}

impl Palette {
    /// Resolve a cell's colour specification.
    ///
    /// An index below 16 is a palette slot, which is the whole point of the ANSI
    /// palette: a theme changes slot 1 and every `SGR 31` on screen follows. An
    /// index of 16 or more is the 256-colour cube and is computed, not looked up.
    pub fn resolve(&self, spec: ColorSpec, default: Rgba) -> Rgba {
        match spec {
            ColorSpec::Default => default,
            // 0-15 are the theme's own slots.
            ColorSpec::Indexed(i) if i < 16 => self.ansi[i as usize],
            // 16-231 are a 6x6x6 cube, 232-255 a 24-step grey ramp. These are
            // fixed by the standard, not by the theme.
            ColorSpec::Indexed(i) => xterm_256(i),
            ColorSpec::Rgb(r, g, b) => pack(r, g, b, 0xFF),
        }
    }
}

/// `SGR 38;5;<n>` above 15: the fixed 6x6x6 cube and the grey ramp.
pub fn xterm_256(index: u8) -> Rgba {
    match index {
        16..=231 => {
            let i = index as u32 - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            let r = steps[(i / 36) as usize];
            let g = steps[((i / 6) % 6) as usize];
            let b = steps[(i % 6) as usize];
            pack(r, g, b, 0xFF)
        }
        232..=255 => {
            let level = 8 + (index as u32 - 232) * 10;
            let v = level.min(255) as u8;
            pack(v, v, v, 0xFF)
        }
        // 0-15 are the theme's slots; reaching here means the caller had no
        // palette, and the xterm defaults are the honest fallback.
        _ => Palette::default().ansi[index as usize],
    }
}

const fn pack(r: u8, g: u8, b: u8, a: u8) -> Rgba {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32
}

/// One cell's resolved style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellStyle {
    pub fg: Rgba,
    pub bg: Rgba,
    /// The engine's style bits, carried through rather than flattened — the
    /// shell decides whether `UNDERCURL` is a wavy line or a straight one, and a
    /// webview can do it with CSS that wgpu would have to draw.
    pub flags: u16,
}

impl CellStyle {
    /// Whether the foreground and background have been swapped.
    pub const fn inverse(&self) -> bool {
        self.flags & Flags::INVERSE.0 != 0
    }

    /// Whether the text is not to be drawn at all.
    pub const fn hidden(&self) -> bool {
        self.flags & Flags::HIDDEN.0 != 0
    }

    /// Serialise for a diff, where the bits have to compare exactly.
    pub fn key(&self) -> (Rgba, Rgba, u16) {
        (self.fg, self.bg, self.flags)
    }
}

/// A stretch of a line that shares one style.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    pub style: CellStyle,
}

/// One row of the frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub runs: Vec<Run>,
    /// The row continues onto the next one, so the shell must not treat it as a
    /// paragraph break when it reflows or copies.
    pub wrapped: bool,
}

impl Line {
    /// An empty line on `background`.
    pub fn blank(background: Rgba) -> Self {
        Self {
            runs: Vec::new(),
            wrapped: false,
        }
        .on(background)
    }

    fn on(mut self, _background: Rgba) -> Self {
        // A blank line carries no runs at all rather than one run of spaces: the
        // shell clears to the background itself, and a run of spaces would be
        // drawn as glyphs.
        self.runs.clear();
        self
    }

    /// The line's text, with each run's styling ignored.
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    pub fn is_blank(&self) -> bool {
        self.runs.iter().all(|r| r.text.trim().is_empty())
    }
}

/// Where the cursor is, and what it looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub col: u16,
    pub row: u16,
    pub visible: bool,
    pub shape: CursorShape,
    /// Whether the cursor should blink.
    pub blinking: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self { col: 0, row: 0, visible: true, shape: CursorShape::Block, blinking: true }
    }
}

/// The cursor's shape, as `DECSCUSR` selects it.
///
/// An enum rather than a string: this crosses a process boundary, and a shell
/// that received `"bar"` would be trusting a spelling. An enum is also what
/// makes an unknown shape impossible to construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

impl CursorShape {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Bar => "bar",
            Self::Underline => "underline",
        }
    }
}

/// Everything the shell needs to paint one frame.
///
/// Carries its own [`CellSize`] because the grid cannot be drawn without it. The
/// cell is the *font's* measurement — `cell.width` is what the engine counted
/// columns with — so a renderer that derives the cell from the buffer instead
/// (`width / cols`) draws a grid whose glyphs drift away from the cells the
/// engine, the PTY and the mouse all believe in. Carrying it makes the two
/// impossible to disagree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridFrame {
    pub cols: u16,
    pub rows: u16,
    /// The cell the grid is laid out on, and the baseline inside it.
    pub cell: CellSize,
    pub cursor: Cursor,
    pub lines: Vec<Line>,
    pub palette: Palette,
    /// Increments per frame, so the shell can drop an out-of-order frame.
    pub revision: u64,
}

/// What changed since the previous frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameDiff {
    pub revision: u64,
    pub cols: u16,
    pub rows: u16,
    pub cursor: Cursor,
    /// `(row index, new line)` for every line whose runs differ.
    pub changed: Vec<(u16, Line)>,
    /// Set when the shell must replace the whole frame: the first frame, or a
    /// resize, or a palette change. The alternative — sending 40 unchanged lines
    /// because the grid got one column wider — is what this flag avoids.
    pub full: bool,
    pub palette: Palette,
}

impl FrameDiff {
    /// Whether there is nothing to do.
    pub fn is_empty(&self) -> bool {
        !self.full && self.changed.is_empty()
    }
}

/// Turn one row into runs.
///
/// A row's trailing blanks are dropped, and a wide glyph's spacer cell is
/// skipped — the spacer exists so the grid's arithmetic works, and emitting it
/// would put a stray space between every CJK character.
pub fn encode_line(row: &Row, palette: &Palette) -> Line {
    let end = row.cells.iter().rposition(|c| !c.is_blank()).map_or(0, |i| i + 1);
    let mut runs: Vec<Run> = Vec::new();

    for cell in &row.cells[..end] {
        // The engine owns this test. Reimplementing it with a bit constant is
        // how the first version of this function got it wrong: it used 1 << 13,
        // which is PROTECTED, so the spacer was emitted and `日本語` came out as
        // `日 本 語`.
        if cell.ch == '\0' || cell.is_wide_spacer() {
            continue;
        }
        let style = style_of(cell, palette);
        match runs.last_mut() {
            // Merging on the exact style key is what keeps a plain line to one
            // run and a syntax-highlighted one to a handful.
            Some(last) if last.style.key() == style.key() => last.text.push(cell.ch),
            _ => runs.push(Run { text: cell.ch.to_string(), style }),
        }
    }

    Line { runs, wrapped: row.wrapped }
}

/// Resolve a cell's style.
pub fn style_of(cell: &otty_term::Cell, palette: &Palette) -> CellStyle {
    // `INVERSE` swaps the two colours here rather than being reported, because
    // every shell would otherwise have to implement the swap, and a shell that
    // forgot would render reverse video as normal text.
    let (mut fg, mut bg) = (
        palette.resolve(cell.fg, palette.foreground),
        palette.resolve(cell.bg, palette.background),
    );
    if cell.flags.0 & Flags::INVERSE.0 != 0 {
        std::mem::swap(&mut fg, &mut bg);
    }
    CellStyle { fg, bg, flags: cell.flags.0 }
}

/// Builds frames and their diffs.
#[derive(Debug)]
pub struct GridDiffer {
    previous: Vec<Line>,
    previous_palette: Option<Palette>,
    previous_size: (u16, u16),
    revision: u64,
}

impl Default for GridDiffer {
    fn default() -> Self {
        Self::new()
    }
}

impl GridDiffer {
    pub fn new() -> Self {
        Self { previous: Vec::new(), previous_palette: None, previous_size: (0, 0), revision: 0 }
    }

    /// The next diff for `frame`.
    ///
    /// The first call is always `full`, because there is nothing to diff against;
    /// so is any call after a resize or a palette change, because in both cases
    /// every line's meaning may have shifted even where its runs compare equal.
    pub fn diff(&mut self, frame: &GridFrame) -> FrameDiff {
        self.revision = self.revision.wrapping_add(1);
        let size_changed = self.previous_size != (frame.cols, frame.rows);
        let palette_changed = self.previous_palette != Some(frame.palette);
        let full = self.previous.is_empty() || size_changed || palette_changed;

        let changed: Vec<(u16, Line)> = if full {
            frame.lines.iter().cloned().enumerate().map(|(i, l)| (i as u16, l)).collect()
        } else {
            frame
                .lines
                .iter()
                .enumerate()
                .filter(|(i, line)| self.previous.get(*i) != Some(*line))
                .map(|(i, l)| (i as u16, l.clone()))
                .collect()
        };

        self.previous = frame.lines.clone();
        self.previous_palette = Some(frame.palette);
        self.previous_size = (frame.cols, frame.rows);

        FrameDiff {
            revision: self.revision,
            cols: frame.cols,
            rows: frame.rows,
            cursor: frame.cursor,
            changed,
            full,
            palette: frame.palette,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_term::{Cell, Parser, TermConfig, Terminal};

    fn palette() -> Palette {
        Palette::default()
    }

    /// Feed escape sequences through the real engine and take the grid.
    fn term_with(script: &str, cols: u16, rows: u16) -> Terminal {
        let mut term = Terminal::new(cols, rows, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, script.as_bytes());
        term
    }

    fn lines_of(term: &Terminal, palette: &Palette) -> Vec<Line> {
        term.grid().rows().map(|r| encode_line(r, palette)).collect()
    }

    fn frame_of(term: &Terminal, palette: &Palette) -> GridFrame {
        let lines = lines_of(term, palette);
        GridFrame {
            cols: term.cols(),
            rows: term.rows(),
            cell: CellSize::default(),
            cursor: Cursor::default(),
            lines,
            palette: *palette,
            revision: 0,
        }
    }

    #[test]
    fn a_plain_line_becomes_one_run() {
        let term = term_with("hello", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert_eq!(l.runs.len(), 1, "a single-style line must be one run");
        assert_eq!(l.text(), "hello");
    }

    #[test]
    fn a_style_change_splits_the_run() {
        let term = term_with("plain\x1b[1mbold", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert_eq!(l.runs.len(), 2, "{:?}", l.runs);
        assert_eq!(l.runs[0].text, "plain");
        assert_eq!(l.runs[1].text, "bold");
        assert_ne!(l.runs[0].style.flags, l.runs[1].style.flags);
        assert!(l.runs[1].style.flags & Flags::BOLD.0 != 0);
    }

    #[test]
    fn a_colour_change_splits_the_run_even_when_the_attributes_match() {
        let term = term_with("\x1b[31mred\x1b[32mgreen", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert_eq!(l.runs.len(), 2);
        assert_ne!(l.runs[0].style.fg, l.runs[1].style.fg);
        // Slot 1 and slot 2 of the default palette, resolved by the theme.
        assert_eq!(l.runs[0].style.fg, palette().ansi[1]);
        assert_eq!(l.runs[1].style.fg, palette().ansi[2]);
    }

    #[test]
    fn trailing_blanks_are_not_emitted() {
        let term = term_with("hi", 40, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert_eq!(l.text(), "hi", "the trailing blanks became characters");
    }

    #[test]
    fn a_wide_glyph_does_not_emit_its_spacer() {
        // A CJK character occupies two cells; the second is a spacer. Emitting it
        // would put a space between every character.
        let term = term_with("日本語", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert_eq!(l.text(), "日本語", "{:?}", l.runs);
    }

    #[test]
    fn an_empty_line_has_no_runs_at_all() {
        let term = term_with("", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert!(l.runs.is_empty(), "{:?}", l.runs);
        assert!(l.is_blank());
    }

    #[test]
    fn inverse_swaps_the_colours_rather_than_being_reported() {
        let term = term_with("x\x1b[7my", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        let plain = &l.runs[0].style;
        let rev = &l.runs[1].style;
        assert_eq!(rev.fg, plain.bg, "the reverse-video foreground is the background");
        assert_eq!(rev.bg, plain.fg);
        assert!(rev.inverse(), "the flag is still carried for the shell to see");
    }

    #[test]
    fn hidden_text_is_marked_so_the_shell_can_skip_it() {
        let term = term_with("a\x1b[8mb", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert!(l.runs[1].style.hidden());
        assert!(!l.runs[0].style.hidden());
    }

    #[test]
    fn the_256_colour_cube_is_computed_not_looked_up() {
        // Index 16 is the first cube entry: 0,0,0.
        assert_eq!(xterm_256(16), 0x000000FF);
        // 231 is the last: 255,255,255.
        assert_eq!(xterm_256(231), 0xFFFFFFFF);
        // 196 is the cube's pure red: 255, 0, 0.
        assert_eq!(xterm_256(196), 0xFF0000FF);
        // The grey ramp starts at 8 and steps by 10.
        assert_eq!(xterm_256(232), 0x080808FF);
        assert_eq!(xterm_256(255), 0xEEEEEEFF);
    }

    #[test]
    fn a_true_colour_sgr_lands_exactly() {
        let term = term_with("\x1b[38;2;18;52;86mx", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert_eq!(l.runs[0].style.fg, pack(18, 52, 86, 0xFF));
    }

    #[test]
    fn the_palette_resolves_an_indexed_colour_through_the_theme_slots() {
        let mut p = palette();
        p.ansi[1] = 0x112233FF;
        assert_eq!(p.resolve(ColorSpec::Indexed(1), p.foreground), 0x112233FF);
        // An index above 15 is not a slot, so a theme change must not move it.
        p.ansi[1] = 0x445566FF;
        assert_eq!(p.resolve(ColorSpec::Indexed(196), p.foreground), 0xFF0000FF);
        assert_eq!(p.resolve(ColorSpec::Default, p.foreground), p.foreground);
    }

    #[test]
    fn a_wrapped_row_is_marked_so_the_shell_can_reflow_it() {
        // 10 columns, 2 rows: wrapping after exactly 10 characters.
        let term = term_with("0123456789abc", 10, 3);
        let lines = lines_of(&term, &palette());
        assert!(lines[0].wrapped, "the first row must be marked as continuing");
        assert!(!lines[1].wrapped);
    }

    #[test]
    fn the_first_diff_is_always_full() {
        let term = term_with("hello", 20, 3);
        let mut d = GridDiffer::new();
        let diff = d.diff(&frame_of(&term, &palette()));
        assert!(diff.full, "there is nothing to diff against");
        assert_eq!(diff.changed.len(), 3, "every row of a full frame is sent");
    }

    #[test]
    fn a_second_identical_frame_produces_an_empty_diff() {
        let term = term_with("hello", 20, 3);
        let mut d = GridDiffer::new();
        let _ = d.diff(&frame_of(&term, &palette()));
        let diff = d.diff(&frame_of(&term, &palette()));
        assert!(!diff.full);
        assert!(diff.changed.is_empty(), "{:?}", diff.changed);
        assert!(diff.is_empty(), "an unchanged frame is nothing to do");
    }

    #[test]
    fn only_the_changed_line_is_sent() {
        let mut term = term_with("one\r\ntwo\r\nthree", 20, 4);
        let mut d = GridDiffer::new();
        let _ = d.diff(&frame_of(&term, &palette()));

        // Overwrite the middle line only.
        let mut parser = Parser::new();
        parser.advance(&mut term, b"\x1b[2;1HCHANGED\x1b[K");
        let diff = d.diff(&frame_of(&term, &palette()));
        assert!(!diff.full);
        assert_eq!(diff.changed.len(), 1, "sent {:?}", diff.changed);
        assert_eq!(diff.changed[0].0, 1, "and it is the row that changed");
        assert!(diff.changed[0].1.text().starts_with("CHANGED"));
    }

    #[test]
    fn clearing_a_line_is_a_change_not_an_absence() {
        let mut term = term_with("gone\r\nkept", 20, 4);
        let mut d = GridDiffer::new();
        let _ = d.diff(&frame_of(&term, &palette()));
        let mut parser = Parser::new();
        parser.advance(&mut term, b"\x1b[1;1H\x1b[2K");
        let diff = d.diff(&frame_of(&term, &palette()));
        assert_eq!(diff.changed.len(), 1);
        assert!(diff.changed[0].1.runs.is_empty(), "the line is now empty");
    }

    #[test]
    fn a_resize_forces_a_full_frame_even_when_every_line_matches() {
        let term = term_with("x", 20, 3);
        let mut d = GridDiffer::new();
        let _ = d.diff(&frame_of(&term, &palette()));
        let other = term_with("x", 40, 3);
        let diff = d.diff(&frame_of(&other, &palette()));
        assert!(diff.full, "a widened grid needs every line resent");
        assert_eq!(diff.cols, 40);
    }

    #[test]
    fn a_palette_change_forces_a_full_frame() {
        let term = term_with("hello", 20, 3);
        let mut d = GridDiffer::new();
        let _ = d.diff(&frame_of(&term, &palette()));

        let mut other = palette();
        other.ansi[3] = 0xABCDEFFF;
        let mut f = frame_of(&term, &other);
        f.palette = other;
        let diff = d.diff(&f);
        assert!(diff.full, "every resolved colour may have moved");
        // The new palette travels with the frame, so the shell does not need a
        // separate channel for it.
        assert_eq!(diff.palette.ansi[3], 0xABCDEFFF);
    }

    #[test]
    fn the_revision_increments_once_per_diff() {
        let term = term_with("x", 20, 3);
        let mut d = GridDiffer::new();
        assert_eq!(d.diff(&frame_of(&term, &palette())).revision, 1);
        assert_eq!(d.diff(&frame_of(&term, &palette())).revision, 2);
        assert_eq!(d.revision(), 2);
    }

    #[test]
    fn the_cursor_travels_with_every_diff_including_an_empty_one() {
        let term = term_with("ab", 20, 3);
        let mut f = frame_of(&term, &palette());
        f.cursor = Cursor { col: 2, row: 0, visible: true, shape: CursorShape::Bar, blinking: false };
        let mut d = GridDiffer::new();
        let _ = d.diff(&f);
        // The grid is unchanged but the cursor moved: the shell still has to know,
        // so the cursor is outside the change set.
        let diff = d.diff(&f);
        assert!(diff.changed.is_empty());
        assert_eq!(diff.cursor.col, 2);
        assert_eq!(diff.cursor.shape, CursorShape::Bar);
        assert!(!diff.cursor.blinking);
    }

    /// The numbers the performance question needs, printed and asserted.
    ///
    /// Run with `--nocapture` to see them. The bounds are loose enough not to
    /// be brittle and tight enough that a regression in run merging shows up.
    #[test]
    fn the_frame_and_diff_payloads_stay_small_on_a_realistic_grid() {
        // A 120x40 grid is the common case on a laptop; 160x50 is a large one.
        let cases = [(120u16, 40u16), (160, 50)];
        for (cols, rows) in cases {
            let mut script = String::new();
            for row in 0..rows {
                // A realistic mix: long plain lines, some styled runs, and the
                // occasional wide glyph — which is what a shell actually emits.
                script.push_str(&format!(
                    "\x1b[{row};1Hbuild {row:02}: compiling otty-term v1.5.4                      \x1b[32mok\x1b[0m \x1b[1;33mwarning\x1b[0m 日本語 ✓"
                ));
            }
            let term = term_with(&script, cols, rows);
            let frame = frame_of(&term, &palette());
            let full = serde_json::to_string(&frame).unwrap().len();

            let mut d = GridDiffer::new();
            let first = d.diff(&frame);
            let unchanged = d.diff(&frame);
            let unchanged_json = serde_json::to_string(&unchanged).unwrap().len();

            // Now change one line: a progress bar stepping, or a new log line.
            let mut script2 = String::new();
            script2.push_str(&script);
            script2.push_str("\x1b[1;1HONE LINE CHANGED");
            let term2 = term_with(&script2, cols, rows);
            let one = d.diff(&frame_of(&term2, &palette()));
            let one_json = serde_json::to_string(&one).unwrap().len();

            println!(
                "{cols}x{rows}: full={full} B, unchanged diff={unchanged_json} B, \
                 one-line diff={one_json} B, lines in first diff={}",
                first.changed.len()
            );

            // A full frame must be affordable to send — not merely "small".
            assert!(full < 256 * 1024, "{cols}x{rows} full frame is {full} B");
            // An unchanged frame costs a cursor and a revision.
            assert!(unchanged_json < 512, "unchanged diff is {unchanged_json} B");
            // One changed line costs one line plus the envelope.
            assert!(one_json < 2048, "one-line diff is {one_json} B");
            assert_eq!(one.changed.len(), 1);
        }
    }

    #[test]
    fn a_frame_round_trips_through_json() {
        // This is the whole point of the type: it crosses a process boundary.
        let term = term_with("\x1b[1;31mred\x1b[0m plain 日本語", 40, 3);
        let frame = frame_of(&term, &palette());
        let json = serde_json::to_string(&frame).unwrap();
        let back: GridFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(back, frame);
        // A run-based frame stays small enough to send every frame.
        assert!(json.len() < 4096, "frame is {} bytes", json.len());
    }

    #[test]
    fn a_diff_round_trips_through_json() {
        let term = term_with("hello", 20, 3);
        let mut d = GridDiffer::new();
        let diff = d.diff(&frame_of(&term, &palette()));
        let json = serde_json::to_string(&diff).unwrap();
        let back: FrameDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back, diff);
    }

    #[test]
    fn a_blank_line_carries_no_runs_rather_than_spaces() {
        // Drawing a run of spaces would paint glyphs where the shell should
        // simply leave the background alone.
        let line = Line::blank(palette().background);
        assert!(line.runs.is_empty());
        assert!(line.is_blank());
        assert_eq!(line.text(), "");
    }

    #[test]
    fn a_line_of_only_spaces_is_blank() {
        let term = term_with("     ", 20, 3);
        let l = encode_line(&term.grid().rows().next().unwrap(), &palette());
        assert!(l.is_blank());
    }

    #[test]
    fn the_default_palette_matches_the_xterm_sixteen() {
        let p = Palette::default();
        assert_eq!(p.ansi[0], 0x000000FF, "slot 0 is black");
        assert_eq!(p.ansi[7], 0xE5E5E5FF, "slot 7 is the light grey");
        assert_eq!(p.ansi[15], 0xFFFFFFFF, "slot 15 is white");
        assert_eq!(p.ansi.len(), 16);
    }
}
