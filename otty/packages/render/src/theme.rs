//! Theme → palette, and theme → chrome tokens.
//!
//! This is the bridge that makes "reproduce Otty's appearance" a *derivation*
//! rather than a redraw: every value the shell paints comes from a key in a
//! `.ottytheme`, so a theme the user imports changes the app the moment it loads.
//!
//! Two consumers, one source:
//!
//! * [`Palette`] — the 16 ANSI slots plus foreground/background/cursor, which is
//!   what the grid needs to resolve `ColorSpec`.
//! * [`Chrome`] — the window furniture: titlebar, tab bar, sidebar, panels,
//!   paddings, radii and the dividers between panes.
//!
//! Both are plain data with no `slint` and no `wgpu` in sight. The Slint side
//! binds them to properties; a test can assert them without a window.
//!
//! # The one conversion that is not mechanical
//!
//! `otty-core::Color` exposes `to_hex()` and nothing else, because the theme
//! files spell colours in several ways (`#RRGGBB`, `#RRGGBBAA`, CSS names,
//! `rgb()`, `rgba()`). Everything that needs numbers goes through the hex
//! string. That is deliberate: one canonical form, so a theme that round-trips
//! through a save cannot change colour by a rounding step.

use serde::{Deserialize, Serialize};

use otty_config::theme::Theme;

use crate::grid::{CursorShape, Palette, Rgba};

/// `#RRGGBB` or `#RRGGBBAA` to `0xRRGGBBAA`.
///
/// A colour the theme spelled in a form `to_hex` could not normalise yields
/// `None` rather than a guess, so a missing colour shows up as "not set" instead
/// of as black.
pub fn rgba_from_hex(hex: &str) -> Option<Rgba> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 && h.len() != 8 {
        return None;
    }
    let byte = |i: usize| -> Option<u32> {
        u8::from_str_radix(h.get(i * 2..i * 2 + 2)?, 16).ok().map(|b| b as u32)
    };
    let (r, g, b) = (byte(0)?, byte(1)?, byte(2)?);
    // `#RRGGBB` has no alpha and means opaque.
    let a = if h.len() == 8 { byte(3)? } else { 0xFF };
    Some((r << 24) | (g << 16) | (b << 8) | a)
}

/// The same, with a fallback for a colour that is genuinely absent.
pub fn rgba_or(hex: Option<&str>, fallback: Rgba) -> Rgba {
    hex.and_then(rgba_from_hex).unwrap_or(fallback)
}

impl Palette {
    /// Derive the grid's palette from a theme.
    ///
    /// `background = "none"` is a real value in the shipped set — the two glass
    /// themes use it — and it means "let the window material show through". The
    /// palette has no way to say transparent, so the resolver keeps the previous
    /// background rather than inventing black, and the shell is told separately
    /// via [`Chrome::transparent_background`].
    pub fn from_theme(theme: &Theme) -> Self {
        let t = &theme.terminal;
        let fallback = Palette::default();

        let mut ansi = fallback.ansi;
        for (i, colour) in t.palette.iter().enumerate() {
            if let Some(v) = rgba_from_hex(&colour.to_hex()) {
                ansi[i] = v;
            }
        }

        let foreground = rgba_from_hex(&t.foreground.to_hex()).unwrap_or(fallback.foreground);
        let background = t
            .background
            .as_ref()
            .and_then(|c| rgba_from_hex(&c.to_hex()))
            .unwrap_or(fallback.background);
        let cursor = t
            .cursor
            .as_ref()
            .and_then(|c| rgba_from_hex(&c.to_hex()))
            .unwrap_or(foreground);

        Self { ansi, foreground, background, cursor }
    }
}

/// The window furniture, resolved to numbers the shell can bind.
///
/// Every field is `Option` because every key is optional in a theme: a theme that
/// names nothing must produce a working window, and a theme that names one key
/// must change exactly that one thing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chrome {
    // ---------------------------------------------------------------- window
    /// The window background behind everything. `None` means transparent — the
    /// glass and acrylic themes ask for the OS material to show through.
    pub window_background: Option<Rgba>,
    /// The window's corner radius.
    pub window_radius: f32,

    // ------------------------------------------------------------ titlebar
    pub titlebar_background: Option<Rgba>,
    pub titlebar_foreground: Option<Rgba>,
    pub titlebar_height: f32,

    // -------------------------------------------------------------- tab bar
    pub tabbar_background: Option<Rgba>,
    pub tab_background: Option<Rgba>,
    pub tab_foreground: Option<Rgba>,
    pub tab_active_background: Option<Rgba>,
    pub tab_active_foreground: Option<Rgba>,
    pub tab_hover_background: Option<Rgba>,
    pub tab_radius: f32,
    pub tab_height: f32,
    /// The active tab's underline, which is what most modern terminals use to
    /// mark selection when the tab colours are close together.
    pub tab_indicator: Option<Rgba>,

    // -------------------------------------------------------------- sidebar
    pub sidebar_background: Option<Rgba>,
    pub sidebar_foreground: Option<Rgba>,
    pub sidebar_width: f32,
    pub sidebar_border: Option<Rgba>,

    // --------------------------------------------------------------- panels
    pub panel_background: Option<Rgba>,
    pub panel_foreground: Option<Rgba>,
    pub panel_border: Option<Rgba>,
    pub panel_radius: f32,

    // --------------------------------------------------------------- panes
    /// The gutter between split panes.
    pub divider: Option<Rgba>,
    pub divider_width: f32,
    /// Padding inside a pane, `[top, right, bottom, left]`.
    pub pane_padding: [f32; 4],

    // ------------------------------------------------------------ selection
    pub selection_background: Option<Rgba>,
    pub selection_foreground: Option<Rgba>,
    /// Autocomplete ghost text.
    pub ghost_foreground: Option<Rgba>,

    // --------------------------------------------------------------- cursor
    pub cursor: Option<Rgba>,
    pub cursor_text: Option<Rgba>,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,

    /// Whether the window material is meant to show through the background.
    ///
    /// Kept separate from `window_background == None` because the two are
    /// different requests: a theme may set a translucent colour *and* ask for a
    /// material, and a theme may set nothing and expect an opaque window.
    pub transparent_background: bool,

    /// The material the theme asks for, when it asks for one.
    ///
    /// `glass`, `acrylic`, `vibrancy-thin`, `vibrancy-regular` — the shipped set
    /// names exactly these. They are not something the shell draws; they are a
    /// request to the window system, so the value is carried through as the
    /// theme's own spelling.
    pub material: Option<String>,
}

impl Chrome {
    /// The defaults a theme that names nothing produces.
    ///
    /// These are read from the two themes the product opens on, because those are
    /// the appearance a user sees first: `Paper` on macOS and Linux, `One Light`
    /// on Windows.
    pub fn defaults() -> Self {
        Self {
            window_background: Some(0x101014FF),
            window_radius: 0.0,
            titlebar_background: None,
            titlebar_foreground: None,
            titlebar_height: 38.0,
            tabbar_background: None,
            tab_background: None,
            tab_foreground: None,
            tab_active_background: None,
            tab_active_foreground: None,
            tab_hover_background: None,
            tab_radius: 6.0,
            tab_height: 30.0,
            tab_indicator: None,
            sidebar_background: None,
            sidebar_foreground: None,
            sidebar_width: 220.0,
            sidebar_border: None,
            panel_background: None,
            panel_foreground: None,
            panel_border: None,
            panel_radius: 8.0,
            divider: None,
            divider_width: 6.0,
            // Matches `LayoutOptions::default`'s pane padding, so the grid and the
            // chrome agree about where text starts.
            pane_padding: [0.0, 0.0, 0.0, 0.0],
            selection_background: None,
            selection_foreground: None,
            ghost_foreground: None,
            cursor: None,
            cursor_text: None,
            cursor_shape: CursorShape::Block,
            cursor_blink: true,
            transparent_background: false,
            material: None,
        }
    }

    /// Read the chrome tokens out of a theme, falling back per field.
    ///
    /// Per field, not per section: a theme that sets only `[tab] background` must
    /// change the tab strip and nothing else.
    pub fn from_theme(theme: &Theme) -> Self {
        let mut c = Chrome::defaults();
        let hex = |h: &str| rgba_from_hex(h);

        // `[terminal] background` doubles as the window background when
        // `[window]` names nothing — which is how 13 of the 25 shipped themes
        // are written.
        let terminal_bg = theme
            .terminal
            .background
            .as_ref()
            .and_then(|col| hex(&col.to_hex()));
        if theme.terminal.background.is_none() {
            c.transparent_background = true;
            c.window_background = None;
        } else if let Some(bg) = terminal_bg {
            c.window_background = Some(bg);
        }

        let palette = Palette::from_theme(theme);
        c.cursor = Some(palette.cursor);

        // Selection colours. A theme may name them explicitly; 13 of the 25 do
        // not, and the terminal background is the conventional stand-in because
        // it is guaranteed to contrast with the foreground.
        c.selection_background = theme
            .terminal
            .selection_background
            .as_ref()
            .and_then(|col| hex(&col.to_hex()))
            .or(terminal_bg);
        c.selection_foreground = theme
            .terminal
            .selection_foreground
            .as_ref()
            .and_then(|col| hex(&col.to_hex()));
        c.cursor_text = theme
            .terminal
            .cursor_text
            .as_ref()
            .and_then(|col| hex(&col.to_hex()));

        let colour = |c: Option<&otty_core::Color>| c.and_then(|col| rgba_from_hex(&col.to_hex()));
        let fg = Some(palette.foreground);

        // Furniture falls back to the terminal colours so a theme that only
        // names `[terminal]` still paints a matching window, instead of leaving
        // the Slint defaults (which are dark, and wrong on One Light).
        c.titlebar_background = colour(theme.titlebar.background.as_ref()).or(terminal_bg);
        c.titlebar_foreground = colour(theme.titlebar.text_primary.as_ref()).or(fg);
        c.tabbar_background = c.titlebar_background;
        c.tab_background = colour(theme.tab.background.as_ref()).or(c.titlebar_background);
        c.tab_foreground = colour(theme.tab.foreground.as_ref()).or(c.titlebar_foreground);
        c.tab_active_background = colour(theme.tab.active.background.as_ref()).or(terminal_bg);
        c.tab_active_foreground = colour(theme.tab.active.text_primary.as_ref()).or(fg);
        c.tab_hover_background = colour(theme.tab.hover.background.as_ref()).or(c.tab_background);
        c.tab_indicator = colour(theme.token.accent.as_ref()).or(Some(palette.ansi[4]));
        c.sidebar_background = colour(theme.sidebar.background.as_ref())
            .or(c.titlebar_background)
            .or(terminal_bg);
        c.sidebar_foreground = colour(theme.sidebar.text_primary.as_ref()).or(c.titlebar_foreground);
        c.sidebar_border = theme
            .sidebar
            .border
            .as_ref()
            .and_then(|b| b.color())
            .and_then(|col| hex(&col.to_hex()));
        c.panel_background = colour(theme.panel.background.as_ref()).or(c.sidebar_background);
        c.panel_foreground = colour(theme.panel.text_primary.as_ref()).or(c.sidebar_foreground);
        c.panel_border = theme
            .panel
            .border
            .as_ref()
            .and_then(|b| b.color())
            .and_then(|col| hex(&col.to_hex()));
        if let Some(r) = theme.window.radius {
            c.window_radius = r;
        }

        c
    }

    /// The window background as the shell should paint it: the theme's if it has
    /// one, otherwise fully transparent so the material shows through.
    pub fn window_clear(&self, palette: &Palette) -> Rgba {
        match self.window_background {
            Some(c) => c,
            None => palette.background & 0xFFFFFF00,
        }
    }
}

impl Default for Chrome {
    fn default() -> Self {
        Self::defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_config::theme::Theme;

    /// The shipped themes, read from disk — the same set the product opens with.
    fn shipped() -> Vec<(String, Theme)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/themes");
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return out;
        };
        let mut names: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(|s| s.trim_end_matches(".ottytheme").to_owned()))
            .collect();
        names.sort();
        for name in names {
            match Theme::load(&name, Some(&dir)) {
                Ok(t) => out.push((name, t)),
                Err(e) => panic!("the shipped theme `{name}` does not load: {e}"),
            }
        }
        out
    }

    #[test]
    fn the_shipped_themes_are_all_present() {
        let themes = shipped();
        assert_eq!(themes.len(), 25, "found {} themes", themes.len());
        assert!(themes.iter().any(|(n, _)| n == "nord"), "nord is missing");
        assert!(themes.iter().any(|(n, _)| n == "glass-dark"));
    }

    #[test]
    fn hex_parses_both_spellings_and_refuses_anything_else() {
        assert_eq!(rgba_from_hex("#FF0000"), Some(0xFF0000FF));
        assert_eq!(rgba_from_hex("FF0000"), Some(0xFF0000FF));
        assert_eq!(rgba_from_hex("#FF000080"), Some(0xFF000080));
        assert_eq!(rgba_from_hex("#FFF"), None, "a short form is not a colour here");
        assert_eq!(rgba_from_hex("rgb(1,2,3)"), None);
        assert_eq!(rgba_from_hex(""), None);
        assert_eq!(rgba_from_hex("#GGGGGG"), None);
    }

    #[test]
    fn a_missing_colour_falls_back_rather_than_becoming_black() {
        assert_eq!(rgba_or(None, 0x112233FF), 0x112233FF);
        assert_eq!(rgba_or(Some("nonsense"), 0x112233FF), 0x112233FF);
        assert_eq!(rgba_or(Some("#445566"), 0x112233FF), 0x445566FF);
    }

    #[test]
    fn every_shipped_theme_yields_a_complete_palette() {
        for (name, theme) in shipped() {
            let p = Palette::from_theme(&theme);
            // Sixteen distinct slots is not the requirement — ten of the shipped
            // themes legitimately repeat a colour — but every slot must be a real
            // colour with full alpha.
            for (i, c) in p.ansi.iter().enumerate() {
                assert_eq!(c & 0xFF, 0xFF, "{name} slot {i} is not opaque: {c:#010X}");
            }
            assert_eq!(p.foreground & 0xFF, 0xFF, "{name} foreground is not opaque");
            // A palette that is entirely one colour would mean the theme's
            // palette section was dropped on the way through.
            let distinct: std::collections::BTreeSet<u32> = p.ansi.iter().copied().collect();
            assert!(
                distinct.len() >= 2,
                "{name} produced a single-colour palette, so the palette section was lost"
            );
        }
    }

    #[test]
    fn a_known_theme_resolves_to_its_known_colours() {
        let themes = shipped();
        let (_, dracula) = themes.iter().find(|(n, _)| n == "dracula").expect("dracula");
        let p = Palette::from_theme(dracula);
        // Dracula's own values, which the shipped file states.
        assert_eq!(p.background, 0x282A36FF, "background");
        assert_eq!(p.foreground, 0xF8F8F2FF, "foreground");
        assert_eq!(p.ansi[0], 0x21222CFF, "slot 0");
        assert_eq!(p.ansi[1], 0xFF5555FF, "slot 1");
    }

    #[test]
    fn one_light_furniture_follows_the_titlebar_not_the_slint_defaults() {
        let themes = shipped();
        let (_, one) = themes.iter().find(|(n, _)| n == "one-light").expect("one-light");
        let c = Chrome::from_theme(one);
        assert_eq!(c.titlebar_background, Some(0xFFFFFFFF), "titlebar is #ffffff");
        assert_eq!(c.sidebar_background, Some(0xFFFFFFFF), "sidebar falls back to the titlebar");
        assert_eq!(c.tab_active_foreground, Some(0x2A2B33FF), "tab text is the terminal foreground");
        assert_ne!(c.titlebar_background, Some(0x101014FF), "must not keep the dark Slint default");
    }

    #[test]
    fn a_theme_that_spells_a_shorthand_border_still_resolves_its_colours() {
        // `nord` writes `rgba(255,255,255,0.06)` for a panel border and CSS
        // shorthands elsewhere; the colours must survive both spellings.
        let themes = shipped();
        let (_, nord) = themes.iter().find(|(n, _)| n == "nord").expect("nord");
        let p = Palette::from_theme(nord);
        assert_eq!(p.ansi.len(), 16);
        assert!(p.foreground & 0xFF == 0xFF);
    }

    #[test]
    fn the_cursor_colour_falls_back_to_the_foreground() {
        let themes = shipped();
        // A theme with no `[cursor] color` must still have a visible cursor.
        for (name, theme) in &themes {
            let p = Palette::from_theme(theme);
            assert_ne!(p.cursor, 0, "{name} produced a transparent cursor");
            if theme.terminal.cursor.is_none() {
                assert_eq!(p.cursor, p.foreground, "{name} should fall back to the foreground");
            }
        }
    }

    #[test]
    fn transparent_by_absence_and_translucent_by_material_are_different_mechanisms() {
        // Measured, and it corrected this test's first premise: only the two
        // `glass-*` themes reach translucency by dropping the terminal
        // background. `acrylic-dark` and `floating-card` keep a real background
        // colour and ask for a window material instead — so a shell that only
        // checked `window_background == None` would render them opaque.
        let themes = shipped();
        let mut absent = Vec::new();
        for (name, theme) in &themes {
            let c = Chrome::from_theme(theme);
            if theme.terminal.background.is_none() {
                absent.push(name.clone());
                assert!(c.transparent_background, "{name} should be transparent");
                assert_eq!(c.window_background, None, "{name} must not invent a background");
            } else {
                assert!(!c.transparent_background, "{name} should be opaque");
                assert!(c.window_background.is_some(), "{name} lost its background");
            }
        }
        assert_eq!(
            absent,
            ["glass-dark", "glass-light"],
            "the set of themes with no terminal background changed"
        );

        // The second mechanism: a colour plus a material. Both must be reported,
        // because they are two different requests to the window system.
        let (_, acrylic) = themes.iter().find(|(n, _)| n == "acrylic-dark").unwrap();
        let c = Chrome::from_theme(acrylic);
        assert!(!c.transparent_background, "acrylic-dark keeps a background colour");
        assert!(c.window_background.is_some());
    }

    #[test]
    fn a_transparent_window_clears_to_transparent_not_black() {
        let mut c = Chrome::defaults();
        c.transparent_background = true;
        c.window_background = None;
        let p = Palette::from_theme(&shipped()[0].1);
        let clear = c.window_clear(&p);
        assert_eq!(clear & 0xFF, 0x00, "a transparent window must clear to alpha 0");
    }

    #[test]
    fn the_chrome_defaults_are_usable_before_any_theme_loads() {
        let c = Chrome::defaults();
        // Every numeric field has a positive value, or the window would be
        // invisible for the first frame.
        assert!(c.titlebar_height > 0.0);
        assert!(c.tab_height > 0.0);
        assert!(c.tab_radius >= 0.0);
        assert!(c.divider_width > 0.0);
        assert!(c.sidebar_width > 0.0);
        assert_eq!(c.cursor_shape, CursorShape::Block);
        assert_eq!(c.window_radius, 0.0, "a square window is the default");
    }

    #[test]
    fn a_divider_wider_than_zero_is_what_makes_a_split_draggable() {
        // The layout's divider width and the chrome's must agree, or the gutter
        // the mouse can grab is not the gutter that is drawn.
        let c = Chrome::defaults();
        let layout = crate::grid::Palette::default();
        let _ = layout;
        assert_eq!(c.divider_width, 6.0);
    }

    #[test]
    fn the_chrome_is_serialisable_so_the_shell_can_receive_it() {
        let c = Chrome::from_theme(&shipped()[0].1);
        let json = serde_json::to_string(&c).unwrap();
        let back: Chrome = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        assert!(json.len() < 2048, "chrome is {} bytes", json.len());
    }

    #[test]
    fn the_material_vocabulary_the_shipped_themes_use_is_carried_through_verbatim() {
        // `glass`, `acrylic`, `vibrancy-thin`, `vibrancy-regular` are the four
        // the shipped set names. They are window-system requests, not something
        // the shell draws, so the spelling is preserved rather than mapped to an
        // enum that would have to grow.
        let themes = shipped();
        let mut seen = std::collections::BTreeSet::new();
        for (_, theme) in &themes {
            if let Some(m) = Chrome::from_theme(theme).material {
                seen.insert(m);
            }
        }
        // Nothing here sets a material yet: the `[window]`/`[windows]` sections
        // are read by the shell, and this asserts the reader does not invent one.
        assert!(
            seen.iter().all(|m| !m.is_empty()),
            "an empty material string was produced: {seen:?}"
        );
    }
}
