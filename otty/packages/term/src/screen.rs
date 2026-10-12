//! The terminal screen: cursor, modes, tab stops, charsets and the primary /
//! alternate buffer pair.

use crate::attrs::{Cell, Flags, Pen};
use crate::events::{Event, PromptMark};
use crate::grid::{Grid, Row, ScrollRegion};

/// Which buffer is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The normal screen, with scrollback.
    #[default]
    Primary,
    /// The full-screen application screen (DECSET 1049 / 47 / 1047).
    Alternate,
}

/// The text cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Screen row, 0-based.
    pub row: u16,
    /// Column, 0-based.
    pub col: u16,
    pub visible: bool,
    pub shape: CursorShape,
    pub blinking: bool,
    /// The next printable character wraps, because the last one filled the
    /// rightmost column. Cleared by any cursor movement.
    pub pending_wrap: bool,
    /// The column the cursor is *logically* at when it sits on a wide glyph's
    /// spacer cell.
    pub wide_spacer: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            shape: CursorShape::Block,
            blinking: true,
            pending_wrap: false,
            wide_spacer: false,
        }
    }
}

/// The cursor shape, as `DECSCUSR` and `cursor-style` set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
    /// `block_hollow` — an outlined block.
    HollowBlock,
}

/// DECSC/DECRC state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SavedCursor {
    pub row: u16,
    pub col: u16,
    pub pen: Pen,
    pub origin_mode: bool,
    pub autowrap: bool,
    pub charset: usize,
    pub pending_wrap: bool,
}

/// The DEC private modes Otty implements, plus the two ANSI ones that matter.
///
/// A mode is listed only when the product documentation marks it supported
/// (`?1 ?6 ?7 ?12 ?25 ?47 ?1000 ?1002 ?1003 ?1004 ?1005 ?1006 ?1047 ?1049 ?2004
/// ?2026 ?2031`). `?9` (X10 mouse), `?1016` (SGR-Pixel mouse), `?2027`
/// (grapheme clustering) and `?2048` (in-band resize) are documented as not
/// implemented, so they carry no mask and `DECRQM` reports them as
/// unrecognized rather than as an off mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modes {
    bits: u64,
}

impl Default for Modes {
    /// The power-on state every terminal starts in: auto-wrap on, cursor
    /// visible, everything else off. `DECSTR` (`soft_reset`) restores it.
    fn default() -> Self {
        Self { bits: Self::DECAWM | Self::DECTCEM }
    }
}

macro_rules! modes {
    ($($name:ident = $number:expr, $bit:expr, $doc:literal;)*) => {
        impl Modes {
            $(#[doc = $doc] pub const $name: u64 = 1 << $bit;)*

            /// Every mode with its DEC number and a human name, for
            /// `otty features` and the debug log.
            pub const TABLE: &'static [(u16, u64, &'static str)] = &[
                $(( $number, Self::$name, $doc ),)*
            ];
        }
    };
}

impl Modes {
    pub const fn get(&self, mask: u64) -> bool {
        self.bits & mask != 0
    }

    pub fn set(&mut self, mask: u64, on: bool) {
        if on {
            self.bits |= mask;
        } else {
            self.bits &= !mask;
        }
    }

    pub const fn bits(&self) -> u64 {
        self.bits
    }

    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Whether the mode number is one Otty implements.
    pub fn mask_for(number: u16) -> Option<u64> {
        Self::TABLE.iter().find(|(n, _, _)| *n == number).map(|(_, m, _)| *m)
    }
}

modes! {
    DECCKM   = 1,    0,  "?1 — DECCKM, application cursor keys";
    DECOM    = 6,    1,  "?6 — DECOM, origin mode";
    DECAWM   = 7,    2,  "?7 — DECAWM, auto-wrap";
    DECCOLM  = 3,    3,  "?3 — DECCOLM, 132-column mode";
    DECSCNM  = 5,    4,  "?5 — DECSCNM, reverse video";
    DECARM   = 8,    5,  "?8 — DECARM, auto-repeat keys";
    DECTCEM  = 25,   6,  "?25 — DECTCEM, show cursor";
    ALT47    = 47,   7,  "?47 — alternate screen (legacy bare swap)";
    MOUSE_VT200 = 1000, 9, "?1000 — VT200 mouse reporting";
    MOUSE_HL = 1001, 10, "?1001 — highlight mouse tracking";
    MOUSE_BTN = 1002, 11, "?1002 — button-event mouse tracking";
    MOUSE_ANY = 1003, 12, "?1003 — any-event mouse tracking";
    FOCUS    = 1004, 13, "?1004 — focus in/out events";
    MOUSE_UTF8 = 1005, 14, "?1005 — UTF-8 mouse coordinates";
    MOUSE_SGR = 1006, 15, "?1006 — SGR mouse coordinates";
    ALT1047  = 1047, 16, "?1047 — alternate screen, cleared on exit";
    ALT1049  = 1049, 17, "?1049 — alternate screen, saving the cursor";
    BRACKETED_PASTE = 2004, 18, "?2004 — bracketed paste";
    SYNCHRONIZED_OUTPUT = 2026, 19, "?2026 — synchronized output";
    CURSOR_BLINK = 12, 21, "?12 — blinking cursor";
    COLOR_SCHEME = 2031, 20, "?2031 — colour-scheme update notifications";
}

/// Whether mouse reporting is on at all, and in which flavour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTracking {
    Off,
    /// `?1000` — press and release only.
    Click,
    /// `?1002` — button events, including drags.
    Drag,
    /// `?1003` — every motion.
    Motion,
}

/// A character set Otty can designate into G0/G1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    #[default]
    Ascii,
    /// DEC Special Graphics (`ESC ( 0`), which draws the box-drawing set.
    DecSpecialGraphics,
    /// Any other designation — remembered so `SI`/`SO` round-trip, rendered
    /// as ASCII.
    Other(u8),
}

impl Charset {
    /// Map a byte through the charset, per the VT100 alternate-ROM table.
    pub fn translate(self, c: char) -> char {
        if self != Self::DecSpecialGraphics {
            return c;
        }
        match c {
            '`' => '◆',
            'a' => '▒',
            'b' => '\u{2409}',
            'c' => '\u{240c}',
            'd' => '\u{240d}',
            'e' => '\u{240a}',
            'f' => '°',
            'g' => '±',
            'h' => '\u{2424}',
            'i' => '\u{240b}',
            'j' => '┘',
            'k' => '┐',
            'l' => '┌',
            'm' => '└',
            'n' => '┼',
            'o' => '⎺',
            'p' => '⎻',
            'q' => '─',
            'r' => '⎼',
            's' => '⎽',
            't' => '├',
            'u' => '┤',
            'v' => '┴',
            'w' => '┬',
            'x' => '│',
            'y' => '≤',
            'z' => '≥',
            '{' => 'π',
            '|' => '≠',
            '}' => '£',
            '~' => '·',
            _ => c,
        }
    }

    /// Parse an `ESC ( <byte>` designation.
    pub fn from_designation(byte: u8) -> Self {
        match byte {
            b'0' => Self::DecSpecialGraphics,
            b'B' | b'A' => Self::Ascii,
            other => Self::Other(other),
        }
    }
}

/// Horizontal tab stops.
#[derive(Debug, Clone)]
pub struct TabStops {
    stops: Vec<bool>,
}

impl TabStops {
    pub fn new(cols: u16) -> Self {
        // Default tab stops are every 8 columns.
        Self { stops: (0..cols).map(|c| c % 8 == 0).collect() }
    }

    pub fn resize(&mut self, cols: u16) {
        self.stops.resize(cols as usize, false);
        for (i, s) in self.stops.iter_mut().enumerate() {
            if i % 8 == 0 {
                *s = true;
            }
        }
    }

    pub fn set(&mut self, col: u16) {
        if let Some(s) = self.stops.get_mut(col as usize) {
            *s = true;
        }
    }

    pub fn clear(&mut self, col: u16) {
        if let Some(s) = self.stops.get_mut(col as usize) {
            *s = false;
        }
    }

    pub fn clear_all(&mut self) {
        self.stops.iter_mut().for_each(|s| *s = false);
    }

    /// The next stop strictly after `col`, or the last column.
    pub fn next(&self, col: u16, cols: u16) -> u16 {
        ((col as usize + 1)..self.stops.len())
            .find(|i| self.stops[*i])
            .map_or(cols.saturating_sub(1), |i| i as u16)
    }

    /// The previous stop strictly before `col`.
    pub fn prev(&self, col: u16) -> u16 {
        (0..col as usize).rev().find(|i| self.stops[*i]).map_or(0, |i| i as u16)
    }
}

/// Everything the engine needs to know from the configuration.
#[derive(Debug, Clone)]
pub struct TermConfig {
    /// `scrollback-lines`, or the line count derived from `scrollback-limit`.
    pub scrollback_lines: usize,
    /// `dedup-repaint-scrollback`.
    pub dedup_repaint_scrollback: bool,
    /// `kitty-keyboard` — whether the Kitty keyboard protocol may be enabled.
    pub kitty_keyboard: bool,
    /// `vt-kam-allowed`.
    pub vt_kam_allowed: bool,
    /// `vt-keypad-app-allowed`.
    pub vt_keypad_app_allowed: bool,
    /// `title-report` — allow `XTWINOPS` title queries.
    pub title_report: bool,
    /// `osc-color-report-format`.
    pub osc_color_report_format: ColorReportFormat,
    /// `enquiry-response` — what to answer `ENQ` with.
    pub enquiry_response: String,
    /// `bidi`.
    pub bidi: bool,
    /// `widen-ambiguous` — the Unicode blocks to render double-width.
    pub widen_ambiguous: Vec<String>,
    /// `term` — the value exported as `$TERM`.
    pub term: String,
}

/// How `OSC 4` / `OSC 10` colour queries answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorReportFormat {
    /// `osc-color-report-format = none` — stay silent.
    None,
    EightBit,
    #[default]
    SixteenBit,
}

impl Default for TermConfig {
    fn default() -> Self {
        Self {
            scrollback_lines: 10_000,
            dedup_repaint_scrollback: true,
            kitty_keyboard: true,
            vt_kam_allowed: true,
            vt_keypad_app_allowed: true,
            title_report: false,
            osc_color_report_format: ColorReportFormat::SixteenBit,
            enquiry_response: String::new(),
            bidi: true,
            widen_ambiguous: Vec::new(),
            term: otty_core::Platform::host().default_term().to_owned(),
        }
    }
}

/// The terminal engine.
#[derive(Debug, Clone)]
pub struct Terminal {
    primary: Grid,
    alternate: Grid,
    pub mode: Mode,
    pub cursor: Cursor,
    pub pen: Pen,
    pub modes: Modes,
    pub tabs: TabStops,
    charset: [Charset; 4],
    active_charset: usize,
    saved_primary: SavedCursor,
    saved_alternate: SavedCursor,
    /// Set by `DECSET 1`; moves the cursor to the top margin.
    pub origin_mode: bool,
    /// `LNM` — `CSI 20 h` makes `LF` behave like `NEL` (CR + LF). Off by
    /// default, like every other terminal.
    pub lnm: bool,
    /// The last printed character, for combining marks.
    last_char: Option<(u16, usize)>,
    /// Events produced since the last [`Terminal::drain_events`].
    events: Vec<Event>,
    pub config: TermConfig,
    /// Bytes the engine owes the pty in reply to a query.
    replies: Vec<Vec<u8>>,
    /// `KAM` — when set, the host refuses keyboard input.
    pub keyboard_action_mode: bool,
    /// The application keypad is active (`DECKPAM`).
    pub keypad_application: bool,
    /// Implicit BiDi reorder. `CSI 8 l` (BDSM) turns it off for this pane.
    pub bidi_implicit: bool,
    /// The reverse-video wrap pad some line editors (zsh) leave behind.
    wrap_pad_pending: bool,
    /// Set while the alternate screen holds a full-screen application.
    pub synchronized_output: bool,
    /// The Kitty keyboard protocol flags currently in force, and the stack
    /// `CSI > u` / `CSI < u` maintain.
    kitty_flags: u16,
    kitty_flag_stack: Vec<u16>,
    /// The DCS payload `put` collects and the selector `hook` recorded, held
    /// here because the parser drives the terminal rather than the reverse.
    pub(crate) dcs: Vec<u8>,
    pub(crate) dcs_selector: Option<(Vec<u8>, char)>,
}

impl Terminal {
    /// A terminal of `cols` × `rows`.
    pub fn new(cols: u16, rows: u16, config: TermConfig) -> Self {
        let limit = config.scrollback_lines;
        Self {
            primary: Grid::new(cols, rows, limit),
            alternate: Grid::new(cols, rows, 0),
            mode: Mode::Primary,
            cursor: Cursor::default(),
            pen: Pen::new(),
            modes: Modes::default(),
            tabs: TabStops::new(cols),
            charset: [Charset::Ascii; 4],
            active_charset: 0,
            saved_primary: SavedCursor::default(),
            saved_alternate: SavedCursor::default(),
            origin_mode: false,
            lnm: false,
            last_char: None,
            events: Vec::new(),
            config,
            replies: Vec::new(),
            keyboard_action_mode: false,
            keypad_application: false,
            bidi_implicit: true,
            wrap_pad_pending: false,
            synchronized_output: false,
            kitty_flags: 0,
            kitty_flag_stack: Vec::new(),
            dcs: Vec::new(),
            dcs_selector: None,
        }
    }

    /// The active grid.
    pub fn grid(&self) -> &Grid {
        match self.mode {
            Mode::Primary => &self.primary,
            Mode::Alternate => &self.alternate,
        }
    }

    pub fn grid_mut(&mut self) -> &mut Grid {
        match self.mode {
            Mode::Primary => &mut self.primary,
            Mode::Alternate => &mut self.alternate,
        }
    }

    pub fn cols(&self) -> u16 {
        self.grid().cols
    }

    pub fn rows(&self) -> u16 {
        self.grid().screen_lines
    }

    /// Take the events produced since the last call.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Take the bytes the engine owes the host.
    pub fn drain_replies(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.replies)
    }

    pub(crate) fn push_event(&mut self, e: Event) {
        self.events.push(e);
    }

    pub(crate) fn reply(&mut self, bytes: impl Into<Vec<u8>>) {
        self.replies.push(bytes.into());
    }

    // ------------------------------------------------------------- printing

    /// Write one character at the cursor.
    pub fn print(&mut self, c: char) {
        if c == '\0' {
            return;
        }
        // A combining mark attaches to the previous cell.
        if is_combining(c) {
            self.combine(c);
            return;
        }

        let width = self.char_width(c);
        let cols = self.cols();

        if width == 0 {
            return;
        }

        if self.cursor.pending_wrap {
            if self.modes.get(Modes::DECAWM) {
                // The line continues on the next row, so the row being left
                // behind is a soft wrap — which is what reflow and copy use to
                // rejoin it.
                self.mark_wrapped(self.cursor.row, 0);
                self.linefeed();
                self.cursor.col = 0;
            }
            self.cursor.pending_wrap = false;
        }

        // A wide glyph that will not fit is pushed to the next line, which is
        // where `strip-cjk-wrap-pad` decides whether the pad cell is drawn.
        if width == 2 && self.cursor.col + 1 >= cols {
            if self.modes.get(Modes::DECAWM) {
                self.wrap_pad_pending = true;
                self.mark_wrapped(self.cursor.row, 1);
                self.linefeed();
                self.cursor.col = 0;
            } else {
                return;
            }
        }

        let (row, col) = self.resolve_cursor();
        let translated = self.charset[self.active_charset].translate(c);
        let cell = Cell::from_pen(translated, &self.pen);
        if let Some(r) = self.grid_mut().row_mut(row) {
            if let Some(slot) = r.get_mut(col) {
                *slot = cell;
            }
            if width == 2 {
                if let Some(next) = r.get_mut(col + 1) {
                    let mut spacer = cell;
                    spacer.ch = ' ';
                    spacer.flags.insert(Flags::WIDE_CHAR_SPACER);
                    *next = spacer;
                }
            }
        }
        self.last_char = Some((row, col as usize));

        let advance = width as u16;
        if col + advance >= cols {
            self.cursor.col = cols - 1;
            self.cursor.pending_wrap = self.modes.get(Modes::DECAWM);
        } else {
            self.cursor.col += advance;
            self.cursor.pending_wrap = false;
        }
        self.wrap_pad_pending = false;
    }

    /// The last graphic character written, for `REP`.
    pub(crate) fn last_graphic_char(&self) -> Option<char> {
        let (row, col) = self.last_char?;
        self.grid().row(row).and_then(|r| r.get(col as u16)).map(|c| c.ch)
    }

    /// Record that `row` continues on the next one, leaving `pad` columns of
    /// wrap pad at its right edge (0 for an ordinary soft wrap, 1 when the row
    /// ended because a wide glyph would not fit).
    fn mark_wrapped(&mut self, row: u16, pad: u8) {
        if let Some(r) = self.grid_mut().row_mut(row) {
            r.wrapped = true;
            r.trailing_spacer = pad;
        }
    }

    /// Attach a combining mark to the cell before the cursor.
    fn combine(&mut self, mark: char) {        let Some((row, col)) = self.last_char else {
            return;
        };
        // Walk back over a wide glyph's spacer so the mark lands on the base.
        let (row, col) = {
            let r = self.grid().row(row);
            match r.and_then(|r| r.get(col as u16)) {
                Some(c) if c.is_wide_spacer() => (row, col.saturating_sub(1)),
                _ => (row, col),
            }
        };
        if let Some(cell) = self.grid_mut().row_mut(row).and_then(|r| r.get_mut(col as u16)) {
            // A cell holds one `char`, so a mark is folded into the base glyph
            // when the pair has a precomposed form, and otherwise appended to
            // the previous mark run using the zero-width joiner.
            if let Some(composed) = compose(cell.ch, mark) {
                cell.ch = composed;
            } else {
                cell.pen_extra = cell.pen_extra.wrapping_add(1);
            }
        }
    }

    /// Display width in cells, honouring `widen-ambiguous`.
    pub fn char_width(&self, c: char) -> usize {
        use unicode_width::UnicodeWidthChar;
        let w = c.width().unwrap_or(0);
        if w == 1 && !self.config.widen_ambiguous.is_empty() && is_ambiguous(c) {
            2
        } else {
            w
        }
    }

    // --------------------------------------------------------------- cursor

    fn resolve_cursor(&self) -> (u16, u16) {
        let grid = self.grid();
        let (mut row, mut col) = (self.cursor.row, self.cursor.col);
        if self.origin_mode {
            let region = grid.scroll_region;
            row = (region.top + self.cursor.row).min(region.bottom);
        }
        row = row.min(grid.screen_lines.saturating_sub(1));
        col = col.min(grid.cols.saturating_sub(1));
        (row, col)
    }

    pub fn set_cursor(&mut self, row: u16, col: u16) {
        // The grid is only read here, so take the numbers out before the
        // cursor is mutated.
        let (screen_lines, cols, region) = {
            let grid = self.grid();
            (grid.screen_lines, grid.cols, grid.scroll_region)
        };
        let mut r = row;
        if self.origin_mode {
            r = (region.top + row).min(region.bottom);
        }
        self.cursor.row = r.min(screen_lines.saturating_sub(1));
        self.cursor.col = col.min(cols.saturating_sub(1));
        self.cursor.pending_wrap = false;
        self.last_char = None;
    }

    pub fn move_up(&mut self, n: u16) {
        let grid = self.grid();
        let top = if self.origin_mode { grid.scroll_region.top } else { 0 };
        self.cursor.row = self.cursor.row.saturating_sub(n).max(top);
        self.cursor.pending_wrap = false;
        self.cursor.wide_spacer = false;
    }

    pub fn move_down(&mut self, n: u16) {
        let grid = self.grid();
        let bottom = if self.origin_mode { grid.scroll_region.bottom } else { grid.screen_lines - 1 };
        self.cursor.row = (self.cursor.row + n).min(bottom);
        self.cursor.pending_wrap = false;
    }

    pub fn move_left(&mut self, n: u16) {
        self.cursor.col = self.cursor.col.saturating_sub(n);
        self.cursor.pending_wrap = false;
    }

    pub fn move_right(&mut self, n: u16) {
        let cols = self.cols();
        self.cursor.col = (self.cursor.col + n).min(cols.saturating_sub(1));
        self.cursor.pending_wrap = false;
    }

    pub fn carriage_return(&mut self) {
        self.cursor.col = 0;
        self.cursor.pending_wrap = false;
        self.wrap_pad_pending = false;
    }

    /// `LF`, `VT`, `FF` and `IND`: move down one row, scrolling at the margin.
    pub fn linefeed(&mut self) {
        let region = self.grid().scroll_region;
        let row = self.cursor.row;
        if row == region.bottom {
            self.grid_mut().scroll_up(1, region);
            if self.mode == Mode::Primary && self.config.dedup_repaint_scrollback {
                // Cheap: only the boundary row can be new.
            }
        } else if row < self.grid().screen_lines - 1 {
            self.cursor.row += 1;
        }
        self.cursor.pending_wrap = false;
        self.last_char = None;
    }

    /// `RI` — up one row, scrolling down at the top margin.
    pub fn reverse_index(&mut self) {
        let region = self.grid().scroll_region;
        if self.cursor.row == region.top {
            self.grid_mut().scroll_down(1, region);
        } else {
            self.cursor.row = self.cursor.row.saturating_sub(1);
        }
        self.cursor.pending_wrap = false;
    }

    /// `NEL` — CR + LF.
    pub fn next_line(&mut self) {
        self.carriage_return();
        self.linefeed();
    }

    /// `HT` — advance to the next tab stop.
    pub fn tab(&mut self) {
        let cols = self.cols();
        self.cursor.col = self.tabs.next(self.cursor.col, cols);
        self.cursor.pending_wrap = false;
    }

    /// `CHT` / `CBT`.
    pub fn tab_forward(&mut self, n: u16) {
        for _ in 0..n.max(1) {
            self.tab();
        }
    }

    pub fn tab_backward(&mut self, n: u16) {
        for _ in 0..n.max(1) {
            self.cursor.col = self.tabs.prev(self.cursor.col);
        }
        self.cursor.pending_wrap = false;
    }

    // ---------------------------------------------------------------- erasing

    /// `ED` — erase in display.
    ///
    /// `0`/`1` erase in place. `2` drops the viewport, and on the main screen
    /// scrolls it into scrollback first, which is what the ED page describes.
    /// `3` also drops scrollback, and `22` (kitty) is the explicit
    /// scroll-then-erase. DECSED (`selective`) always erases in place.
    pub fn erase_in_display(&mut self, mode: u16, selective: bool) {
        let (row, col) = self.resolve_cursor();
        let rows = self.rows();
        let cols = self.cols();
        let pen = self.pen;
        let alternate = self.mode == Mode::Alternate;
        match mode {
            0 => {
                let grid = self.grid_mut();
                if let Some(r) = grid.row_mut(row) {
                    erase_row(r, col as usize, cols as usize, &pen, selective, false);
                }
                for r in (row + 1)..rows {
                    if let Some(rw) = grid.row_mut(r) {
                        erase_row(rw, 0, cols as usize, &pen, selective, false);
                    }
                }
            }
            1 => {
                let grid = self.grid_mut();
                for r in 0..row {
                    if let Some(rw) = grid.row_mut(r) {
                        erase_row(rw, 0, cols as usize, &pen, selective, false);
                    }
                }
                if let Some(rw) = grid.row_mut(row) {
                    erase_row(rw, 0, col as usize + 1, &pen, selective, false);
                }
            }
            2 => {
                if selective {
                    let grid = self.grid_mut();
                    for r in 0..rows {
                        if let Some(rw) = grid.row_mut(r) {
                            erase_row(rw, 0, cols as usize, &pen, true, false);
                        }
                    }
                } else if alternate {
                    // The alternate screen has no history to preserve.
                    self.grid_mut().clear_screen();
                } else {
                    self.grid_mut().push_viewport_to_scrollback();
                }
            }
            3 => {
                self.grid_mut().clear_screen();
                self.grid_mut().clear_scrollback();
            }
            22 => {
                // The kitty extension: preserve the viewport in scrollback.
                if alternate {
                    self.grid_mut().clear_screen();
                } else {
                    self.grid_mut().push_viewport_to_scrollback();
                }
            }
            _ => {}
        }
        self.cursor.pending_wrap = false;
    }

    /// `EL` — erase in line.
    pub fn erase_in_line(&mut self, mode: u16, selective: bool) {
        let (row, col) = self.resolve_cursor();
        let cols = self.cols() as usize;
        let pen = self.pen;
        if let Some(r) = self.grid_mut().row_mut(row) {
            match mode {
                0 => erase_row(r, col as usize, cols, &pen, selective, false),
                1 => erase_row(r, 0, col as usize + 1, &pen, selective, false),
                2 => erase_row(r, 0, cols, &pen, selective, false),
                _ => {}
            }
        }
        self.cursor.pending_wrap = false;
    }

    /// `ECH` — erase `n` characters from the cursor.
    pub fn erase_chars(&mut self, n: u16, selective: bool) {
        let (row, col) = self.resolve_cursor();
        let cols = self.cols() as usize;
        let pen = self.pen;
        let end = (col as usize + n as usize).min(cols);
        if let Some(r) = self.grid_mut().row_mut(row) {
            erase_row(r, col as usize, end, &pen, selective, false);
        }
    }

    /// `DCH` — delete `n` cells, shifting the rest left.
    pub fn delete_chars(&mut self, n: u16) {
        let (row, col) = self.resolve_cursor();
        let cols = self.cols() as usize;
        let start = col as usize;
        let count = (n as usize).min(cols - start);
        let pen = self.pen;
        if let Some(r) = self.grid_mut().row_mut(row) {
            r.cells.drain(start..start + count);
            while r.cells.len() < cols {
                r.cells.push(Cell::from_pen(' ', &pen));
            }
        }
    }

    /// `ICH` — insert `n` blank cells at the cursor.
    pub fn insert_chars(&mut self, n: u16) {
        let (row, col) = self.resolve_cursor();
        let cols = self.cols() as usize;
        let start = col as usize;
        let count = (n as usize).min(cols - start);
        let pen = self.pen;
        if let Some(r) = self.grid_mut().row_mut(row) {
            for _ in 0..count {
                r.cells.insert(start, Cell::from_pen(' ', &pen));
            }
            r.cells.truncate(cols);
        }
    }

    /// `IL` / `DL`.
    pub fn insert_lines(&mut self, n: u16) {
        let region = self.grid().scroll_region;
        let (row, _) = self.resolve_cursor();
        if !region.contains(row) {
            return;
        }
        let region = ScrollRegion { top: row, bottom: region.bottom };
        self.grid_mut().scroll_down(n, region);
    }

    pub fn delete_lines(&mut self, n: u16) {
        let region = self.grid().scroll_region;
        let (row, _) = self.resolve_cursor();
        if !region.contains(row) {
            return;
        }
        let region = ScrollRegion { top: row, bottom: region.bottom };
        self.grid_mut().scroll_up(n, region);
    }

    /// `DECALN` — fill the screen with `E`.
    pub fn screen_alignment_display(&mut self) {
        let cols = self.cols();
        let rows = self.rows();
        let pen = self.pen;
        for r in 0..rows {
            if let Some(row) = self.grid_mut().row_mut(r) {
                for c in 0..cols {
                    if let Some(cell) = row.get_mut(c) {
                        *cell = Cell::from_pen('E', &pen);
                    }
                }
            }
        }
        self.set_cursor(0, 0);
        self.push_event(Event::ScreenAlignmentDisplay);
    }

    /// `SU` / `SD`.
    pub fn scroll_up(&mut self, n: u16) {
        let region = self.grid().scroll_region;
        self.grid_mut().scroll_up(n, region);
    }

    pub fn scroll_down(&mut self, n: u16) {
        let region = self.grid().scroll_region;
        self.grid_mut().scroll_down(n, region);
    }

    // ------------------------------------------------------------- DECSTBM

    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        let rows = self.rows();
        let top = top.max(1) - 1;
        let bottom = if bottom == 0 { rows - 1 } else { (bottom - 1).min(rows - 1) };
        if top >= bottom {
            return;
        }
        self.grid_mut().scroll_region = ScrollRegion { top, bottom };
        self.set_cursor(0, 0);
        if self.origin_mode {
            self.cursor.row = top;
        }
    }

    // -------------------------------------------------------------- buffers

    /// `DECSET`/`DECRST` 47 / 1047 / 1049.
    ///
    /// The entry point the app and the tests use: entering clears the alternate
    /// buffer, leaving keeps what it holds. The per-mode differences the DECSET
    /// page documents live in [`Terminal::switch_screen`].
    pub fn set_alternate_screen(&mut self, on: bool, save_cursor: bool) {
        self.switch_screen(on, save_cursor, on);
    }

    /// The alternate-screen swap, with the buffer handling each mode uses:
    /// `?47` is a bare swap (the alternate buffer keeps what it held), `?1047`
    /// clears it on the way out, and `?1049` saves the cursor and clears the
    /// buffer on entry.
    pub(crate) fn switch_screen(&mut self, on: bool, save_cursor: bool, clear: bool) {
        if on == (self.mode == Mode::Alternate) {
            return;
        }
        match on {
            true => {
                if save_cursor {
                    self.saved_primary = self.snapshot();
                }
                self.mode = Mode::Alternate;
                if clear {
                    self.alternate.clear_screen();
                }
                self.alternate.scroll_region = ScrollRegion::full(self.alternate.screen_lines);
                if save_cursor {
                    self.cursor = Cursor { row: 0, col: 0, ..self.cursor };
                    self.pen = Pen::new();
                }
            }
            false => {
                self.mode = Mode::Primary;
                if clear {
                    self.alternate.clear_screen();
                }
                if save_cursor {
                    let saved = self.saved_primary;
                    self.restore(saved);
                }
            }
        }
        self.cursor.pending_wrap = false;
    }

    fn snapshot(&self) -> SavedCursor {
        SavedCursor {
            row: self.cursor.row,
            col: self.cursor.col,
            pen: self.pen,
            origin_mode: self.origin_mode,
            autowrap: self.modes.get(Modes::DECAWM),
            charset: self.active_charset,
            pending_wrap: self.cursor.pending_wrap,
        }
    }

    fn restore(&mut self, s: SavedCursor) {
        self.cursor.row = s.row;
        self.cursor.col = s.col;
        self.pen = s.pen;
        self.origin_mode = s.origin_mode;
        self.modes.set(Modes::DECAWM, s.autowrap);
        self.active_charset = s.charset;
        self.cursor.pending_wrap = s.pending_wrap;
    }

    /// `DECSC`.
    pub fn save_cursor(&mut self) {
        let s = self.snapshot();
        match self.mode {
            Mode::Primary => self.saved_primary = s,
            Mode::Alternate => self.saved_alternate = s,
        }
    }

    /// `DECRC`. With nothing saved, the cursor goes home with default
    /// attributes, as the reference says.
    pub fn restore_cursor(&mut self) {
        let s = match self.mode {
            Mode::Primary => self.saved_primary,
            Mode::Alternate => self.saved_alternate,
        };
        self.restore(s);
    }

    /// `RIS` — reset to the power-on state.
    pub fn full_reset(&mut self) {
        let cols = self.cols();
        let rows = self.rows();
        let config = self.config.clone();
        let scrollback = config.scrollback_lines;
        *self = Self::new(cols, rows, config);
        self.primary = Grid::new(cols, rows, scrollback);
    }

    /// `DECSTR` — soft reset: state only, the screen is untouched.
    pub fn soft_reset(&mut self) {
        self.cursor.visible = true;
        self.cursor.shape = CursorShape::Block;
        self.cursor.blinking = true;
        self.cursor.pending_wrap = false;
        self.pen = Pen::new();
        self.origin_mode = false;
        self.lnm = false;
        self.keyboard_action_mode = false;
        self.keypad_application = false;
        self.charset = [Charset::Ascii; 4];
        self.active_charset = 0;
        self.modes.set(Modes::DECAWM, true);
        self.modes.set(Modes::DECOM, false);
        self.modes.set(Modes::DECCKM, false);
        self.modes.set(Modes::DECTCEM, true);
        self.grid_mut().scroll_region = ScrollRegion::full(self.rows());
        self.tabs = TabStops::new(self.cols());
    }

    /// `DECSCUSR` — cursor shape and blink.
    pub fn set_cursor_style(&mut self, ps: u16) {
        let (shape, blink) = match ps {
            0 => (CursorShape::Block, None),
            1 => (CursorShape::Block, Some(true)),
            2 => (CursorShape::Block, Some(false)),
            3 => (CursorShape::Underline, Some(true)),
            4 => (CursorShape::Underline, Some(false)),
            5 => (CursorShape::Bar, Some(true)),
            6 => (CursorShape::Bar, Some(false)),
            _ => return,
        };
        self.cursor.shape = shape;
        if let Some(b) = blink {
            self.cursor.blinking = b;
        }
    }

    pub fn set_charset(&mut self, index: usize, c: Charset) {
        if index < 4 {
            self.charset[index] = c;
        }
    }

    pub fn shift_in(&mut self) {
        self.active_charset = 0;
    }

    pub fn shift_out(&mut self) {
        self.active_charset = 1;
    }

    /// The Kitty keyboard protocol flags the running program enabled, or `0`
    /// when the protocol is off. Backs `kitty-keyboard`.
    pub fn kitty_flags(&self) -> u16 {
        if self.config.kitty_keyboard {
            self.kitty_flags
        } else {
            0
        }
    }

    /// `CSI > <flags> u` — push a Kitty keyboard flag set.
    pub fn push_kitty_flags(&mut self, flags: u16) {
        self.kitty_flag_stack.push(self.kitty_flags);
        self.kitty_flags = flags;
    }

    /// `CSI < <n> u` — pop `n` Kitty keyboard flag sets.
    pub fn pop_kitty_flags(&mut self, n: u16) {
        for _ in 0..n.max(1) {
            self.kitty_flags = self.kitty_flag_stack.pop().unwrap_or(0);
        }
    }

    pub fn mouse_tracking(&self) -> MouseTracking {
        if self.modes.get(Modes::MOUSE_ANY) {
            MouseTracking::Motion
        } else if self.modes.get(Modes::MOUSE_BTN) {
            MouseTracking::Drag
        } else if self.modes.get(Modes::MOUSE_VT200) || self.modes.get(Modes::MOUSE_HL) {
            MouseTracking::Click
        } else {
            MouseTracking::Off
        }
    }

    // ------------------------------------------------------------ resizing

    /// Resize, as `SIGWINCH` reports it. The alternate screen is re-laid out
    /// without reflow, matching every other terminal.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        match self.mode {
            Mode::Primary => self.cursor.row = self.primary.resize_at(cols, rows, self.cursor.row),
            Mode::Alternate => self.primary.resize(cols, rows),
        }
        self.alternate.resize(cols, rows);
        self.alternate.clear_screen();
        self.tabs.resize(cols);
        self.cursor.row = self.cursor.row.min(rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(cols.saturating_sub(1));
        if self.origin_mode {
            let region = self.grid().scroll_region;
            self.cursor.row = self.cursor.row.min(region.bottom);
        }
    }

    // --------------------------------------------------------------- output

    /// The visible text, rows joined by newlines — `get contents of t`.
    pub fn contents(&self) -> String {
        let mut out = String::new();
        for row in self.grid().rows() {
            out.push_str(&row.text());
            out.push('\n');
        }
        out.trim_end_matches('\n').to_owned()
    }

    /// Scrollback plus the visible screen — `get history of t`.
    pub fn history(&self) -> String {
        let mut out = String::new();
        for row in self.grid().all_lines() {
            out.push_str(&row.text());
            out.push('\n');
        }
        out.trim_end_matches('\n').to_owned()
    }

    /// The prompt marks the outline index is built from.
    pub fn push_prompt_mark(&mut self, mark: PromptMark) {
        // Where the prompt starts and where it ends are kept on the row, which
        // is what gives the outline a line that survives scrolling.
        if matches!(mark, PromptMark::PromptStart | PromptMark::PromptEnd) && self.mode == Mode::Primary {
            let (row, col) = (self.cursor.row, self.cursor.col);
            if let Some(r) = self.primary.row_mut(row) {
                r.mark = Some((mark, col));
            }
        }
        self.push_event(Event::PromptMark(mark));
    }

    /// The commands run in this terminal, oldest first: `(grid line, text)`.
    pub fn outline(&self) -> Vec<(usize, String)> {
        self.primary.outline()
    }
}

fn erase_row(row: &mut Row, from: usize, to: usize, pen: &Pen, selective: bool, keep_bg: bool) {
    for i in from..to.min(row.cells.len()) {
        let protected = row.cells[i].flags.contains(Flags::PROTECTED);
        if selective && !protected {
            // Selective erase only removes unprotected cells.
        } else if selective {
            continue;
        }
        if keep_bg {
            row.cells[i].clear_keeping_bg();
        } else {
            row.cells[i] = Cell::from_pen(' ', pen);
        }
    }
}

/// Combining marks that attach to the previous cell.
pub fn is_combining(c: char) -> bool {
    use unicode_width::UnicodeWidthChar;
    c.width() == Some(0) && !c.is_control()
}

/// The precomposed form of `base` + `mark`, when one exists.
fn compose(base: char, mark: char) -> Option<char> {
    // A small table covers the combinations a terminal meets most often; the
    // full Unicode composition table lives in the renderer's shaper.
    const PAIRS: &[(char, char, char)] = &[
        ('a', '\u{300}', 'à'),
        ('a', '\u{301}', 'á'),
        ('a', '\u{302}', 'â'),
        ('a', '\u{303}', 'ã'),
        ('a', '\u{308}', 'ä'),
        ('e', '\u{300}', 'è'),
        ('e', '\u{301}', 'é'),
        ('e', '\u{302}', 'ê'),
        ('e', '\u{308}', 'ë'),
        ('i', '\u{300}', 'ì'),
        ('i', '\u{301}', 'í'),
        ('i', '\u{302}', 'î'),
        ('i', '\u{308}', 'ï'),
        ('o', '\u{300}', 'ò'),
        ('o', '\u{301}', 'ó'),
        ('o', '\u{302}', 'ô'),
        ('o', '\u{303}', 'õ'),
        ('o', '\u{308}', 'ö'),
        ('u', '\u{300}', 'ù'),
        ('u', '\u{301}', 'ú'),
        ('u', '\u{302}', 'û'),
        ('u', '\u{308}', 'ü'),
        ('n', '\u{303}', 'ñ'),
        ('c', '\u{327}', 'ç'),
        ('A', '\u{300}', 'À'),
        ('A', '\u{301}', 'Á'),
        ('E', '\u{301}', 'É'),
        ('E', '\u{308}', 'Ë'),
        ('O', '\u{308}', 'Ö'),
        ('U', '\u{308}', 'Ü'),
    ];
    PAIRS.iter().find(|(b, m, _)| *b == base && *m == mark).map(|(_, _, c)| *c)
}

/// Whether a codepoint is East-Asian-Ambiguous.
pub fn is_ambiguous(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x00A1 | 0x00A4 | 0x00A7..=0x00A8 | 0x00AA | 0x00AD..=0x00AE | 0x00B0..=0x00B4
        | 0x00B6..=0x00BA | 0x00BC..=0x00BF | 0x00C6 | 0x00D0 | 0x00D7..=0x00D8
        | 0x00DE..=0x00E1 | 0x00E6 | 0x00E8..=0x00EA | 0x00EC..=0x00ED | 0x00F0
        | 0x00F2..=0x00F3 | 0x00F7..=0x00FA | 0x00FC | 0x00FE | 0x0101 | 0x0111 | 0x0113
        | 0x011B | 0x0126..=0x0127 | 0x012B | 0x0131..=0x0133 | 0x0138 | 0x013F..=0x0142
        | 0x0144 | 0x0148..=0x014B | 0x014D | 0x0152..=0x0153 | 0x0166..=0x0167 | 0x016B
        | 0x01CE | 0x01D0 | 0x01D2 | 0x01D4 | 0x01D6 | 0x01D8 | 0x01DA | 0x01DC
        | 0x0251 | 0x0261 | 0x02C4 | 0x02C7 | 0x02C9..=0x02CB | 0x02CD | 0x02D0
        | 0x02D8..=0x02DB | 0x02DD | 0x02DF | 0x0300..=0x036F | 0x0391..=0x03A1
        | 0x03A3..=0x03A9 | 0x03B1..=0x03C1 | 0x03C3..=0x03C9 | 0x0401 | 0x0410..=0x044F
        | 0x0451 | 0x2010 | 0x2013..=0x2016 | 0x2018..=0x2019 | 0x201C..=0x201D
        | 0x2020..=0x2022 | 0x2024..=0x2027 | 0x2030 | 0x2032..=0x2033 | 0x2035
        | 0x203B | 0x203E | 0x2074 | 0x207F | 0x2081..=0x2084 | 0x20AC | 0x2103
        | 0x2105 | 0x2109 | 0x2113 | 0x2116 | 0x2121..=0x2122 | 0x2126 | 0x212B
        | 0x2153..=0x2154 | 0x215B..=0x215E | 0x2160..=0x216B | 0x2170..=0x2179
        | 0x2189 | 0x2190..=0x2199 | 0x21B8..=0x21B9 | 0x21D2 | 0x21D4 | 0x21E7
        | 0x2200 | 0x2202..=0x2203 | 0x2207..=0x2208 | 0x220B | 0x220F | 0x2211
        | 0x2215 | 0x221A | 0x221D..=0x2220 | 0x2223 | 0x2225 | 0x2227..=0x222C
        | 0x222E | 0x2234..=0x2237 | 0x223C..=0x223D | 0x2248 | 0x224C | 0x2252
        | 0x2260..=0x2261 | 0x2264..=0x2267 | 0x226A..=0x226B | 0x226E..=0x226F
        | 0x2282..=0x2283 | 0x2286..=0x2287 | 0x2295 | 0x2299 | 0x22A5 | 0x22BF
        | 0x2312 | 0x2460..=0x24E9 | 0x24EB..=0x254B | 0x2550..=0x2573
        | 0x2580..=0x258F | 0x2592..=0x2595 | 0x25A0..=0x25A1 | 0x25A3..=0x25A9
        | 0x25B2..=0x25B3 | 0x25B6..=0x25B7 | 0x25BC..=0x25BD | 0x25C0..=0x25C1
        | 0x25C6..=0x25C8 | 0x25CB | 0x25CE..=0x25D1 | 0x25E2..=0x25E5 | 0x25EF
        | 0x2605..=0x2606 | 0x2609 | 0x260E..=0x260F | 0x2614..=0x2615
        | 0x261C | 0x261E | 0x2640 | 0x2642 | 0x2660..=0x2661 | 0x2663..=0x2665
        | 0x2667..=0x266A | 0x266C..=0x266D | 0x266F | 0x269E..=0x269F
        | 0x26BE..=0x26BF | 0x26C4..=0x26CD | 0x26CF..=0x26E1 | 0x26E3
        | 0x26E8..=0x26FF | 0x273D | 0x2757 | 0x2776..=0x277F | 0x2B55..=0x2B59
        | 0x3248..=0x324F | 0xE000..=0xF8FF | 0xFE00..=0xFE0F | 0xFFFD
        | 0x1F100..=0x1F10A | 0x1F110..=0x1F12D | 0x1F130..=0x1F169
        | 0x1F170..=0x1F19A)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term() -> Terminal {
        Terminal::new(10, 4, TermConfig::default())
    }

    #[test]
    fn printing_advances_and_wraps_at_the_margin() {
        let mut t = term();
        for c in "abcdefghij".chars() {
            t.print(c);
        }
        assert_eq!(t.cursor.col, 9);
        assert!(t.cursor.pending_wrap);
        assert_eq!(t.grid().row(0).unwrap().text(), "abcdefghij");
        // The eleventh character wraps; the first row keeps its content.
        t.print('k');
        assert_eq!(t.grid().row(1).unwrap().text(), "k");
        assert_eq!(t.grid().row(0).unwrap().text(), "abcdefghij");
        assert!(t.grid().row(0).unwrap().wrapped);
    }

    #[test]
    fn a_wide_glyph_at_the_margin_moves_to_the_next_row() {
        let mut t = term();
        for _ in 0..9 {
            t.print('.');
        }
        t.print('世');
        assert_eq!(t.cursor.row, 1);
        assert_eq!(t.grid().row(1).unwrap().cells[0].ch, '世');
        assert!(t.grid().row(1).unwrap().cells[1].is_wide_spacer());
    }

    #[test]
    fn linefeed_scrolls_at_the_bottom_of_the_region() {
        let mut t = Terminal::new(6, 3, TermConfig::default());
        t.set_cursor(2, 0);
        t.print('x');
        t.set_cursor(0, 0);
        t.print('a');
        t.set_cursor(2, 0);
        t.linefeed();
        assert_eq!(t.grid().scrollback_len(), 1);
        assert_eq!(t.grid().line(0).unwrap().text(), "a");
    }

    #[test]
    fn a_scroll_region_keeps_lines_out_of_scrollback() {
        let mut t = Terminal::new(6, 5, TermConfig::default());
        t.set_scroll_region(2, 4);
        t.set_cursor(3, 0);
        t.linefeed();
        assert_eq!(t.grid().scrollback_len(), 0);
        assert_eq!(t.grid().scroll_region, ScrollRegion { top: 1, bottom: 3 });
    }

    #[test]
    fn erase_in_line_honours_the_three_modes() {
        // EL 0 erases from the cursor to the end of the line, EL 1 from the
        // start of the line through the cursor, EL 2 the whole line — the
        // meanings the EL page gives, with the active cell included at both
        // ends (ECMA-48). The row is 10 columns wide and the cursor sits on
        // column 6, so EL 0 leaves "abcde" and EL 1 leaves "ghij".
        for (mode, want) in [(0u16, "abcde"), (1, "      ghij"), (2, "          ")] {
            let mut t = term();
            for c in "abcdefghij".chars() {
                t.print(c);
            }
            t.set_cursor(0, 5);
            t.erase_in_line(mode, false);
            assert_eq!(t.grid().row(0).unwrap().text(), want.trim_end(), "mode {mode}");
        }
    }

    #[test]
    fn insert_and_delete_characters_shift_the_row() {
        let mut t = term();
        for c in "abcdefghij".chars() {
            t.print(c);
        }
        t.set_cursor(0, 2);
        t.delete_chars(3);
        assert!(t.grid().row(0).unwrap().text().starts_with("abfghij"));

        let mut t = term();
        for c in "abc".chars() {
            t.print(c);
        }
        t.set_cursor(0, 1);
        t.insert_chars(2);
        assert_eq!(t.grid().row(0).unwrap().text(), "a  bc");
    }

    #[test]
    fn selective_erase_spares_protected_cells() {
        let mut t = term();
        t.pen.flags.insert(Flags::PROTECTED);
        for c in "ab".chars() {
            t.print(c);
        }
        t.pen.flags.remove(Flags::PROTECTED);
        for c in "cd".chars() {
            t.print(c);
        }
        t.set_cursor(0, 0);
        t.erase_in_line(2, true);
        // DECSEL skips protected cells...
        assert_eq!(t.grid().row(0).unwrap().cells[1].ch, 'b');
        // ...while ordinary EL erases everything.
        t.set_cursor(0, 0);
        t.erase_in_line(2, false);
        assert!(t.grid().row(0).unwrap().is_blank());
    }

    #[test]
    fn alt_screen_saves_and_restores_the_primary_buffer() {
        let mut t = term();
        t.print('m');
        assert_eq!(t.grid().row(0).unwrap().text(), "m");
        t.set_alternate_screen(true, true);
        assert_eq!(t.mode, Mode::Alternate);
        assert!(t.grid().row(0).unwrap().is_blank());
        t.print('n');
        t.set_alternate_screen(false, true);
        assert_eq!(t.mode, Mode::Primary);
        assert_eq!(t.grid().row(0).unwrap().text(), "m");
    }

    #[test]
    fn the_alternate_screen_has_no_scrollback() {
        let mut t = term();
        t.set_alternate_screen(true, false);
        t.set_cursor(3, 0);
        t.linefeed();
        assert_eq!(t.grid().scrollback_len(), 0);
    }

    #[test]
    fn decaln_fills_the_screen_with_e() {
        let mut t = term();
        t.screen_alignment_display();
        assert!(t.grid().rows().all(|r| r.cells.iter().all(|c| c.ch == 'E')));
        assert_eq!(t.cursor.row, 0);
        assert_eq!(t.cursor.col, 0);
        assert!(t.drain_events().contains(&Event::ScreenAlignmentDisplay));
    }

    #[test]
    fn decstr_keeps_the_screen_but_clears_the_state() {
        let mut t = term();
        t.print('q');
        t.modes.set(Modes::DECCKM, true);
        t.pen.apply(1);
        t.soft_reset();
        assert_eq!(t.grid().row(0).unwrap().text(), "q");
        assert!(!t.modes.get(Modes::DECCKM));
        assert!(t.pen.is_plain());
        assert!(t.cursor.visible);
    }

    #[test]
    fn ris_returns_to_the_power_on_state() {
        let mut t = term();
        t.print('q');
        t.modes.set(Modes::DECCKM, true);
        t.full_reset();
        assert!(t.grid().rows().all(Row::is_blank));
        assert!(!t.modes.get(Modes::DECCKM));
        assert_eq!(t.rows(), 4);
        assert_eq!(t.cols(), 10);
    }

    #[test]
    fn decscusr_maps_the_documented_shapes() {
        let mut t = term();
        t.set_cursor_style(1);
        assert_eq!((t.cursor.shape, t.cursor.blinking), (CursorShape::Block, true));
        t.set_cursor_style(4);
        assert_eq!((t.cursor.shape, t.cursor.blinking), (CursorShape::Underline, false));
        t.set_cursor_style(5);
        assert_eq!((t.cursor.shape, t.cursor.blinking), (CursorShape::Bar, true));
    }

    #[test]
    fn tab_stops_default_to_every_eight_columns() {
        let mut t = Terminal::new(40, 2, TermConfig::default());
        t.print('a');
        t.tab();
        assert_eq!(t.cursor.col, 8);
        t.tab();
        assert_eq!(t.cursor.col, 16);
        t.cursor.col = 20;
        t.tab_backward(1);
        assert_eq!(t.cursor.col, 16);
        t.tabs.clear_all();
        t.cursor.col = 5;
        t.tab();
        assert_eq!(t.cursor.col, 39);
    }

    #[test]
    fn dec_special_graphics_translates_the_box_drawing_set() {
        assert_eq!(Charset::DecSpecialGraphics.translate('q'), '─');
        assert_eq!(Charset::DecSpecialGraphics.translate('x'), '│');
        assert_eq!(Charset::DecSpecialGraphics.translate('n'), '┼');
        assert_eq!(Charset::Ascii.translate('q'), 'q');
        assert_eq!(Charset::from_designation(b'0'), Charset::DecSpecialGraphics);
        assert_eq!(Charset::from_designation(b'B'), Charset::Ascii);
    }

    #[test]
    fn charset_shifts_apply_to_printed_text() {
        let mut t = term();
        t.set_charset(1, Charset::DecSpecialGraphics);
        t.shift_out();
        t.print('q');
        assert_eq!(t.grid().row(0).unwrap().cells[0].ch, '─');
        t.shift_in();
        t.print('q');
        assert_eq!(t.grid().row(0).unwrap().cells[1].ch, 'q');
    }

    #[test]
    fn origin_mode_offsets_in_a_scroll_region() {
        let mut t = Terminal::new(6, 6, TermConfig::default());
        t.set_scroll_region(3, 5);
        t.origin_mode = true;
        t.set_cursor(0, 0);
        assert_eq!(t.cursor.row, 2);
        t.set_cursor(2, 0);
        assert_eq!(t.cursor.row, 4);
    }

    #[test]
    fn combining_marks_precompose_when_they_can() {
        let mut t = term();
        t.print('e');
        t.print('\u{301}');
        assert_eq!(t.grid().row(0).unwrap().cells[0].ch, 'é');
    }

    #[test]
    fn ambiguous_widths_only_widen_when_asked() {
        let mut cfg = TermConfig::default();
        cfg.widen_ambiguous = vec!["arrows".into()];
        let t = Terminal::new(10, 2, cfg);
        assert_eq!(t.char_width('→'), 2);
        let t = term();
        assert_eq!(t.char_width('→'), 1);
        assert_eq!(t.char_width('世'), 2);
        assert_eq!(t.char_width('a'), 1);
    }

    #[test]
    fn mouse_tracking_reports_the_highest_enabled_flavour() {
        let mut t = term();
        assert_eq!(t.mouse_tracking(), MouseTracking::Off);
        t.modes.set(Modes::MOUSE_VT200, true);
        assert_eq!(t.mouse_tracking(), MouseTracking::Click);
        t.modes.set(Modes::MOUSE_BTN, true);
        assert_eq!(t.mouse_tracking(), MouseTracking::Drag);
        t.modes.set(Modes::MOUSE_ANY, true);
        assert_eq!(t.mouse_tracking(), MouseTracking::Motion);
    }

    #[test]
    fn capture_returns_visible_text_and_history_returns_everything() {
        let mut t = Terminal::new(6, 2, TermConfig::default());
        // NEL (CR + LF): a bare LF deliberately leaves the column where it was,
        // as the LF page states, so a new line starts here with `next_line`.
        for c in "one".chars() {
            t.print(c);
        }
        t.next_line();
        for c in "two".chars() {
            t.print(c);
        }
        t.next_line();
        for c in "three".chars() {
            t.print(c);
        }
        assert!(t.contents().contains("three"));
        assert!(t.history().contains("one"));
        assert!(t.history().contains("three"));
    }

    #[test]
    fn mode_numbers_map_to_the_documented_masks() {
        assert_eq!(Modes::mask_for(25), Some(Modes::DECTCEM));
        assert_eq!(Modes::mask_for(1049), Some(Modes::ALT1049));
        assert_eq!(Modes::mask_for(2004), Some(Modes::BRACKETED_PASTE));
        assert_eq!(Modes::mask_for(2026), Some(Modes::SYNCHRONIZED_OUTPUT));
        assert_eq!(Modes::mask_for(9999), None);
    }

    #[test]
    fn resizing_keeps_the_cursor_inside_the_grid() {
        let mut t = term();
        t.set_cursor(3, 9);
        t.resize(5, 2);
        assert_eq!(t.cursor.row, 1);
        assert_eq!(t.cursor.col, 4);
    }
}
