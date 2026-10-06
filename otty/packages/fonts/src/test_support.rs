//! Shared test fixtures.
//!
//! Every test that needs a real face works from the fonts this repository ships
//! under `resources/fonts/` — the same seven files `docs/TYPOGRAPHY.md` records,
//! copied into a scratch drop-in directory so discovery sees them exactly as it
//! would see a user's. That keeps the suite honest: the metrics, the style
//! ladder and the shaper are exercised against real font tables, not against
//! hand-written metadata.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::catalog::{DiscoveryOptions, FontCatalog};

/// The terminal default, and the face in the bundle with `Regular` + `Italic`
/// only — which is what makes the four style modes distinguishable.
pub const JETBRAINS_MONO: &str = "JetBrains Mono";

/// The bundled serif-mono, whose four faces are all present.
pub const OFFICE_CODE_PRO: &str = "Office Code Pro";

/// The bundled icon font — the documented fallback for Private Use Area glyphs.
pub const SYMBOLS_NERD_FONT: &str = "Symbols Nerd Font Mono";

/// The bundled files the fixture copies, as `(file, nested)` pairs. One file
/// lives in a subdirectory so the scan's recursion is covered too.
const BUNDLED: [(&str, bool); 7] = [
    ("JetBrainsMono.ttf", false),
    ("JetBrainsMono-Italic.ttf", false),
    ("OfficeCodePro-Regular.ttf", false),
    ("OfficeCodePro-Italic.ttf", false),
    ("OfficeCodePro-BoldItalic.ttf", false),
    ("OfficeCodePro-Bold.ttf", true),
    ("SymbolsNerdFontMono-Regular.ttf", false),
];

/// Files discovery must ignore, and the reason it does.
const IGNORED: [&str; 2] = ["notes.txt", "README.md"];

/// `resources/fonts/`, one level up from this package.
pub fn resources_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/fonts")
}

/// `resources/themes/`, one level up from this package.
pub fn themes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/themes")
}

/// A scratch directory for the whole test run.
///
/// Cargo does not set `CARGO_TARGET_TMPDIR` for a unit-test target, so the
/// fixture owns a directory of its own under the system temp directory, names it
/// after the process so two concurrent runs cannot collide, and starts from a
/// clean slate so the expected file counts hold.
fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("otty-fonts-fixture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory under the temp directory");
    dir
}

/// A catalog built from copies of the bundled fonts, plus the files discovery
/// must skip. Returned by value and shared across threads, so the copy happens
/// once per test binary.
pub fn user_catalog() -> (FontCatalog, PathBuf) {
    static FIXTURE: OnceLock<(FontCatalog, PathBuf)> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let dir = scratch();
            for (name, nested) in BUNDLED {
                let target = if nested { dir.join("nested").join(name) } else { dir.join(name) };
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                std::fs::copy(resources_dir().join(name), &target)
                    .unwrap_or_else(|e| panic!("copying {name}: {e}"));
            }
            for name in IGNORED {
                std::fs::write(dir.join(name), b"not a font\n").unwrap();
            }
            let catalog = FontCatalog::discover(&DiscoveryOptions::user_only(&dir));
            assert!(!catalog.is_empty(), "the fixture loaded no fonts from {}", dir.display());
            (catalog, dir)
        })
        .clone()
}
