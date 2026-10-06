//! Compositing and rasterisation knobs: `font-blending` and
//! `windows-text-rendering`.
//!
//! These are *decisions*, not pixels. The renderer consumes them — nothing here
//! touches a GPU buffer. Modelling them as enums with their documented meaning
//! attached is what lets the Settings panel describe a mode, the theme suggest
//! one, and the config layer validate one, from a single place.

use otty_core::Platform;
use serde::{Deserialize, Serialize};

/// `font-blending` — how a glyph's coverage is composited onto the background.
///
/// Aliases in the config: `font-smooth`, `font-antialiased` (all three keys hold
/// the same enum, so [`FontBlending::parse`] is the single reader for them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FontBlending {
    /// Blends in the gamma-encoded framebuffer — Otty's baseline, and the
    /// documented default for the key. Stroke weight lands where the font
    /// designer intended.
    #[default]
    SrgbOver,
    /// Keeps non-linear blending in the OS-native colour space (on macOS, the
    /// Display P3 path) — closest to how other native apps render text.
    MacosLike,
    /// Physically-correct linear-light blending. Pure white stays pure white,
    /// but thin dark strokes come out roughly 15% lighter.
    Linear,
    /// Like [`FontBlending::Linear`], but boosts thin dark text back toward its
    /// intended weight.
    Perceptual,
}

impl FontBlending {
    /// Every mode, in the order the Settings panel lists them.
    pub const ALL: [Self; 4] = [Self::SrgbOver, Self::MacosLike, Self::Linear, Self::Perceptual];

    /// The canonical spelling, which is the value the config holds.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SrgbOver => "srgb-over",
            Self::MacosLike => "macos-like",
            Self::Linear => "linear",
            Self::Perceptual => "perceptual",
        }
    }

    /// Parse a concrete mode.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "srgb-over" | "srgb_over" | "srgb" => Self::SrgbOver,
            "macos-like" | "macos" => Self::MacosLike,
            "linear" => Self::Linear,
            "perceptual" => Self::Perceptual,
            _ => return None,
        })
    }

    /// Parse the *setting*, where `default` — defer to the theme — is a value in
    /// its own right and distinct from any concrete mode.
    ///
    /// Returns `Some(None)` for `default`, `Some(Some(mode))` for a mode, and
    /// `None` when the text names neither.
    pub fn parse_setting(s: &str) -> Option<Option<Self>> {
        if s.trim().eq_ignore_ascii_case("default") {
            return Some(None);
        }
        Self::parse(s).map(Some)
    }

    /// What the mode does, as the Settings panel words it.
    pub const fn description(self) -> &'static str {
        match self {
            Self::SrgbOver => "Baseline blending in the gamma-encoded framebuffer.",
            Self::MacosLike => "Non-linear blending in the OS-native colour space.",
            Self::Linear => "Physically-correct linear-light blending; thins dark strokes.",
            Self::Perceptual => "Linear-light blending with thin dark text boosted back.",
        }
    }

    /// The mode that actually applies, given the user's setting and the active
    /// theme's suggestion.
    ///
    /// An explicit choice wins; otherwise the theme decides; otherwise the
    /// baseline. This is the documented "Default — defer to the active theme
    /// (which falls back to srgb-over)" ladder. The theme's own suggestion is
    /// read by [`crate::theme::theme_blending`].
    pub fn resolve(user: Option<Self>, theme: Option<Self>) -> Self {
        user.or(theme).unwrap_or_default()
    }
}

/// `windows-text-rendering` — the DirectWrite rasterisation path. Ignored on
/// every other platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowsTextRendering {
    /// Grayscale antialiasing with grid-fitting. The default.
    #[default]
    Natural,
    /// Grayscale antialiasing, slightly softer.
    NaturalSymmetric,
    /// GDI-style hinting.
    Gdi,
    /// Subpixel ClearType.
    Cleartype,
    /// No antialiasing at all.
    Aliased,
}

impl WindowsTextRendering {
    /// Every mode, in the order the schema lists them.
    pub const ALL: [Self; 5] = [
        Self::Natural,
        Self::NaturalSymmetric,
        Self::Gdi,
        Self::Cleartype,
        Self::Aliased,
    ];

    /// The canonical spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::NaturalSymmetric => "natural-symmetric",
            Self::Gdi => "gdi",
            Self::Cleartype => "cleartype",
            Self::Aliased => "aliased",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "natural" => Self::Natural,
            "natural-symmetric" => Self::NaturalSymmetric,
            "gdi" => Self::Gdi,
            "cleartype" | "clear-type" => Self::Cleartype,
            "aliased" | "alias" => Self::Aliased,
            _ => return None,
        })
    }

    /// What the mode does.
    pub const fn description(self) -> &'static str {
        match self {
            Self::Natural => "Grayscale antialiasing with grid-fitting.",
            Self::NaturalSymmetric => "Grayscale antialiasing, slightly softer.",
            Self::Gdi => "GDI-style hinting.",
            Self::Cleartype => "Subpixel ClearType.",
            Self::Aliased => "No antialiasing.",
        }
    }

    /// Whether the mode antialiases at all.
    pub const fn is_antialiased(self) -> bool {
        !matches!(self, Self::Aliased)
    }

    /// Whether the mode uses subpixel (RGB-order) coverage rather than grayscale.
    pub const fn uses_subpixel_coverage(self) -> bool {
        matches!(self, Self::Cleartype)
    }

    /// Whether the mode grid-fits the glyph box to whole pixels.
    ///
    /// The hinted paths (and the GDI path in particular) advance by whole
    /// pixels, so the renderer snaps [`crate::CellMetrics`] for them; the
    /// grayscale and subpixel paths keep the fractional box.
    pub const fn grid_fits(self) -> bool {
        matches!(self, Self::Natural | Self::NaturalSymmetric | Self::Gdi)
    }

    /// The setting that applies on `platform`, or `None` where the key is
    /// documented as ignored.
    pub fn resolve(platform: Platform, user: Option<Self>) -> Option<Self> {
        match platform {
            Platform::Windows => Some(user.unwrap_or_default()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blending_keys_default_to_the_theme_then_the_baseline() {
        assert_eq!(FontBlending::resolve(None, None), FontBlending::SrgbOver);
        assert_eq!(FontBlending::resolve(None, Some(FontBlending::Linear)), FontBlending::Linear);
        assert_eq!(
            FontBlending::resolve(Some(FontBlending::Perceptual), Some(FontBlending::Linear)),
            FontBlending::Perceptual
        );
    }

    #[test]
    fn default_is_a_setting_not_a_mode() {
        assert_eq!(FontBlending::parse_setting("default"), Some(None));
        assert_eq!(FontBlending::parse_setting("Linear"), Some(Some(FontBlending::Linear)));
        assert_eq!(FontBlending::parse_setting("nonsense"), None);
        assert_eq!(FontBlending::parse("nonsense"), None);
    }

    #[test]
    fn every_blending_spelling_round_trips() {
        for mode in FontBlending::ALL {
            assert_eq!(FontBlending::parse(mode.as_str()), Some(mode));
            assert!(!mode.description().is_empty());
            assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{}\"", mode.as_str()));
        }
    }

    #[test]
    fn windows_rendering_round_trips_and_is_windows_only() {
        for mode in WindowsTextRendering::ALL {
            assert_eq!(WindowsTextRendering::parse(mode.as_str()), Some(mode));
            assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{}\"", mode.as_str()));
        }
        assert!(!WindowsTextRendering::Aliased.is_antialiased());
        assert!(WindowsTextRendering::Natural.is_antialiased());
        assert!(WindowsTextRendering::Cleartype.uses_subpixel_coverage());
        assert!(!WindowsTextRendering::Natural.uses_subpixel_coverage());
        assert!(WindowsTextRendering::Gdi.grid_fits());
        assert!(!WindowsTextRendering::Cleartype.grid_fits());

        assert_eq!(
            WindowsTextRendering::resolve(Platform::Windows, None),
            Some(WindowsTextRendering::Natural)
        );
        assert_eq!(
            WindowsTextRendering::resolve(Platform::Windows, Some(WindowsTextRendering::Gdi)),
            Some(WindowsTextRendering::Gdi)
        );
        assert_eq!(WindowsTextRendering::resolve(Platform::Macos, None), None);
        assert_eq!(WindowsTextRendering::resolve(Platform::Linux, Some(WindowsTextRendering::Gdi)), None);
    }
}
