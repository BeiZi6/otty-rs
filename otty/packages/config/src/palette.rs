//! The 16-colour ANSI palette and how a theme, the config overrides and an
//! `OSC 4` runtime set combine into the colours the grid actually draws.
//!
//! Precedence, lowest to highest:
//!
//! 1. the fallback palette (Dracula-based) that applies when no theme is set;
//! 2. the active theme's `[terminal]` colours;
//! 3. the config keys `foreground`, `background`, `palette-0`…`palette-15`,
//!    `palette = N=COLOR`, `cursor-color`, `selection-background`;
//! 4. runtime `OSC 4` / `OSC 10` / `OSC 11` / `OSC 12` sets, which are
//!    per-session and never written to disk.

use otty_core::Color;

/// The 16-colour palette used when no theme is active.
///
/// Documented in the Configuration Reference as the "fallback palette", which
/// is what `foreground` / `background` default to before `theme = Paper` (the
/// shipped default) takes over.
pub const FALLBACK_PALETTE: [Color; 16] = [
    Color::rgb(0x28, 0x2a, 0x36), // 0  black
    Color::rgb(0xff, 0x55, 0x55), // 1  red
    Color::rgb(0x50, 0xfa, 0x7b), // 2  green
    Color::rgb(0xf1, 0xfa, 0x8c), // 3  yellow
    Color::rgb(0xbd, 0x93, 0xf9), // 4  blue
    Color::rgb(0xff, 0x79, 0xc6), // 5  magenta
    Color::rgb(0x8b, 0xe9, 0xfd), // 6  cyan
    Color::rgb(0xf8, 0xf8, 0xf2), // 7  white
    Color::rgb(0x62, 0x72, 0xa4), // 8  bright black
    Color::rgb(0xff, 0x6e, 0x6e), // 9  bright red
    Color::rgb(0x69, 0xff, 0x94), // 10 bright green
    Color::rgb(0xff, 0xff, 0xa5), // 11 bright yellow
    Color::rgb(0xd6, 0xac, 0xff), // 12 bright blue
    Color::rgb(0xff, 0x92, 0xdf), // 13 bright magenta
    Color::rgb(0xa4, 0xff, 0xff), // 14 bright cyan
    Color::rgb(0xff, 0xff, 0xff), // 15 bright white
];

/// `foreground`'s default, which only shows when no theme is set.
pub const DEFAULT_FOREGROUND: Color = Color::rgb(0xd4, 0xd4, 0xd4);
/// `background`'s default, which only shows when no theme is set.
pub const DEFAULT_BACKGROUND: Color = Color::rgb(0x1e, 0x1e, 0x1e);

/// The 16 ANSI slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette(pub [Color; 16]);

impl Default for Palette {
    fn default() -> Self {
        Self(FALLBACK_PALETTE)
    }
}

impl Palette {
    pub const fn get(&self, index: u8) -> Color {
        self.0[(index & 0x0f) as usize]
    }

    pub fn set(&mut self, index: u8, c: Color) {
        if index < 16 {
            self.0[index as usize] = c;
        }
    }

    /// The bright counterpart of an ANSI 0-7 index, as `bold-color = bright`
    /// resolves it.
    pub const fn bright_of(&self, index: u8) -> Color {
        self.get(index % 8 + 8)
    }

    /// The xterm 256-colour lookup, with 0-15 served from the live palette so a
    /// theme reaches `SGR 38;5;<n>` too.
    pub fn indexed(&self, index: u8) -> Color {
        if index < 16 {
            self.get(index)
        } else {
            otty_core::color::xterm_256(index)
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Color> {
        self.0.iter()
    }
}

impl From<[Color; 16]> for Palette {
    fn from(v: [Color; 16]) -> Self {
        Self(v)
    }
}

/// The resolved colours a session draws with, after theme + config + runtime
/// overrides have been folded together.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedColors {
    pub foreground: Color,
    pub background: Color,
    pub palette: Palette,
    pub cursor: Color,
    pub cursor_text: Color,
    pub selection_foreground: Option<Color>,
    pub selection_background: Option<Color>,
    pub bold: BoldColor,
    pub faint_opacity: f32,
    pub minimum_contrast: f32,
}

/// `bold-color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoldColor {
    /// Keep the cell's own colour.
    #[default]
    Color,
    /// Use the bright palette variant.
    Bright,
    /// A fixed colour.
    Fixed(Color),
}

impl ResolvedColors {
    /// The foreground a cell with the given ANSI index and bold flag draws in.
    pub fn cell_foreground(&self, ansi: u8, bold: bool) -> Color {
        let base = if bold {
            match self.bold {
                BoldColor::Bright => self.palette.bright_of(ansi),
                BoldColor::Color => self.palette.get(ansi),
                BoldColor::Fixed(c) => c,
            }
        } else {
            self.palette.get(ansi)
        };
        if self.minimum_contrast > 1.0 {
            base.enforce_contrast(self.background, self.minimum_contrast)
        } else {
            base
        }
    }

    /// The selection colours, defaulting to the documented fg↔bg behaviour.
    pub fn selection(&self) -> (Color, Color) {
        let fg = match self.selection_foreground {
            Some(c) => c,
            // Unset means "keep each cell's original foreground"; the renderer
            // reads that as the terminal foreground for a block selection.
            None => self.foreground,
        };
        let bg = match self.selection_background {
            Some(c) => c,
            None => self.foreground,
        };
        (fg, bg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_palette_matches_the_documented_dracula_values() {
        assert_eq!(FALLBACK_PALETTE[0].to_hex(), "#282a36");
        assert_eq!(FALLBACK_PALETTE[1].to_hex(), "#ff5555");
        assert_eq!(FALLBACK_PALETTE[7].to_hex(), "#f8f8f2");
        assert_eq!(FALLBACK_PALETTE[8].to_hex(), "#6272a4");
        assert_eq!(FALLBACK_PALETTE[15].to_hex(), "#ffffff");
    }

    #[test]
    fn documented_no_theme_defaults_are_used() {
        assert_eq!(DEFAULT_FOREGROUND.to_hex(), "#d4d4d4");
        assert_eq!(DEFAULT_BACKGROUND.to_hex(), "#1e1e1e");
    }

    #[test]
    fn indexed_lookup_serves_low_slots_from_the_live_palette() {
        let mut p = Palette::default();
        p.set(3, Color::rgb(1, 2, 3));
        assert_eq!(p.indexed(3), Color::rgb(1, 2, 3));
        assert_eq!(p.indexed(16), Color::rgb(0, 0, 0));
        assert_eq!(p.indexed(196), Color::rgb(255, 0, 0));
    }

    #[test]
    fn bright_lookup_wraps_around() {
        let p = Palette::default();
        assert_eq!(p.bright_of(1), p.get(9));
        assert_eq!(p.bright_of(9), p.get(9));
    }

    #[test]
    fn bold_bright_uses_the_bright_half() {
        let c = ResolvedColors {
            foreground: DEFAULT_FOREGROUND,
            background: DEFAULT_BACKGROUND,
            palette: Palette::default(),
            cursor: DEFAULT_FOREGROUND,
            cursor_text: DEFAULT_BACKGROUND,
            selection_foreground: None,
            selection_background: None,
            bold: BoldColor::Bright,
            faint_opacity: 0.5,
            minimum_contrast: 1.0,
        };
        assert_eq!(c.cell_foreground(1, false), FALLBACK_PALETTE[1]);
        assert_eq!(c.cell_foreground(1, true), FALLBACK_PALETTE[9]);
    }

    #[test]
    fn minimum_contrast_is_applied_on_the_way_out() {
        let mut c = ResolvedColors {
            foreground: DEFAULT_FOREGROUND,
            background: Color::OPAQUE_WHITE,
            palette: Palette::default(),
            cursor: DEFAULT_FOREGROUND,
            cursor_text: DEFAULT_BACKGROUND,
            selection_foreground: None,
            selection_background: None,
            bold: BoldColor::Color,
            faint_opacity: 0.5,
            minimum_contrast: 1.0,
        };
        c.palette.set(7, Color::rgb(250, 250, 250));
        assert_eq!(c.cell_foreground(7, false).to_hex(), "#fafafa");
        c.minimum_contrast = 4.5;
        assert!(c.cell_foreground(7, false).contrast_ratio(Color::OPAQUE_WHITE) >= 4.5);
    }

    #[test]
    fn selection_defaults_follow_the_documented_fallback() {
        let c = ResolvedColors {
            foreground: Color::rgb(0xd4, 0xd4, 0xd4),
            background: Color::rgb(0x1e, 0x1e, 0x1e),
            palette: Palette::default(),
            cursor: DEFAULT_FOREGROUND,
            cursor_text: DEFAULT_BACKGROUND,
            selection_foreground: None,
            selection_background: None,
            bold: BoldColor::Color,
            faint_opacity: 0.5,
            minimum_contrast: 1.0,
        };
        assert_eq!(c.selection(), (Color::rgb(0xd4, 0xd4, 0xd4), Color::rgb(0xd4, 0xd4, 0xd4)));
    }
}
