//! Bundled syntect grammars (`resources/syntaxes.bin`) for file-pane highlighting.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use syntect::dumps::from_uncompressed_data;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();

pub fn syntaxes_bin(resources: &Path) -> PathBuf {
    resources.join("syntaxes.bin")
}

pub fn load_syntax_set(path: &Path) -> Result<SyntaxSet, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    from_uncompressed_data(&bytes).map_err(|e| format!("syntaxes.bin: {e}"))
}

pub fn bundled_syntax_set(resources: &Path) -> Result<&'static SyntaxSet, String> {
    if let Some(s) = SYNTAXES.get() {
        return Ok(s);
    }
    let set = load_syntax_set(&syntaxes_bin(resources))?;
    Ok(SYNTAXES.get_or_init(|| set))
}

/// Highlight `source` as the grammar for `path`'s extension. Returns ANSI-colored lines.
pub fn highlight_file(resources: &Path, path: &Path, source: &str) -> Result<String, String> {
    let set = bundled_syntax_set(resources)?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("txt");
    let syntax = set
        .find_syntax_by_extension(ext)
        .or_else(|| set.find_syntax_by_name("Rust"))
        .or_else(|| Some(set.find_syntax_plain_text()))
        .unwrap();
    let theme = default_theme();
    let mut h = HighlightLines::new(syntax, &theme);
    let mut out = String::new();
    for line in source.lines() {
        let ranges = h
            .highlight_line(line, set)
            .map_err(|e| format!("highlight: {e}"))?;
        out.push_str(&as_24_bit_terminal_escaped(&ranges[..], false));
        out.push('\n');
    }
    Ok(out)
}

fn default_theme() -> Theme {
    ThemeSet::load_defaults()
        .themes
        .remove("base16-ocean.dark")
        .or_else(|| ThemeSet::load_defaults().themes.into_values().next())
        .unwrap_or_else(Theme::default)
}

#[cfg(test)]
pub fn bundled_resources() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_a_rust_file() {
        let resources = bundled_resources();
        let src = "fn main() {\n    println!(\"hi\");\n}\n";
        let out = highlight_file(&resources, Path::new("main.rs"), src).unwrap();
        assert!(out.contains("main"), "highlighted output lost source text: {out:?}");
        assert!(
            out.contains('\u{1b}') || out.contains("println"),
            "expected ANSI or source in highlight"
        );
        let set = bundled_syntax_set(&resources).unwrap();
        assert!(
            set.find_syntax_by_extension("rs").is_some(),
            "syntaxes.bin should include Rust"
        );
    }
}
