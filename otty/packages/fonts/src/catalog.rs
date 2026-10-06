//! Font discovery: which files are on the machine, where each came from, and the
//! lookups everything else in this crate is built on.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{SkipReason, SkippedFile};
use crate::faces::FaceStyle;
use crate::stack::{FamilyRef, GenericFamily};

/// The extensions discovery loads, case-insensitively. Anything else in the
/// drop-in directory is ignored and reported.
pub const ACCEPTED_EXTENSIONS: [&str; 4] = ["ttf", "otf", "ttc", "otc"];

/// How deep the drop-in scan descends, so a symlink loop or an accidentally
/// copied tree cannot hang the launch path.
const MAX_SCAN_DEPTH: usize = 4;

/// The extension of `path`, lowercased, or `None` when there is none.
pub fn extension_of(path: &Path) -> Option<String> {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// Whether discovery loads this path: its extension, case-insensitively, is one
/// of [`ACCEPTED_EXTENSIONS`].
///
/// This is the extension rule only — the scan checks the file type separately,
/// because a *directory* named `fonts.ttf` is not a font either.
pub fn is_accepted_font_file(path: &Path) -> bool {
    extension_of(path).is_some_and(|e| ACCEPTED_EXTENSIONS.contains(&e.as_str()))
}

/// Where a face came from, which is what `otty font list --system/--user`
/// filters on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontSource {
    /// Discovered through the platform's own font directories.
    System,
    /// A drop-in file under `~/.config/otty/fonts/`.
    User,
    /// A face the product ships, from [`bundled_dir`].
    ///
    /// Ranked *below* the other two: the chain has to be able to say
    /// `JetBrains Mono` and get the shipped copy on a machine that never
    /// installed it — that is what "bundled default" in `docs/TYPOGRAPHY.md`
    /// means — but a copy the user installed themselves is theirs and wins.
    Bundled,
}

impl FontSource {
    /// The CLI spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Bundled => "bundled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "system" => Self::System,
            "user" => Self::User,
            "bundled" => Self::Bundled,
            _ => return None,
        })
    }
}

/// The directory the product's own faces live in.
///
/// Two candidates, in order:
///
/// 1. `fonts/` beside the executable, which is how a packaged build is laid
///    out — the only one that works on a machine that has never seen the
///    source tree.
/// 2. `resources/fonts/` in the checkout, recorded at build time, which is what
///    a `cargo run` from the repository uses.
///
/// Neither existing is not an error. A machine whose fonts are all installed
/// needs no bundle, and the resolver reports what it could not find instead of
/// failing the launch.
pub fn bundled_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|parent| parent.join("fonts")))
        .filter(|dir| dir.is_dir())
    {
        return dir;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/fonts")
}

/// One face, as discovery found it.
#[derive(Debug, Clone, PartialEq)]
pub struct FontEntry {
    /// The `fontdb` id, which is how every other API here names the face.
    pub id: fontdb::ID,
    /// Every localized family name; the first is the English US one, which is
    /// what a theme or the config names.
    pub families: Vec<String>,
    pub post_script_name: String,
    pub style: FaceStyle,
    /// The `usWeightClass` the face declares.
    pub weight: u16,
    /// The `usWidthClass` the face declares.
    pub stretch: fontdb::Stretch,
    /// Whether the face declares itself monospaced — what `--monospace` filters.
    pub monospaced: bool,
    pub source: FontSource,
    /// The file the face came from, when it came from one.
    pub path: Option<PathBuf>,
}

impl FontEntry {
    /// The primary family name — the English US name when the face has one.
    pub fn family(&self) -> &str {
        self.families.first().map(String::as_str).unwrap_or("")
    }

    /// Whether the face declares itself monospaced, which is what
    /// `otty font list --monospace` filters on.
    pub const fn is_monospace(&self) -> bool {
        self.monospaced
    }

    /// Whether any of the face's family names equals `name`, ignoring case.
    pub fn matches_family(&self, name: &str) -> bool {
        self.families.iter().any(|f| f.eq_ignore_ascii_case(name.trim()))
    }

    /// Whether the face's family names or PostScript name contain `needle`,
    /// ignoring case — the `otty font list --family` filter.
    pub fn matches_needle(&self, needle: &str) -> bool {
        let needle = needle.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return true;
        }
        self.families.iter().any(|f| f.to_ascii_lowercase().contains(&needle))
            || self.post_script_name.to_ascii_lowercase().contains(&needle)
    }
}

/// What `otty font list` narrows the catalog down to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontFilter {
    /// Sources to keep. Empty means every source, which is also what passing
    /// both `--system` and `--user` selects.
    pub sources: Vec<FontSource>,
    /// Case-insensitive substring of a family name or a PostScript name.
    pub family: Option<String>,
    /// `Some(true)` keeps monospaced faces only (`--monospace`), `Some(false)`
    /// keeps the rest, `None` keeps everything.
    pub monospace: Option<bool>,
}

impl FontFilter {
    /// Everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source to keep. Adding the same one twice changes nothing.
    pub fn with_source(mut self, source: FontSource) -> Self {
        if !self.sources.contains(&source) {
            self.sources.push(source);
        }
        self
    }

    /// Keep faces whose family or PostScript name contains `needle`.
    pub fn with_family(mut self, needle: impl Into<String>) -> Self {
        self.family = Some(needle.into());
        self
    }

    /// Keep monospaced faces only.
    pub const fn monospace_only(mut self) -> Self {
        self.monospace = Some(true);
        self
    }

    /// Keep non-monospaced faces only.
    pub const fn proportional_only(mut self) -> Self {
        self.monospace = Some(false);
        self
    }

    /// Whether one face passes.
    pub fn matches(&self, entry: &FontEntry) -> bool {
        if !self.sources.is_empty() && !self.sources.contains(&entry.source) {
            return false;
        }
        if let Some(wanted) = self.monospace {
            if entry.is_monospace() != wanted {
                return false;
            }
        }
        match &self.family {
            Some(needle) => entry.matches_needle(needle),
            None => true,
        }
    }
}

/// What to scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiscoveryOptions {
    /// Scan the platform's font directories (`--system`).
    pub system: bool,
    /// Scan this drop-in directory (`--user`). The product path is
    /// [`otty_config::Config::fonts_dir`].
    pub user: Option<PathBuf>,
    /// Scan the faces the product ships, from [`bundled_dir`]. Scanned **last**
    /// so an installed copy of the same family answers first.
    pub bundled: Option<PathBuf>,
}

impl DiscoveryOptions {
    /// System fonts only.
    pub fn system_only() -> Self {
        Self { system: true, user: None, bundled: None }
    }

    /// One drop-in directory only.
    pub fn user_only(dir: impl Into<PathBuf>) -> Self {
        Self { system: false, user: Some(dir.into()), bundled: None }
    }

    /// System fonts, the product's drop-in directory
    /// (`~/.config/otty/fonts/`) and the shipped faces.
    pub fn all() -> Self {
        Self {
            system: true,
            user: Some(otty_config::Config::fonts_dir()),
            bundled: Some(bundled_dir()),
        }
    }

    /// Point the user half at `dir`.
    pub fn with_user_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.user = Some(dir.into());
        self
    }

    /// Drop the user half.
    pub fn without_user(mut self) -> Self {
        self.user = None;
        self
    }

    /// Point the shipped half at `dir`, which is the one a test uses to hold the
    /// bundle still while the machine's own fonts change underneath it.
    pub fn with_bundled_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.bundled = Some(dir.into());
        self
    }

    /// Drop the shipped half, for the callers that must report on the machine
    /// alone.
    pub fn without_bundled(mut self) -> Self {
        self.bundled = None;
        self
    }
}

/// The faces this machine has, and everything that was passed over.
///
/// Built by [`FontCatalog::discover`]. The catalog is cheap to clone (the whole
/// database is) and every lookup is a linear scan of a few thousand entries,
/// which is what makes it usable from the Settings panel on every keystroke.
#[derive(Debug, Clone)]
pub struct FontCatalog {
    db: fontdb::Database,
    entries: Vec<FontEntry>,
    skipped: Vec<SkippedFile>,
    user_dir: Option<PathBuf>,
}

impl FontCatalog {
    /// An empty catalog — the state a machine with no readable fonts is in.
    pub fn empty() -> Self {
        Self { db: fontdb::Database::new(), entries: Vec::new(), skipped: Vec::new(), user_dir: None }
    }

    /// Build a catalog by scanning what `options` asks for.
    ///
    /// Nothing here fails: a malformed face, an unreadable file and an
    /// unsupported extension are all recorded in [`FontCatalog::skipped`].
    pub fn discover(options: &DiscoveryOptions) -> Self {
        let mut db = fontdb::Database::new();
        if options.system {
            db.load_system_fonts();
        }

        let mut skipped = Vec::new();
        // The drop-in directory first, then the shipped faces: `fontdb` answers
        // with the first face it loaded for a family, so this order is what
        // makes an installed or dropped-in copy win over the bundle.
        for (dir, label) in [
            (options.user.as_ref(), "drop-in fonts"),
            (options.bundled.as_ref(), "bundled fonts"),
        ] {
            let Some(dir) = dir else { continue };
            let mut accepted = Vec::new();
            scan_dir(dir, 0, &mut accepted, &mut skipped);
            accepted.sort();
            skipped.sort_by(|a, b| a.path.cmp(&b.path));
            let offered = accepted.len();
            let mut loaded = 0usize;
            for path in accepted {
                match db.load_font_file(&path) {
                    Ok(()) => loaded += 1,
                    Err(e) => skipped.push(SkippedFile {
                        path,
                        reason: SkipReason::Malformed(e.to_string()),
                    }),
                }
            }
            log::debug!(
                "{label}: {loaded} of {offered} file(s) under {} loaded, {} skipped",
                dir.display(),
                skipped.len()
            );
        }

        let user_dir = options.user.clone();
        let bundled_dir = options.bundled.clone();
        let entries: Vec<FontEntry> = db
            .faces()
            .map(|face| {
                let path = source_path(&face.source);
                let source = match (&path, &user_dir, &bundled_dir) {
                    (Some(p), Some(dir), _) if p.starts_with(dir) => FontSource::User,
                    (Some(p), _, Some(dir)) if p.starts_with(dir) => FontSource::Bundled,
                    _ => FontSource::System,
                };
                FontEntry {
                    id: face.id,
                    families: face.families.iter().map(|(name, _)| name.clone()).collect(),
                    post_script_name: face.post_script_name.clone(),
                    style: FaceStyle::from_face(face.weight.0, is_italic(face.style)),
                    weight: face.weight.0,
                    stretch: face.stretch,
                    monospaced: face.monospaced,
                    source,
                    path,
                }
            })
            .collect();

        Self { db, entries, skipped, user_dir }
    }

    /// System fonts plus the product's drop-in directory.
    pub fn discover_default() -> Self {
        Self::discover(&DiscoveryOptions::all())
    }

    /// The underlying database, for the callers that need it directly (the
    /// rasteriser asks it for face data).
    pub fn database(&self) -> &fontdb::Database {
        &self.db
    }

    /// Every face, in discovery order.
    pub fn entries(&self) -> &[FontEntry] {
        &self.entries
    }

    /// How many faces were loaded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing was loaded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every file discovery saw and did not load.
    pub fn skipped(&self) -> &[SkippedFile] {
        &self.skipped
    }

    /// The drop-in directory this catalog scanned, when it scanned one.
    pub fn user_dir(&self) -> Option<&Path> {
        self.user_dir.as_deref()
    }

    /// One face by id.
    pub fn entry(&self, id: fontdb::ID) -> Option<&FontEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The faces `filter` keeps, sorted by family then weight then style, which
    /// is the order `otty font list` prints.
    pub fn list(&self, filter: &FontFilter) -> Vec<&FontEntry> {
        let mut kept: Vec<&FontEntry> = self.entries.iter().filter(|e| filter.matches(e)).collect();
        kept.sort_by(|a, b| {
            a.family()
                .to_ascii_lowercase()
                .cmp(&b.family().to_ascii_lowercase())
                .then(a.weight.cmp(&b.weight))
                .then(a.style.as_str().cmp(b.style.as_str()))
                .then(a.post_script_name.cmp(&b.post_script_name))
        });
        kept
    }

    /// Every distinct family name, sorted — what the Settings panel's family
    /// picker is built from.
    pub fn family_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .entries
            .iter()
            .flat_map(|e| e.families.iter().cloned())
            .filter(|n| !n.is_empty())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Whether any face claims `family`, ignoring case.
    pub fn has_family(&self, family: &str) -> bool {
        self.entries.iter().any(|e| e.matches_family(family))
    }

    /// The face of `family` with exactly this style, if the family ships one.
    ///
    /// A family that does not ship the style answers `None` — which is the
    /// distinction the four text-style modes are built on, so it must never be
    /// papered over by a nearest-match fallback here.
    pub fn find_face(&self, family: &str, style: FaceStyle) -> Option<fontdb::ID> {
        self.entries
            .iter()
            .find(|e| e.style == style && e.matches_family(family))
            .map(|e| e.id)
    }

    /// [`FontCatalog::find_face`] for a chain entry, so a CSS generic resolves
    /// through `fontdb`'s own cascade.
    pub fn family_face(&self, family: &FamilyRef, style: FaceStyle) -> Option<fontdb::ID> {
        match family {
            FamilyRef::Named(name) => self.find_face(name, style),
            FamilyRef::Generic(generic) => self.find_generic(*generic, style),
        }
    }

    /// The face a CSS generic names, for a given style.
    ///
    /// `fontdb`'s generic names are the platform defaults it hard-codes
    /// (`Courier New`, `Arial`, `Times New Roman`). When the machine has none of
    /// them the catalog falls back to its *own* faces — a monospaced one for
    /// `monospace`, any face otherwise — because a terminal that cannot find
    /// `Courier New` still has to put glyphs on screen. The style is checked
    /// before the answer is accepted: a generic whose family ships no bold face
    /// has no bold face, which is what `primary-only` and `auto` decide on.
    pub fn find_generic(&self, generic: GenericFamily, style: FaceStyle) -> Option<fontdb::ID> {
        let family = generic.as_fontdb();
        let query = fontdb::Query {
            families: std::slice::from_ref(&family),
            weight: style.weight(),
            stretch: fontdb::Stretch::Normal,
            style: style.db_style(),
        };
        if let Some(id) = self.db.query(&query) {
            if self.entry(id).is_some_and(|e| e.style == style) {
                return Some(id);
            }
        }

        if generic == GenericFamily::Monospace {
            if let Some(entry) = self.entries.iter().find(|e| e.monospaced && e.style == style) {
                return Some(entry.id);
            }
            return self.entries.iter().find(|e| e.monospaced).map(|e| e.id);
        }
        self.entries
            .iter()
            .find(|e| e.style == style)
            .or_else(|| self.entries.first())
            .map(|e| e.id)
    }
}

/// Whether a `fontdb` face style is an italic one. `fontdb` distinguishes a
/// designed italic from an oblique; a terminal treats both as the italic face.
const fn is_italic(style: fontdb::Style) -> bool {
    matches!(style, fontdb::Style::Italic | fontdb::Style::Oblique)
}

/// The file a face came from, when it came from one.
fn source_path(source: &fontdb::Source) -> Option<PathBuf> {
    match source {
        fontdb::Source::File(path) => Some(path.clone()),
        // `Source::Binary` and (with the `memmap` feature) `Source::SharedFile`
        // are both matched by the wildcard: only a file-backed face can be
        // classified, and those two are not one.
        _ => None,
    }
}

/// Walk `dir`, collecting accepted font files and recording everything else.
///
/// A directory that does not exist is not an error — the drop-in folder is
/// absent until the user first opens it — so only a *readable-but-failing*
/// directory is recorded.
fn scan_dir(dir: &Path, depth: usize, accepted: &mut Vec<PathBuf>, skipped: &mut Vec<SkippedFile>) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            skipped.push(SkippedFile {
                path: dir.to_path_buf(),
                reason: SkipReason::Unreadable(e.to_string()),
            });
            return;
        }
    };
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            skipped.push(SkippedFile { path, reason: SkipReason::NotAFile });
        } else if kind.is_dir() {
            scan_dir(&path, depth + 1, accepted, skipped);
        } else if is_accepted_font_file(&path) {
            accepted.push(path);
        } else {
            let extension = extension_of(&path).unwrap_or_default();
            skipped.push(SkippedFile { path, reason: SkipReason::Extension(extension) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{user_catalog, OFFICE_CODE_PRO, JETBRAINS_MONO, SYMBOLS_NERD_FONT};

    #[test]
    fn accepted_extensions_are_case_insensitive_and_exact() {
        for name in ["a.ttf", "a.TTF", "b.otf", "c.OTC", "d.ttc"] {
            assert!(is_accepted_font_file(Path::new(name)), "{name}");
        }
        for name in ["a.woff2", "a.woff", "a.ttf.bak", "a.pfb", "ttf", "a."] {
            assert!(!is_accepted_font_file(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn a_drop_in_directory_loads_only_fonts_and_reports_the_rest() {
        let (catalog, dir) = user_catalog();
        assert_eq!(catalog.user_dir(), Some(dir.as_path()));
        assert_eq!(catalog.len(), 7, "six bundled faces at the top level, one nested");
        assert!(catalog.entries().iter().all(|e| e.source == FontSource::User));
        assert!(catalog.entries().iter().all(|e| e.path.is_some()));
        assert!(catalog.has_family(JETBRAINS_MONO), "{:?}", catalog.family_names());
        assert!(catalog.has_family(OFFICE_CODE_PRO), "{:?}", catalog.family_names());

        let skipped: Vec<String> = catalog.skipped().iter().map(SkippedFile::line).collect();
        assert_eq!(skipped.len(), 2, "{skipped:?}");
        assert!(skipped.iter().any(|l| l.contains("notes.txt") && l.contains(".txt")), "{skipped:?}");
        assert!(skipped.iter().any(|l| l.contains("README.md") && l.contains(".md")), "{skipped:?}");
    }

    #[test]
    fn a_system_only_scan_tags_every_entry_as_system() {
        let catalog = FontCatalog::discover(&DiscoveryOptions::system_only());
        assert!(!catalog.is_empty(), "this machine has system fonts");
        assert!(catalog.entries().iter().all(|e| e.source == FontSource::System));
        assert!(catalog.entries().iter().all(|e| !e.families.is_empty()));
        assert!(catalog.skipped().is_empty(), "a system scan records no skips");
        // The picker's list is the sorted, deduped set of family names.
        let names = catalog.family_names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn a_missing_drop_in_directory_is_not_an_error() {
        let options = DiscoveryOptions::user_only(PathBuf::from("/definitely/not/here/otty-fonts"));
        let catalog = FontCatalog::discover(&options);
        assert!(catalog.is_empty());
        assert!(catalog.skipped().is_empty(), "{:?}", catalog.skipped());
    }

    /// The faces this build ships are in the catalog, attributed to the bundle.
    ///
    /// Without this the grid role walks past `JetBrains Mono`, past `Menlo`, and
    /// lands on whatever the platform calls `monospace` — measured on Windows:
    /// Courier New, a hairline face that reads as broken text at 13 px, while the
    /// JetBrains Mono the product ships sat unopened in `resources/fonts`.
    #[test]
    fn the_shipped_faces_are_in_the_catalog() {
        let catalog = FontCatalog::discover(&DiscoveryOptions::default().with_bundled_dir(bundled_dir()));
        assert_eq!(catalog.len(), 7, "the bundle holds seven faces: {:?}", catalog.family_names());
        assert!(catalog.entries().iter().all(|e| e.source == FontSource::Bundled));
        assert!(catalog.has_family(JETBRAINS_MONO), "{:?}", catalog.family_names());
        assert!(catalog.has_family(SYMBOLS_NERD_FONT), "{:?}", catalog.family_names());
        assert!(catalog.skipped().is_empty(), "{:?}", catalog.skipped());
    }

    /// The bundle is a default, not an override: what the machine installed is
    /// what gets used, and which one answered is recorded rather than inferred.
    #[test]
    fn an_installed_copy_wins_over_the_shipped_one() {
        let (_installed, dir) = user_catalog();
        let options = DiscoveryOptions::user_only(&dir).with_bundled_dir(bundled_dir());
        let catalog = FontCatalog::discover(&options);
        assert_eq!(catalog.len(), 14, "both copies are loaded: {:?}", catalog.family_names());

        let id = catalog
            .find_face(JETBRAINS_MONO, FaceStyle::Regular)
            .expect("JetBrains Mono is in both directories");
        let entry = catalog.entry(id).expect("the face has an entry");
        assert_eq!(entry.source, FontSource::User, "the installed copy must win");
        assert!(
            entry.path.as_ref().is_some_and(|p| p.starts_with(&dir)),
            "the face came from {:?}, not from {dir:?}",
            entry.path
        );
    }

    /// The reason for shipping a grid font at all: the terminal role resolves to
    /// it on a machine that never installed it, instead of falling through to the
    /// generic. `is_exact` is the assertion that matters — the resolver reports a
    /// substitution for every chain entry it walked past, and this is the case
    /// where there is nothing to report.
    #[test]
    fn the_shipped_grid_font_answers_the_terminal_role() {
        let catalog = FontCatalog::discover(&DiscoveryOptions::default().with_bundled_dir(bundled_dir()));
        let resolved = catalog.resolve_role(&crate::stack::TypeStack::product_default(), crate::stack::FontRole::TerminalGrid);
        assert_eq!(resolved.report.answered_by.as_deref(), Some(JETBRAINS_MONO), "{:?}", resolved.report);
        assert!(!resolved.report.substituted, "{:?}", resolved.report.lines);
        assert!(resolved.report.is_exact(), "{:?}", resolved.report);
        let entry = catalog.entry(resolved.face().expect("a face answered")).expect("the entry");
        assert_eq!(entry.source, FontSource::Bundled);
    }

    #[test]
    fn the_filter_narrows_by_source_family_and_monospace() {
        let (catalog, _dir) = user_catalog();

        let all = catalog.list(&FontFilter::new());
        assert_eq!(all.len(), catalog.len());

        let user = catalog.list(&FontFilter::new().with_source(FontSource::User));
        assert_eq!(user.len(), catalog.len());
        let system = catalog.list(&FontFilter::new().with_source(FontSource::System));
        assert!(system.is_empty(), "the drop-in catalog has no system faces");

        let family = catalog.list(&FontFilter::new().with_family("code"));
        assert!(!family.is_empty());
        assert!(family
            .iter()
            .all(|e| e.families.iter().any(|f| f.to_ascii_lowercase().contains("code"))));

        // A family filter that matches nothing keeps nothing.
        assert!(catalog.list(&FontFilter::new().with_family("styrène")).is_empty());
        // The needle is case-insensitive, and an empty one keeps everything.
        assert_eq!(catalog.list(&FontFilter::new().with_family("CODE")).len(), family.len());
        assert_eq!(catalog.list(&FontFilter::new().with_family("  ")).len(), catalog.len());

        // Adding a source twice is the same set as adding it once.
        let twice = FontFilter::new()
            .with_source(FontSource::User)
            .with_source(FontSource::User);
        assert_eq!(twice.sources.len(), 1);
        assert_eq!(catalog.list(&twice).len(), user.len());
    }

    #[test]
    fn the_monospace_filter_reads_the_flag_discovery_recorded() {
        let (catalog, _dir) = user_catalog();
        let mono = catalog.list(&FontFilter::new().monospace_only());
        let proportional = catalog.list(&FontFilter::new().proportional_only());
        assert!(mono.iter().all(|e| e.monospaced));
        assert!(proportional.iter().all(|e| !e.monospaced));
        // The two halves partition the catalog.
        assert_eq!(mono.len() + proportional.len(), catalog.len());
        assert!(!mono.is_empty(), "the bundled faces are coding fonts");

        // The filter decides on the entry's own flag, whatever it is.
        let mut proportional = catalog.entries()[0].clone();
        proportional.monospaced = false;
        assert!(!FontFilter::new().monospace_only().matches(&proportional));
        assert!(FontFilter::new().proportional_only().matches(&proportional));
        assert!(FontFilter::new().matches(&proportional));
        assert!(!FontFilter::new()
            .with_source(FontSource::System)
            .matches(&proportional));
    }

    #[test]
    fn style_lookups_are_exact_or_nothing() {
        let (catalog, _dir) = user_catalog();
        for style in FaceStyle::ALL {
            assert!(
                catalog.find_face(OFFICE_CODE_PRO, style).is_some(),
                "Office Code Pro ships a {style} face"
            );
        }
        // JetBrains Mono ships regular + italic here, and no bold at all — which
        // is what makes the four text-style modes distinguishable.
        assert!(catalog.find_face(JETBRAINS_MONO, FaceStyle::Regular).is_some());
        assert!(catalog.find_face(JETBRAINS_MONO, FaceStyle::Italic).is_some());
        assert!(catalog.find_face(JETBRAINS_MONO, FaceStyle::Bold).is_none());
        assert!(catalog.find_face("nothing like this", FaceStyle::Regular).is_none());
        assert!(catalog.find_face("jetbrains mono", FaceStyle::Regular).is_some());
    }

    #[test]
    fn a_generic_resolves_to_a_real_face_even_without_the_platform_default() {
        let (catalog, _dir) = user_catalog();
        let id = catalog
            .find_generic(GenericFamily::Monospace, FaceStyle::Regular)
            .expect("a monospace fallback");
        let entry = catalog.entry(id).expect("from this catalog");
        assert!(entry.monospaced, "{entry:?}");

        // The temp directory holds no `Courier New`, so this path is the
        // last-resort cascade over the catalog's own faces.
        assert!(!catalog.has_family("Courier New"));
        assert_eq!(catalog.find_generic(GenericFamily::Monospace, FaceStyle::Regular), Some(id));
        assert!(FontCatalog::empty()
            .find_generic(GenericFamily::Monospace, FaceStyle::Regular)
            .is_none());
        assert!(FontCatalog::empty()
            .find_generic(GenericFamily::SansSerif, FaceStyle::Regular)
            .is_none());
    }

    #[test]
    fn an_empty_catalog_answers_nothing_but_does_not_fail() {
        let catalog = FontCatalog::empty();
        assert!(catalog.is_empty());
        assert_eq!(catalog.len(), 0);
        assert!(catalog.family_names().is_empty());
        assert!(!catalog.has_family("JetBrains Mono"));
        assert!(catalog.list(&FontFilter::new()).is_empty());
        assert!(catalog.entry(fontdb::ID::dummy()).is_none());
    }
}
