//! Exporting Otty's configuration to another terminal.
//!
//! `otty export ghostty|kitty|alacritty`. Each exporter emits the colour theme
//! plus the settings the target terminal actually understands; anything Otty
//! has no counterpart for is listed in a trailing comment block rather than
//! silently dropped, so the output explains itself.

use crate::theme::{Mode, Theme};
use crate::Config;

/// Which terminal to write a config for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    Ghostty,
    Kitty,
    Alacritty,
}

impl ExportTarget {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "ghostty" => Self::Ghostty,
            "kitty" => Self::Kitty,
            "alacritty" => Self::Alacritty,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ghostty => "ghostty",
            Self::Kitty => "kitty",
            Self::Alacritty => "alacritty",
        }
    }

    /// The file a user would normally drop the output into.
    pub const fn default_path(self) -> &'static str {
        match self {
            Self::Ghostty => "~/.config/ghostty/config",
            Self::Kitty => "~/.config/kitty/kitty.conf",
            Self::Alacritty => "~/.config/alacritty/alacritty.toml",
        }
    }
}

/// A rendered export plus the Otty settings that had no counterpart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub target: ExportTarget,
    pub text: String,
    /// Otty config keys the target terminal cannot express.
    pub dropped: Vec<String>,
}

impl Export {
    /// Write the export to `path`, or return it for stdout when `path` is `None`.
    pub fn write(&self, path: Option<&std::path::Path>) -> std::io::Result<()> {
        match path {
            Some(p) => crate::write_atomic(p, self.text.as_bytes()),
            None => {
                use std::io::Write;
                std::io::stdout().write_all(self.text.as_bytes())
            }
        }
    }
}

/// Render the config for `target`.
pub fn export(config: &Config, theme: &Theme, target: ExportTarget) -> Export {
    match target {
        ExportTarget::Ghostty => export_ghostty(config, theme),
        ExportTarget::Kitty => export_kitty(config, theme),
        ExportTarget::Alacritty => export_alacritty(config, theme),
    }
}

/// Otty keys each target can express. Everything else is reported as dropped.
fn supported(target: ExportTarget) -> &'static [&'static str] {
    match target {
        ExportTarget::Ghostty => &[
            "font-family", "font-family-fallback", "font-size", "font-blending", "text-bold",
            "text-italic", "font-ligatures", "font-thicken", "adjust-cell-height",
            "cursor-style", "cursor-style-blink", "cursor-color", "cursor-text",
            "background-opacity", "foreground", "background", "palette-0", "palette-1",
            "palette-2", "palette-3", "palette-4", "palette-5", "palette-6", "palette-7",
            "palette-8", "palette-9", "palette-10", "palette-11", "palette-12", "palette-13",
            "palette-14", "palette-15", "selection-background", "selection-foreground",
            "scrollback-limit", "window-width-px", "window-height-px", "window-cols",
            "window-rows", "copy-on-select", "mouse-hide-while-typing", "mouse-reporting",
            "macos-option-as-alt", "working-directory", "shell-integration", "clipboard-read",
            "clipboard-write", "keybind", "minimum-contrast", "background-opacity",
            "unfocused-split-opacity", "bold-color", "faint-opacity",
        ],
        ExportTarget::Kitty => &[
            "font-family", "font-family-fallback", "font-size", "text-bold", "text-italic",
            "font-ligatures", "adjust-cell-height", "cursor-style", "cursor-style-blink",
            "cursor-color", "cursor-text", "background-opacity", "foreground", "background",
            "palette-0", "palette-1", "palette-2", "palette-3", "palette-4", "palette-5",
            "palette-6", "palette-7", "palette-8", "palette-9", "palette-10", "palette-11",
            "palette-12", "palette-13", "palette-14", "palette-15", "selection-background",
            "selection-foreground", "scrollback-lines", "window-cols", "window-rows",
            "copy-on-select", "mouse-hide-while-typing", "mouse-reporting", "shell-integration",
            "window-layout", "url-color", "link-schemes",
        ],
        ExportTarget::Alacritty => &[
            "font-family", "font-family-bold", "font-family-italic", "font-family-bold-italic",
            "font-size", "adjust-cell-height", "cursor-style", "cursor-style-blink",
            "cursor-color", "cursor-text", "background-opacity", "background-opacity",
            "foreground", "background", "palette-0", "palette-1", "palette-2", "palette-3",
            "palette-4", "palette-5", "palette-6", "palette-7", "palette-8", "palette-9",
            "palette-10", "palette-11", "palette-12", "palette-13", "palette-14", "palette-15",
            "selection-background", "selection-foreground", "scrollback-lines", "window-cols",
            "window-rows", "copy-on-select", "mouse-hide-while-typing", "working-directory",
        ],
    }
}

fn dropped_keys(config: &Config, target: ExportTarget) -> Vec<String> {
    let ok = supported(target);
    let mut out: Vec<String> = config
        .to_json()
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    out.retain(|k| !ok.contains(&k.as_str()) && !k.starts_with("palette"));
    out.sort();
    out
}

fn header(target: ExportTarget, theme: &Theme) -> String {
    format!(
        "# Generated by Otty {} — do not edit by hand.\n\
         # Theme: {} ({})\n\
         # Drop this file at {}.\n\n",
        otty_core::VERSION,
        theme.display_name,
        theme.mode.as_str(),
        target.default_path()
    )
}

fn footer(target: ExportTarget, config: &Config) -> String {
    let dropped = dropped_keys(config, target);
    if dropped.is_empty() {
        return String::new();
    }
    let mut s = format!(
        "\n# Otty settings with no {} counterpart ({}):\n",
        target.as_str(),
        dropped.len()
    );
    for k in dropped {
        s.push_str(&format!("#   {k}\n"));
    }
    s
}

fn hex(c: otty_core::Color) -> String {
    c.to_hex()
}

/// `ghostty` uses the same `key = value` shape as Otty, so the export is mostly
/// a rename plus the colour block.
pub fn export_ghostty(config: &Config, theme: &Theme) -> Export {
    let colors = config.resolved_colors(theme.mode == Mode::Dark);
    let mut s = header(ExportTarget::Ghostty, theme);
    s.push_str("# --- Colours ---\n");
    s.push_str(&format!("background = {}\n", hex(colors.background)));
    s.push_str(&format!("foreground = {}\n", hex(colors.foreground)));
    if let Some(c) = config.get_color("cursor-color") {
        s.push_str(&format!("cursor-color = {}\n", hex(c)));
    }
    if let Some(c) = config.get_color("selection-background") {
        s.push_str(&format!("selection-background = {}\n", hex(c)));
    }
    for i in 0..16u8 {
        s.push_str(&format!("palette = {i}={}\n", hex(colors.palette.get(i))));
    }

    s.push_str("\n# --- Font ---\n");
    if let Some(f) = config.get_str("font-family") {
        s.push_str(&format!("font-family = {f}\n"));
    }
    if let Some(size) = config.get_float("font-size") {
        s.push_str(&format!("font-size = {size}\n"));
    }
    let fallbacks = config.get_list("font-family-fallback");
    if !fallbacks.is_empty() {
        s.push_str(&format!("font-family = {}\n", fallbacks.join(", ")));
    }
    for (otty, ghostty) in [
        ("font-blending", "font-blending"),
        ("text-bold", "font-bold"),
        ("text-italic", "font-italic"),
        ("font-ligatures", "font-ligatures"),
        ("font-thicken", "font-thicken"),
        ("adjust-cell-height", "adjust-cell-height"),
    ] {
        if let Some(v) = config.get(otty) {
            s.push_str(&format!("{ghostty} = {}\n", v.render()));
        }
    }

    s.push_str("\n# --- Window ---\n");
    for (otty, ghostty) in [
        ("background-opacity", "background-opacity"),
        ("unfocused-split-opacity", "unfocused-split-opacity"),
        ("window-width-px", "window-width"),
        ("window-height-px", "window-height"),
        ("window-cols", "window-cols"),
        ("window-rows", "window-rows"),
        ("minimum-contrast", "minimum-contrast"),
        ("copy-on-select", "copy-on-select"),
        ("mouse-hide-while-typing", "mouse-hide-while-typing"),
        ("macos-option-as-alt", "macos-option-as-alt"),
        ("scrollback-lines", "scrollback-limit"),
    ] {
        if let Some(v) = config.get(otty) {
            s.push_str(&format!("{ghostty} = {}\n", v.render()));
        }
    }
    if let Some(v) = config.get_str("command") {
        s.push_str(&format!("command = {v}\n"));
    }
    for entry in config.get_list("env") {
        s.push_str(&format!("env = {entry}\n"));
    }
    for entry in config.get_list("keybind") {
        // Otty spells the separator `=`, Ghostty spells it `=`, actions differ.
        s.push_str(&format!("# keybind = {entry}\n"));
    }

    s.push_str(&footer(ExportTarget::Ghostty, config));
    Export { target: ExportTarget::Ghostty, text: s, dropped: dropped_keys(config, ExportTarget::Ghostty) }
}

/// `kitty` uses whitespace-separated directives.
pub fn export_kitty(config: &Config, theme: &Theme) -> Export {
    let colors = config.resolved_colors(theme.mode == Mode::Dark);
    let mut s = header(ExportTarget::Kitty, theme);
    s.push_str("# --- Colours ---\n");
    s.push_str(&format!("background {}\n", hex(colors.background)));
    s.push_str(&format!("foreground {}\n", hex(colors.foreground)));
    s.push_str(&format!("cursor {}\n", hex(colors.cursor)));
    s.push_str(&format!("cursor_text_color {}\n", hex(colors.cursor_text)));
    if let Some(c) = colors.selection_background {
        s.push_str(&format!("selection_background {}\n", hex(c)));
    }
    if let Some(c) = colors.selection_foreground {
        s.push_str(&format!("selection_foreground {}\n", hex(c)));
    }
    for i in 0..16u8 {
        s.push_str(&format!("color{i} {}\n", hex(colors.palette.get(i))));
    }

    s.push_str("\n# --- Font ---\n");
    if let Some(f) = config.get_str("font-family") {
        s.push_str(&format!("font_family {f}\n"));
    }
    if let Some(size) = config.get_float("font-size") {
        s.push_str(&format!("font_size {size}\n"));
    }
    for f in config.get_list("font-family-fallback") {
        s.push_str(&format!("symbol_map U+E000-U+F8FF {f}\n"));
    }
    match config.get_str("font-ligatures").as_deref() {
        Some("off") => s.push_str("disable_ligatures always\n"),
        Some("dlig") => s.push_str("disable_ligatures never\n"),
        _ => {}
    }
    match config.get_str("cursor-style").as_deref() {
        Some("block") => s.push_str("cursor_shape block\n"),
        Some("bar") => s.push_str("cursor_shape beam\n"),
        Some("underline") | Some("block_hollow") => s.push_str("cursor_shape underline\n"),
        _ => {}
    }
    if let Some(b) = config.get_bool("cursor-style-blink") {
        s.push_str(&format!("cursor_blink_interval {}\n", if b { 0.5 } else { 0.0 }));
    }

    s.push_str("\n# --- Window ---\n");
    if let Some(o) = config.get_float("background-opacity") {
        s.push_str(&format!("background_opacity {o}\n"));
    }
    if let Some(v) = config.get_int("scrollback-lines") {
        s.push_str(&format!("scrollback_lines {v}\n"));
    }
    if let Some(v) = config.get_bool("copy-on-select") {
        s.push_str(&format!("copy_on_select {}\n", if v { "yes" } else { "no" }));
    }
    if let Some(v) = config.get_bool("mouse-hide-while-typing") {
        s.push_str(&format!(
            "mouse_hide_wait {}\n",
            if v { 0.0 } else { -1.0 }
        ));
    }
    s.push_str(&footer(ExportTarget::Kitty, config));
    Export { target: ExportTarget::Kitty, text: s, dropped: dropped_keys(config, ExportTarget::Kitty) }
}

/// `alacritty` uses TOML with nested colour tables.
pub fn export_alacritty(config: &Config, theme: &Theme) -> Export {
    let colors = config.resolved_colors(theme.mode == Mode::Dark);
    let p = |i: u8| hex(colors.palette.get(i));
    let mut s = header(ExportTarget::Alacritty, theme);

    // TOML has no `null`, so `[terminal]` is emitted only when there is a shell
    // to name — an empty table is the same as omitting the section.
    if let Some(c) = config.get_str("command") {
        s.push_str(&format!("[terminal]\nshell = {c:?}\n\n"));
    }
    s.push_str("\n[window]\n");
    if let Some(o) = config.get_float("background-opacity") {
        s.push_str(&format!("opacity = {o}\n"));
    }
    if let Some(c) = config.get_int("window-cols") {
        s.push_str(&format!("columns = {c}\n"));
    }
    if let Some(r) = config.get_int("window-rows") {
        s.push_str(&format!("lines = {r}\n"));
    }

    s.push_str("\n[font]\n");
    if let Some(size) = config.get_float("font-size") {
        s.push_str(&format!("size = {size}\n"));
    }
    s.push_str("\n[font.normal]\n");
    s.push_str(&format!(
        "family = {:?}\n",
        config.get_str("font-family").unwrap_or_else(|| "monospace".into())
    ));

    s.push_str("\n[colors.primary]\n");
    s.push_str(&format!("background = \"{}\"\n", hex(colors.background)));
    s.push_str(&format!("foreground = \"{}\"\n", hex(colors.foreground)));

    s.push_str("\n[colors.cursor]\n");
    s.push_str(&format!("cursor = \"{}\"\n", hex(colors.cursor)));
    if config.get_color("cursor-text").is_some() {
        s.push_str(&format!("text = \"{}\"\n", hex(colors.cursor_text)));
    }

    s.push_str("\n[colors.selection]\n");
    if let Some(c) = colors.selection_background {
        s.push_str(&format!("background = \"{}\"\n", hex(c)));
    }
    if let Some(c) = colors.selection_foreground {
        s.push_str(&format!("text = \"{}\"\n", hex(c)));
    }

    s.push_str("\n[colors.normal]\n");
    for (i, name) in ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]
        .iter()
        .enumerate()
    {
        s.push_str(&format!("{name} = \"{}\"\n", p(i as u8)));
    }
    s.push_str("\n[colors.bright]\n");
    for (i, name) in ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]
        .iter()
        .enumerate()
    {
        s.push_str(&format!("{name} = \"{}\"\n", p(i as u8 + 8)));
    }

    s.push_str("\n[scrolling]\n");
    if let Some(v) = config.get_int("scrollback-lines") {
        s.push_str(&format!("history = {v}\n"));
    }

    match config.get_str("cursor-style").as_deref() {
        Some(shape @ ("block" | "bar" | "underline")) => {
            s.push_str(&format!("\n[cursor]\nstyle = {{ shape = \"{shape}\" }}\n"));
            if let Some(b) = config.get_bool("cursor-style-blink") {
                s.push_str(&format!("blink = {b}\n"));
            }
        }
        _ => {}
    }

    s.push_str(&footer(ExportTarget::Alacritty, config));
    Export {
        target: ExportTarget::Alacritty,
        text: s,
        dropped: dropped_keys(config, ExportTarget::Alacritty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(extra: &str) -> Config {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            format!("theme = tokyo-night\nfont-family = \"JetBrains Mono\"\nfont-size = 13\n{extra}"),
        )
        .unwrap();
        let cfg = Config::load_from(dir.path());
        std::mem::forget(dir);
        cfg
    }

    #[test]
    fn targets_parse_and_default_paths_are_documented() {
        assert_eq!(ExportTarget::parse("Ghostty"), Some(ExportTarget::Ghostty));
        assert_eq!(ExportTarget::parse("kitty"), Some(ExportTarget::Kitty));
        assert_eq!(ExportTarget::parse("alacritty"), Some(ExportTarget::Alacritty));
        assert!(ExportTarget::parse("iterm").is_none());
        assert_eq!(ExportTarget::Ghostty.default_path(), "~/.config/ghostty/config");
        assert_eq!(ExportTarget::Kitty.default_path(), "~/.config/kitty/kitty.conf");
        assert_eq!(ExportTarget::Alacritty.default_path(), "~/.config/alacritty/alacritty.toml");
    }

    #[test]
    fn ghostty_export_carries_every_palette_slot() {
        let cfg = fixture("");
        let theme = cfg.active_theme(true);
        let e = export(&cfg, &theme, ExportTarget::Ghostty);
        for i in 0..16 {
            assert!(e.text.contains(&format!("palette = {i}=#")), "slot {i} missing");
        }
        assert!(e.text.contains("font-family = JetBrains Mono"));
        assert!(e.text.contains("background-opacity"));
    }

    #[test]
    fn kitty_export_uses_whitespace_directives() {
        let cfg = fixture("selection-background = \"#283457\"\n");
        let theme = cfg.active_theme(true);
        let e = export(&cfg, &theme, ExportTarget::Kitty);
        assert!(e.text.contains("color15 #"));
        assert!(e.text.contains("selection_background #283457"));
        assert!(e.text.contains("font_family JetBrains Mono"));
        assert!(!e.text.contains("color16"));
    }

    #[test]
    fn alacritty_export_is_valid_toml() {
        let cfg = fixture("");
        let theme = cfg.active_theme(true);
        let e = export(&cfg, &theme, ExportTarget::Alacritty);
        let parsed: toml::Table = toml::from_str(&e.text)
            .unwrap_or_else(|err| panic!("alacritty export is not TOML: {err}\n{}", e.text));
        let colors = parsed["colors"].as_table().unwrap();
        assert_eq!(colors["normal"].as_table().unwrap().len(), 8);
        assert_eq!(colors["bright"].as_table().unwrap().len(), 8);
        assert!(colors["primary"]["background"].is_str());
    }

    #[test]
    fn exports_name_the_theme_and_where_to_put_the_file() {
        let cfg = fixture("");
        let theme = cfg.active_theme(true);
        for t in [ExportTarget::Ghostty, ExportTarget::Kitty, ExportTarget::Alacritty] {
            let e = export(&cfg, &theme, t);
            assert!(e.text.starts_with("# Generated by Otty"), "{:?}", t);
            assert!(e.text.contains(t.default_path()));
        }
    }

    #[test]
    fn unsupported_settings_are_listed_rather_than_silently_dropped() {
        let cfg = fixture(
            "remote-access = true\nrecipe-replay-saved = auto\ntab-rule = 'path=\"/tmp\"'\n",
        );
        let theme = cfg.active_theme(true);
        let e = export(&cfg, &theme, ExportTarget::Alacritty);
        assert!(e.dropped.contains(&"remote-access".to_owned()));
        assert!(e.dropped.contains(&"recipe-replay-saved".to_owned()));
        assert!(e.text.contains("#   remote-access"));
    }

    #[test]
    fn writing_to_a_file_creates_the_path() {
        let cfg = fixture("");
        let theme = cfg.active_theme(true);
        let e = export(&cfg, &theme, ExportTarget::Kitty);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep").join("kitty.conf");
        e.write(Some(&path)).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("foreground #"));
    }
}
