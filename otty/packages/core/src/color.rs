//! Colours.
//!
//! Otty accepts colour values in four spellings in both `config.toml` and theme
//! files: `#rgb`, `#rrggbb`, `#rrggbbaa`, and the 148 CSS/SVG colour names.
//! An extra keyword, `transparent`, is accepted where alpha is meaningful.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An 8-bit-per-channel RGBA colour.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self::OPAQUE_BLACK
    }
}

impl Color {
    pub const OPAQUE_BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const OPAQUE_WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, a CSS colour name, or
    /// `transparent`. A missing `#` is accepted, matching the loader's leniency.
    pub fn parse(input: &str) -> Result<Self, ColorError> {
        let s = input.trim();
        if s.is_empty() {
            return Err(ColorError::Empty);
        }
        if s.eq_ignore_ascii_case("transparent") {
            return Ok(Self::rgba(0, 0, 0, 0));
        }
        let hex = s.strip_prefix('#').unwrap_or(s);
        if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Self::parse_hex(hex);
        }
        // Functional notation. Theme files use `rgba(255, 255, 255, 0.06)` for
        // translucent chrome borders, which the hex spellings cannot express at
        // that precision. `rgb()` and `hsl()`-free `rgba()` only: the product
        // never emits the other CSS functions.
        if let Some(inner) = strip_function(s, "rgba").or_else(|| strip_function(s, "rgb")) {
            return Self::parse_functional(inner);
        }
        if let Some(c) = NamedColor::lookup(s) {
            return Ok(c);
        }
        Err(ColorError::Unknown(s.to_owned()))
    }

    /// Parse the inside of `rgb(...)` / `rgba(...)`.
    ///
    /// Channels may be `0-255` or a percentage; alpha may be `0-1` or a
    /// percentage, per CSS. A trailing `/` separator (the modern CSS spelling)
    /// is accepted too.
    pub fn parse_functional(inner: &str) -> Result<Self, ColorError> {
        // Accept every CSS spelling: comma-separated, space-separated, and the
        // modern `rgb(10 20 30 / 50%)` with a slash before the alpha.
        let mut parts: Vec<&str> = Vec::new();
        for segment in inner.split(',') {
            for piece in segment.split(|c: char| c.is_whitespace() || c == '/') {
                if !piece.is_empty() {
                    parts.push(piece);
                }
            }
        }
        if parts.len() < 3 || parts.len() > 4 {
            return Err(ColorError::BadHex);
        }
        let channel = |p: &str| -> Result<u8, ColorError> {
            if let Some(pct) = p.strip_suffix('%') {
                let v: f32 = pct.trim().parse().map_err(|_| ColorError::BadHex)?;
                return Ok((v / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            let v: f32 = p.trim().parse().map_err(|_| ColorError::BadHex)?;
            Ok(v.round().clamp(0.0, 255.0) as u8)
        };
        let alpha = if parts.len() == 4 {
            let p = parts[3];
            let v = if let Some(pct) = p.strip_suffix('%') {
                pct.trim().parse::<f32>().map_err(|_| ColorError::BadHex)? / 100.0
            } else {
                p.trim().parse::<f32>().map_err(|_| ColorError::BadHex)?
            };
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        } else {
            255
        };
        Ok(Self::rgba(channel(parts[0])?, channel(parts[1])?, channel(parts[2])?, alpha))
    }

    /// Parse the digits of a hex colour (no `#`).
    pub fn parse_hex(hex: &str) -> Result<Self, ColorError> {
        let d = |i: usize| -> Result<u8, ColorError> {
            u8::from_str_radix(&hex[i..i + 1], 16).map_err(|_| ColorError::BadHex)
        };
        let dd = |i: usize| -> Result<u8, ColorError> {
            u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ColorError::BadHex)
        };
        Ok(match hex.len() {
            3 => Self::rgb(d(0)? * 17, d(1)? * 17, d(2)? * 17),
            4 => Self::rgba(d(0)? * 17, d(1)? * 17, d(2)? * 17, d(3)? * 17),
            6 => Self::rgb(dd(0)?, dd(2)?, dd(4)?),
            8 => Self::rgba(dd(0)?, dd(2)?, dd(4)?, dd(6)?),
            _ => return Err(ColorError::BadHex),
        })
    }

    /// `#rrggbb`, or `#rrggbbaa` when the colour is not fully opaque.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Linear interpolation in gamma-encoded space, used by theme derivation.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Self::rgba(l(self.r, other.r), l(self.g, other.g), l(self.b, other.b), l(self.a, other.a))
    }

    /// Multiply the alpha channel — used for `faint-opacity` and pane dimming.
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a: (self.a as f32 * a.clamp(0.0, 1.0)).round() as u8, ..self }
    }

    pub fn with_alpha_u8(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let f = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(self.r) + 0.7152 * f(self.g) + 0.0722 * f(self.b)
    }

    /// WCAG contrast ratio, 1.0 – 21.0. Backs `minimum-contrast`.
    pub fn contrast_ratio(self, other: Self) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Push `self` away from `bg` until it reaches `ratio`, mirroring how a
    /// terminal honours `minimum-contrast` without changing the hue.
    pub fn enforce_contrast(self, bg: Self, ratio: f32) -> Self {
        if ratio <= 1.0 || self.contrast_ratio(bg) >= ratio {
            return self;
        }
        let towards_white = bg.luminance() < 0.5;
        let target = if towards_white { Self::OPAQUE_WHITE } else { Self::OPAQUE_BLACK };
        let mut best = self;
        for step in 1..=100 {
            let c = self.lerp(target, step as f32 / 100.0);
            if c.contrast_ratio(bg) >= ratio {
                return c;
            }
            best = c;
        }
        best
    }

    /// Whether the colour carries any transparency.
    pub const fn is_transparent(self) -> bool {
        self.a != 255
    }

    /// Whether the value is one of the two "not a real colour" keywords the
    /// theme importer has to special-case.
    pub fn is_keyword_auto(s: &str) -> bool {
        s.trim().eq_ignore_ascii_case("auto")
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl From<Color> for String {
    fn from(c: Color) -> Self {
        c.to_hex()
    }
}

impl From<String> for Color {
    fn from(s: String) -> Self {
        Color::parse(&s).unwrap_or(Color::OPAQUE_BLACK)
    }
}

impl From<&str> for Color {
    fn from(s: &str) -> Self {
        Color::parse(s).unwrap_or(Color::OPAQUE_BLACK)
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::rgb(r, g, b)
    }
}

impl From<Color> for [f32; 4] {
    /// sRGB-encoded floats, the shape wgpu wants for a render target.
    fn from(c: Color) -> Self {
        [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
    }
}

impl From<Color> for [f64; 4] {
    fn from(c: Color) -> Self {
        [c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0, c.a as f64 / 255.0]
    }
}

/// Errors from [`Color::parse`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ColorError {
    #[error("empty colour value")]
    Empty,
    #[error("not a hex colour, CSS colour name, or `transparent`")]
    Unknown(String),
    #[error("hex colour must have 3, 4, 6 or 8 digits")]
    BadHex,
}

impl NamedColor {
    /// Case-insensitive lookup of the 148 CSS/SVG colour names.
    pub fn lookup(name: &str) -> Option<Color> {
        let n: String = name
            .trim()
            .chars()
            .filter(|c| *c != ' ' && *c != '-' && *c != '_')
            .flat_map(char::to_lowercase)
            .collect();
        CSS_NAMES
            .iter()
            .find(|(k, _)| *k == n)
            .map(|(_, v)| Color::rgb((v >> 16) as u8, (v >> 8) as u8, *v as u8))
    }

    /// The canonical names, for completion and validation messages.
    pub fn names() -> impl Iterator<Item = &'static str> {
        CSS_NAMES.iter().map(|(k, _)| *k)
    }
}

/// Namespace marker for the CSS colour table.
#[derive(Debug, Clone, Copy)]
pub struct NamedColor;

macro_rules! css_names {
    ($($name:literal => $hex:literal),* $(,)?) => {
        static CSS_NAMES: &[(&str, u32)] = &[$(($name, $hex)),*];
    };
}

css_names! {
    "aliceblue" => 0xF0F8FF, "antiquewhite" => 0xFAEBD7, "aqua" => 0x00FFFF,
    "aquamarine" => 0x7FFFD4, "azure" => 0xF0FFFF, "beige" => 0xF5F5DC,
    "bisque" => 0xFFE4C4, "black" => 0x000000, "blanchedalmond" => 0xFFEBCD,
    "blue" => 0x0000FF, "blueviolet" => 0x8A2BE2, "brown" => 0xA52A2A,
    "burlywood" => 0xDEB887, "cadetblue" => 0x5F9EA0, "chartreuse" => 0x7FFF00,
    "chocolate" => 0xD2691E, "coral" => 0xFF7F50, "cornflowerblue" => 0x6495ED,
    "cornsilk" => 0xFFF8DC, "crimson" => 0xDC143C, "cyan" => 0x00FFFF,
    "darkblue" => 0x00008B, "darkcyan" => 0x008B8B, "darkgoldenrod" => 0xB8860B,
    "darkgray" => 0xA9A9A9, "darkgreen" => 0x006400, "darkgrey" => 0xA9A9A9,
    "darkkhaki" => 0xBDB76B, "darkmagenta" => 0x8B008B, "darkolivegreen" => 0x556B2F,
    "darkorange" => 0xFF8C00, "darkorchid" => 0x9932CC, "darkred" => 0x8B0000,
    "darksalmon" => 0xE9967A, "darkseagreen" => 0x8FBC8F, "darkslateblue" => 0x483D8B,
    "darkslategray" => 0x2F4F4F, "darkslategrey" => 0x2F4F4F, "darkturquoise" => 0x00CED1,
    "darkviolet" => 0x9400D3, "deeppink" => 0xFF1493, "deepskyblue" => 0x00BFFF,
    "dimgray" => 0x696969, "dimgrey" => 0x696969, "dodgerblue" => 0x1E90FF,
    "firebrick" => 0xB22222, "floralwhite" => 0xFFFAF0, "forestgreen" => 0x228B22,
    "fuchsia" => 0xFF00FF, "gainsboro" => 0xDCDCDC, "ghostwhite" => 0xF8F8FF,
    "gold" => 0xFFD700, "goldenrod" => 0xDAA520, "gray" => 0x808080,
    "green" => 0x008000, "greenyellow" => 0xADFF2F, "grey" => 0x808080,
    "honeydew" => 0xF0FFF0, "hotpink" => 0xFF69B4, "indianred" => 0xCD5C5C,
    "indigo" => 0x4B0082, "ivory" => 0xFFFFF0, "khaki" => 0xF0E68C,
    "lavender" => 0xE6E6FA, "lavenderblush" => 0xFFF0F5, "lawngreen" => 0x7CFC00,
    "lemonchiffon" => 0xFFFACD, "lightblue" => 0xADD8E6, "lightcoral" => 0xF08080,
    "lightcyan" => 0xE0FFFF, "lightgoldenrodyellow" => 0xFAFAD2, "lightgray" => 0xD3D3D3,
    "lightgreen" => 0x90EE90, "lightgrey" => 0xD3D3D3, "lightpink" => 0xFFB6C1,
    "lightsalmon" => 0xFFA07A, "lightseagreen" => 0x20B2AA, "lightskyblue" => 0x87CEFA,
    "lightslategray" => 0x778899, "lightslategrey" => 0x778899, "lightsteelblue" => 0xB0C4DE,
    "lightyellow" => 0xFFFFE0, "lime" => 0x00FF00, "limegreen" => 0x32CD32,
    "linen" => 0xFAF0E6, "magenta" => 0xFF00FF, "maroon" => 0x800000,
    "mediumaquamarine" => 0x66CDAA, "mediumblue" => 0x0000CD, "mediumorchid" => 0xBA55D3,
    "mediumpurple" => 0x9370DB, "mediumseagreen" => 0x3CB371, "mediumslateblue" => 0x7B68EE,
    "mediumspringgreen" => 0x00FA9A, "mediumturquoise" => 0x48D1CC, "mediumvioletred" => 0xC71585,
    "midnightblue" => 0x191970, "mintcream" => 0xF5FFFA, "mistyrose" => 0xFFE4E1,
    "moccasin" => 0xFFE4B5, "navajowhite" => 0xFFDEAD, "navy" => 0x000080,
    "oldlace" => 0xFDF5E6, "olive" => 0x808000, "olivedrab" => 0x6B8E23,
    "orange" => 0xFFA500, "orangered" => 0xFF4500, "orchid" => 0xDA70D6,
    "palegoldenrod" => 0xEEE8AA, "palegreen" => 0x98FB98, "paleturquoise" => 0xAFEEEE,
    "palevioletred" => 0xDB7093, "papayawhip" => 0xFFEFD5, "peachpuff" => 0xFFDAB9,
    "peru" => 0xCD853F, "pink" => 0xFFC0CB, "plum" => 0xDDA0DD,
    "powderblue" => 0xB0E0E6, "purple" => 0x800080, "rebeccapurple" => 0x663399,
    "red" => 0xFF0000, "rosybrown" => 0xBC8F8F, "royalblue" => 0x4169E1,
    "saddlebrown" => 0x8B4513, "salmon" => 0xFA8072, "sandybrown" => 0xF4A460,
    "seagreen" => 0x2E8B57, "seashell" => 0xFFF5EE, "sienna" => 0xA0522D,
    "silver" => 0xC0C0C0, "skyblue" => 0x87CEEB, "slateblue" => 0x6A5ACD,
    "slategray" => 0x708090, "slategrey" => 0x708090, "snow" => 0xFFFAFA,
    "springgreen" => 0x00FF7F, "steelblue" => 0x4682B4, "tan" => 0xD2B48C,
    "teal" => 0x008080, "thistle" => 0xD8BFD8, "tomato" => 0xFF6347,
    "turquoise" => 0x40E0D0, "violet" => 0xEE82EE, "wheat" => 0xF5DEB3,
    "white" => 0xFFFFFF, "whitesmoke" => 0xF5F5F5, "yellow" => 0xFFFF00,
    "yellowgreen" => 0x9ACD32,
}

/// `name(…)` → `…`, case-insensitively, when the call spans the whole string.
fn strip_function<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let t = s.trim();
    if t.len() < name.len() + 2 {
        return None;
    }
    let (head, rest) = t.split_at(name.len());
    if !head.eq_ignore_ascii_case(name) {
        return None;
    }
    rest.trim_start().strip_prefix('(')?.strip_suffix(')')
}

/// The xterm 256-colour palette entry `<n>`, as used by `SGR 38;5;<n>`.
pub fn xterm_256(index: u8) -> Color {
    const BASE: [u32; 16] = [
        0x1e1e1e, 0xff5555, 0x50fa7b, 0xf1fa8c, 0xbd93f9, 0xff79c6, 0x8be9fd, 0xf8f8f2,
        0x282a36, 0xff6e6e, 0x69ff94, 0xffffa5, 0xd6acff, 0xff92df, 0xa4ffff, 0xffffff,
    ];
    if let Some(v) = BASE.get(index as usize) {
        return Color::rgb((v >> 16) as u8, (v >> 8) as u8, *v as u8);
    }
    if index < 232 {
        let i = index - 16;
        let steps = [0u8, 95, 135, 175, 215, 255];
        let r = steps[(i / 36) as usize];
        let g = steps[((i % 36) / 6) as usize];
        let b = steps[(i % 6) as usize];
        return Color::rgb(r, g, b);
    }
    let v = 8 + (index - 232) * 10;
    Color::rgb(v, v, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_hex_width() {
        assert_eq!(Color::parse("#f00").unwrap(), Color::rgb(255, 0, 0));
        assert_eq!(Color::parse("f00").unwrap(), Color::rgb(255, 0, 0));
        assert_eq!(Color::parse("#ff0000").unwrap(), Color::rgb(255, 0, 0));
        assert_eq!(Color::parse("#ff000080").unwrap(), Color::rgba(255, 0, 0, 128));
        assert_eq!(Color::parse("#f008").unwrap(), Color::rgba(255, 0, 0, 136));
        assert_eq!(Color::parse("transparent").unwrap(), Color::rgba(0, 0, 0, 0));
    }

    #[test]
    fn parses_css_functional_notation() {
        assert_eq!(Color::parse("rgba(255, 255, 255, 0.06)").unwrap(), Color::rgba(255, 255, 255, 15));
        assert_eq!(Color::parse("rgba(46,62,74,0.16)").unwrap(), Color::rgba(46, 62, 74, 41));
        assert_eq!(Color::parse("rgb(10, 20, 30)").unwrap(), Color::rgb(10, 20, 30));
        assert_eq!(Color::parse("RGBA(0,0,0,1)").unwrap(), Color::rgba(0, 0, 0, 255));
        assert_eq!(Color::parse("rgb(100%, 0%, 0%)").unwrap(), Color::rgb(255, 0, 0));
        assert_eq!(Color::parse("rgb(10 20 30 / 50%)").unwrap(), Color::rgba(10, 20, 30, 128));
        assert!(Color::parse("rgb(1,2)").is_err());
        assert!(Color::parse("nope(1,2,3)").is_err());
    }

    #[test]
    fn parses_css_names_leniently() {
        assert_eq!(Color::parse("cornflowerblue").unwrap(), Color::rgb(100, 149, 237));
        assert_eq!(Color::parse("Cornflower Blue").unwrap(), Color::rgb(100, 149, 237));
        assert_eq!(Color::parse("light-grey").unwrap(), Color::rgb(211, 211, 211));
        assert!(Color::parse("not-a-colour").is_err());
    }

    #[test]
    fn hex_round_trips_and_drops_alpha_when_opaque() {
        assert_eq!(Color::rgb(1, 2, 3).to_hex(), "#010203");
        assert_eq!(Color::rgba(1, 2, 3, 4).to_hex(), "#01020304");
        for s in ["#000000", "#ffffff", "#123456", "#abcdef80"] {
            assert_eq!(Color::parse(s).unwrap().to_hex(), s);
        }
    }

    #[test]
    fn xterm_cube_matches_the_standard_ramp() {
        assert_eq!(xterm_256(16), Color::rgb(0, 0, 0));
        assert_eq!(xterm_256(231), Color::rgb(255, 255, 255));
        assert_eq!(xterm_256(196), Color::rgb(255, 0, 0));
        assert_eq!(xterm_256(255), Color::rgb(238, 238, 238));
    }

    #[test]
    fn minimum_contrast_keeps_raising_until_the_ratio_is_met() {
        let bg = Color::OPAQUE_WHITE;
        let fg = Color::rgb(250, 250, 250);
        assert!(fg.contrast_ratio(bg) < 4.5);
        let fixed = fg.enforce_contrast(bg, 4.5);
        assert!(fixed.contrast_ratio(bg) >= 4.5, "{fixed} on {bg}");
    }
}
