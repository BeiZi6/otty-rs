//! Importing themes and settings from other terminals.
//!
//! Supported sources, all auto-detected from the text:
//!
//! | Source | Detection | What comes across |
//! |---|---|---|
//! | Otty `.ottytheme` | real TOML with a `[terminal]` table | everything |
//! | iTerm2 `.itermcolors` | XML plist with `Ansi 0 Color` keys | the 16 palette slots, fg, bg, cursor, selection |
//! | kitty `.conf` | `color0`…`color15` directives | the palette, fg, bg, cursor, selection |
//! | Alacritty `.toml` | `[colors.normal]` / `[colors.bright]` | the palette, fg, bg, cursor |
//! | Ghostty config | `palette = N=#rrggbb` | the palette, fg, bg, cursor, selection |
//!
//! A theme file is converted into Otty's own theme format; anything the source
//! does not carry is left out rather than guessed, so `inherits` plus the
//! fallback palette fill the gaps.

use otty_core::{Color, Platform};

use crate::theme::{slugify, Mode, Theme};
use crate::value::unquote;
use crate::palette::FALLBACK_PALETTE;

/// Which family a file belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    Otty,
    Iterm2,
    Kitty,
    Alacritty,
    Ghostty,
}

impl ThemeSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Otty => "Otty",
            Self::Iterm2 => "iTerm2",
            Self::Kitty => "kitty",
            Self::Alacritty => "Alacritty",
            Self::Ghostty => "Ghostty",
        }
    }
}

impl std::fmt::Display for ThemeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A theme read from another terminal, before it is written to disk.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedTheme {
    pub source: ThemeSource,
    /// The display name guessed from the file or from the palette.
    pub name: String,
    pub slug: String,
    pub mode: Mode,
    pub theme: Theme,
}

/// A settings import from another terminal's config file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportedConfig {
    pub source: Option<ThemeSource>,
    /// `key -> value` pairs, already in Otty's key names.
    pub settings: Vec<(String, String)>,
    /// Keys the source set that Otty has no equivalent for.
    pub unsupported: Vec<String>,
    /// Keys that differ from the current config.
    pub conflicts: Vec<String>,
}

/// What the caller believes the file is. `None` means auto-detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportHint {
    Auto,
    Theme(ThemeSource),
    Config(ThemeSource),
}

/// Guess the source family from the text alone.
pub fn detect(text: &str) -> Option<ThemeSource> {
    let t = text.trim_start();
    if t.starts_with("<?xml") || t.contains("<key>Ansi 0 Color</key>") {
        return Some(ThemeSource::Iterm2);
    }
    if t.contains("Ansi 0 Color") {
        return Some(ThemeSource::Iterm2);
    }
    if t.contains("[terminal]") || t.contains("inherits") {
        return Some(ThemeSource::Otty);
    }
    if t.contains("[colors.normal]") || t.contains("[colors.bright]") || t.contains("[window]") {
        return Some(ThemeSource::Alacritty);
    }
    if t.contains("[colors]") && t.contains("primary") {
        return Some(ThemeSource::Alacritty);
    }
    if text
        .lines()
        .any(|l| l.trim_start().starts_with("color0 ") || l.trim_start().starts_with("color0#"))
    {
        return Some(ThemeSource::Kitty);
    }
    if text.contains("palette") && text.contains('=') {
        return Some(ThemeSource::Ghostty);
    }
    if text.contains("background") && text.contains("foreground") {
        return Some(ThemeSource::Ghostty);
    }
    None
}

/// Parse a `.itermcolors` plist.
///
/// The file is a flat `<dict>` of `<key>Ansi N Color</key><dict>` blocks whose
/// components are `Red`/`Green`/`Blue` floats in 0…1.
pub fn parse_itermcolors(text: &str) -> Result<ImportedTheme, ImportError> {
    let comp = |label: &str| -> Option<Color> {
        let start = text.find(&format!("<key>{label}</key>"))?;
        let rest = &text[start..];
        let dict_end = rest.find("</dict>")?;
        let dict = &rest[..dict_end];
        // iTerm2 writes the channels as `Red Component`, `Green Component`,
        // `Blue Component` (and sometimes `Alpha Component`).
        let get = |name: &str| -> Option<f32> {
            let i = dict.find(&format!("<key>{name} Component</key>"))?;
            let after = &dict[i + name.len() + 11..];
            let ls = after.find("<real>")? + 6;
            let le = after[ls..].find("</real>")? + ls;
            after[ls..le].trim().parse::<f32>().ok()
        };
        let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Some(Color::rgb(to8(get("Red")?), to8(get("Green")?), to8(get("Blue")?)))
    };

    let mut palette = FALLBACK_PALETTE;
    for i in 0..16usize {
        if let Some(c) = comp(&format!("Ansi {i} Color")) {
            palette[i] = c;
        }
    }
    let foreground = comp("Foreground Color").unwrap_or(palette[7]);
    let background = comp("Background Color").unwrap_or(palette[0]);
    let cursor = comp("Cursor Color");
    let selection = comp("Selection Color");

    Ok(finish(
        ThemeSource::Iterm2,
        None,
        foreground,
        Some(background),
        palette,
        cursor,
        selection,
        None,
    ))
}

/// Parse a kitty `colorN` theme.
pub fn parse_kitty(text: &str) -> Result<ImportedTheme, ImportError> {
    let kv: Vec<(String, String)> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once(char::is_whitespace).or_else(|| l.split_once('='))?;
            let v = v.trim().trim_matches('"');
            let v = v.split_whitespace().next().unwrap_or(v);
            Some((k.trim().to_lowercase(), v.to_owned()))
        })
        .collect();
    let get = |k: &str| kv.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());

    let mut palette = FALLBACK_PALETTE;
    for i in 0..16usize {
        if let Some(v) = get(&format!("color{i}")) {
            if let Ok(c) = Color::parse(&v) {
                palette[i] = c;
            }
        }
    }
    let foreground = get("foreground").and_then(|v| Color::parse(&v).ok()).unwrap_or(palette[7]);
    let background = get("background").and_then(|v| Color::parse(&v).ok()).unwrap_or(palette[0]);
    let cursor = get("cursor").and_then(|v| Color::parse(&v).ok());
    let selection = get("selection_background").and_then(|v| Color::parse(&v).ok());
    Ok(finish(
        ThemeSource::Kitty,
        get("name"),
        foreground,
        Some(background),
        palette,
        cursor,
        selection,
        None,
    ))
}

/// Parse an Alacritty `[colors]` table.
pub fn parse_alacritty(text: &str) -> Result<ImportedTheme, ImportError> {
    let table: toml::Table = toml::from_str(text).map_err(|e| ImportError::Syntax {
        origin: ThemeSource::Alacritty,
        message: e.to_string(),
    })?;
    let colors = table
        .get("colors")
        .and_then(|v| v.as_table())
        .ok_or_else(|| ImportError::MissingSection {
            origin: ThemeSource::Alacritty,
            section: "[colors]".into(),
        })?;

    let slot = |key: &str, name: &str| -> Option<Color> {
        colors
            .get(key)
            .and_then(|v| v.as_table())
            .and_then(|t| t.get(name))
            .and_then(|v| v.as_str())
            .and_then(|s| Color::parse(s).ok())
    };
    let order = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    let mut palette = FALLBACK_PALETTE;
    for (i, name) in order.iter().enumerate() {
        if let Some(c) = slot("normal", name) {
            palette[i] = c;
        }
        if let Some(c) = slot("bright", name) {
            palette[i + 8] = c;
        }
    }
    let primary = colors.get("primary").and_then(|v| v.as_table());
    let foreground = primary
        .and_then(|t| t.get("foreground"))
        .and_then(|v| v.as_str())
        .and_then(|s| Color::parse(s).ok())
        .unwrap_or(palette[7]);
    let background = primary
        .and_then(|t| t.get("background"))
        .and_then(|v| v.as_str())
        .and_then(|s| Color::parse(s).ok())
        .unwrap_or(palette[0]);
    let cursor = colors
        .get("cursor")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("cursor"))
        .and_then(|v| v.as_str())
        .and_then(|s| Color::parse(s).ok());
    let selection = colors
        .get("selection")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("background"))
        .and_then(|v| v.as_str())
        .and_then(|s| Color::parse(s).ok());

    Ok(finish(
        ThemeSource::Alacritty,
        None,
        foreground,
        Some(background),
        palette,
        cursor,
        selection,
        None,
    ))
}

/// Parse a Ghostty config.
///
/// Ghostty's palette syntax is `palette = 0=#000000`, one slot per line, which
/// is also Otty's alternative `palette` spelling.
pub fn parse_ghostty(text: &str) -> Result<ImportedTheme, ImportError> {
    let mut palette = FALLBACK_PALETTE;
    let mut foreground = None;
    let mut background = None;
    let mut cursor = None;
    let mut selection = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (k.trim(), unquote(v.trim()));
        match key {
            "palette" => {
                // `N=#rrggbb`; a comma-separated run is also accepted.
                for entry in value.split(',') {
                    if let Some((idx, color)) = entry.split_once('=') {
                        if let (Ok(i), Ok(c)) =
                            (idx.trim().parse::<usize>(), Color::parse(color.trim()))
                        {
                            if i < 16 {
                                palette[i] = c;
                            }
                        }
                    }
                }
            }
            "foreground" => foreground = Color::parse(&value).ok(),
            "background" => background = Color::parse(&value).ok(),
            "cursor-color" => cursor = Color::parse(&value).ok(),
            "selection-background" => selection = Color::parse(&value).ok(),
            _ => {}
        }
    }

    Ok(finish(
        ThemeSource::Ghostty,
        None,
        foreground.unwrap_or(palette[7]),
        Some(background.unwrap_or(palette[0])),
        palette,
        cursor,
        selection,
        None,
    ))
}

#[allow(clippy::too_many_arguments)]
fn finish(
    source: ThemeSource,
    name: Option<String>,
    foreground: Color,
    background: Option<Color>,
    palette: [Color; 16],
    cursor: Option<Color>,
    selection: Option<Color>,
    author: Option<String>,
) -> ImportedTheme {
    let bg = background.unwrap_or(Color::OPAQUE_BLACK);
    let inferred = match name {
        Some(n) if !n.trim().is_empty() => n,
        _ => format!("{} {}", source.as_str(), bg.to_hex()),
    };
    let mode = Mode::detect(bg);
    let slug = slugify(&inferred);
    let theme = Theme {
        slug: slug.clone(),
        display_name: inferred.clone(),
        mode,
        author,
        description: Some(format!("Imported from {}.", source.as_str())),
        inherits: None,
        terminal: crate::theme::Terminal {
            foreground,
            background,
            palette,
            cursor,
            cursor_text: None,
            selection_foreground: None,
            selection_background: selection,
        },
        token: Default::default(),
        container: Default::default(),
        ui: Default::default(),
        panel: Default::default(),
        sidebar: Default::default(),
        titlebar: Default::default(),
        tab: Default::default(),
        selection: Default::default(),
        cursor: Default::default(),
        ghost: Default::default(),
        window: Default::default(),
        mac: Default::default(),
        windows: Default::default(),
        mac_sidebar: Default::default(),
        windows_sidebar: Default::default(),
        // An imported theme has no `[terminal]`-bearing source table of its own,
        // so `to_toml` materialises the typed view from scratch.
        raw: Default::default(),
        extra_tables: Default::default(),
        header: String::new(),
    };
    ImportedTheme { source, name: inferred, slug, mode, theme }
}

/// Import a theme from text, auto-detecting the format.
pub fn import_theme(text: &str, hint: ImportHint) -> Result<ImportedTheme, ImportError> {
    let source = match hint {
        ImportHint::Theme(s) => s,
        ImportHint::Auto | ImportHint::Config(_) => detect(text).ok_or(ImportError::UnknownFormat)?,
    };
    match source {
        ThemeSource::Otty => {
            let slug = "imported";
            let theme = Theme::parse(slug, text).map_err(|e| ImportError::Syntax { origin: source,
                message: e.to_string(),
            })?;
            let name = theme.display_name.clone();
            Ok(ImportedTheme {
                source,
                slug: slugify(&name),
                name,
                mode: theme.mode,
                theme,
            })
        }
        ThemeSource::Iterm2 => parse_itermcolors(text),
        ThemeSource::Kitty => parse_kitty(text),
        ThemeSource::Alacritty => parse_alacritty(text),
        ThemeSource::Ghostty => parse_ghostty(text),
    }
}

/// The `.ottytheme` text an [`ImportedTheme`] should be written as.
///
/// Only the keys the theme actually carries are emitted, so an imported theme
/// stays readable and an `inherits` line is easy to add by hand.
pub fn render_theme(imported: &ImportedTheme) -> String {
    let t = &imported.theme;
    let mut s = String::new();
    s.push_str(&format!("# Otty theme — {}\n", imported.name));
    s.push_str(&format!("# Imported from {}\n\n", imported.source.as_str()));
    s.push_str("[meta]\n");
    s.push_str(&format!("name = {:?}\n", imported.name));
    s.push_str(&format!("mode = \"{}\"\n", imported.mode.as_str()));
    if let Some(a) = &t.author {
        s.push_str(&format!("author = {a:?}\n"));
    }
    if let Some(d) = &t.description {
        s.push_str(&format!("description = {d:?}\n"));
    }
    s.push_str("\n[terminal]\n");
    s.push_str(&format!("foreground = \"{}\"\n", t.terminal.foreground.to_hex()));
    match t.terminal.background {
        Some(b) => s.push_str(&format!("background = \"{}\"\n", b.to_hex())),
        None => s.push_str("background = \"none\"\n"),
    }
    s.push_str("palette = [\n");
    for chunk in 0..4 {
        s.push_str("    ");
        for i in 0..4 {
            s.push_str(&format!("\"{}\", ", t.terminal.palette[chunk * 4 + i].to_hex()));
        }
        s.push_str(&format!("  # {}\n", PALETTE_ROW_LABELS[chunk]));
    }
    s.push_str("]\n");
    if let Some(c) = t.terminal.cursor {
        s.push_str(&format!("cursor = \"{}\"\n", c.to_hex()));
    }
    if let Some(c) = t.terminal.cursor_text {
        s.push_str(&format!("cursor-text = \"{}\"\n", c.to_hex()));
    }
    if let Some(c) = t.terminal.selection_background {
        s.push_str(&format!("selection-background = \"{}\"\n", c.to_hex()));
    }
    if let Some(c) = t.terminal.selection_foreground {
        s.push_str(&format!("selection-foreground = \"{}\"\n", c.to_hex()));
    }
    if let Some(sel) = t.selection.background {
        s.push_str("\n[selection]\n");
        s.push_str(&format!("background = \"{}\"\n", sel.to_hex()));
        if let Some(f) = t.selection.foreground {
            s.push_str(&format!("foreground = \"{}\"\n", f.to_hex()));
        }
    }
    s.push_str("\n[token]\n");
    s.push_str("font-ui = [\"Styrene B\", \"Styrene A\"]\n");
    s
}

const PALETTE_ROW_LABELS: [&str; 4] = [
    "0-3:  black red green yellow",
    "4-7:  blue magenta cyan white",
    "8-11: bright black red green yellow",
    "12-15: bright blue magenta cyan white",
];

/// Where a slug has to land on disk, honouring the `-1`, `-2` suffix rule.
pub fn allocate_slug(themes_dir: &std::path::Path, slug: &str, overwrite: bool) -> String {
    let path_for = |s: &str| themes_dir.join(format!("{s}.ottytheme"));
    if overwrite || !path_for(slug).exists() {
        return slug.to_owned();
    }
    for n in 1..1000 {
        let candidate = format!("{slug}-{n}");
        if !path_for(&candidate).exists() {
            return candidate;
        }
    }
    slug.to_owned()
}

/// Map another terminal's config keys onto Otty's.
///
/// Only settings with a direct counterpart are translated; everything else is
/// reported in [`ImportedConfig::unsupported`] rather than silently dropped.
pub fn import_settings(text: &str, source: ThemeSource) -> Result<ImportedConfig, ImportError> {
    let mut out = ImportedConfig { source: Some(source), ..Default::default() };
    let put = |out: &mut ImportedConfig, otty_key: &str, value: String| {
        if crate::spec::is_known(otty_key) {
            out.settings.push((otty_key.to_owned(), value));
        } else {
            out.unsupported.push(otty_key.to_owned());
        }
    };

    // The translation tables hold the keys each terminal spells the same way as
    // Otty, plus the handful of deliberate renames.
    let map: &[(&str, &str)] = match source {
        ThemeSource::Ghostty => &[
            ("background", "background"),
            ("foreground", "foreground"),
            ("palette", "palette"),
            ("cursor-color", "cursor-color"),
            ("cursor-text", "cursor-text"),
            ("selection-background", "selection-background"),
            ("selection-foreground", "selection-foreground"),
            ("font-family", "font-family"),
            ("font-size", "font-size"),
            ("font-bold", "text-bold"),
            ("font-italic", "text-italic"),
            ("font-ligatures", "font-ligatures"),
            ("background-opacity", "background-opacity"),
            ("cursor-style", "cursor-style"),
            ("scrollback-limit", "scrollback-limit"),
            ("window-width", "window-width-px"),
            ("window-height", "window-height-px"),
            ("mouse-hide-while-typing", "mouse-hide-while-typing"),
            ("copy-on-select", "copy-on-select"),
            ("confirm-close-surface", "confirm-close-tab"),
            ("clipboard-read", "clipboard-read"),
            ("clipboard-write", "clipboard-write"),
            ("macos-option-as-alt", "macos-option-as-alt"),
            ("shell-integration", "shell-integration"),
            ("working-directory", "working-directory"),
            ("font-thicken", "font-thicken"),
            ("adjust-cell-height", "adjust-cell-height"),
            ("theme", "theme"),
            ("keybind", "keybind"),
        ],
        ThemeSource::Kitty => &[
            ("font_family", "font-family"),
            ("font_size", "font-size"),
            ("background_opacity", "background-opacity"),
            ("cursor_shape", "cursor-style"),
            ("cursor_blink_interval", "cursor-style-blink"),
            ("scrollback_lines", "scrollback-lines"),
            ("copy_on_select", "copy-on-select"),
            ("window_padding_width", "adjust-cell-height"),
            ("shell_integration", "shell-integration"),
            ("enabled_layouts", "window-layout"),
            ("hide_window_decorations", "window-layout"),
        ],
        ThemeSource::Alacritty => &[
            ("font.normal.family", "font-family"),
            ("font.size", "font-size"),
            ("window.opacity", "background-opacity"),
            ("cursor.style", "cursor-style"),
            ("cursor.blink", "cursor-style-blink"),
            ("scrolling.history", "scrollback-lines"),
            ("selection.save_to_clipboard", "copy-on-select"),
            ("window.dimensions.columns", "window-cols"),
            ("window.dimensions.lines", "window-rows"),
            ("font.bold.family", "font-family-bold"),
            ("font.italic.family", "font-family-italic"),
        ],
        _ => &[],
    };

    let table: Vec<(String, String)> = if source == ThemeSource::Alacritty {
        let parsed: toml::Table = toml::from_str(text).map_err(|e| ImportError::Syntax { origin: source,
            message: e.to_string(),
        })?;
        let mut flat = Vec::new();
        flatten_table("", &parsed, &mut flat);
        flat
    } else {
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| {
                let (k, v) = l.split_once('=').or_else(|| l.split_once(char::is_whitespace))?;
                Some((k.trim().to_owned(), unquote(v.trim()).to_owned()))
            })
            .collect()
    };

    for (k, v) in &table {
        match map.iter().find(|(from, _)| from == k) {
            Some((_, to)) => put(&mut out, to, v.clone()),
            None => out.unsupported.push(k.clone()),
        }
    }
    out.settings.sort_by(|a, b| a.0.cmp(&b.0));
    out.settings.dedup_by(|a, b| a.0 == b.0);
    out.unsupported.sort();
    out.unsupported.dedup();
    Ok(out)
}

/// Flatten `a.b.c = value` into dotted keys, so one translation table can
/// describe a nested TOML source.
pub fn flatten_table(prefix: &str, table: &toml::Table, out: &mut Vec<(String, String)>) {
    for (k, v) in table {
        let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
        match v {
            toml::Value::Table(t) => flatten_table(&key, t, out),
            toml::Value::String(s) => out.push((key, s.clone())),
            toml::Value::Integer(i) => out.push((key, i.to_string())),
            // Render floats the way the source spelled them: `12.0`, not `12`.
            toml::Value::Float(f) => out.push((
                key,
                if f.fract() == 0.0 { format!("{f:.1}") } else { f.to_string() },
            )),
            toml::Value::Boolean(b) => out.push((key, b.to_string())),
            _ => {}
        }
    }
}

/// The existing theme whose palette is most similar to an import — used to
/// suggest an `inherits` base.
pub fn similar_lookup(imported: &ImportedTheme, candidates: &[(String, Theme)]) -> Option<String> {
    let target = &imported.theme.terminal;
    let mut best: Option<(f64, &str)> = None;
    for (slug, theme) in candidates {
        let mut d = 0f64;
        for i in 0..16 {
            let (a, b) = (target.palette[i], theme.terminal.palette[i]);
            d += (a.r as f64 - b.r as f64).powi(2)
                + (a.g as f64 - b.g as f64).powi(2)
                + (a.b as f64 - b.b as f64).powi(2);
        }
        let bg = theme.terminal.background;
        if let (Some(a), Some(b)) = (target.background, bg) {
            d += 3.0
                * ((a.r as f64 - b.r as f64).powi(2)
                    + (a.g as f64 - b.g as f64).powi(2)
                    + (a.b as f64 - b.b as f64).powi(2));
        }
        if best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, slug));
        }
    }
    best.map(|(_, s)| s.to_owned())
}

/// The platform default a theme without an explicit `[window]` should assume.
pub fn default_material_for(platform: Platform, mode: Mode) -> &'static str {
    match (platform, mode) {
        (Platform::Windows, Mode::Dark) => "acrylic",
        (Platform::Windows, Mode::Light) => "mica",
        (Platform::Macos, Mode::Dark) => "vibrancy-regular",
        (Platform::Macos, Mode::Light) => "vibrancy-thin",
        _ => "none",
    }
}

/// Why an import could not be completed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("could not tell which terminal this file came from")]
    UnknownFormat,
    #[error("not valid {origin}: {message}")]
    Syntax { origin: ThemeSource, message: String },
    #[error("{origin} file has no {section} section")]
    MissingSection { origin: ThemeSource, section: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITERM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Background Color</key>
	<dict>
		<key>Red Component</key><real>0.10196078431372549</real>
		<key>Green Component</key><real>0.10196078431372549</real>
		<key>Blue Component</key><real>0.15294117647058825</real>
	</dict>
	<key>Foreground Color</key>
	<dict>
		<key>Red Component</key><real>1</real>
		<key>Green Component</key><real>1</real>
		<key>Blue Component</key><real>1</real>
	</dict>
	<key>Ansi 1 Color</key>
	<dict>
		<key>Red Component</key><real>1</real>
		<key>Green Component</key><real>0.3333333333333333</real>
		<key>Blue Component</key><real>0.3333333333333333</real>
	</dict>
	<key>Cursor Color</key>
	<dict>
		<key>Red Component</key><real>1</real>
		<key>Green Component</key><real>0.5</real>
		<key>Blue Component</key><real>0</real>
	</dict>
</dict>
</plist>
"#;

    const KITTY: &str = "\
# a kitty theme
background #1a1b26
foreground #c0caf5
cursor #c0caf5
color0 #15161e
color1 #f7768e
color15 #c0caf5
selection_background #283457
";

    const ALACRITTY: &str = "\
[colors.primary]
background = \"#282c34\"
foreground = \"#abb2bf\"

[colors.cursor]
cursor = \"#528bff\"

[colors.normal]
black = \"#282c34\"
red = \"#e06c75\"

[colors.bright]
black = \"#545862\"
red = \"#e06c75\"

[font]
size = 12.0
";

    const GHOSTTY: &str = "\
# ghostty
background = #0a0a14
foreground = #e0e0ff
cursor-color = #ffa830
palette = 0=#0a0a14
palette = 1=#ff4d6d
palette = 15=#ffffff
";

    #[test]
    fn detection_picks_the_right_family() {
        assert_eq!(detect(ITERM), Some(ThemeSource::Iterm2));
        assert_eq!(detect(KITTY), Some(ThemeSource::Kitty));
        assert_eq!(detect(ALACRITTY), Some(ThemeSource::Alacritty));
        assert_eq!(detect(GHOSTTY), Some(ThemeSource::Ghostty));
        assert_eq!(
            detect("[terminal]\nforeground = \"#fff\"\n"),
            Some(ThemeSource::Otty)
        );
        assert_eq!(detect("just some prose"), None);
    }

    #[test]
    fn iterm2_floats_convert_to_bytes() {
        let t = import_theme(ITERM, ImportHint::Auto).unwrap();
        assert_eq!(t.source, ThemeSource::Iterm2);
        assert_eq!(t.theme.terminal.foreground.to_hex(), "#ffffff");
        assert_eq!(t.theme.terminal.background.unwrap().to_hex(), "#1a1a27");
        assert_eq!(t.theme.terminal.palette[1].to_hex(), "#ff5555");
        assert_eq!(t.theme.terminal.cursor.unwrap().to_hex(), "#ff8000");
        assert_eq!(t.mode, Mode::Dark);
    }

    #[test]
    fn kitty_directives_land_in_the_right_slots() {
        let t = import_theme(KITTY, ImportHint::Auto).unwrap();
        assert_eq!(t.source, ThemeSource::Kitty);
        assert_eq!(t.theme.terminal.palette[1].to_hex(), "#f7768e");
        assert_eq!(t.theme.terminal.palette[15].to_hex(), "#c0caf5");
        assert_eq!(t.theme.terminal.selection_background.unwrap().to_hex(), "#283457");
    }

    #[test]
    fn alacritty_nested_tables_are_read() {
        let t = import_theme(ALACRITTY, ImportHint::Auto).unwrap();
        assert_eq!(t.theme.terminal.background.unwrap().to_hex(), "#282c34");
        assert_eq!(t.theme.terminal.palette[1].to_hex(), "#e06c75");
        assert_eq!(t.theme.terminal.palette[9].to_hex(), "#e06c75");
        assert_eq!(t.theme.terminal.cursor.unwrap().to_hex(), "#528bff");
    }

    #[test]
    fn ghostty_palette_lines_are_read() {
        let t = import_theme(GHOSTTY, ImportHint::Auto).unwrap();
        assert_eq!(t.theme.terminal.background.unwrap().to_hex(), "#0a0a14");
        assert_eq!(t.theme.terminal.palette[1].to_hex(), "#ff4d6d");
        assert_eq!(t.theme.terminal.cursor.unwrap().to_hex(), "#ffa830");
        // Slots the source does not set keep the fallback palette.
        assert_eq!(t.theme.terminal.palette[7].to_hex(), crate::palette::FALLBACK_PALETTE[7].to_hex());
    }

    #[test]
    fn an_imported_theme_renders_back_into_a_loadable_ottytheme() {
        for src in [ITERM, KITTY, ALACRITTY, GHOSTTY] {
            let t = import_theme(src, ImportHint::Auto).unwrap();
            let text = render_theme(&t);
            let reparsed = Theme::parse(&t.slug, &text)
                .unwrap_or_else(|e| panic!("{} render did not reparse: {e}\n{text}", t.source.as_str()));
            assert_eq!(reparsed.terminal.foreground, t.theme.terminal.foreground);
            assert_eq!(reparsed.terminal.palette, t.theme.terminal.palette);
            assert_eq!(reparsed.mode, t.mode);
            assert_eq!(reparsed.token.font_ui.as_deref(), Some(&["Styrene B".to_owned(), "Styrene A".to_owned()][..]));
        }
    }

    #[test]
    fn slugs_do_not_collide_and_gain_a_counter_suffix() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(allocate_slug(dir.path(), "nord", false), "nord");
        std::fs::write(dir.path().join("nord.ottytheme"), "").unwrap();
        assert_eq!(allocate_slug(dir.path(), "nord", false), "nord-1");
        assert_eq!(allocate_slug(dir.path(), "nord", true), "nord");
        std::fs::write(dir.path().join("nord-1.ottytheme"), "").unwrap();
        assert_eq!(allocate_slug(dir.path(), "nord", false), "nord-2");
    }

    #[test]
    fn settings_translation_keeps_known_keys_and_reports_the_rest() {
        let out = import_settings(GHOSTTY, ThemeSource::Ghostty).unwrap();
        let keys: Vec<&str> = out.settings.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"foreground"));
        assert!(keys.contains(&"palette"));
        // `palette` is a known key, so it is not reported as unsupported.
        assert!(!out.unsupported.contains(&"palette".to_owned()));

        let out = import_settings(ALACRITTY, ThemeSource::Alacritty).unwrap();
        let pairs: Vec<(&str, &str)> =
            out.settings.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(pairs.contains(&("font-size", "12.0")));
        // The fixture sets no bold family, so the key is simply absent.
        assert!(!pairs.iter().any(|(k, _)| *k == "font-family-bold"));
    }

    #[test]
    fn similar_theme_lookup_picks_the_closest_palette() {
        let imported = import_theme(KITTY, ImportHint::Auto).unwrap();
        let a = Theme::load("dracula", None).unwrap();
        let b = Theme::parse(
            "identical",
            &render_theme(&imported).replace("mode = \"dark\"", "mode = \"dark\""),
        )
        .unwrap();
        let candidates = vec![("dracula".to_owned(), a), ("identical".to_owned(), b)];
        assert_eq!(similar_lookup(&imported, &candidates).as_deref(), Some("identical"));
    }

    #[test]
    fn platform_material_defaults_are_sane() {
        assert_eq!(default_material_for(Platform::Windows, Mode::Dark), "acrylic");
        assert_eq!(default_material_for(Platform::Macos, Mode::Dark), "vibrancy-regular");
        assert_eq!(default_material_for(Platform::Linux, Mode::Dark), "none");
    }

    #[test]
    fn an_unrecognisable_file_is_reported_rather_than_guessed() {
        assert_eq!(import_theme("hello", ImportHint::Auto), Err(ImportError::UnknownFormat));
    }
}
