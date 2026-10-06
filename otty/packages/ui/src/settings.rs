//! Settings: persist a key through `otty-config` and apply it live.
//!
//! The window never writes toml itself. `set_setting` patches one line and
//! returns the file `--show-origin` will name.

use std::path::{Path, PathBuf};

use otty_config::{Config, SaveScope};
use otty_core::CellSize;

use crate::view::{cell_size_for_font, ViewError};

/// Patch `key` and return the file the effective value now lives in.
pub fn set_setting(
    cfg: &mut Config,
    key: &str,
    raw: &str,
    scope: SaveScope,
) -> Result<PathBuf, otty_config::ConfigError> {
    cfg.set_value(key, raw, scope)?;
    Ok(cfg
        .key_origin(key)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| cfg.dir().join("config.toml")))
}

/// What `otty config get KEY --show-origin` prints as the origin line.
pub fn show_origin(cfg: &Config, key: &str) -> String {
    cfg.key_origin(key)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(default)".to_owned())
}

/// Grid cell after a Settings font-size change — this is what the running
/// window uses to lay out the terminal.
pub fn live_font_cell(cfg: &Config, scale: f32) -> Result<CellSize, ViewError> {
    cell_size_for_font(cfg, scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_font_size_is_visible_and_show_origin_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "# keep me\nfont-size = 13\n").unwrap();
        let mut cfg = Config::load_from(dir.path());
        let before = live_font_cell(&cfg, 1.0).unwrap();

        let origin = set_setting(&mut cfg, "font-size", "26", SaveScope::Auto).unwrap();
        assert_eq!(origin.file_name().unwrap(), "config.toml");
        assert_eq!(cfg.get_float("font-size"), Some(26.0));
        assert!(show_origin(&cfg, "font-size").ends_with("config.toml"));
        let text = std::fs::read_to_string(&origin).unwrap();
        assert!(text.contains("# keep me"), "header comment must survive");
        assert!(text.contains("font-size = 26"));

        let after = live_font_cell(&cfg, 1.0).unwrap();
        assert!(
            after.height > before.height,
            "font-size 26 should grow the cell ({:?} -> {:?})",
            before,
            after
        );
    }

    #[test]
    fn local_scope_show_origin_names_config_local() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "font-size = 13\n").unwrap();
        let mut cfg = Config::load_from(dir.path());
        let origin = set_setting(&mut cfg, "font-size", "18", SaveScope::Local).unwrap();
        assert_eq!(origin.file_name().unwrap(), "config.local.toml");
        assert!(show_origin(&cfg, "font-size").ends_with("config.local.toml"));
    }
}
