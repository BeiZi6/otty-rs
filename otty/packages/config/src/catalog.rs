//! Localization catalogs copied from the shipped app (`en.json` / `zh.json`).
//!
//! Settings, menus, the palette and empty-state copy all live in these two
//! files. Action ids are the keys of `settings.key_bindings.actions`; labels
//! follow the selected language. Do not invent action ids.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The `language` config key: `system` / `english` / `chinese`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLanguage {
    System,
    English,
    Chinese,
}

impl UiLanguage {
    pub fn from_config_value(value: &str) -> Self {
        match value {
            "english" => Self::English,
            "chinese" => Self::Chinese,
            _ => Self::System,
        }
    }

    /// `system` follows the host locale (`LANG` / `LC_ALL`, else English).
    pub fn resolve(self) -> ResolvedLanguage {
        match self {
            Self::English => ResolvedLanguage::English,
            Self::Chinese => ResolvedLanguage::Chinese,
            Self::System => system_language(),
        }
    }
}

/// A concrete catalog after resolving `system`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedLanguage {
    English,
    Chinese,
}

impl ResolvedLanguage {
    pub fn file_name(self) -> &'static str {
        match self {
            Self::English => "en.json",
            Self::Chinese => "zh.json",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Chinese => "zh",
        }
    }
}

fn system_language() -> ResolvedLanguage {
    let hint = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if hint.starts_with("zh") {
        ResolvedLanguage::Chinese
    } else {
        ResolvedLanguage::English
    }
}

/// One loaded `en.json` / `zh.json` tree.
#[derive(Debug, Clone)]
pub struct Catalog {
    lang: ResolvedLanguage,
    root: Value,
    /// English tree when `lang` is Chinese, so missing keys fall back.
    fallback: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl Catalog {
    pub fn load_file(path: &Path) -> Result<Self, CatalogError> {
        let bytes = std::fs::read(path).map_err(|source| CatalogError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let root: Value = serde_json::from_slice(&bytes).map_err(|source| CatalogError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let lang = if name.starts_with("zh") {
            ResolvedLanguage::Chinese
        } else {
            ResolvedLanguage::English
        };
        Ok(Self {
            lang,
            root,
            fallback: None,
        })
    }

    pub fn load_for(lang: UiLanguage, resources: &Path) -> Result<Self, CatalogError> {
        let resolved = lang.resolve();
        let mut cat = Self::load_file(&resources.join(resolved.file_name()))?;
        if resolved == ResolvedLanguage::Chinese {
            if let Ok(en) = Self::load_file(&resources.join(ResolvedLanguage::English.file_name()))
            {
                cat.fallback = Some(en.root);
            }
        }
        Ok(cat)
    }

    pub fn language(&self) -> ResolvedLanguage {
        self.lang
    }

    /// Dotted path into the JSON object, e.g. `settings.key_bindings.actions.copy`.
    /// Missing keys on the Chinese catalog fall back to English.
    pub fn get(&self, path: &str) -> Option<&str> {
        lookup(&self.root, path).or_else(|| self.fallback.as_ref().and_then(|f| lookup(f, path)))
    }

    pub fn action_label(&self, id: &str) -> Option<&str> {
        self.get(&format!("settings.key_bindings.actions.{id}"))
    }

    pub fn menu_title(&self, section: &str) -> Option<&str> {
        self.get(&format!("menu.{section}"))
    }

    pub fn settings_title(&self) -> Option<&str> {
        self.get("settings.title")
    }

    /// `(id, label)` pairs from `settings.key_bindings.actions`, sorted by id.
    pub fn actions(&self) -> Vec<(String, String)> {
        let Some(map) = self
            .root
            .pointer("/settings/key_bindings/actions")
            .and_then(|v| v.as_object())
        else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = map
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

fn lookup<'a>(root: &'a Value, path: &str) -> Option<&'a str> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    cur.as_str()
}

pub fn bundled_resources() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ACTIONS;

    #[test]
    fn loads_en_and_zh_and_language_key_selects_catalog() {
        let dir = bundled_resources();
        let en = Catalog::load_for(UiLanguage::English, &dir).unwrap();
        let zh = Catalog::load_for(UiLanguage::Chinese, &dir).unwrap();
        assert_eq!(en.language(), ResolvedLanguage::English);
        assert_eq!(zh.language(), ResolvedLanguage::Chinese);
        assert_eq!(en.action_label("command_palette"), Some("Command Palette"));
        assert_eq!(zh.action_label("command_palette"), Some("命令面板"));
        assert_eq!(en.actions().len(), 152);
        assert_eq!(zh.actions().len(), 152);
    }

    #[test]
    fn actions_table_matches_en_json_regeneration() {
        let en = Catalog::load_for(UiLanguage::English, &bundled_resources()).unwrap();
        let from_json = en.actions();
        assert_eq!(from_json.len(), ACTIONS.len());
        for ((id, label), (got_id, got_label)) in ACTIONS.iter().zip(from_json.iter()) {
            assert_eq!(*id, got_id.as_str(), "id drift vs en.json");
            assert_eq!(*label, got_label.as_str(), "label drift vs en.json for {id}");
        }
    }

    #[test]
    fn settings_language_key_switches_catalog() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "language = chinese\n").unwrap();
        let cfg = crate::Config::load_from(dir.path());
        assert_eq!(cfg.ui_language(), UiLanguage::Chinese);
        let cat = cfg.load_catalog(&bundled_resources()).unwrap();
        assert_eq!(cat.action_label("command_palette"), Some("命令面板"));

        std::fs::write(dir.path().join("config.toml"), "language = english\n").unwrap();
        let cfg = crate::Config::load_from(dir.path());
        assert_eq!(cfg.ui_language(), UiLanguage::English);
        let cat = cfg.load_catalog(&bundled_resources()).unwrap();
        assert_eq!(cat.action_label("command_palette"), Some("Command Palette"));
    }

    #[test]
    fn system_language_follows_lang_env() {
        let prev_lang = std::env::var_os("LANG");
        let prev_lc = std::env::var_os("LC_ALL");
        std::env::set_var("LC_ALL", "zh_CN.UTF-8");
        std::env::remove_var("LANG");
        assert_eq!(UiLanguage::System.resolve(), ResolvedLanguage::Chinese);
        std::env::set_var("LC_ALL", "en_US.UTF-8");
        assert_eq!(UiLanguage::System.resolve(), ResolvedLanguage::English);
        match prev_lc {
            Some(v) => std::env::set_var("LC_ALL", v),
            None => std::env::remove_var("LC_ALL"),
        }
        match prev_lang {
            Some(v) => std::env::set_var("LANG", v),
            None => std::env::remove_var("LANG"),
        }
    }

    #[test]
    fn chinese_menus_and_settings_are_zh_missing_keys_fall_back_to_en() {
        let zh = Catalog::load_for(UiLanguage::Chinese, &bundled_resources()).unwrap();
        assert_eq!(zh.menu_title("file"), Some("文件"));
        assert_eq!(zh.settings_title(), Some("设置"));
        assert_eq!(zh.action_label("open_settings"), Some("设置"));

        let mut thin = Catalog::load_for(UiLanguage::Chinese, &bundled_resources()).unwrap();
        thin.root = serde_json::json!({"menu": {"file": "文件"}});
        assert_eq!(thin.menu_title("file"), Some("文件"));
        assert_eq!(
            thin.action_label("command_palette"),
            Some("Command Palette"),
            "missing zh key must fall back to en"
        );
    }
}
