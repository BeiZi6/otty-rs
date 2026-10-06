//! Otty's configuration layer.
//!
//! # Layout on disk
//!
//! ```text
//! ~/.config/otty/
//!   config.toml            the shared config — sync it between machines
//!   config.local.toml      this machine only; never synced; its keys win
//!   themes/<slug>.ottytheme
//!   fonts/                 drop-in font files (.ttf .otf .ttc .otc)
//!   views/<id>/            working directories for Details-Panel tui views
//! ```
//!
//! # Reading
//!
//! `config.toml` may `include` other files. All includes are read first, in the
//! order they appear; the main file is read after them and overrides; and
//! `config.local.toml` is read last and overrides everything. Within one file
//! the last assignment of a scalar key wins.
//!
//! Repeatable keys accumulate across layers. The *pairs* families (`env`,
//! `cli-alias`, `agent-command`, `open-with-app`, `open-with-command`) are keyed
//! by their left-hand side, so a later layer replaces an earlier entry with the
//! same name rather than appending a duplicate.
//!
//! # Writing
//!
//! Nothing regenerates the file. Every write edits the file the value already
//! comes from, in place, through [`toml_lite::Document`].

pub mod actions;
pub mod catalog;
pub mod export;
pub mod menu;
pub mod import;
pub mod keys;
pub mod keybind;
pub mod palette;
pub mod rules;
pub mod spec;
pub mod theme;
pub mod toml_lite;
pub mod value;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use otty_core::{expand_path, Color, Platform};

pub use catalog::{Catalog, CatalogError, ResolvedLanguage, UiLanguage};
pub use menu::{clickable_actions, menu_bar, Menu, MenuItem};
pub use palette::{BoldColor, Palette, ResolvedColors};
pub use rules::{DetailsView, QuickAccess, TabRule};
pub use spec::{Default as KeyDefault, KeySpec, ListStyle};
pub use theme::{Mode as ThemeMode, Selection, Theme};
pub use toml_lite::{write_atomic, Document, Line};
pub use value::{KeyKind, Value, ValueError};

/// Which file a write should land in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveScope {
    /// Write to the file the value currently comes from; a brand-new key goes
    /// to `config.toml`. This is what the Settings window and `otty config set`
    /// do by default.
    #[default]
    Auto,
    /// Force `config.local.toml` — "this machine only".
    Local,
    /// Force `config.toml`.
    Global,
}

impl SaveScope {
    /// Whether this scope is allowed to write `key` in `file`.
    pub fn covers(self, file: ConfigFile, _key: &str) -> bool {
        match self {
            Self::Auto => true,
            Self::Local => file == ConfigFile::Local,
            Self::Global => file == ConfigFile::Main,
        }
    }
}

/// The files a config can be spread across.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigFile {
    /// One of the files pulled in by `include`.
    Include,
    /// `config.toml`.
    Main,
    /// `config.local.toml`.
    Local,
}

impl ConfigFile {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Include => "included file",
            Self::Main => "config.toml",
            Self::Local => "config.local.toml",
        }
    }
}

/// A problem the loader found but worked around. Nothing here is fatal: an
/// unparseable value is reported and the default is used instead.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub key: String,
    /// `None` for shape problems that are not tied to one key.
    pub file: Option<PathBuf>,
    pub line: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.file, self.line) {
            (Some(p), Some(l)) => write!(f, "{}:{}: {}", p.display(), l, self.message),
            (Some(p), None) => write!(f, "{}: {}", p.display(), self.message),
            _ => f.write_str(&self.message),
        }
    }
}

/// The loaded configuration.
#[derive(Debug, Clone)]
pub struct Config {
    dir: PathBuf,
    main: Document,
    local: Option<Document>,
    includes: Vec<Document>,
    /// Effective values after layering and defaults, keyed by canonical name.
    effective: BTreeMap<String, Value>,
    /// Which file each effective value came from.
    origin: BTreeMap<String, PathBuf>,
    warnings: Vec<Warning>,
}

impl Default for Config {
    fn default() -> Self {
        Self::empty_in(otty_core::config_dir())
    }
}

impl Config {
    /// The user config directory (`~/.config/otty`).
    pub fn config_dir() -> PathBuf {
        otty_core::config_dir()
    }

    /// The application-support directory, used for caches and state.
    pub fn app_support_dir() -> PathBuf {
        otty_core::app_support_dir()
    }

    /// `~/.config/otty/config.toml`.
    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// `~/.config/otty/config.local.toml`.
    pub fn local_config_path() -> PathBuf {
        Self::config_dir().join("config.local.toml")
    }

    /// The themes directory.
    pub fn themes_dir() -> PathBuf {
        Self::config_dir().join("themes")
    }

    /// The drop-in fonts directory.
    pub fn fonts_dir() -> PathBuf {
        Self::config_dir().join("fonts")
    }

    /// The Details-Panel view working directories.
    pub fn views_dir() -> PathBuf {
        Self::config_dir().join("views")
    }

    /// Bundled Settings page (`settings-ui.html`) copied from the shipped app.
    pub fn settings_ui_html(resources: &Path) -> PathBuf {
        resources.join("settings-ui.html")
    }

    /// `language` key: `system` / `english` / `chinese`.
    pub fn ui_language(&self) -> crate::catalog::UiLanguage {
        crate::catalog::UiLanguage::from_config_value(
            self.get_str("language").as_deref().unwrap_or("system"),
        )
    }

    /// Load `en.json` / `zh.json` from `resources` using the `language` key.
    pub fn load_catalog(
        &self,
        resources: &Path,
    ) -> Result<crate::catalog::Catalog, crate::catalog::CatalogError> {
        crate::catalog::Catalog::load_for(self.ui_language(), resources)
    }

    /// A config with no files behind it, rooted at `dir`. Used by tests and by
    /// the first launch, before anything has been written.
    pub fn empty_in(dir: PathBuf) -> Self {
        let mut cfg = Self {
            dir,
            main: Document::default(),
            local: None,
            includes: Vec::new(),
            effective: BTreeMap::new(),
            origin: BTreeMap::new(),
            warnings: Vec::new(),
        };
        cfg.recompute();
        cfg
    }

    /// Load from the default locations, tolerating absent files.
    pub fn load() -> Self {
        Self::load_from(&Self::config_dir())
    }

    /// Load `config.toml` + `config.local.toml` from `dir`.
    pub fn load_from(dir: &Path) -> Self {
        let mut warnings = Vec::new();
        let main_path = dir.join("config.toml");
        let main = match Document::read(&main_path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Document::default(),
            Err(e) => {
                warnings.push(Warning {
                    key: String::new(),
                    file: Some(main_path.clone()),
                    line: None,
                    message: format!("could not read config.toml: {e}"),
                });
                Document::default()
            }
        };
        let local_path = dir.join("config.local.toml");
        let local = match Document::read(&local_path) {
            Ok(d) => Some(d),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                warnings.push(Warning {
                    key: String::new(),
                    file: Some(local_path),
                    line: None,
                    message: format!("could not read config.local.toml: {e}"),
                });
                None
            }
        };

        let mut includes = Vec::new();
        for inc in main.includes() {
            let expanded = expand_path(&unquote_path(&inc));
            let mut path = PathBuf::from(&expanded);
            // A relative `include` resolves against the config directory, not
            // the process working directory — `include = base.toml` is how the
            // reference documents a sibling file.
            if path.is_relative() {
                path = dir.join(path);
            }
            match Document::read(&path) {
                Ok(mut d) => {
                    d.path = Some(path);
                    includes.push(d);
                }
                Err(e) => warnings.push(Warning {
                    key: "include".into(),
                    file: Some(path.clone()),
                    line: None,
                    message: format!("include could not be read: {e}"),
                }),
            }
        }

        let mut cfg = Self {
            dir: dir.to_path_buf(),
            main,
            local,
            includes,
            effective: BTreeMap::new(),
            origin: BTreeMap::new(),
            warnings,
        };
        cfg.recompute();
        cfg
    }

    /// Every key Otty recognises.
    pub fn known_keys() -> &'static [KeySpec] {
        spec::all()
    }

    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The include chain, main file last — the order values are resolved in.
    fn layers(&self) -> Vec<(ConfigFile, &Document)> {
        let mut v: Vec<(ConfigFile, &Document)> = Vec::new();
        for d in &self.includes {
            v.push((ConfigFile::Include, d));
        }
        v.push((ConfigFile::Main, &self.main));
        if let Some(l) = &self.local {
            v.push((ConfigFile::Local, l));
        }
        v
    }

    /// Re-resolve every effective value from the layers.
    fn recompute(&mut self) {
        self.effective.clear();
        self.origin.clear();
        // Keep the warnings the loader raised (a missing `include`, an
        // unreadable file) and drop only the per-key ones, which the loop below
        // re-derives.
        self.warnings.retain(|w| !spec::is_known(&w.key));

        // Snapshot the layers as owned data first: the body below mutates the
        // maps the borrow would otherwise pin.
        let layers: Vec<(ConfigFile, PathBuf, Vec<(String, String)>)> = self
            .layers()
            .into_iter()
            .map(|(kind, doc)| {
                let path = doc.path.clone().unwrap_or_else(|| default_path_for(&self.dir, kind));
                let entries = doc
                    .lines
                    .iter()
                    .filter_map(|l| match l {
                        Line::Entry { key, value, .. } => Some((key.clone(), value.clone())),
                        _ => None,
                    })
                    .collect();
                (kind, path, entries)
            })
            .collect();

        for (_kind, path, entries) in layers {
            for (key, value) in entries {
                let Some(spec) = spec::lookup(&key) else {
                    // Unknown keys are silently ignored — a config written for a
                    // newer Otty stays loadable on an older build.
                    continue;
                };
                let canonical = spec.key;
                let entry = value::unquote(&value);

                if spec.list_style == Some(ListStyle::Repeated) {
                    let list = self
                        .effective
                        .entry(canonical.to_owned())
                        .or_insert_with(|| Value::List(Vec::new()));
                    if let Value::List(items) = list {
                        if spec.kind == KeyKind::Pairs {
                            // Keyed by the left-hand side: a later layer
                            // replaces the same name instead of duplicating it.
                            match value::split_first_eq(&entry) {
                                Some((k, _)) => {
                                    match items.iter_mut().find(|e| {
                                        value::split_first_eq(e).map(|(x, _)| x) == Some(k)
                                    }) {
                                        Some(slot) => *slot = entry,
                                        None => items.push(entry),
                                    }
                                }
                                None => items.push(entry),
                            }
                        } else if !items.contains(&entry) {
                            items.push(entry);
                        }
                    }
                    self.origin.insert(canonical.to_owned(), path.clone());
                    continue;
                }

                match Value::parse(&value, spec.kind) {
                    Ok(v) => {
                        if spec.kind == KeyKind::Enum && !spec.accepts_literal(&entry) {
                            self.warnings.push(Warning {
                                key: canonical.to_owned(),
                                file: Some(path.clone()),
                                line: None,
                                message: format!(
                                    "`{entry}` is not one of the documented values for `{canonical}`: {}",
                                    spec.enum_values.join(", ")
                                ),
                            });
                        }
                        self.effective.insert(canonical.to_owned(), v);
                        self.origin.insert(canonical.to_owned(), path.clone());
                    }
                    Err(e) => {
                        self.warnings.push(Warning {
                            key: canonical.to_owned(),
                            file: Some(path.clone()),
                            line: None,
                            message: format!("{canonical}: {e} — using the default"),
                        });
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- reading

    /// The effective value of `key`, or the key's documented default.
    pub fn get(&self, key: &str) -> Option<Value> {
        let spec = spec::lookup(key)?;
        self.effective
            .get(spec.key)
            .cloned()
            .or_else(|| spec.default.value())
    }

    /// The raw, unresolved value: what the file actually says, or `None`.
    pub fn get_value(&self, key: &str) -> Option<Value> {
        let spec = spec::lookup(key)?;
        self.get(spec.key)
    }

    /// The file the effective value came from, for `config get --show-origin`.
    pub fn key_origin(&self, key: &str) -> Option<&Path> {
        let spec = spec::lookup(key)?;
        self.origin.get(spec.key).map(PathBuf::as_path)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.as_int())
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(|v| v.as_float())
    }

    pub fn get_str(&self, key: &str) -> Option<String> {
        self.get(key).and_then(|v| v.as_str().map(str::to_owned))
    }

    pub fn get_color(&self, key: &str) -> Option<Color> {
        self.get(key).and_then(|v| v.as_color())
    }

    /// A list-valued key, whether it is spelled inline or repeated.
    pub fn get_list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(Value::List(l)) => l,
            Some(Value::Str(s)) if !s.is_empty() => vec![s],
            _ => Vec::new(),
        }
    }

    /// A `Pairs`-shaped key as `(name, value)` entries.
    pub fn get_pairs(&self, key: &str) -> Vec<(String, String)> {
        self.get_list(key)
            .iter()
            .filter_map(|e| {
                value::split_first_eq(e)
                    .or_else(|| value::split_first_pipe(e))
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
            })
            .collect()
    }

    /// The parsed `tab-rule` list, highest priority first.
    pub fn tab_rules(&self) -> Vec<TabRule> {
        self.get_list("tab-rule").iter().filter_map(|s| TabRule::parse(s).ok()).collect()
    }

    /// The parsed `details-view` list, in tab order.
    pub fn details_views(&self) -> Vec<DetailsView> {
        self.get_list("details-view")
            .iter()
            .filter_map(|s| DetailsView::parse(s).ok())
            .collect()
    }

    /// The pinned entries for the `+` menu, the tray menu and the Jump List.
    pub fn quick_access(&self) -> Vec<QuickAccess> {
        self.get_list("quick-access")
            .iter()
            .filter_map(|s| QuickAccess::parse(s).ok())
            .collect()
    }

    /// The keymap: the built-in defaults with the user's `keybind` lines
    /// applied on top, in order.
    pub fn keymap(&self) -> keybind::Keymap {
        keybind::Keymap::defaults().with_overrides(&self.get_list("keybind"))
    }

    // ----------------------------------------------------------- theme colours

    /// The active theme slug, honouring `auto-theme-dark-mode` and the OS
    /// appearance.
    pub fn active_theme_slug(&self, dark_mode: bool) -> String {
        let (light, dark) = theme::default_theme_names(Platform::host());
        let light = self.get_str("theme").unwrap_or_else(|| light.to_owned());
        if dark_mode && self.get_bool("auto-theme-dark-mode").unwrap_or(true) {
            self.get_str("theme-dark").unwrap_or_else(|| dark.to_owned())
        } else {
            light
        }
    }

    /// Load the active theme. Falls back to the platform default when the named
    /// theme does not exist.
    pub fn active_theme(&self, dark_mode: bool) -> Theme {
        let dir = Self::themes_dir();
        let slug = theme::slugify(&self.active_theme_slug(dark_mode));
        Theme::load(&slug, Some(&dir)).unwrap_or_else(|e| {
            log::warn!("theme `{slug}` could not be loaded ({e}); using the platform default");
            let (light, dark) = theme::default_theme_names(Platform::host());
            let fallback = if dark_mode { dark } else { light };
            Theme::load(fallback, Some(&dir))
                .expect("a built-in default theme must always parse")
        })
    }

    /// Fold the theme, the config's colour overrides and the bold/faint rules
    /// into the colour set the renderer draws with.
    pub fn resolved_colors(&self, dark_mode: bool) -> ResolvedColors {
        let theme = self.active_theme(dark_mode);
        let mut palette = Palette(theme.terminal.palette);

        for i in 0..16u8 {
            if let Some(c) = self.get_color(&format!("palette-{i}")) {
                palette.set(i, c);
            }
        }
        for entry in self.get_list("palette") {
            if let Some((idx, color)) = entry.split_once('=') {
                if let (Ok(i), Ok(c)) = (idx.trim().parse::<u8>(), Color::parse(color.trim())) {
                    palette.set(i, c);
                }
            }
        }

        let background = self
            .get_color("background")
            .or(theme.terminal.background)
            .unwrap_or(palette::DEFAULT_BACKGROUND);
        let foreground = self
            .get_color("foreground")
            .unwrap_or(theme.terminal.foreground);

        let bold = match self.get_str("bold-color").as_deref() {
            Some("bright") => BoldColor::Bright,
            Some("none") | None => BoldColor::Color,
            Some(other) => Color::parse(other).map(BoldColor::Fixed).unwrap_or(BoldColor::Color),
        };

        ResolvedColors {
            foreground,
            background,
            palette,
            cursor: self
                .get_color("cursor-color")
                .or(theme.terminal.cursor)
                .unwrap_or(foreground),
            cursor_text: self
                .get_color("cursor-text")
                .or(theme.terminal.cursor_text)
                .unwrap_or(background),
            selection_foreground: self
                .get_color("selection-foreground")
                .or(theme.selection.foreground),
            selection_background: self
                .get_color("selection-background")
                .or(theme.selection.background),
            bold,
            faint_opacity: self.get_float("faint-opacity").unwrap_or(0.5) as f32,
            minimum_contrast: self.get_float("minimum-contrast").unwrap_or(1.0) as f32,
        }
    }

    // ---------------------------------------------------------------- writing

    /// Set `key`, writing the value into the file it should live in.
    ///
    /// Returns the file that was actually written.
    pub fn set_value(&mut self, key: &str, raw: &str, scope: SaveScope) -> Result<ConfigFile, ConfigError> {
        let spec = spec::lookup(key).ok_or_else(|| ConfigError::UnknownKey {
            key: key.to_owned(),
            suggestions: suggest(key),
        })?;
        if scope != SaveScope::Global
            && matches!(spec.kind, KeyKind::Enum | KeyKind::Str)
            && spec.kind == KeyKind::Enum
            && !spec.accepts_literal(raw)
        {
            return Err(ConfigError::BadValue {
                key: spec.key.to_owned(),
                value: raw.to_owned(),
                expected: spec.enum_values.join(", "),
            });
        }
        let value = Value::parse(raw, spec.kind).map_err(|e| ConfigError::BadValue {
            key: spec.key.to_owned(),
            value: raw.to_owned(),
            expected: e.to_string(),
        })?;

        if spec.list_style == Some(ListStyle::Repeated) {
            return self.set_list_entry(spec.key, raw, scope);
        }

        let target = self.target_file(spec.key, scope);
        let rendered = value.render();
        // Resolve the path before taking the mutable borrow on the document.
        let path = default_path_for(&self.dir, target);
        let doc = self.doc_mut(target);
        // A legacy spelling already in the file is rewritten under its real
        // name, so resetting it later actually clears it.
        for alias in spec.aliases {
            doc.rename(alias, spec.key);
        }
        doc.set_raw(spec.key, &rendered);
        doc.write_atomic(&path)?;
        self.recompute();
        Ok(target)
    }

    /// Append (or, for pairs, replace) one entry of a repeated key.
    pub fn set_list_entry(
        &mut self,
        key: &str,
        raw: &str,
        scope: SaveScope,
    ) -> Result<ConfigFile, ConfigError> {
        let spec = spec::lookup(key).ok_or_else(|| ConfigError::UnknownKey {
            key: key.to_owned(),
            suggestions: suggest(key),
        })?;
        let target = self.target_file(spec.key, scope);
        let mut items = {
            let doc = self.doc_ref(target);
            doc.get_all_raw(spec.key).iter().map(|s| value::unquote(s)).collect::<Vec<_>>()
        };

        if spec.kind == KeyKind::Pairs {
            let entry = value::unquote(raw);
            match value::split_first_eq(&entry) {
                Some((name, _)) => {
                    match items
                        .iter_mut()
                        .find(|e| value::split_first_eq(e).map(|(x, _)| x) == Some(name))
                    {
                        Some(slot) => *slot = entry,
                        None => items.push(entry),
                    }
                }
                None => items.push(entry),
            }
        } else {
            let entry = value::unquote(raw);
            if !items.contains(&entry) {
                items.push(entry);
            }
        }

        let path = default_path_for(&self.dir, target);
        let doc = self.doc_mut(target);
        doc.set_all_raw(spec.key, &items);
        doc.write_atomic(&path)?;
        self.recompute();
        Ok(target)
    }

    /// Remove `key` everywhere — both from `config.toml` and from
    /// `config.local.toml`, as the CLI reference states.
    pub fn unset(&mut self, key: &str) -> Result<usize, ConfigError> {
        let spec = spec::lookup(key).ok_or_else(|| ConfigError::UnknownKey {
            key: key.to_owned(),
            suggestions: suggest(key),
        })?;
        let mut removed = self.main.remove(spec.key);
        if let Some(local) = &mut self.local {
            removed += local.remove(spec.key);
            local.write_atomic(&Self::local_config_path())?;
        }
        self.main.write_atomic(&Self::config_path())?;
        self.recompute();
        Ok(removed)
    }

    /// Edit a list value with the signed-entry syntax:
    ///
    /// * `+ENTRY` adds an entry, or replaces the one with the same name;
    /// * `-NAME` removes the entry whose name — or whose whole text — matches.
    ///
    /// An edit that mixes signed and unsigned entries is rejected rather than
    /// guessed at.
    pub fn edit_list_value(
        &mut self,
        key: &str,
        entries: &[String],
        scope: SaveScope,
    ) -> Result<Vec<String>, ConfigError> {
        let spec = spec::lookup(key).ok_or_else(|| ConfigError::UnknownKey {
            key: key.to_owned(),
            suggestions: suggest(key),
        })?;
        if spec.list_style != Some(ListStyle::Repeated) {
            return Err(ConfigError::NotAList { key: spec.key.to_owned() });
        }
        let signed: Vec<bool> = entries
            .iter()
            .map(|e| e.starts_with('+') || e.starts_with('-'))
            .collect();
        if signed.iter().any(|s| *s) && signed.iter().any(|s| !*s) {
            return Err(ConfigError::MixedListEdit { key: spec.key.to_owned() });
        }

        let mut items: Vec<String> = self.get_list(spec.key);
        if !signed.iter().any(|s| *s) {
            items = entries.to_vec();
        } else {
            for e in entries {
                let (add, body) = match e.split_at(1) {
                    ("+", rest) => (true, rest),
                    ("-", rest) => (false, rest),
                    _ => (true, e.as_str()),
                };
                let name = value::split_first_eq(body).map(|(k, _)| k.to_owned());
                if add {
                    match name.and_then(|n| {
                        items
                            .iter_mut()
                            .find(|e| value::split_first_eq(e).map(|(k, _)| k) == Some(n.as_str()))
                    }) {
                        Some(slot) => *slot = body.to_owned(),
                        None => items.push(body.to_owned()),
                    }
                } else {
                    items.retain(|e| e != body && value::split_first_eq(e).map(|(k, _)| k) != Some(body));
                }
            }
        }

        let target = self.target_file(spec.key, scope);
        let path = default_path_for(&self.dir, target);
        let doc = self.doc_mut(target);
        doc.set_all_raw(spec.key, &items);
        doc.write_atomic(&path)?;
        self.recompute();
        Ok(items)
    }

    fn target_file(&self, key: &str, scope: SaveScope) -> ConfigFile {
        match scope {
            SaveScope::Local => ConfigFile::Local,
            SaveScope::Global => ConfigFile::Main,
            SaveScope::Auto => match self.origin.get(key) {
                Some(p) if *p == Self::local_config_path() => ConfigFile::Local,
                // A key that lives in an include keeps living there.
                Some(p) if self.includes.iter().any(|d| d.path.as_deref() == Some(p.as_path())) => {
                    ConfigFile::Include
                }
                _ => ConfigFile::Main,
            },
        }
    }

    fn doc_ref(&self, file: ConfigFile) -> &Document {
        match file {
            ConfigFile::Local => self.local.as_ref().unwrap_or(&self.main),
            // A write that has to land in an include goes to the last include
            // read, which is the one that currently wins for that key.
            ConfigFile::Include => self.includes.last().unwrap_or(&self.main),
            ConfigFile::Main => &self.main,
        }
    }

    fn doc_mut(&mut self, file: ConfigFile) -> &mut Document {
        match file {
            ConfigFile::Local => self
                .local
                .get_or_insert_with(|| Document {
                    path: Some(Self::local_config_path()),
                    ..Document::default()
                }),
            ConfigFile::Include => self.includes.last_mut().unwrap_or(&mut self.main),
            ConfigFile::Main => &mut self.main,
        }
    }

    /// Render the normalised config — what `otty config show` prints.
    pub fn serialize(&self) -> String {
        let mut out = String::from("# Normalized Otty configuration\n");
        for spec in spec::all() {
            if let Some(v) = self.effective.get(spec.key) {
                out.push_str(&format!("{:<46} = {}\n", spec.key, v.render()));
            }
        }
        out
    }

    /// The effective config as JSON, keyed by canonical name.
    pub fn to_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.effective {
            let json = match v {
                Value::Bool(b) => serde_json::Value::Bool(*b),
                Value::Int(i) => serde_json::Value::Number((*i).into()),
                Value::Float(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                Value::Str(s) => serde_json::Value::String(s.clone()),
                Value::Color(c) => serde_json::Value::String(c.to_hex()),
                Value::List(l) => serde_json::Value::Array(
                    l.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
                ),
            };
            map.insert(k.clone(), json);
        }
        serde_json::Value::Object(map)
    }

    /// Turn a `key=value` fragment from the CLI into a resolved pair.
    pub fn parse_key_value(input: &str) -> Result<(String, String), ConfigError> {
        let (k, v) = value::split_first_eq(input).ok_or_else(|| ConfigError::NotAnAssignment {
            input: input.to_owned(),
        })?;
        let canonical = spec::canonical(k).unwrap_or(k).to_owned();
        Ok((canonical, v.to_owned()))
    }

    /// Re-read everything from disk, as `otty config reload` asks the running
    /// app to do.
    pub fn reload(&mut self) {
        let dir = self.dir.clone();
        *self = Self::load_from(&dir);
    }
}

fn unquote_path(s: &str) -> String {
    value::unquote(s)
}

/// Where a layer's file lives, when the document did not carry its own path.
fn default_path_for(dir: &Path, kind: ConfigFile) -> PathBuf {
    match kind {
        ConfigFile::Local => dir.join("config.local.toml"),
        _ => dir.join("config.toml"),
    }
}

/// Close matches for a mistyped key, for the error message.
pub fn suggest(key: &str) -> Vec<&'static str> {
    let mut scored: Vec<(usize, &'static str)> = spec::all()
        .iter()
        .map(|k| (levenshtein(key, k.key), k.key))
        .filter(|(d, _)| *d <= 4)
        .collect();
    scored.sort();
    scored.into_iter().take(3).map(|(_, k)| k).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Things that can go wrong when writing config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("unknown config key `{key}`{}", suggestion_hint(suggestions))]
    UnknownKey { key: String, suggestions: Vec<&'static str> },
    #[error("`{key}` cannot be set to `{value}`: {expected}")]
    BadValue { key: String, value: String, expected: String },
    #[error("`{key}` is not a list")]
    NotAList { key: String },
    #[error("an edit of `{key}` cannot mix `+`/`-` entries with plain ones")]
    MixedListEdit { key: String },
    #[error("`{input}` is not a `key=value` assignment")]
    NotAnAssignment { input: String },
    #[error("could not write the config file: {0}")]
    Io(#[from] std::io::Error),
}

fn suggestion_hint(s: &[&'static str]) -> String {
    if s.is_empty() {
        String::new()
    } else {
        format!(" — did you mean {}?", s.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "# my config\ntheme = Dracula\nfont-size = 14\nenv = EDITOR=nvim\nenv = VISUAL=vim\nshell = /bin/zsh\n",
        )
        .unwrap();
        let cfg = Config::load_from(dir.path());
        (dir, cfg)
    }

    #[test]
    fn loads_values_and_falls_back_to_documented_defaults() {
        let (_d, cfg) = fixture();
        assert_eq!(cfg.get_str("theme").as_deref(), Some("Dracula"));
        assert_eq!(cfg.get_float("font-size"), Some(14.0));
        // Not in the file, so the documented default applies.
        assert_eq!(cfg.get_float("font-size"), Some(14.0));
        assert_eq!(cfg.get_str("cursor-style").as_deref(), Some("block"));
        assert_eq!(cfg.get_int("scrollback-lines"), Some(10000));
        assert_eq!(cfg.get_bool("text-underline"), Some(true));
    }

    #[test]
    fn repeatable_pair_keys_accumulate_in_order() {
        let (_d, cfg) = fixture();
        assert_eq!(cfg.get_list("env"), ["EDITOR=nvim", "VISUAL=vim"]);
        assert_eq!(
            cfg.get_pairs("env"),
            [("EDITOR".to_owned(), "nvim".to_owned()), ("VISUAL".to_owned(), "vim".to_owned())]
        );
    }

    #[test]
    fn local_overrides_the_main_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "font-size = 13\n").unwrap();
        std::fs::write(dir.path().join("config.local.toml"), "font-size = 18\n").unwrap();
        let cfg = Config::load_from(dir.path());
        assert_eq!(cfg.get_float("font-size"), Some(18.0));
        assert!(cfg.key_origin("font-size").unwrap().ends_with("config.local.toml"));
    }

    #[test]
    fn includes_are_read_before_the_main_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("base.toml"),
            "font-size = 11\ntheme = Nord\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "include = base.toml\ntheme = Dracula\n",
        )
        .unwrap();
        let cfg = Config::load_from(dir.path());
        // The include supplies what the main file does not...
        assert_eq!(cfg.get_float("font-size"), Some(11.0));
        // ...and the main file wins where both speak.
        assert_eq!(cfg.get_str("theme").as_deref(), Some("Dracula"));
    }

    #[test]
    fn a_missing_include_is_a_warning_not_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "include = nope.toml\ntheme = Nord\n").unwrap();
        let cfg = Config::load_from(dir.path());
        assert_eq!(cfg.get_str("theme").as_deref(), Some("Nord"));
        assert!(cfg.warnings().iter().any(|w| w.key == "include"));
    }

    #[test]
    fn unknown_keys_are_ignored_silently() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "from-a-newer-otty = whatever\n").unwrap();
        let cfg = Config::load_from(dir.path());
        assert!(cfg.warnings().iter().all(|w| !w.key.contains("from-a-newer")));
    }

    #[test]
    fn a_bad_value_warns_and_the_default_stands() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "font-size = wide\n").unwrap();
        let cfg = Config::load_from(dir.path());
        assert_eq!(cfg.get_float("font-size"), Some(13.0));
        assert!(cfg.warnings().iter().any(|w| w.key == "font-size"));
    }

    #[test]
    fn an_unknown_enum_literal_warns_but_keeps_the_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "cursor-style = triangle\n").unwrap();
        let cfg = Config::load_from(dir.path());
        assert_eq!(cfg.get_str("cursor-style").as_deref(), Some("triangle"));
        assert!(cfg.warnings().iter().any(|w| w.key == "cursor-style"));
    }

    #[test]
    fn setting_a_key_edits_the_file_in_place_and_keeps_comments() {
        let (dir, mut cfg) = fixture();
        cfg.set_value("font-size", "16", SaveScope::Auto).unwrap();
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.starts_with("# my config"));
        assert!(text.contains("font-size = 16"));
        assert!(text.contains("theme = Dracula"));
        assert_eq!(cfg.get_float("font-size"), Some(16.0));
    }

    #[test]
    fn settings_one_key_diff_keeps_the_header_comment() {
        let dir = tempfile::tempdir().unwrap();
        let before = "# Otty config — do not rewrite me\nfont-size = 13\ntheme = Dracula\n";
        std::fs::write(dir.path().join("config.toml"), before).unwrap();
        let mut cfg = Config::load_from(dir.path());
        cfg.set_value("font-size", "16", SaveScope::Auto).unwrap();
        let after = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(after.starts_with("# Otty config — do not rewrite me\n"));
        let changed: Vec<_> = before
            .lines()
            .zip(after.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(changed, vec![("font-size = 13", "font-size = 16.0")]);
        assert_eq!(after.lines().count(), before.lines().count());
    }

    #[test]
    fn bundled_settings_ui_html_is_the_shipped_page() {
        let html = Config::settings_ui_html(&catalog::bundled_resources());
        let meta = std::fs::metadata(&html).unwrap();
        assert!(meta.len() > 1_000_000, "settings-ui.html {} bytes", meta.len());
        let text = std::fs::read_to_string(&html).unwrap();
        assert!(text.contains("<!DOCTYPE html>"));
        assert!(text.contains("font-size"));
        assert!(text.contains("key_bindings") || text.contains("keybindings"));
    }

    #[test]
    fn a_new_key_is_appended_at_the_end() {
        let (dir, mut cfg) = fixture();
        cfg.set_value("bidi", "true", SaveScope::Auto).unwrap();
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.trim_end().ends_with("bidi = true"));
    }

    #[test]
    fn a_legacy_alias_is_rewritten_under_its_real_name() {
        let (dir, mut cfg) = fixture();
        cfg.set_value("command", "/opt/homebrew/bin/fish", SaveScope::Auto).unwrap();
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.contains("command = /opt/homebrew/bin/fish"));
        assert!(!text.contains("\nshell = "));
    }

    #[test]
    fn local_scope_writes_the_machine_only_file() {
        let (dir, mut cfg) = fixture();
        let file = cfg.set_value("command", "/opt/homebrew/bin/fish", SaveScope::Local).unwrap();
        assert_eq!(file, ConfigFile::Local);
        let local = std::fs::read_to_string(dir.path().join("config.local.toml")).unwrap();
        assert!(local.contains("command = /opt/homebrew/bin/fish"));
        // The main file keeps its own value.
        assert_eq!(cfg.get_str("command").as_deref(), Some("/opt/homebrew/bin/fish"));
    }

    #[test]
    fn unset_clears_the_key_from_both_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "font-size = 13\n").unwrap();
        std::fs::write(dir.path().join("config.local.toml"), "font-size = 20\n").unwrap();
        let mut cfg = Config::load_from(dir.path());
        assert_eq!(cfg.unset("font-size").unwrap(), 2);
        assert_eq!(cfg.get_float("font-size"), Some(13.0)); // back to the default
    }

    #[test]
    fn signed_entries_change_one_list_item_at_a_time() {
        let (_d, mut cfg) = fixture();
        let items = cfg
            .edit_list_value("env", &["+EDITOR=nvim".into()], SaveScope::Auto)
            .unwrap();
        assert_eq!(items, ["EDITOR=nvim", "VISUAL=vim"]);

        let items = cfg.edit_list_value("env", &["-VISUAL".into()], SaveScope::Auto).unwrap();
        assert_eq!(items, ["EDITOR=nvim"]);
    }

    #[test]
    fn mixing_signed_and_plain_entries_is_refused() {
        let (_d, mut cfg) = fixture();
        let e = cfg
            .edit_list_value("env", &["+A=1".into(), "B=2".into()], SaveScope::Auto)
            .unwrap_err();
        assert!(matches!(e, ConfigError::MixedListEdit { .. }), "{e}");
    }

    #[test]
    fn setting_an_unknown_key_suggests_close_names() {
        let (_d, mut cfg) = fixture();
        let e = cfg.set_value("font-siz", "14", SaveScope::Auto).unwrap_err();
        match e {
            ConfigError::UnknownKey { suggestions, .. } => {
                assert!(suggestions.contains(&"font-size"), "{suggestions:?}");
            }
            other => panic!("{other}"),
        }
    }

    #[test]
    fn an_out_of_range_enum_is_refused_before_anything_is_written() {
        let (_d, mut cfg) = fixture();
        let e = cfg.set_value("cursor-style", "triangle", SaveScope::Auto).unwrap_err();
        assert!(matches!(e, ConfigError::BadValue { .. }), "{e}");
    }

    #[test]
    fn the_resolved_palette_follows_theme_then_config_then_runtime() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "theme = dracula\npalette-1 = \"#010203\"\npalette = 2=#040506\n",
        )
        .unwrap();
        let cfg = Config::load_from(dir.path());
        // Light mode, so `theme` applies rather than `theme-dark` (the default
        // dark theme is Nord, which would supply slot 4).
        let c = cfg.resolved_colors(false);
        assert_eq!(c.palette.get(1).to_hex(), "#010203");
        assert_eq!(c.palette.get(2).to_hex(), "#040506");
        // Untouched slots still come from dracula.
        assert_eq!(c.palette.get(4).to_hex(), "#bd93f9");
    }

    #[test]
    fn theme_selection_follows_auto_theme_dark_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "theme = paper\ntheme-dark = dracula\n").unwrap();
        let cfg = Config::load_from(dir.path());
        assert_eq!(cfg.active_theme_slug(false), "paper");
        assert_eq!(cfg.active_theme_slug(true), "dracula");

        std::fs::write(
            dir.path().join("config.toml"),
            "theme = paper\ntheme-dark = dracula\nauto-theme-dark-mode = false\n",
        )
        .unwrap();
        let cfg = Config::load_from(dir.path());
        assert_eq!(cfg.active_theme_slug(true), "paper");
    }

    #[test]
    fn json_and_normalized_output_round_trip_the_effective_values() {
        let (_d, cfg) = fixture();
        let j = cfg.to_json();
        assert_eq!(j["theme"], serde_json::json!("Dracula"));
        assert_eq!(j["font-size"], serde_json::json!(14.0));
        assert!(cfg.serialize().contains("theme"));
    }

    #[test]
    fn key_value_fragments_resolve_aliases() {
        let (k, v) = Config::parse_key_value("shell=/bin/bash").unwrap();
        assert_eq!(k, "command");
        assert_eq!(v, "/bin/bash");
        assert!(Config::parse_key_value("nonsense").is_err());
    }

    #[test]
    fn reload_picks_up_external_edits() {
        let (dir, mut cfg) = fixture();
        std::fs::write(dir.path().join("config.toml"), "theme = Nord\n").unwrap();
        cfg.reload();
        assert_eq!(cfg.get_str("theme").as_deref(), Some("Nord"));
    }

    #[test]
    fn levenshtein_distance_is_sane() {
        assert_eq!(levenshtein("font-size", "font-size"), 0);
        assert_eq!(levenshtein("font-size", "font-siz"), 1);
        assert_eq!(levenshtein("", "abc"), 3);
    }
}
