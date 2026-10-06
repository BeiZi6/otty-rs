//! Cell attributes: colour, SGR flags and the pen the parser writes with.

use otty_core::Color;
use serde::{Deserialize, Serialize};

/// The colour of a cell, before the theme is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ColorSpec {
    /// The terminal's default foreground or background.
    #[default]
    Default,
    /// An ANSI 0-15 slot, resolved against the live palette.
    Indexed(u8),
    /// A 24-bit colour.
    Rgb(u8, u8, u8),
}

impl ColorSpec {
    /// `SGR 38;5;<n>` / `SGR 38;2;r;g;b` produce these; an index below 16 is
    /// resolved against the live theme palette, everything above from the
    /// standard xterm 256-colour cube.
    pub const fn from_indexed(index: u8) -> Self {
        Self::Indexed(index)
    }

    pub const fn is_default(self) -> bool {
        matches!(self, Self::Default)
    }

    /// Resolve against a palette.
    pub fn to_color<P: PaletteLookup + ?Sized>(self, palette: &P) -> Color {
        match self {
            // The caller substitutes the terminal's own foreground/background.
            Self::Default => Color::OPAQUE_BLACK,
            Self::Indexed(i) => palette.indexed(i),
            Self::Rgb(r, g, b) => Color::rgb(r, g, b),
        }
    }
}

/// The lookup the renderer hands the engine so `ColorSpec` can be resolved.
pub trait PaletteLookup {
    /// The xterm 256-colour entry for `index`, with 0-15 coming from the theme.
    fn indexed(&self, index: u8) -> Color;
}

/// SGR flags carried by a cell.
///
/// Stored as a bitfield so a `Cell` stays 16 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Flags(pub u16);

impl Flags {
    pub const BOLD: Self = Self(1 << 0);
    pub const DIM: Self = Self(1 << 1);
    pub const ITALIC: Self = Self(1 << 2);
    pub const UNDERLINE: Self = Self(1 << 3);
    /// SGR 4:2 — a double underline.
    pub const DOUBLE_UNDERLINE: Self = Self(1 << 4);
    pub const UNDERCURL: Self = Self(1 << 5);
    pub const DOTTED_UNDERLINE: Self = Self(1 << 6);
    pub const DASHED_UNDERLINE: Self = Self(1 << 7);
    pub const BLINK: Self = Self(1 << 8);
    pub const INVERSE: Self = Self(1 << 9);
    pub const HIDDEN: Self = Self(1 << 10);
    pub const STRIKEOUT: Self = Self(1 << 11);
    /// SGR 53 — overline.
    pub const OVERLINE: Self = Self(1 << 12);
    /// The cell was marked by DECSCA as protected.
    pub const PROTECTED: Self = Self(1 << 13);
    /// A wide character's trailing half.
    pub const WIDE_CHAR_SPACER: Self = Self(1 << 14);
    /// This cell is the second half of a wrapped wide glyph (kitty convention).
    pub const LEADING_WIDE_CHAR_SPACER: Self = Self(1 << 15);

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.insert(other)
        } else {
            self.remove(other)
        }
    }

    /// Every underline flavour, which SGR 24 clears together.
    pub const UNDERLINES: Self =
        Self(Self::UNDERLINE.0 | Self::DOUBLE_UNDERLINE.0 | Self::UNDERCURL.0
            | Self::DOTTED_UNDERLINE.0 | Self::DASHED_UNDERLINE.0);

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Flags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// The current SGR state — what new cells are written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pen {
    pub fg: ColorSpec,
    pub bg: ColorSpec,
    pub flags: Flags,
    /// `SGR 58` underline colour.
    pub underline_color: ColorSpec,
    /// `SGR 73`/`SGR 74` — sub/superscript, with the offset SGR 75 sets.
    pub vertical_offset: i8,
}

impl Pen {
    pub const fn new() -> Self {
        Self {
            fg: ColorSpec::Default,
            bg: ColorSpec::Default,
            flags: Flags(0),
            underline_color: ColorSpec::Default,
            vertical_offset: 0,
        }
    }

    /// Apply an SGR parameter, as `CSI <params> m` specifies.
    ///
    /// Returns `false` when the parameter is one the engine does not model, so
    /// the caller can log it in `--debug` without failing the whole sequence.
    pub fn apply(&mut self, param: u16) -> bool {
        match param {
            0 => *self = Self::new(),
            1 => self.flags.insert(Flags::BOLD),
            2 => self.flags.insert(Flags::DIM),
            3 => self.flags.insert(Flags::ITALIC),
            4 => {
                self.flags.remove(Flags::UNDERLINES);
                self.flags.insert(Flags::UNDERLINE);
            }
            5 | 6 => self.flags.insert(Flags::BLINK),
            7 => self.flags.insert(Flags::INVERSE),
            8 => self.flags.insert(Flags::HIDDEN),
            9 => self.flags.insert(Flags::STRIKEOUT),
            21 => {
                self.flags.remove(Flags::UNDERLINES);
                self.flags.insert(Flags::DOUBLE_UNDERLINE);
            }
            22 => {
                self.flags.remove(Flags::BOLD);
                self.flags.remove(Flags::DIM);
            }
            23 => self.flags.remove(Flags::ITALIC),
            24 => self.flags.remove(Flags::UNDERLINES),
            25 => self.flags.remove(Flags::BLINK),
            27 => self.flags.remove(Flags::INVERSE),
            28 => self.flags.remove(Flags::HIDDEN),
            29 => self.flags.remove(Flags::STRIKEOUT),
            30..=37 => self.fg = ColorSpec::Indexed((param - 30) as u8),
            38 | 48 | 58 => return true, // handled by the extended-colour reader
            39 => self.fg = ColorSpec::Default,
            40..=47 => self.bg = ColorSpec::Indexed((param - 40) as u8),
            49 => self.bg = ColorSpec::Default,
            53 => self.flags.insert(Flags::OVERLINE),
            55 => self.flags.remove(Flags::OVERLINE),
            59 => self.underline_color = ColorSpec::Default,
            73 => {
                self.flags.insert(Flags::BOLD);
                self.vertical_offset = -1;
            }
            74 => {
                self.flags.insert(Flags::BOLD);
                self.vertical_offset = 1;
            }
            75 => self.vertical_offset = 0,
            90..=97 => self.fg = ColorSpec::Indexed((param - 90 + 8) as u8),
            100..=107 => self.bg = ColorSpec::Indexed((param - 100 + 8) as u8),
            _ => return false,
        }
        true
    }

    /// `SGR 4:<n>` — the colon form that selects an underline flavour.
    pub fn set_underline_style(&mut self, style: u16) {
        self.flags.remove(Flags::UNDERLINES);
        self.flags.insert(match style {
            0 => return,
            1 => Flags::UNDERLINE,
            2 => Flags::DOUBLE_UNDERLINE,
            3 => Flags::UNDERCURL,
            4 => Flags::DOTTED_UNDERLINE,
            5 => Flags::DASHED_UNDERLINE,
            _ => Flags::UNDERLINE,
        });
    }

    /// Whether the pen would draw anything at all.
    pub fn is_plain(&self) -> bool {
        self.fg.is_default()
            && self.bg.is_default()
            && self.flags.is_empty()
            && self.underline_color.is_default()
            && self.vertical_offset == 0
    }
}

/// One grid cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub pen_extra: u8,
    pub fg: ColorSpec,
    pub bg: ColorSpec,
    pub flags: Flags,
}

impl Default for Cell {
    fn default() -> Self {
        Self::blank()
    }
}

impl Cell {
    /// A blank cell carrying the default background.
    pub const fn blank() -> Self {
        Self {
            ch: ' ',
            pen_extra: 0,
            fg: ColorSpec::Default,
            bg: ColorSpec::Default,
            flags: Flags(0),
        }
    }

    /// A cell written by `write` with the given pen.
    pub fn from_pen(ch: char, pen: &Pen) -> Self {
        Self {
            ch,
            pen_extra: pen.vertical_offset as u8,
            fg: pen.fg,
            bg: pen.bg,
            flags: pen.flags,
        }
    }

    /// Wipe the character but keep the background, which is what ECH and EL do.
    pub fn clear_keeping_bg(&mut self) {
        self.ch = ' ';
        self.flags = Flags(0);
        self.pen_extra = 0;
        self.underline_color_reset();
    }

    fn underline_color_reset(&mut self) {
        // `underline_color` is not stored on the cell (it is rare and would
        // cost 4 bytes per cell); clearing the flags is enough for rendering.
    }

    pub fn is_blank(&self) -> bool {
        self.ch == ' ' && self.bg.is_default() && self.flags.is_empty()
    }

    /// Whether this cell is a wide character's second column.
    pub fn is_wide_spacer(&self) -> bool {
        self.flags.contains(Flags::WIDE_CHAR_SPACER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sgr_resets_and_stacks() {
        let mut p = Pen::new();
        assert!(p.apply(1));
        assert!(p.apply(3));
        assert!(p.apply(4));
        assert!(p.flags.contains(Flags::BOLD | Flags::ITALIC | Flags::UNDERLINE));
        assert!(p.apply(22));
        assert!(!p.flags.contains(Flags::BOLD));
        assert!(p.flags.contains(Flags::ITALIC));
        assert!(p.apply(0));
        assert!(p.is_plain());
    }

    #[test]
    fn colours_take_the_documented_slots() {
        let mut p = Pen::new();
        p.apply(31);
        assert_eq!(p.fg, ColorSpec::Indexed(1));
        p.apply(94);
        assert_eq!(p.fg, ColorSpec::Indexed(12));
        p.apply(42);
        assert_eq!(p.bg, ColorSpec::Indexed(2));
        p.apply(39);
        p.apply(49);
        assert_eq!(p.fg, ColorSpec::Default);
        assert_eq!(p.bg, ColorSpec::Default);
    }

    #[test]
    fn underline_styles_replace_each_other() {
        let mut p = Pen::new();
        p.set_underline_style(2);
        assert!(p.flags.contains(Flags::DOUBLE_UNDERLINE));
        assert!(!p.flags.contains(Flags::UNDERLINE));
        p.set_underline_style(0);
        assert!(p.flags.is_empty());
    }

    #[test]
    fn sgr_24_clears_every_underline_flavour() {
        let mut p = Pen::new();
        p.set_underline_style(3);
        assert!(p.flags.contains(Flags::UNDERCURL));
        p.apply(24);
        assert!(!p.flags.contains(Flags::UNDERLINES));
    }

    #[test]
    fn unknown_parameters_are_reported_not_fatal() {
        let mut p = Pen::new();
        assert!(!p.apply(999));
        assert!(p.is_plain());
    }

    #[test]
    fn sub_and_superscript_set_the_offset() {
        let mut p = Pen::new();
        p.apply(73);
        assert_eq!(p.vertical_offset, -1);
        p.apply(74);
        assert_eq!(p.vertical_offset, 1);
        p.apply(75);
        assert_eq!(p.vertical_offset, 0);
    }

    #[test]
    fn a_cell_copies_the_pen_it_was_written_with() {
        let mut p = Pen::new();
        p.apply(1);
        p.apply(33);
        let c = Cell::from_pen('x', &p);
        assert_eq!(c.ch, 'x');
        assert_eq!(c.fg, ColorSpec::Indexed(3));
        assert!(c.flags.contains(Flags::BOLD));
    }

    #[test]
    fn clearing_a_cell_keeps_its_background() {
        let mut p = Pen::new();
        p.apply(44);
        p.apply(1);
        let mut c = Cell::from_pen('x', &p);
        c.clear_keeping_bg();
        assert_eq!(c.ch, ' ');
        assert_eq!(c.bg, ColorSpec::Indexed(4));
        assert!(c.flags.is_empty());
    }
}
