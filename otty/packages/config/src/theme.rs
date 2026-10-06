//! Theme files: `~/.config/otty/themes/<slug>.ottytheme`.
//!
//! Unlike the main config, a theme is **real TOML**. Only `[terminal]` is
//! required. A theme may `inherits = "other-theme"` and then override
//! individual keys, which is implemented as a deep table merge against the
//! named base before the typed view is built — so an override only has to name
//! what it changes.
//!
//! The backdrop sections are the one place where a theme can be platform
//! specific: `[window]` is the fallback every platform reads, `[mac]` and
//! `[windows]` override it per OS, and a material written for the *other*
//! platform is mapped across rather than dropped.

use std::fmt;
use std::path::{Path, PathBuf};

use otty_core::{Color, Platform};

/// Whether a theme is meant for light or dark mode. Drives the automatic
/// light/dark switch (`auto-theme-dark-mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Dark,
    Light,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            _ => None,
        }
    }

    /// A theme's mode is detected from its background when `[meta].mode` is
    /// absent: a background whose luminance is below mid-grey is dark.
    pub fn detect(background: Color) -> Self {
        if background.luminance() < 0.35 {
            Self::Dark
        } else {
            Self::Light
        }
    }
}

/// A window backdrop material.
///
/// The macOS and Windows names are distinct effects, not aliases, so the enum
/// keeps both sets and knows how to translate between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Material {
    None,
    // macOS
    VibrancyThin,
    VibrancyRegular,
    VibrancyThick,
    Glass,
    // Windows
    AcrylicThin,
    Acrylic,
    Mica,
    MicaAlt,
}

impl Material {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "vibrancy-thin" => Self::VibrancyThin,
            "vibrancy-regular" => Self::VibrancyRegular,
            "vibrancy-thick" => Self::VibrancyThick,
            "glass" => Self::Glass,
            "acrylic-thin" => Self::AcrylicThin,
            "acrylic" => Self::Acrylic,
            "mica" => Self::Mica,
            "mica-alt" => Self::MicaAlt,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::VibrancyThin => "vibrancy-thin",
            Self::VibrancyRegular => "vibrancy-regular",
            Self::VibrancyThick => "vibrancy-thick",
            Self::Glass => "glass",
            Self::AcrylicThin => "acrylic-thin",
            Self::Acrylic => "acrylic",
            Self::Mica => "mica",
            Self::MicaAlt => "mica-alt",
        }
    }

    /// Whether the material is one the given platform can actually render.
    pub const fn is_native_to(self, platform: Platform) -> bool {
        match platform {
            Platform::Macos => matches!(
                self,
                Self::None | Self::VibrancyThin | Self::VibrancyRegular | Self::VibrancyThick
                    | Self::Glass
            ),
            Platform::Windows => matches!(
                self,
                Self::None | Self::AcrylicThin | Self::Acrylic | Self::Mica | Self::MicaAlt
            ),
            // Linux reads `[window]` only and has no system blur vocabulary.
            _ => matches!(self, Self::None),
        }
    }

    /// Translate a material written for another platform.
    ///
    /// The mapping is the one documented for theme portability:
    /// `vibrancy-thin`↔`acrylic-thin`, `vibrancy-regular`↔`mica`,
    /// `vibrancy-thick`↔`mica-alt`, `glass`↔`acrylic`.
    pub const fn cross_platform(self) -> Self {
        match self {
            Self::VibrancyThin => Self::AcrylicThin,
            Self::VibrancyRegular => Self::Mica,
            Self::VibrancyThick => Self::MicaAlt,
            Self::Glass => Self::Acrylic,
            Self::AcrylicThin => Self::VibrancyThin,
            Self::Acrylic => Self::Glass,
            Self::Mica => Self::VibrancyRegular,
            Self::MicaAlt => Self::VibrancyThick,
            Self::None => Self::None,
        }
    }

    /// This material as the given platform would spell it, or [`Material::None`]
    /// when the platform has no equivalent effect at all.
    ///
    /// A single translation is not always enough: `glass` on Linux has no
    /// vocabulary, and `vibrancy-thick` translated for Windows gives `mica-alt`,
    /// which *is* native. The loop handles both by translating once and
    /// checking the result.
    pub const fn for_platform(self, platform: Platform) -> Self {
        if self.is_native_to(platform) {
            return self;
        }
        let translated = self.cross_platform();
        if translated.is_native_to(platform) {
            translated
        } else {
            Self::None
        }
    }
}

/// A window backdrop: material, its tint, and how strongly the tint beats the
/// blurred desktop.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Backdrop {
    pub material: Option<Material>,
    pub color: Option<Color>,
    pub opacity: Option<f32>,
    pub radius: Option<f32>,
}

impl Backdrop {
    /// Fill every unset field from `base`.
    pub fn or(self, base: Self) -> Self {
        Self {
            material: self.material.or(base.material),
            color: self.color.or(base.color),
            opacity: self.opacity.or(base.opacity),
            radius: self.radius.or(base.radius),
        }
    }

    /// Resolve the material for a platform, translating across the two families
    /// when the theme only named one.
    pub fn material_for(&self, platform: Platform) -> Material {
        self.material.unwrap_or(Material::None).for_platform(platform)
    }
}

/// The `[token]` section: typography and shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub accent: Option<Color>,
    /// Terminal grid font, as a family name or an ordered fallback stack.
    pub font_mono: Option<Vec<String>>,
    /// Window-chrome font.
    pub font_ui: Option<Vec<String>>,
    pub font_size: Option<f32>,
    /// Ghostty-compatible line-height delta: `Npx`, `N`, `N%`, or `0`.
    pub adjust_cell_height: Option<String>,
}

impl Default for Token {
    fn default() -> Self {
        Self {
            accent: None,
            font_mono: None,
            font_ui: None,
            font_size: None,
            adjust_cell_height: None,
        }
    }
}

/// The `[container]` section — the rounded card the terminal grid sits in.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Container {
    pub radius: Option<f32>,
    pub shadow: Option<String>,
    pub border: Option<String>,
    /// `[top, right, bottom, left]`, or a single scalar applied to all edges.
    pub padding: Option<[f32; 4]>,
    pub margin: Option<[f32; 4]>,
}

/// A border as the theme files spell it. The product writes both spellings in
/// the same key: a CSS shorthand (`1px solid #FFFFFF14`) on chrome, and a bare
/// colour (`rgba(255,255,255,0.06)`) on panels.
///
/// The [`BorderSpec::Shorthand`] arm keeps the source text verbatim on purpose.
/// Pulling a colour out of `1px solid #FFFFFF14` would be a normalisation, and
/// the shipped theme bytes are the specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorderSpec {
    /// A bare colour, which [`Color::parse`] accepted as written.
    Color(Color),
    /// Anything else — the CSS shorthand the chrome writes — kept exactly as
    /// the theme file spelled it.
    Shorthand(String),
}

impl BorderSpec {
    /// Read a `border` value: a colour when [`Color::parse`] takes it, the raw
    /// text when it does not. `slug` and `key` only shape the error.
    pub fn parse_at(slug: &str, key: &str, v: &toml::Value) -> Result<Self, ThemeError> {
        let Some(s) = v.as_str() else {
            return Err(ThemeError::BadColor {
                slug: slug.to_owned(),
                key: key.to_owned(),
                value: v.to_string(),
            });
        };
        Ok(match Color::parse(s) {
            Ok(c) => Self::Color(c),
            Err(_) => Self::Shorthand(s.to_owned()),
        })
    }

    /// [`BorderSpec::parse_at`] for a caller that has the value but no key to
    /// name in an error.
    pub fn parse(v: &toml::Value) -> Result<Self, ThemeError> {
        Self::parse_at("", "border", v)
    }

    /// The colour, when the border was spelled as one.
    ///
    /// A shorthand reports `None`: its colour is a property of the shorthand
    /// grammar, not of the key, and picking it apart is the normalisation the
    /// raw text exists to avoid.
    pub const fn color(&self) -> Option<Color> {
        match self {
            Self::Color(c) => Some(*c),
            Self::Shorthand(_) => None,
        }
    }

    /// The border as text, in the spelling the theme file used: a bare colour
    /// is written as hex, a shorthand is handed back verbatim.
    pub fn as_str(&self) -> String {
        match self {
            Self::Color(c) => c.to_hex(),
            Self::Shorthand(s) => s.clone(),
        }
    }
}

impl fmt::Display for BorderSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Color(c) => write!(f, "{c}"),
            Self::Shorthand(s) => f.write_str(s),
        }
    }
}

/// A generic surface: the shape `[panel]`, `[sidebar]`, `[titlebar]` share.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Surface {
    pub background: Option<Color>,
    pub surface: Option<Color>,
    /// Both spellings the format allows; see [`BorderSpec`].
    pub border: Option<BorderSpec>,
    pub text_primary: Option<Color>,
    pub text_secondary: Option<Color>,
    pub text_tertiary: Option<Color>,
    pub hover: Option<Color>,
    pub active: Option<Color>,
    /// Only meaningful on `[sidebar]` / `[windows.sidebar]`.
    pub backdrop: Backdrop,
}

/// The `[selection]` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
}

/// The `[terminal]` section: the only required one.
#[derive(Debug, Clone, PartialEq)]
pub struct Terminal {
    pub foreground: Color,
    /// `background = "none"` makes the terminal background transparent.
    pub background: Option<Color>,
    pub palette: [Color; 16],
    pub cursor: Option<Color>,
    pub cursor_text: Option<Color>,
    pub selection_foreground: Option<Color>,
    pub selection_background: Option<Color>,
}

/// The `[cursor]` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShape {
    #[default]
    Block,
    Underline,
    Beam,
}

impl CursorShape {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "block" => Self::Block,
            "underline" => Self::Underline,
            "beam" | "bar" => Self::Beam,
            _ => return None,
        })
    }

    /// The canonical spelling, which is the one [`Theme::to_toml`] writes back:
    /// `bar` parses as [`CursorShape::Beam`], and `beam` is what it is called.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Underline => "underline",
            Self::Beam => "beam",
        }
    }
}

/// A fully resolved theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// The file-name slug, which is how `theme = <name>` names it.
    pub slug: String,
    pub display_name: String,
    pub mode: Mode,
    pub author: Option<String>,
    pub description: Option<String>,
    pub inherits: Option<String>,
    pub terminal: Terminal,
    pub token: Token,
    pub container: Container,
    pub ui: Surface,
    pub panel: Surface,
    pub sidebar: Surface,
    pub titlebar: Surface,
    pub tab: TabStyle,
    pub selection: Selection,
    pub cursor: CursorStyleSpec,
    pub ghost: Ghost,
    pub window: Backdrop,
    pub mac: Backdrop,
    pub windows: Backdrop,
    /// `[mac.sidebar]` / `[windows.sidebar]` — the chrome-column-only backdrop.
    pub mac_sidebar: Backdrop,
    pub windows_sidebar: Backdrop,
    /// The fully `inherits`-merged table the typed view above was read from,
    /// exactly as parsed. This is the theme's own superset: the typed fields are
    /// a *reading* of it, so [`Theme::to_toml`] starts here and a theme carrying
    /// keys this build does not model survives being re-saved.
    pub raw: toml::Table,
    /// `raw`'s top-level tables that the typed view does not model — the keys a
    /// newer Otty may understand and this build must not destroy. Derived from
    /// `raw` when the theme is built; read through [`Theme::extra`].
    pub(crate) extra_tables: toml::Table,
    /// The leading comment banner of the source text, re-emitted by
    /// [`Theme::to_toml`] so a saved theme keeps its own header.
    pub(crate) header: String,
}

/// The `[tab]` section, with its `[tab.active]` and `[tab.hover]` children.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TabStyle {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub active: Surface,
    pub hover: Surface,
}

/// The `[cursor]` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorStyleSpec {
    pub color: Option<Color>,
    pub text: Option<Color>,
    pub style: Option<CursorShape>,
    pub blink: Option<bool>,
}

/// The `[ghost]` section — autocomplete ghost text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ghost {
    pub foreground: Option<Color>,
}

/// Failure to load a theme.
#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("theme `{slug}` is not valid TOML: {source}")]
    Toml { slug: String, source: Box<toml::de::Error> },
    #[error("theme `{slug}` is missing the required `[terminal]` section")]
    NoTerminal { slug: String },
    #[error("theme `{slug}` must define `[terminal].foreground`")]
    NoForeground { slug: String },
    #[error("theme `{slug}`: `{key}` is not a colour: {value}")]
    BadColor { slug: String, key: String, value: String },
    #[error("theme `{slug}`: `[terminal].palette` must hold exactly 16 colours, found {found}")]
    BadPalette { slug: String, found: usize },
    #[error("theme `{slug}`: inherits `{base}`, which does not exist")]
    UnknownBase { slug: String, base: String },
    #[error("theme `{slug}`: inheritance cycle through `{base}`")]
    InheritCycle { slug: String, base: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// The themes compiled into the binary, keyed by slug.
///
/// The files under `resources/themes/` are the single source of truth: they are
/// embedded here *and* shipped in the app bundle, so a user can copy one out,
/// edit it, and drop it back into `~/.config/otty/themes/`.
pub const BUILTIN_THEMES: &[(&str, &str)] = &[
    ("acrylic-dark", include_str!("../../../resources/themes/acrylic-dark.ottytheme")),
    ("april", include_str!("../../../resources/themes/april.ottytheme")),
    ("april-dark", include_str!("../../../resources/themes/april-dark.ottytheme")),
    ("ayu-dark", include_str!("../../../resources/themes/ayu-dark.ottytheme")),
    ("ayu-light", include_str!("../../../resources/themes/ayu-light.ottytheme")),
    ("catppuccin-mocha", include_str!("../../../resources/themes/catppuccin-mocha.ottytheme")),
    ("dracula", include_str!("../../../resources/themes/dracula.ottytheme")),
    ("floating-card", include_str!("../../../resources/themes/floating-card.ottytheme")),
    ("glass-dark", include_str!("../../../resources/themes/glass-dark.ottytheme")),
    ("glass-light", include_str!("../../../resources/themes/glass-light.ottytheme")),
    ("gruvbox-dark", include_str!("../../../resources/themes/gruvbox-dark.ottytheme")),
    ("monokai-classic", include_str!("../../../resources/themes/monokai-classic.ottytheme")),
    ("newsprint", include_str!("../../../resources/themes/newsprint.ottytheme")),
    ("night", include_str!("../../../resources/themes/night.ottytheme")),
    ("nord", include_str!("../../../resources/themes/nord.ottytheme")),
    ("one-dark", include_str!("../../../resources/themes/one-dark.ottytheme")),
    ("one-light", include_str!("../../../resources/themes/one-light.ottytheme")),
    ("owl", include_str!("../../../resources/themes/owl.ottytheme")),
    ("paper", include_str!("../../../resources/themes/paper.ottytheme")),
    ("pink", include_str!("../../../resources/themes/pink.ottytheme")),
    ("rose-pine", include_str!("../../../resources/themes/rose-pine.ottytheme")),
    ("seafoam-pastel", include_str!("../../../resources/themes/seafoam-pastel.ottytheme")),
    ("solarized-dark", include_str!("../../../resources/themes/solarized-dark.ottytheme")),
    ("solarized-light", include_str!("../../../resources/themes/solarized-light.ottytheme")),
    ("tokyo-night", include_str!("../../../resources/themes/tokyo-night.ottytheme")),
];

/// The light / dark defaults. macOS and Linux open on Paper / Nord; Windows uses
/// the two themes that carry the Windows chrome treatment.
pub fn default_theme_names(platform: Platform) -> (&'static str, &'static str) {
    match platform {
        Platform::Windows => ("one-light", "acrylic-dark"),
        _ => ("paper", "nord"),
    }
}

/// Turn a display name into a file slug the way the importer does.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "theme".to_owned()
    } else {
        out
    }
}

fn color_of(slug: &str, key: &str, v: &toml::Value) -> Result<Option<Color>, ThemeError> {
    let Some(s) = v.as_str() else {
        return Err(ThemeError::BadColor {
            slug: slug.to_owned(),
            key: key.to_owned(),
            value: v.to_string(),
        });
    };
    if s.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    Color::parse(s)
        .map(Some)
        .map_err(|_| ThemeError::BadColor {
            slug: slug.to_owned(),
            key: key.to_owned(),
            value: s.to_owned(),
        })
}

fn get_color(t: &toml::Table, slug: &str, key: &str) -> Result<Option<Color>, ThemeError> {
    match t.get(key) {
        Some(v) => color_of(slug, key, v),
        None => Ok(None),
    }
}

/// A `border` key, which the format spells two ways; see [`BorderSpec`].
fn get_border(t: &toml::Table, slug: &str, key: &str) -> Result<Option<BorderSpec>, ThemeError> {
    match t.get(key) {
        Some(v) => BorderSpec::parse_at(slug, key, v).map(Some),
        None => Ok(None),
    }
}

fn get_f32(t: &toml::Table, key: &str) -> Option<f32> {
    t.get(key).and_then(|v| match v {
        toml::Value::Float(f) => Some(*f as f32),
        toml::Value::Integer(i) => Some(*i as f32),
        _ => None,
    })
}

fn get_str(t: &toml::Table, key: &str) -> Option<String> {
    t.get(key).and_then(|v| v.as_str()).map(str::to_owned)
}

/// A font stack: a bare string, or an inline array of names.
fn get_font_stack(t: &toml::Table, key: &str) -> Option<Vec<String>> {
    match t.get(key)? {
        toml::Value::String(s) => Some(vec![s.clone()]),
        toml::Value::Array(a) => Some(
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    }
}

fn get_edges(t: &toml::Table, key: &str) -> Option<[f32; 4]> {
    match t.get(key)? {
        toml::Value::Integer(i) => Some([*i as f32; 4]),
        toml::Value::Float(f) => Some([*f as f32; 4]),
        toml::Value::Array(a) => {
            let v: Vec<f32> = a
                .iter()
                .filter_map(|x| match x {
                    toml::Value::Integer(i) => Some(*i as f32),
                    toml::Value::Float(f) => Some(*f as f32),
                    _ => None,
                })
                .collect();
            match v.len() {
                1 => Some([v[0]; 4]),
                2 => Some([v[0], v[1], v[0], v[1]]),
                4 => Some([v[0], v[1], v[2], v[3]]),
                _ => None,
            }
        }
        _ => None,
    }
}

fn parse_backdrop(t: &toml::Table, slug: &str, section: &str) -> Result<Backdrop, ThemeError> {
    let material = get_str(t, "material").and_then(|m| {
        Material::parse(&m).or_else(|| {
            log::warn!("theme `{slug}`: [{section}] material `{m}` is unknown, ignoring");
            None
        })
    });
    // `color` is the documented short spelling of `background`.
    let color = match get_color(t, slug, "color")? {
        Some(c) => Some(c),
        None => get_color(t, slug, "background")?,
    };
    Ok(Backdrop {
        material,
        color,
        opacity: get_f32(t, "opacity"),
        radius: get_f32(t, "radius"),
    })
}

fn parse_surface(t: &toml::Table, slug: &str) -> Result<Surface, ThemeError> {
    Ok(Surface {
        background: get_color(t, slug, "background")?,
        surface: get_color(t, slug, "surface")?,
        border: get_border(t, slug, "border")?,
        text_primary: get_color(t, slug, "text-primary")?,
        text_secondary: get_color(t, slug, "text-secondary")?,
        text_tertiary: get_color(t, slug, "text-tertiary")?,
        hover: get_color(t, slug, "hover")?,
        active: get_color(t, slug, "active")?,
        backdrop: Backdrop::default(),
    })
}

/// The one top-level name per typed section. Every other name in a theme — and
/// every key inside these sections that the typed view does not read — is
/// carried by [`Theme::raw`] alone, which is what keeps a save lossless.
const MODELLED_SECTIONS: &[&str] = &[
    "meta", "terminal", "token", "container", "ui", "panel", "sidebar", "titlebar", "tab",
    "selection", "cursor", "ghost", "window", "mac", "windows", "inherits",
];

/// The leading comment banner of a theme file: the `#` lines before the first
/// key or section header, blank lines between them included.
fn leading_comment(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            out.push_str(line);
            out.push('\n');
        } else {
            break;
        }
    }
    out
}

// ── Writing the typed view back over `raw` ────────────────────────────────
//
// `Theme::to_toml` clones `raw` and hands the clone to `Over`, one call per
// modelled key. What decides whether a key is written is *change*, not presence:
// the comparisons read a second table — `raw` again, but with every modelled key
// spelled the way `raw` itself reads it — so a key is written exactly when the
// typed field no longer agrees with the source.
//
// That is what makes a save lossless. A theme nobody touched in memory comes
// back with nothing written at all, so the source's own bytes stand — uppercase
// hex, `rgba(...)` where the hex would be shorter, `1px solid #E0DFD5` — and
// every key the typed view does not read, 201 of them across the shipped themes,
// is never even looked at. It also keeps the typed view's fallbacks out of the
// file: a detected `mode`, the default palette, a `[selection]` that the theme
// spells as `[terminal].selection-*` are all readings, and a reading that has
// not been changed is not something to record over the author's text.

/// The table one overlay pass compares against.
#[derive(Clone, Copy)]
struct Over<'a> {
    /// What the comparisons read: for the emitting pass, `raw` with every
    /// modelled key spelled the way `raw` reads it.
    reference: &'a toml::Table,
    /// True when there is nothing to compare against — a theme an importer built
    /// in memory, with no source text at all — so every key the typed view has
    /// is written out rather than diffed.
    write_all: bool,
}

/// The sub-table at `path`, when there is one.
fn section<'t>(t: &'t toml::Table, path: &[&str]) -> Option<&'t toml::Table> {
    let mut cur = t;
    for p in path {
        cur = cur.get(*p)?.as_table()?;
    }
    Some(cur)
}

/// The sub-table at `path` in the table being emitted, created when missing.
fn table_at<'t>(t: &'t mut toml::Table, path: &[&str]) -> &'t mut toml::Table {
    let mut cur = t;
    for p in path {
        if !cur.get(*p).is_some_and(toml::Value::is_table) {
            cur.insert((*p).to_owned(), toml::Value::Table(toml::Table::new()));
        }
        cur = cur
            .get_mut(*p)
            .and_then(toml::Value::as_table_mut)
            .expect("the entry was just made a table");
    }
    cur
}

fn text_at(t: &toml::Table, path: &[&str], key: &str) -> Option<String> {
    section(t, path).and_then(|s| get_str(s, key))
}

fn color_at(t: &toml::Table, path: &[&str], key: &str) -> Option<Color> {
    section(t, path)
        .and_then(|s| s.get(key))
        .and_then(toml::Value::as_str)
        .and_then(|s| Color::parse(s).ok())
}

fn number_at(t: &toml::Table, path: &[&str], key: &str) -> Option<f32> {
    section(t, path).and_then(|s| get_f32(s, key))
}

fn bool_at(t: &toml::Table, path: &[&str], key: &str) -> Option<bool> {
    section(t, path).and_then(|s| s.get(key)).and_then(toml::Value::as_bool)
}

fn stack_at(t: &toml::Table, path: &[&str], key: &str) -> Option<Vec<String>> {
    section(t, path).and_then(|s| get_font_stack(s, key))
}

fn edges_at(t: &toml::Table, path: &[&str], key: &str) -> Option<[f32; 4]> {
    section(t, path).and_then(|s| get_edges(s, key))
}

impl<'a> Over<'a> {
    /// Diff against a reading of the source: write a key only when the typed
    /// value does not match it.
    fn against(reference: &'a toml::Table) -> Self {
        Over { reference, write_all: false }
    }

    /// Write every key the typed view has, for a theme with no source.
    fn everything(reference: &'a toml::Table) -> Self {
        Over { reference, write_all: true }
    }

    /// `reads_the_same` is the whole decision: a typed value that matches what
    /// the source reads as is already in the file, in the source's own spelling.
    fn writes(&self, reads_the_same: bool) -> bool {
        self.write_all || !reads_the_same
    }

    fn put(
        &self,
        out: &mut toml::Table,
        path: &[&str],
        key: &str,
        value: toml::Value,
        reads_the_same: bool,
    ) {
        if self.writes(reads_the_same) {
            table_at(out, path).insert(key.to_owned(), value);
        }
    }

    fn put_str(&self, out: &mut toml::Table, path: &[&str], key: &str, v: &str) {
        let same = text_at(self.reference, path, key).as_deref() == Some(v);
        self.put(out, path, key, toml::Value::String(v.to_owned()), same);
    }

    fn put_opt_str(&self, out: &mut toml::Table, path: &[&str], key: &str, v: Option<&str>) {
        if let Some(v) = v {
            self.put_str(out, path, key, v);
        }
    }

    fn put_f32(&self, out: &mut toml::Table, path: &[&str], key: &str, v: f32) {
        let same = number_at(self.reference, path, key) == Some(v);
        self.put(out, path, key, toml::Value::Float(v as f64), same);
    }

    fn put_opt_f32(&self, out: &mut toml::Table, path: &[&str], key: &str, v: Option<f32>) {
        if let Some(v) = v {
            self.put_f32(out, path, key, v);
        }
    }

    fn put_color(&self, out: &mut toml::Table, path: &[&str], key: &str, c: Color) {
        let same = color_at(self.reference, path, key) == Some(c);
        self.put(out, path, key, toml::Value::String(c.to_hex()), same);
    }

    fn put_opt_color(&self, out: &mut toml::Table, path: &[&str], key: &str, c: Option<Color>) {
        if let Some(c) = c {
            self.put_color(out, path, key, c);
        }
    }

    /// A backdrop tint, which the format spells `color` or `background`. Both
    /// spellings are compared: a tint the source wrote as `background` must not
    /// be written again as `color`.
    fn put_backdrop_color(&self, out: &mut toml::Table, path: &[&str], c: Color) {
        let same = color_at(self.reference, path, "color") == Some(c)
            || color_at(self.reference, path, "background") == Some(c);
        self.put(out, path, "color", toml::Value::String(c.to_hex()), same);
    }

    fn put_opt_stack(
        &self,
        out: &mut toml::Table,
        path: &[&str],
        key: &str,
        names: Option<&[String]>,
    ) {
        let Some(names) = names else { return };
        let same = stack_at(self.reference, path, key).as_deref() == Some(names);
        let value = names.iter().map(|n| toml::Value::String(n.clone())).collect();
        self.put(out, path, key, toml::Value::Array(value), same);
    }

    fn put_opt_edges(
        &self,
        out: &mut toml::Table,
        path: &[&str],
        key: &str,
        edges: Option<[f32; 4]>,
    ) {
        let Some(edges) = edges else { return };
        let same = edges_at(self.reference, path, key) == Some(edges);
        let value = edges.iter().map(|e| toml::Value::Float(*e as f64)).collect();
        self.put(out, path, key, toml::Value::Array(value), same);
    }

    /// The one key the format spells two ways. A shorthand is compared as text —
    /// it is the source's own bytes — and a colour as the colour it is.
    fn put_opt_border(
        &self,
        out: &mut toml::Table,
        path: &[&str],
        key: &str,
        border: Option<&BorderSpec>,
    ) {
        let Some(border) = border else { return };
        let same = match border {
            BorderSpec::Shorthand(s) => {
                text_at(self.reference, path, key).as_deref() == Some(s.as_str())
            }
            BorderSpec::Color(c) => color_at(self.reference, path, key) == Some(*c),
        };
        self.put(out, path, key, toml::Value::String(border.as_str()), same);
    }

    fn put_material(&self, out: &mut toml::Table, path: &[&str], m: Material) {
        let same = text_at(self.reference, path, "material").and_then(|s| Material::parse(&s))
            == Some(m);
        self.put(out, path, "material", toml::Value::String(m.as_str().to_owned()), same);
    }

    fn put_mode(&self, out: &mut toml::Table, m: Mode) {
        let same = text_at(self.reference, &["meta"], "mode").and_then(|s| Mode::parse(&s))
            == Some(m);
        self.put(out, &["meta"], "mode", toml::Value::String(m.as_str().to_owned()), same);
    }

    fn put_cursor_shape(&self, out: &mut toml::Table, s: CursorShape) {
        let same = text_at(self.reference, &["cursor"], "style").and_then(|s| CursorShape::parse(&s))
            == Some(s);
        self.put(out, &["cursor"], "style", toml::Value::String(s.as_str().to_owned()), same);
    }

    fn put_bool(&self, out: &mut toml::Table, path: &[&str], key: &str, b: bool) {
        let same = bool_at(self.reference, path, key) == Some(b);
        self.put(out, path, key, toml::Value::Boolean(b), same);
    }

    /// The 16 ANSI colours. The typed palette is never `None` — a theme with no
    /// `palette` reads the fallback — so this is the key where diffing rather
    /// than presence matters most: an untouched theme that left its palette to
    /// `inherits` must not have 16 colours written over it.
    fn put_palette(&self, out: &mut toml::Table, palette: &[Color; 16]) {
        let same = section(self.reference, &["terminal"])
            .and_then(|t| t.get("palette"))
            .and_then(toml::Value::as_array)
            .is_some_and(|a| {
                a.len() == palette.len()
                    && a.iter()
                        .zip(palette)
                        .all(|(v, c)| v.as_str().and_then(|s| Color::parse(s).ok()) == Some(*c))
            });
        let value = palette.iter().map(|c| toml::Value::String(c.to_hex())).collect();
        self.put(out, &["terminal"], "palette", toml::Value::Array(value), same);
    }

    fn put_surface(&self, out: &mut toml::Table, path: &[&str], s: &Surface) {
        self.put_opt_color(out, path, "background", s.background);
        self.put_opt_color(out, path, "surface", s.surface);
        self.put_opt_border(out, path, "border", s.border.as_ref());
        self.put_opt_color(out, path, "text-primary", s.text_primary);
        self.put_opt_color(out, path, "text-secondary", s.text_secondary);
        self.put_opt_color(out, path, "text-tertiary", s.text_tertiary);
        self.put_opt_color(out, path, "hover", s.hover);
        self.put_opt_color(out, path, "active", s.active);
    }

    fn put_backdrop(&self, out: &mut toml::Table, path: &[&str], backdrop: &Backdrop) {
        if let Some(m) = backdrop.material {
            self.put_material(out, path, m);
        }
        if let Some(c) = backdrop.color {
            self.put_backdrop_color(out, path, c);
        }
        self.put_opt_f32(out, path, "opacity", backdrop.opacity);
        self.put_opt_f32(out, path, "radius", backdrop.radius);
    }
}

/// Deep-merge `over` into `base`: tables merge recursively, everything else is
/// replaced. This is what makes `inherits` cheap for the theme author.
fn merge(base: &toml::Table, over: &toml::Table) -> toml::Table {
    let mut out = base.clone();
    for (k, v) in over {
        match (out.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => {
                *b = merge(b, o);
            }
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    out
}

/// The table a theme is built from, after `inherits` has been resolved, along
/// with this file's leading comment banner.
fn resolve_table(
    slug: &str,
    user_dir: Option<&Path>,
    seen: &mut Vec<String>,
) -> Result<(toml::Table, String), ThemeError> {
    if seen.iter().any(|s| s == slug) {
        return Err(ThemeError::InheritCycle {
            slug: slug.to_owned(),
            base: slug.to_owned(),
        });
    }
    seen.push(slug.to_owned());

    let text: String = if let Some((_, t)) = BUILTIN_THEMES.iter().find(|(s, _)| *s == slug) {
        (*t).to_owned()
    } else if let Some(dir) = user_dir {
        let path = dir.join(format!("{slug}.ottytheme"));
        match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                seen.pop();
                return Err(ThemeError::Io(e));
            }
        }
    } else {
        seen.pop();
        return Err(ThemeError::UnknownBase {
            slug: slug.to_owned(),
            base: slug.to_owned(),
        });
    };

    let table: toml::Table = toml::from_str(&text).map_err(|e| ThemeError::Toml {
        slug: slug.to_owned(),
        source: Box::new(e),
    })?;
    let header = leading_comment(&text);

    let merged = match table.get("inherits").and_then(|v| v.as_str()) {
        Some(base) => {
            let base_slug = slugify(base);
            let parent = resolve_table(&base_slug, user_dir, seen)
                .map(|(parent, _)| parent)
                .map_err(|e| match e {
                    ThemeError::Io(_) | ThemeError::UnknownBase { .. } => ThemeError::UnknownBase {
                        slug: slug.to_owned(),
                        base: base.to_owned(),
                    },
                    other => other,
                })?;
            merge(&parent, &table)
        }
        None => table,
    };
    seen.pop();
    Ok((merged, header))
}

impl Theme {
    /// Load a theme by slug, resolving `inherits` against the built-ins and then
    /// the user's themes directory.
    pub fn load(slug: &str, user_dir: Option<&Path>) -> Result<Self, ThemeError> {
        let (table, header) = resolve_table(slug, user_dir, &mut Vec::new())?;
        Self::from_table(slug, table, header)
    }

    /// Parse a theme from text that is not on disk yet (import preview).
    pub fn parse(slug: &str, text: &str) -> Result<Self, ThemeError> {
        let table: toml::Table = toml::from_str(text).map_err(|e| ThemeError::Toml {
            slug: slug.to_owned(),
            source: Box::new(e),
        })?;
        Self::from_table(slug, table, leading_comment(text))
    }

    fn from_table(slug: &str, table: toml::Table, header: String) -> Result<Self, ThemeError> {
        let terminal = table
            .get("terminal")
            .and_then(|v| v.as_table())
            .ok_or_else(|| ThemeError::NoTerminal { slug: slug.to_owned() })?;

        let foreground = get_color(terminal, slug, "foreground")?
            .ok_or_else(|| ThemeError::NoForeground { slug: slug.to_owned() })?;

        let palette = match terminal.get("palette") {
            Some(toml::Value::Array(a)) => {
                if a.len() != 16 {
                    return Err(ThemeError::BadPalette {
                        slug: slug.to_owned(),
                        found: a.len(),
                    });
                }
                let mut out = [Color::OPAQUE_BLACK; 16];
                for (i, v) in a.iter().enumerate() {
                    out[i] = color_of(slug, "terminal.palette", v)?
                        .unwrap_or(Color::OPAQUE_BLACK);
                }
                out
            }
            // A theme may inherit the palette from its base; when nothing set
            // one at all, fall back to the documented Dracula-based default.
            _ => crate::palette::FALLBACK_PALETTE,
        };

        let background = get_color(terminal, slug, "background")?;
        let meta = table.get("meta").and_then(|v| v.as_table());
        let display_name = meta
            .and_then(|m| get_str(m, "name"))
            .unwrap_or_else(|| slug.to_owned());
        let mode = meta
            .and_then(|m| get_str(m, "mode"))
            .and_then(|m| Mode::parse(&m))
            .unwrap_or_else(|| Mode::detect(background.unwrap_or(Color::OPAQUE_BLACK)));

        let token_t = table.get("token").and_then(|v| v.as_table());
        let container_t = table.get("container").and_then(|v| v.as_table());

        let sec = |name: &str| -> Result<Surface, ThemeError> {
            match table.get(name).and_then(|v| v.as_table()) {
                Some(t) => parse_surface(t, slug),
                None => Ok(Surface::default()),
            }
        };

        let mut sidebar = sec("sidebar")?;
        sidebar.backdrop = parse_backdrop(
            table.get("sidebar").and_then(|v| v.as_table()).unwrap_or(&toml::Table::new()),
            slug,
            "sidebar",
        )?;

        let window = parse_backdrop(
            table.get("window").and_then(|v| v.as_table()).unwrap_or(&toml::Table::new()),
            slug,
            "window",
        )?;

        let platform_backdrop = |name: &str| -> Result<Backdrop, ThemeError> {
            let Some(t) = table.get(name).and_then(|v| v.as_table()) else {
                return Ok(Backdrop::default());
            };
            parse_backdrop(t, slug, name)
        };

        // `[mac.sidebar]` / `[windows.sidebar]`: the same shape, one level down.
        let platform_sidebar = |name: &str| -> Result<Backdrop, ThemeError> {
            let Some(t) = table
                .get(name)
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("sidebar"))
                .and_then(|v| v.as_table())
            else {
                return Ok(Backdrop::default());
            };
            parse_backdrop(t, slug, &format!("{name}.sidebar"))
        };
        let mac_sidebar = platform_sidebar("mac")?;
        let windows_sidebar = platform_sidebar("windows")?;

        let cursor_t = table.get("cursor").and_then(|v| v.as_table());
        let cursor = CursorStyleSpec {
            color: cursor_t
                .map(|t| get_color(t, slug, "color"))
                .transpose()?
                .flatten()
                .or(get_color(terminal, slug, "cursor")?),
            text: cursor_t
                .map(|t| get_color(t, slug, "text"))
                .transpose()?
                .flatten()
                .or(get_color(terminal, slug, "cursor-text")?),
            style: cursor_t.and_then(|t| get_str(t, "style")).and_then(|s| CursorShape::parse(&s)),
            blink: cursor_t.and_then(|t| t.get("blink")).and_then(|v| v.as_bool()),
        };

        // `[selection]` overrides the legacy `[terminal].selection-*` pair, and
        // falls back to it so a theme written against the old spelling works.
        let selection_t = table.get("selection").and_then(|v| v.as_table());
        let selection = Selection {
            foreground: selection_t
                .map(|t| get_color(t, slug, "foreground"))
                .transpose()?
                .flatten()
                .or(get_color(terminal, slug, "selection-foreground")?),
            background: selection_t
                .map(|t| get_color(t, slug, "background"))
                .transpose()?
                .flatten()
                .or(get_color(terminal, slug, "selection-background")?),
        };

        let tab_t = table.get("tab").and_then(|v| v.as_table());
        let tab = TabStyle {
            background: tab_t.map(|t| get_color(t, slug, "background")).transpose()?.flatten(),
            foreground: tab_t.map(|t| get_color(t, slug, "foreground")).transpose()?.flatten(),
            active: tab_t
                .and_then(|t| t.get("active"))
                .and_then(|v| v.as_table())
                .map(|t| parse_surface(t, slug))
                .transpose()?
                .unwrap_or_default(),
            hover: tab_t
                .and_then(|t| t.get("hover"))
                .and_then(|v| v.as_table())
                .map(|t| parse_surface(t, slug))
                .transpose()?
                .unwrap_or_default(),
        };

        let extra_tables = table
            .iter()
            .filter(|(k, _)| !MODELLED_SECTIONS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Ok(Self {
            slug: slug.to_owned(),
            display_name,
            mode,
            author: meta.and_then(|m| get_str(m, "author")),
            description: meta.and_then(|m| get_str(m, "description")),
            inherits: table.get("inherits").and_then(|v| v.as_str()).map(str::to_owned),
            terminal: Terminal {
                foreground,
                background,
                palette,
                cursor: get_color(terminal, slug, "cursor")?,
                cursor_text: get_color(terminal, slug, "cursor-text")?,
                selection_foreground: get_color(terminal, slug, "selection-foreground")?,
                selection_background: get_color(terminal, slug, "selection-background")?,
            },
            token: Token {
                accent: token_t.map(|t| get_color(t, slug, "accent")).transpose()?.flatten(),
                font_mono: token_t.and_then(|t| get_font_stack(t, "font-mono")),
                font_ui: token_t.and_then(|t| get_font_stack(t, "font-ui")),
                font_size: token_t.and_then(|t| get_f32(t, "font-size")),
                adjust_cell_height: token_t.and_then(|t| get_str(t, "adjust-cell-height")),
            },
            container: Container {
                radius: container_t.and_then(|t| get_f32(t, "radius")),
                shadow: container_t.and_then(|t| get_str(t, "shadow")),
                border: container_t.and_then(|t| get_str(t, "border")),
                padding: container_t.and_then(|t| get_edges(t, "padding")),
                margin: container_t.and_then(|t| get_edges(t, "margin")),
            },
            ui: sec("ui")?,
            panel: sec("panel")?,
            sidebar,
            titlebar: sec("titlebar")?,
            tab,
            selection,
            cursor,
            ghost: Ghost {
                foreground: table
                    .get("ghost")
                    .and_then(|v| v.as_table())
                    .map(|t| get_color(t, slug, "foreground"))
                    .transpose()?
                    .flatten(),
            },
            window,
            mac: platform_backdrop("mac")?,
            windows: platform_backdrop("windows")?,
            mac_sidebar,
            windows_sidebar,
            extra_tables,
            // The merged table, kept whole: the typed fields above are a
            // reading of it, and anything they do not cover stays here.
            raw: table.clone(),
            header,
        })
    }

    /// The top-level tables of [`Theme::raw`] that the typed view does not
    /// model — the keys a newer Otty may understand and this build must not
    /// destroy. [`Theme::to_toml`] emits them verbatim.
    ///
    /// Keys nested *inside* a modelled section (`[token].secondary`,
    /// `[tab.active.unfocused]`, `[container].background`) are not unmodelled
    /// keys with nowhere to live: they are in `raw` too, and they survive a
    /// save. Reach for `raw` when you need those.
    pub fn extra(&self) -> &toml::Table {
        &self.extra_tables
    }

    /// This theme as theme-file text: `raw` with every typed field the code has
    /// changed written back over it.
    ///
    /// A theme that was parsed and not changed comes back with every key-path,
    /// every value, and every spelling the source used — `rgba(...)` stays
    /// `rgba(...)`, a shorthand border stays a shorthand — because nothing has
    /// changed to write. What the printer does redo is layout: sections come back
    /// in sorted order and inline comments are gone, since TOML does not carry
    /// them. The leading banner is re-emitted verbatim. A field changed in memory
    /// lands, and every key this build does not model is carried through.
    pub fn to_toml(&self) -> String {
        let mut table = self.raw.clone();
        match Theme::from_table(&self.slug, self.raw.clone(), String::new()) {
            Ok(baseline) => {
                // `raw` as `raw` reads: the same overlay, told to write every
                // key, over the typed view of `raw` itself. Diffing against that
                // is what tells a field the code changed from one that merely
                // has a fallback.
                let mut reference = self.raw.clone();
                baseline.overlay(&mut reference, &Over::everything(&self.raw));
                self.overlay(&mut table, &Over::against(&reference));
            }
            // The source is not a theme at all — no `[terminal]`, a palette of
            // the wrong size. There is nothing to preserve, so everything the
            // typed view has is written out.
            Err(_) => self.overlay(&mut table, &Over::everything(&self.raw)),
        }
        let body = match toml::to_string_pretty(&table) {
            Ok(body) => body,
            Err(e) => {
                // A `toml::Table` of parsed values always serialises; this arm
                // exists so a save can never panic on a theme.
                log::error!("theme `{}` cannot be written as TOML: {e}", self.slug);
                return String::new();
            }
        };
        format!("{}{body}", self.header)
    }

    /// Overlay the typed fields onto `table`, a clone of `raw`.
    ///
    /// Which keys that actually writes is [`Over`]'s business: a theme nobody
    /// touched in memory comes back with nothing written at all, so the source's
    /// own spellings — and every key this build does not model — survive a save.
    fn overlay(&self, table: &mut toml::Table, over: &Over<'_>) {
        if let Some(base) = &self.inherits {
            over.put_str(table, &[], "inherits", base);
        }

        over.put_str(table, &["meta"], "name", &self.display_name);
        over.put_mode(table, self.mode);
        over.put_opt_str(table, &["meta"], "author", self.author.as_deref());
        over.put_opt_str(table, &["meta"], "description", self.description.as_deref());

        let t = &self.terminal;
        over.put_color(table, &["terminal"], "foreground", t.foreground);
        over.put_opt_color(table, &["terminal"], "background", t.background);
        over.put_palette(table, &t.palette);
        over.put_opt_color(table, &["terminal"], "cursor", t.cursor);
        over.put_opt_color(table, &["terminal"], "cursor-text", t.cursor_text);
        over.put_opt_color(table, &["terminal"], "selection-foreground", t.selection_foreground);
        over.put_opt_color(table, &["terminal"], "selection-background", t.selection_background);

        let t = &self.token;
        over.put_opt_color(table, &["token"], "accent", t.accent);
        over.put_opt_stack(table, &["token"], "font-mono", t.font_mono.as_deref());
        over.put_opt_stack(table, &["token"], "font-ui", t.font_ui.as_deref());
        over.put_opt_f32(table, &["token"], "font-size", t.font_size);
        over.put_opt_str(
            table,
            &["token"],
            "adjust-cell-height",
            t.adjust_cell_height.as_deref(),
        );

        let c = &self.container;
        over.put_opt_f32(table, &["container"], "radius", c.radius);
        over.put_opt_str(table, &["container"], "shadow", c.shadow.as_deref());
        over.put_opt_str(table, &["container"], "border", c.border.as_deref());
        over.put_opt_edges(table, &["container"], "padding", c.padding);
        over.put_opt_edges(table, &["container"], "margin", c.margin);

        over.put_surface(table, &["ui"], &self.ui);
        over.put_surface(table, &["panel"], &self.panel);
        over.put_surface(table, &["titlebar"], &self.titlebar);
        over.put_surface(table, &["sidebar"], &self.sidebar);
        // `[sidebar]` also carries the chrome column's backdrop, in the same
        // table as its surface colours.
        over.put_backdrop(table, &["sidebar"], &self.sidebar.backdrop);

        over.put_opt_color(table, &["tab"], "background", self.tab.background);
        over.put_opt_color(table, &["tab"], "foreground", self.tab.foreground);
        over.put_surface(table, &["tab", "active"], &self.tab.active);
        over.put_surface(table, &["tab", "hover"], &self.tab.hover);

        // `[selection]` and `[cursor]` both fall back to `[terminal]`, so an
        // untouched theme diffing clean here is also what keeps a save from
        // adding a second spelling of a fact `[terminal]` already holds.
        over.put_opt_color(table, &["selection"], "foreground", self.selection.foreground);
        over.put_opt_color(table, &["selection"], "background", self.selection.background);
        over.put_opt_color(table, &["cursor"], "color", self.cursor.color);
        over.put_opt_color(table, &["cursor"], "text", self.cursor.text);
        if let Some(style) = self.cursor.style {
            over.put_cursor_shape(table, style);
        }
        if let Some(blink) = self.cursor.blink {
            over.put_bool(table, &["cursor"], "blink", blink);
        }

        over.put_opt_color(table, &["ghost"], "foreground", self.ghost.foreground);

        over.put_backdrop(table, &["window"], &self.window);
        over.put_backdrop(table, &["mac"], &self.mac);
        over.put_backdrop(table, &["windows"], &self.windows);
        over.put_backdrop(table, &["mac", "sidebar"], &self.mac_sidebar);
        over.put_backdrop(table, &["windows", "sidebar"], &self.windows_sidebar);
    }

    /// The backdrop to use on `platform`, with each field falling back to
    /// `[window]` on its own — exactly as the theme format describes.
    pub fn backdrop_for(&self, platform: Platform) -> Backdrop {
        match platform {
            Platform::Macos => self.mac.or(self.window),
            Platform::Windows => self.windows.or(self.window),
            // Linux reads `[window]` only.
            _ => self.window,
        }
    }

    /// Whether the theme asks for the two-column Windows 11 shape, where only
    /// the chrome column is translucent and the content column keeps its opaque
    /// fill and runs the full height of the window.
    pub fn sideways_backdrop_for(&self, platform: Platform) -> Backdrop {
        let per_platform = match platform {
            Platform::Macos => self.mac_sidebar,
            Platform::Windows => self.windows_sidebar,
            _ => Backdrop::default(),
        };
        // `[sidebar]` (shared) falls back to the platform backdrop, which itself
        // falls back to `[window]`.
        self.sidebar.backdrop
            .or(per_platform)
            .or(self.backdrop_for(platform))
    }

    /// True when only the sidebar is translucent, which the shell renders as the
    /// two-column shape instead of a full-window blur.
    pub fn sidebar_only_backdrop(&self, platform: Platform) -> bool {
        self.sidebar.backdrop.material.is_some()
            || self.mac_sidebar.material.is_some()
            || self.windows_sidebar.material.is_some()
    }

    pub fn is_builtin(slug: &str) -> bool {
        BUILTIN_THEMES.iter().any(|(s, _)| *s == slug)
    }

    /// Every theme available to `theme = <name>`: the built-ins plus whatever is
    /// in the user's themes directory, sorted by slug.
    pub fn available(user_dir: Option<&Path>) -> Vec<String> {
        let mut out: Vec<String> = BUILTIN_THEMES.iter().map(|(s, _)| (*s).to_owned()).collect();
        if let Some(dir) = user_dir {
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ottytheme")) {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            let slug = slugify(stem);
                            if !out.contains(&slug) {
                                out.push(slug);
                            }
                        }
                    }
                }
            }
        }
        out.sort();
        out
    }

    /// The themes directory (`~/.config/otty/themes`).
    pub fn user_dir() -> PathBuf {
        otty_core::config_dir().join("themes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_parses_and_has_16_colours() {
        for (slug, text) in BUILTIN_THEMES {
            let t = Theme::parse(slug, text).unwrap_or_else(|e| panic!("{slug}: {e}"));
            assert_eq!(t.slug, *slug);
            assert_eq!(t.terminal.palette.len(), 16);
            assert!(!t.display_name.is_empty(), "{slug} has no display name");
            // The UI typeface is the Anthropic stack, in every shipped theme.
            assert_eq!(
                t.token.font_ui.as_deref(),
                Some(&["Styrene B".to_owned(), "Styrene A".to_owned()][..]),
                "{slug} does not carry the Styrene UI stack"
            );
        }
    }

    #[test]
    fn builtin_slugs_are_sorted_and_unique() {
        let mut slugs: Vec<&str> = BUILTIN_THEMES.iter().map(|(s, _)| *s).collect();
        let sorted = slugs.clone();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), BUILTIN_THEMES.len(), "duplicate built-in theme slug");
        assert_eq!(slugs.len(), sorted.len());
        for (s, _) in BUILTIN_THEMES {
            assert_eq!(*s, slugify(s), "{s} is not a canonical slug");
        }
    }

    #[test]
    fn platform_defaults_match_the_documented_pairing() {
        assert_eq!(default_theme_names(Platform::Macos), ("paper", "nord"));
        assert_eq!(default_theme_names(Platform::Linux), ("paper", "nord"));
        assert_eq!(default_theme_names(Platform::Windows), ("one-light", "acrylic-dark"));
    }

    #[test]
    fn materials_map_across_platforms() {
        assert_eq!(Material::VibrancyThin.for_platform(Platform::Windows), Material::AcrylicThin);
        assert_eq!(Material::VibrancyRegular.for_platform(Platform::Windows), Material::Mica);
        assert_eq!(Material::VibrancyThick.for_platform(Platform::Windows), Material::MicaAlt);
        assert_eq!(Material::Glass.for_platform(Platform::Windows), Material::Acrylic);
        assert_eq!(Material::Acrylic.for_platform(Platform::Macos), Material::Glass);
        assert_eq!(Material::MicaAlt.for_platform(Platform::Macos), Material::VibrancyThick);
        // Native materials are left alone.
        assert_eq!(Material::Glass.for_platform(Platform::Macos), Material::Glass);
        assert_eq!(Material::Mica.for_platform(Platform::Windows), Material::Mica);
        // Linux has no blur vocabulary, so everything collapses to none.
        assert_eq!(Material::Glass.for_platform(Platform::Linux), Material::None);
    }

    #[test]
    fn inheritance_only_overrides_what_the_child_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("child.ottytheme"),
            "inherits = \"nord\"\n\n[terminal]\nbackground = \"#000000\"\n",
        )
        .unwrap();
        let base = Theme::load("nord", None).unwrap();
        let child = Theme::load("child", Some(dir.path())).unwrap();
        assert_eq!(child.terminal.background.unwrap().to_hex(), "#000000");
        // Everything else still comes from nord.
        assert_eq!(child.terminal.foreground, base.terminal.foreground);
        assert_eq!(child.terminal.palette, base.terminal.palette);
        // `raw` is the merged table, and a save writes the merged theme.
        assert_eq!(child.raw.get("inherits").and_then(|v| v.as_str()), Some("nord"));
        assert!(child.raw.contains_key("panel"), "the base's sections must be in `raw`");
        assert_eq!(child.raw.get("panel"), base.raw.get("panel"));
        let again = Theme::parse("child", &child.to_toml()).unwrap();
        assert_eq!(child, again, "a merged theme does not survive a round trip");
    }

    #[test]
    fn inheritance_cycles_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.ottytheme"), "inherits = \"b\"\n[terminal]\nforeground = \"#fff\"\n").unwrap();
        std::fs::write(dir.path().join("b.ottytheme"), "inherits = \"a\"\n[terminal]\nforeground = \"#fff\"\n").unwrap();
        assert!(matches!(
            Theme::load("a", Some(dir.path())),
            Err(ThemeError::InheritCycle { .. })
        ));
    }

    #[test]
    fn a_missing_optional_section_falls_back_rather_than_failing() {
        let t = Theme::parse(
            "minimal",
            "[terminal]\nforeground = \"#e0e0ff\"\nbackground = \"#0a0a14\"\npalette = [\
             \"#0a0a14\",\"#ff4d6d\",\"#7be0ad\",\"#ffd66b\",\
             \"#82aaff\",\"#c792ea\",\"#89ddff\",\"#d0d0ff\",\
             \"#3b3b50\",\"#ff6b88\",\"#9eebbf\",\"#ffe28a\",\
             \"#a0c0ff\",\"#daa6f7\",\"#a8e6ff\",\"#ffffff\"]\n",
        )
        .unwrap();
        assert_eq!(t.mode, Mode::Dark);
        assert!(t.panel.background.is_none());
        assert_eq!(t.backdrop_for(Platform::Macos).material_for(Platform::Macos), Material::None);
    }

    #[test]
    fn a_transparent_background_stays_none_instead_of_parsing_as_a_colour() {
        let t = Theme::parse(
            "glass",
            "[terminal]\nforeground = \"#fff\"\nbackground = \"none\"\n",
        )
        .unwrap();
        assert!(t.terminal.background.is_none());
        assert_eq!(t.terminal.foreground.to_hex(), "#ffffff");
    }

    #[test]
    fn a_wrong_sized_palette_is_a_hard_error() {
        let e = Theme::parse("bad", "[terminal]\nforeground = \"#fff\"\npalette = [\"#000\"]\n")
            .unwrap_err();
        assert!(matches!(e, ThemeError::BadPalette { found: 1, .. }), "{e}");
    }

    #[test]
    fn missing_terminal_section_is_a_hard_error() {
        let e = Theme::parse("empty", "[meta]\nname = \"x\"\n").unwrap_err();
        assert!(matches!(e, ThemeError::NoTerminal { .. }), "{e}");
    }

    #[test]
    fn mode_is_inferred_from_the_background_when_unset() {
        assert_eq!(Mode::detect(Color::rgb(10, 10, 20)), Mode::Dark);
        assert_eq!(Mode::detect(Color::rgb(250, 249, 245)), Mode::Light);
    }

    #[test]
    fn slugify_matches_the_importers_naming() {
        assert_eq!(slugify("Catppuccin Mocha"), "catppuccin-mocha");
        assert_eq!(slugify("  Rosé Pine  "), "ros-pine");
        assert_eq!(slugify("One_Light"), "one-light");
        assert_eq!(slugify("!!!"), "theme");
    }

    #[test]
    fn unknown_sections_survive_a_round_trip() {
        let t = Theme::parse(
            "future",
            "[terminal]\nforeground = \"#fff\"\n\n[future-section]\nkey = \"value\"\n",
        )
        .unwrap();
        assert_eq!(
            t.extra()
                .get("future-section")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("key")),
            Some(&toml::Value::String("value".into()))
        );
        // `extra()` is a view of `raw`, so what it reports is what a save keeps.
        let saved: toml::Table = toml::from_str(&t.to_toml()).unwrap();
        assert_eq!(saved["future-section"]["key"].as_str(), Some("value"));
    }

    #[test]
    fn a_border_may_be_a_shorthand_or_a_colour() {
        let text = "\
[terminal]
foreground = \"#fff\"

[panel]
border = \"rgba(255,255,255,0.06)\"

[tab.active]
border = \"1px solid #FFFFFF14\"
";
        let t = Theme::parse("borders", text).unwrap();

        // The chrome shorthand keeps its text, which is all it can do: the
        // format does not model `1px solid`.
        let shorthand = t.tab.active.border.clone().unwrap();
        assert_eq!(shorthand, BorderSpec::Shorthand("1px solid #FFFFFF14".into()));
        assert_eq!(shorthand.as_str(), "1px solid #FFFFFF14");
        assert_eq!(shorthand.to_string(), "1px solid #FFFFFF14");
        assert_eq!(shorthand.color(), None);

        // A bare colour is one, and still reports itself as hex.
        let colour = t.panel.border.clone().unwrap();
        assert_eq!(colour.color(), Some(Color::rgba(255, 255, 255, 15)));
        assert_eq!(colour.as_str(), "#ffffff0f");
        assert_eq!(colour.to_string(), "#ffffff0f");

        // Neither spelling is renormalised on the way out.
        let emitted = t.to_toml();
        assert!(emitted.contains("\"1px solid #FFFFFF14\""), "{emitted}");
        assert!(emitted.contains("\"rgba(255,255,255,0.06)\""), "{emitted}");
        let again = Theme::parse("borders", &emitted).unwrap();
        assert_eq!(again.tab.active.border, t.tab.active.border);
        assert_eq!(again.panel.border, t.panel.border);

        // A border that is neither is still a hard error.
        let e = Theme::parse("bad", "[terminal]\nforeground = \"#fff\"\n\n[panel]\nborder = 3\n")
            .unwrap_err();
        assert!(matches!(e, ThemeError::BadColor { .. }), "{e}");
    }

    /// The five shipped chrome shorthands that used to fail the built-in sweep,
    /// and the two panels whose `border` is genuinely a bare colour.
    #[test]
    fn the_shipped_borders_type_as_the_format_spells_them() {
        let shorthand = [
            "acrylic-dark",
            "floating-card",
            "glass-dark",
            "glass-light",
            "paper",
        ];
        for slug in shorthand {
            let t = Theme::load(slug, None).unwrap_or_else(|e| panic!("{slug}: {e}"));
            match &t.tab.active.border {
                Some(BorderSpec::Shorthand(text)) => {
                    assert!(text.starts_with("1px solid "), "{slug}: {text}");
                }
                other => panic!("{slug}: expected the CSS shorthand, found {other:?}"),
            }
        }
        for slug in ["nord", "glass-light"] {
            let t = Theme::load(slug, None).unwrap_or_else(|e| panic!("{slug}: {e}"));
            assert!(
                matches!(t.panel.border, Some(BorderSpec::Color(_))),
                "{slug}: expected a bare colour on [panel].border"
            );
        }
    }

    #[test]
    fn keys_the_typed_view_does_not_model_survive_to_toml() {
        let text = "\
# Banner
# Second line
[terminal]
foreground = \"#fff\"

[token]
secondary = \"#C0C7D3\"
hover-bg  = \"rgba(255,255,255,0.06)\"

[titlebar]
material = \"vibrancy-regular\"

[sidebar]
border-right  = \"1px solid #434C5E\"
width         = 220
padding-right = 0.0

[container.tabs-top]
margin = 12

[tab.active]
background      = \"#3B4252\"
indicator-color = \"#88C0D0\"

[tab.active.unfocused]
background = \"#3B4252\"

[cursor.block]
color = \"#E5E9F0\"

[window.unfocused]
opacity = 0.92

[future-section]
key = \"value\"
";
        let t = Theme::parse("future", text).unwrap();
        let emitted = t.to_toml();
        assert!(emitted.starts_with("# Banner\n# Second line\n"), "{emitted}");

        // Every key-path and every value in the source comes back out, the
        // uppercase hex included: nothing the typed view ignores may be dropped
        // or renormalised.
        let shipped: toml::Table = toml::from_str(text).unwrap();
        let saved: toml::Table = toml::from_str(&emitted).unwrap();
        assert_eq!(saved, shipped, "{emitted}");
    }

    #[test]
    fn an_edit_made_in_memory_lands_and_nothing_else_moves() {
        let text = "\
[terminal]
foreground = \"#000\"

[token]
font-size = 12

[panel]
background = \"#EEEEEE\"
border = \"1px solid #DDDDDD\"

[tab.active]
background = \"#CCCCCC\"
";
        let mut t = Theme::parse("edit", text).unwrap();
        t.panel.background = Some(Color::rgb(0x11, 0x22, 0x33));
        t.tab.active.border = Some(BorderSpec::Color(Color::rgba(1, 2, 3, 4)));
        t.token.font_size = Some(15.0);

        let emitted = t.to_toml();
        let back = Theme::parse("edit", &emitted).unwrap();
        assert_eq!(back.panel.background, Some(Color::rgb(0x11, 0x22, 0x33)));
        assert_eq!(back.tab.active.border, Some(BorderSpec::Color(Color::rgba(1, 2, 3, 4))));
        assert_eq!(back.token.font_size, Some(15.0));
        // Only the edited keys moved: the untouched shorthand is still a
        // shorthand, and `#000` was not rewritten as `#000000`.
        assert!(emitted.contains("1px solid #DDDDDD"), "{emitted}");
        assert!(emitted.contains("\"#000\""), "{emitted}");
        assert!(emitted.contains("\"#112233\""), "{emitted}");
    }

    #[test]
    fn a_theme_with_no_source_text_is_written_out_in_full() {
        // The shape an importer builds: typed fields set, no source table
        // behind them, so there is nothing to preserve and everything lands.
        let mut t = Theme::parse("seed", "[terminal]\nforeground = \"#fff\"\n").unwrap();
        t.raw = toml::Table::new();
        t.extra_tables = toml::Table::new();
        t.display_name = "Imported".to_owned();
        t.token.font_size = Some(13.0);

        let back = Theme::parse("seed", &t.to_toml()).unwrap();
        assert_eq!(back.display_name, "Imported");
        assert_eq!(back.token.font_size, Some(13.0));
        assert_eq!(back.terminal.foreground, t.terminal.foreground);
        assert_eq!(back.terminal.palette, t.terminal.palette);
    }

    #[test]
    fn to_toml_output_reparses_to_an_equal_theme() {
        for (slug, _) in BUILTIN_THEMES {
            let t = Theme::load(slug, None).unwrap_or_else(|e| panic!("{slug}: {e}"));
            let emitted = t.to_toml();
            let again =
                Theme::parse(slug, &emitted).unwrap_or_else(|e| panic!("{slug}: {e}\n{emitted}"));
            assert_eq!(t, again, "{slug} does not survive a round trip through to_toml()");
        }
    }

    #[test]
    fn every_builtin_theme_round_trips_without_losing_a_key_path() {
        let (mut shipped_total, mut emitted_total, mut untyped_total) = (0usize, 0usize, 0usize);
        println!(
            "{:<18} {:>8} {:>8} {:>8} {:>6} {:>8} {:>8}",
            "theme", "shipped", "emitted", "missing", "added", "changed", "untyped"
        );
        for (slug, shipped) in shipped_themes() {
            let t = Theme::load(slug, None).unwrap_or_else(|e| panic!("{slug}: {e}"));
            let emitted = t.to_toml();
            let again = Theme::parse(slug, &emitted)
                .unwrap_or_else(|e| panic!("{slug}: the emitted text does not reparse: {e}\n{emitted}"));
            assert_eq!(t, again, "{slug}: the emitted text reads back as a different theme");
            let saved: toml::Table =
                toml::from_str(&emitted).unwrap_or_else(|e| panic!("{slug}: {e}\n{emitted}"));

            let shipped_paths = key_paths(&toml::Value::Table(shipped.clone()));
            let saved_paths = key_paths(&toml::Value::Table(saved.clone()));
            let missing: Vec<&String> =
                shipped_paths.keys().filter(|p| !saved_paths.contains_key(*p)).collect();
            let added: Vec<&String> =
                saved_paths.keys().filter(|p| !shipped_paths.contains_key(*p)).collect();
            let changed: Vec<&String> = shipped_paths
                .iter()
                .filter(|(p, v)| saved_paths.get(*p).is_some_and(|o| o != *v))
                .map(|(p, _)| p)
                .collect();
            let untyped = shipped_paths
                .iter()
                .filter(|(_, v)| !v.is_table())
                .filter(|(p, v)| is_untyped(slug, &t, &shipped, p, v))
                .count();

            println!(
                "{slug:<18} {:>8} {:>8} {:>8} {:>6} {:>8} {:>8}",
                shipped_paths.len(),
                saved_paths.len(),
                missing.len(),
                added.len(),
                changed.len(),
                untyped
            );
            if !missing.is_empty() {
                println!("  missing: {missing:?}");
            }
            if !added.is_empty() {
                println!("  added:   {added:?}");
            }
            if !changed.is_empty() {
                println!("  changed: {changed:?}");
            }

            shipped_total += shipped_paths.len();
            emitted_total += saved_paths.len();
            untyped_total += untyped;
            assert!(missing.is_empty(), "{slug} lost {missing:?}");
            assert!(changed.is_empty(), "{slug} renormalised {changed:?}");
        }
        println!(
            "TOTAL: {shipped_total} shipped key-paths, {emitted_total} emitted, \
             {untyped_total} carried by `raw` alone — diff: {}",
            if shipped_total == emitted_total { "zero missing keys" } else { "MISMATCH" }
        );
    }

    /// Whether the typed view has no field for `path`: change the key in the
    /// source and see whether the reading moves at all.
    ///
    /// This is what makes the gap measurable without a second hand-written list
    /// of fields to drift out of step with the parser.
    fn is_untyped(
        slug: &str,
        typed_theme: &Theme,
        shipped: &toml::Table,
        path: &str,
        value: &toml::Value,
    ) -> bool {
        for probe in probe_values(value) {
            let mut table = shipped.clone();
            set_key_path(&mut table, path, probe);
            let Ok(text) = toml::to_string_pretty(&table) else { return false };
            let moved = match Theme::parse(slug, &text) {
                Ok(moved) => typed(&moved) != typed(typed_theme),
                // The parser rejects the probe, so it plainly reads the key.
                Err(_) => true,
            };
            if moved {
                return false;
            }
        }
        true
    }

    /// Values of the same TOML type as `v` that read differently from it. More
    /// than one, because an enum key (`mode`, `material`, `style`) ignores an
    /// unknown spelling and falls back rather than moving.
    fn probe_values(v: &toml::Value) -> Vec<toml::Value> {
        let s = |t: &str| toml::Value::String(t.to_owned());
        match v {
            toml::Value::String(_) => {
                vec![s("light"), s("dark"), s("none"), s("probe-that-is-not-a-keyword")]
            }
            toml::Value::Integer(i) => vec![toml::Value::Integer(i + 7), toml::Value::Integer(0)],
            toml::Value::Float(f) => vec![toml::Value::Float(f + 0.75), toml::Value::Float(0.0)],
            toml::Value::Boolean(b) => vec![toml::Value::Boolean(!b)],
            // A one-element array moves any array key: a palette becomes the
            // wrong size, a font stack becomes a different stack.
            toml::Value::Array(_) => vec![toml::Value::Array(vec![s("probe")])],
            other => vec![other.clone()],
        }
    }

    /// Replace the value at a dotted `path`.
    fn set_key_path(t: &mut toml::Table, path: &str, value: toml::Value) {
        let mut cur = t;
        let mut parts: Vec<&str> = path.split('.').collect();
        let last = parts.pop().expect("a key path has at least one segment");
        for p in parts {
            cur = cur
                .get_mut(p)
                .and_then(toml::Value::as_table_mut)
                .expect("every parent of a key path is a table");
        }
        cur.insert(last.to_owned(), value);
    }

    /// Every key-path in a table, tables included, mapped to the sub-tree it
    /// holds. A table path carries the whole of it, so an empty section is a
    /// path too.
    fn key_paths(value: &toml::Value) -> std::collections::BTreeMap<String, toml::Value> {
        fn walk(
            prefix: &str,
            value: &toml::Value,
            out: &mut std::collections::BTreeMap<String, toml::Value>,
        ) {
            if let toml::Value::Table(t) = value {
                for (k, child) in t {
                    let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                    out.insert(path.clone(), child.clone());
                    walk(&path, child, out);
                }
            }
        }
        let mut out = std::collections::BTreeMap::new();
        walk("", value, &mut out);
        out
    }

    /// The typed view alone: `raw`, its `extra` cache and the banner are the
    /// source text, not a reading of it.
    fn typed(t: &Theme) -> Theme {
        let mut c = t.clone();
        c.raw = toml::Table::new();
        c.extra_tables = toml::Table::new();
        c.header.clear();
        c
    }

    /// The themes the app ships, read from disk — the bytes that are the spec.
    fn shipped_themes() -> Vec<(&'static str, toml::Table)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/themes");
        BUILTIN_THEMES
            .iter()
            .map(|(slug, _)| {
                let path = format!("{dir}/{slug}.ottytheme");
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
                let table: toml::Table =
                    toml::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
                (*slug, table)
            })
            .collect()
    }

    /// What the typed view leaves to `raw`: every key-path the built-in themes
    /// spell that no field reads, which is the size of the gap a save has to
    /// carry. The inventory is printed; the promise asserted is that each one
    /// survives a save unchanged.
    #[test]
    fn unmodelled_key_paths_are_carried_by_raw() {
        let mut leaves: std::collections::BTreeMap<String, Vec<&str>> =
            std::collections::BTreeMap::new();
        let mut tables: std::collections::BTreeMap<String, Vec<&str>> =
            std::collections::BTreeMap::new();
        for (slug, shipped) in shipped_themes() {
            let t = Theme::load(slug, None).unwrap();
            let saved: toml::Table = toml::from_str(&t.to_toml()).unwrap();
            let shipped_paths = key_paths(&toml::Value::Table(shipped.clone()));
            let saved_paths = key_paths(&toml::Value::Table(saved));
            let is_leaf = |p: &String| !shipped_paths[p].is_table();
            let below = |parent: &str| -> Vec<String> {
                shipped_paths
                    .keys()
                    .filter(|p| is_leaf(p) && p.starts_with(&format!("{parent}.")))
                    .cloned()
                    .collect()
            };

            let mut untyped: Vec<String> = Vec::new();
            for (path, value) in shipped_paths.iter().filter(|(p, _)| is_leaf(p)) {
                if !is_untyped(slug, &t, &shipped, path, value) {
                    continue;
                }
                untyped.push(path.clone());
                assert_eq!(
                    saved_paths.get(path),
                    shipped_paths.get(path),
                    "{slug}: `{path}` is not read and must be carried through a save"
                );
                leaves.entry(path.clone()).or_default().push(slug);
            }

            // A whole section nothing reads is carried the same way as a key:
            // every leaf under it unread, or the section spelled but empty.
            for (path, _) in shipped_paths.iter().filter(|(_, v)| v.is_table()) {
                let inside = below(path);
                let all_unread = !inside.is_empty() && inside.iter().all(|p| untyped.contains(p));
                let empty_unread = inside.is_empty()
                    && !MODELLED_SECTIONS.contains(&path.split('.').next().unwrap_or(path));
                if all_unread || empty_unread {
                    tables.entry(path.clone()).or_default().push(slug);
                }
            }
        }

        let count = |m: &std::collections::BTreeMap<String, Vec<&str>>| {
            m.values().map(Vec::len).sum::<usize>()
        };
        let names: std::collections::BTreeSet<&str> = leaves
            .keys()
            .chain(tables.keys())
            .map(|p| p.rsplit('.').next().unwrap_or(p))
            .collect();
        println!(
            "carried by `raw` alone: {} key-path instances over {} distinct paths, {} distinct \
             names — plus {} whole-section instances over {} paths",
            count(&leaves),
            leaves.len(),
            names.len(),
            count(&tables),
            tables.len()
        );
        for (path, themes) in leaves.iter().chain(tables.iter()) {
            println!("  {path:<38} {} theme(s)", themes.len());
        }
        assert!(!leaves.is_empty(), "the shipped themes must be a superset of the typed view");
    }
}
