//! The error type the font layer reports, and the record of what discovery
//! refused to load.

use std::path::PathBuf;

use crate::catalog::ACCEPTED_EXTENSIONS;
use crate::stack::FontRole;

/// A failure the font layer reports rather than working around.
///
/// Everything survivable is recorded on the [`crate::FontCatalog`] as a
/// [`SkippedFile`] instead of being raised: a drop-in file with an extension
/// Otty does not accept, an unreadable directory, a face `fontdb` will not
/// parse. `otty doctor` has to be able to *print* those, so one bad file in
/// `~/.config/otty/fonts/` never fails the whole scan. What is left here are the
/// failures that mean the caller asked for something the catalog cannot answer
/// at all.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// The face id is not in this catalog — it belongs to another database, or
    /// the catalog was built without the source that holds it.
    #[error("no face with id {id} is in this catalog")]
    UnknownFace { id: fontdb::ID },

    /// The face is present but `swash` cannot read its tables.
    #[error("the face `{family}` has no readable metrics")]
    NoMetrics { family: String },

    /// A `adjust-cell-height` value Ghostty does not accept.
    #[error("`adjust-cell-height = {value:?}` is not a value Otty accepts: {reason}")]
    BadCellHeight { value: String, reason: String },

    /// `font-size` must be a positive number of points.
    #[error("font size {size} is not a positive number of points")]
    BadFontSize { size: f32 },

    /// Nothing in the catalog answered for this role.
    #[error("no font answered for the {role} role")]
    RoleUnresolved { role: FontRole },
}

/// Why discovery did not load a file it saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// The extension is not one of `.ttf`, `.otf`, `.ttc` or `.otc`. Carries the
    /// extension as discovery read it, lowercased — empty when there is none.
    Extension(String),
    /// A symlink (not followed) or a directory the scan does not descend into.
    NotAFile,
    /// The file could not be read.
    Unreadable(String),
    /// `fontdb` could not parse it, or found no family name in it.
    Malformed(String),
}

impl SkipReason {
    /// The reason as a full sentence, for `otty doctor`.
    pub fn describe(&self) -> String {
        let accepted = ACCEPTED_EXTENSIONS
            .iter()
            .map(|e| format!(".{e}"))
            .collect::<Vec<_>>()
            .join(", ");
        match self {
            Self::Extension(ext) if ext.is_empty() => {
                format!("has no extension (only {accepted} are loaded)")
            }
            Self::Extension(ext) => format!("extension .{ext} is not loaded (only {accepted} are)"),
            Self::NotAFile => "is not a regular file (symlinks are not followed)".to_owned(),
            Self::Unreadable(e) => format!("could not be read: {e}"),
            Self::Malformed(e) => format!("is not a font Otty can read: {e}"),
        }
    }
}

/// A file discovery saw and did not load, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    pub path: PathBuf,
    pub reason: SkipReason,
}

impl SkippedFile {
    /// One line, `path: reason`, in the shape `otty doctor` prints.
    pub fn line(&self) -> String {
        format!("{}: {}", self.path.display(), self.reason.describe())
    }
}

impl std::fmt::Display for SkippedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.line())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_reasons_read_as_sentences() {
        let line = SkippedFile {
            path: PathBuf::from("/fonts/notes.txt"),
            reason: SkipReason::Extension("txt".into()),
        };
        assert_eq!(
            line.line(),
            "/fonts/notes.txt: extension .txt is not loaded (only .ttf, .otf, .ttc, .otc are)"
        );
        assert!(SkipReason::Malformed("malformed font".into())
            .describe()
            .ends_with("is not a font Otty can read: malformed font"));
    }

    #[test]
    fn errors_carry_the_value_that_failed() {
        let e = FontError::BadCellHeight { value: "3em".into(), reason: "bad point delta".into() };
        assert!(e.to_string().contains("3em"), "{e}");
        assert!(FontError::RoleUnresolved { role: FontRole::UiChrome }
            .to_string()
            .contains("ui"));
    }
}
