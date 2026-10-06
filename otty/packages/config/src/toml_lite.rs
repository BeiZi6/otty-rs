//! The lenient `key = value` document model.
//!
//! Otty edits the user's config **in place** rather than regenerating it, so the
//! reader and the writer have to agree on a line-level model that keeps
//! everything it did not understand:
//!
//! * comments, blank lines and key order survive;
//! * a key that is already present is updated where it sits;
//! * a new key is appended at the end;
//! * a reset key is *commented out* with its old value, as
//!   `# cursor-style = bar (reset to default)`;
//! * keys Otty does not recognise are left alone, so a file shared with a newer
//!   build keeps its newer keys.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// One physical line of the config file.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Blank,
    Comment(String),
    /// `key = value`, with the raw value text preserved verbatim so a value the
    /// reader does not understand still round-trips.
    Entry {
        indent: String,
        key: String,
        value: String,
        /// Trailing `# …` on the same line, without the `#`.
        trailing: Option<String>,
    },
    /// A line that is neither blank, a comment, nor an assignment. Kept verbatim
    /// so hand-edited files are never damaged.
    Unknown(String),
}

impl Line {
    fn render_into(&self, out: &mut String) {
        match self {
            Self::Blank => out.push('\n'),
            Self::Comment(c) => {
                out.push_str(c);
                out.push('\n');
            }
            Self::Unknown(u) => {
                out.push_str(u);
                out.push('\n');
            }
            Self::Entry { indent, key, value, trailing } => {
                out.push_str(indent);
                out.push_str(key);
                out.push_str(" = ");
                out.push_str(value);
                if let Some(t) = trailing {
                    out.push_str("  # ");
                    out.push_str(t);
                }
                out.push('\n');
            }
        }
    }

    fn key_of(&self) -> Option<&str> {
        match self {
            Self::Entry { key, .. } => Some(key),
            _ => None,
        }
    }
}

/// A parsed config file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub lines: Vec<Line>,
    /// Path this document was read from, when it came from disk.
    pub path: Option<PathBuf>,
    /// Whether the source text ended in a newline. Preserved so writing an
    /// untouched file back is byte-identical.
    pub trailing_newline: bool,
}

impl Document {
    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        for raw in text.split_inclusive('\n') {
            let line = raw.strip_suffix('\n').unwrap_or(raw);
            let line = line.strip_suffix('\r').unwrap_or(line);
            lines.push(parse_line(line));
        }
        let trailing_newline = text.ends_with('\n');
        // `split_inclusive` leaves a trailing empty element for a file that ends
        // without a newline; drop it so rendering does not grow the file.
        if matches!(lines.last(), Some(Line::Blank)) && !trailing_newline {
            lines.pop();
        }
        Self { lines, path: None, trailing_newline }
    }

    pub fn read(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut doc = Self::parse(&text);
        doc.path = Some(path.to_path_buf());
        Ok(doc)
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, l) in self.lines.iter().enumerate() {
            let last = i + 1 == self.lines.len();
            l.render_into(&mut out);
            if last && !self.trailing_newline && out.ends_with('\n') {
                out.pop();
            }
        }
        out
    }

    /// Every `include = <path>` directive, in file order.
    pub fn includes(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Entry { key, value, .. } if key == "include" => {
                    Some(crate::value::unquote(value))
                }
                _ => None,
            })
            .collect()
    }

    /// The raw value of `key`, from the *last* occurrence.
    pub fn get_raw(&self, key: &str) -> Option<&str> {
        self.lines
            .iter()
            .rev()
            .find_map(|l| match l {
                Line::Entry { key: k, value, .. } if k == key => Some(value.as_str()),
                _ => None,
            })
    }

    /// Every occurrence of a repeatable key, in order.
    pub fn get_all_raw(&self, key: &str) -> Vec<&str> {
        self.lines
            .iter()
            .filter_map(|l| match l {
                Line::Entry { key: k, value, .. } if k == key => Some(value.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.lines.iter().any(|l| l.key_of() == Some(key))
    }

    /// The line index of the *first* occurrence of `key`.
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.lines.iter().position(|l| l.key_of() == Some(key))
    }

    /// Set `key` to `value`, updating the first occurrence in place and
    /// appending a new line only when the key is absent.
    ///
    /// Returns `true` when the key has to be added.
    pub fn set_raw(&mut self, key: &str, value: &str) -> bool {
        let idx = self
            .lines
            .iter()
            .position(|l| matches!(l, Line::Entry { key: k, .. } if k == key));
        match idx {
            Some(i) => {
                if let Line::Entry { value: v, .. } = &mut self.lines[i] {
                    *v = value.to_owned();
                }
                false
            }
            None => {
                self.push_entry(key, value);
                true
            }
        }
    }

    /// Append `key = value` at the end, after a blank separator if the file is
    /// non-empty and does not already end in one.
    pub fn push_entry(&mut self, key: &str, value: &str) {
        if !matches!(self.lines.last(), None | Some(Line::Blank)) {
            self.lines.push(Line::Blank);
        }
        self.lines.push(Line::Entry {
            indent: String::new(),
            key: key.to_owned(),
            value: value.to_owned(),
            trailing: None,
        });
        // An edit always leaves a well-formed file, with a final newline.
        self.trailing_newline = true;
    }

    /// Remove every occurrence of `key`. Returns how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.lines.len();
        self.lines.retain(|l| l.key_of() != Some(key));
        before - self.lines.len()
    }

    /// Replace every occurrence of `key` with a commented-out record of the old
    /// value, the shape the Settings window writes when you reset a setting:
    /// `# cursor-style = bar (reset to default)`.
    pub fn comment_out(&mut self, key: &str, reason: &str) -> usize {
        let mut n = 0;
        for l in &mut self.lines {
            if let Line::Entry { key: k, value, .. } = l {
                if k == key {
                    *l = Line::Comment(format!("# {key} = {value} ({reason})"));
                    n += 1;
                }
            }
        }
        n
    }

    /// Rename a key in place, keeping its value — used for the legacy spellings
    /// (`shell` → `command`, `font-bold` → `text-bold`).
    pub fn rename(&mut self, from: &str, to: &str) -> usize {
        let mut n = 0;
        for l in &mut self.lines {
            if let Line::Entry { key, .. } = l {
                if key == from {
                    *key = to.to_owned();
                    n += 1;
                }
            }
        }
        n
    }

    /// Replace a repeatable key's whole set of entries, keeping the position of
    /// the first one and dropping the rest.
    pub fn set_all_raw(&mut self, key: &str, values: &[String]) {
        let indices: Vec<usize> = self
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.key_of() == Some(key))
            .map(|(i, _)| i)
            .collect();
        match indices.split_first() {
            None => {
                for v in values {
                    self.push_entry(key, v);
                }
            }
            Some((&first, rest)) => {
                for &i in rest.iter().rev() {
                    self.lines.remove(i);
                }
                let rebuild: Vec<Line> = values
                    .iter()
                    .map(|v| Line::Entry {
                        indent: String::new(),
                        key: key.to_owned(),
                        value: v.clone(),
                        trailing: None,
                    })
                    .collect();
                self.lines.splice(first..=first, rebuild);
            }
        }
    }

    /// Write the document back atomically, creating parent directories.
    pub fn write_atomic(&self, path: &Path) -> std::io::Result<()> {
        crate::write_atomic(path, self.render().as_bytes())
    }
}

/// Parse a single line.
fn parse_line(line: &str) -> Line {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Line::Blank;
    }
    if trimmed.starts_with('#') {
        return Line::Comment(line.to_owned());
    }
    let Some(eq) = find_assignment(line) else {
        return Line::Unknown(line.to_owned());
    };
    let key = line[..eq].trim();
    if !is_valid_key(key) {
        return Line::Unknown(line.to_owned());
    }
    let (value, trailing) = split_trailing_comment(&line[eq + 1..]);
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    Line::Entry {
        indent,
        key: key.to_owned(),
        value: value.trim().to_owned(),
        trailing: trailing.map(|t| t.trim().to_owned()),
    }
}

/// A key is a lowercase dashed identifier, exactly as the generated editor
/// syntax highlights it: `^[a-z][a-z0-9-]*$`.
fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Index of the `=` separating key from value, skipping quoted regions.
fn find_assignment(line: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => return None, // a comment before any `=`
                '=' => return Some(i),
                _ => {}
            },
        }
    }
    None
}

/// Split ` 13  # note` into (` 13 `, `note`).
///
/// A `#` only starts a comment when it is preceded by whitespace and the text
/// before it is not just an unterminated assignment — which is what keeps
/// `cursor-color = #ff0000` a colour and `font-size = 13 # px` a comment.
fn split_trailing_comment(v: &str) -> (&str, Option<&str>) {
    let mut quote: Option<char> = None;
    let bytes: Vec<(usize, char)> = v.char_indices().collect();
    for (n, (i, c)) in bytes.iter().enumerate() {
        match quote {
            Some(q) if *c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(*c),
                '#' => {
                    let prev_is_space = n > 0 && bytes[n - 1].1.is_whitespace();
                    let head = v[..*i].trim();
                    let assignment_only = head.is_empty() || head.ends_with('=');
                    if prev_is_space && !assignment_only {
                        return (v[..*i].trim_end(), Some(&v[i + 1..]));
                    }
                }
                _ => {}
            },
        }
    }
    (v, None)
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

/// Render a value into a form a human would have typed.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("otty-tmp");
    std::fs::write(&tmp, bytes)?;
    // `rename` replaces atomically on POSIX; on Windows it fails when the
    // destination exists, so fall back to a remove-then-rename.
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp, path)?;
    }
    Ok(())
}

/// Build a `# generated by …` banner plus the given entries.
pub fn render_entries(title: &str, entries: &[(String, String)]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# {title}");
    for (k, v) in entries {
        let _ = writeln!(s, "{k} = {v}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Otty config example
theme = Dracula
font-family = JetBrains Mono
font-size = 14        # points

# Semi-transparent background (minimum 0.5)
background-opacity = 0.9

cursor-color = #ff8800
unknown-future-key = whatever
";

    #[test]
    fn parses_entries_comments_and_blanks() {
        let d = Document::parse(SAMPLE);
        assert_eq!(d.get_raw("theme"), Some("Dracula"));
        assert_eq!(d.get_raw("font-size"), Some("14"));
        assert_eq!(d.get_raw("background-opacity"), Some("0.9"));
        assert_eq!(d.get_raw("cursor-color"), Some("#ff8800"));
        assert_eq!(d.get_raw("unknown-future-key"), Some("whatever"));
        assert!(d.lines.iter().any(|l| matches!(l, Line::Comment(c) if c.starts_with("# Otty"))));
    }

    #[test]
    fn trailing_comment_is_split_but_colours_are_not() {
        let d = Document::parse(SAMPLE);
        let entry = d
            .lines
            .iter()
            .find(|l| l.key_of() == Some("font-size"))
            .unwrap();
        match entry {
            Line::Entry { value, trailing, .. } => {
                assert_eq!(value, "14");
                assert_eq!(trailing.as_deref(), Some("points"));
            }
            _ => unreachable!(),
        }
        assert_eq!(d.get_raw("cursor-color"), Some("#ff8800"));
    }

    #[test]
    fn rendering_is_lossless_for_untouched_files() {
        let d = Document::parse(SAMPLE);
        // The only normalisation is the gap before a trailing comment, which is
        // rewritten to two spaces; every comment, blank line, key and value
        // survives byte for byte.
        let want = SAMPLE.replace("font-size = 14        #", "font-size = 14  #");
        assert_eq!(d.render(), want);
    }

    #[test]
    fn set_updates_in_place_and_appends_new_keys() {
        let mut d = Document::parse(SAMPLE);
        assert!(!d.set_raw("theme", "Nord"));
        assert_eq!(d.get_raw("theme"), Some("Nord"));
        // The comment above it is untouched.
        assert_eq!(d.lines[0], Line::Comment("# Otty config example".into()));

        assert!(d.set_raw("bidi", "true"));
        assert_eq!(d.get_raw("bidi"), Some("true"));
        assert!(d.render().ends_with("bidi = true\n"));
    }

    #[test]
    fn reset_comments_the_key_out_with_a_reason() {
        let mut d = Document::parse(SAMPLE);
        assert_eq!(d.comment_out("theme", "reset to default"), 1);
        assert!(d.render().contains("# theme = Dracula (reset to default)"));
        assert_eq!(d.get_raw("theme"), None);
    }

    #[test]
    fn rename_rewrites_legacy_spellings() {
        let mut d = Document::parse("shell = /opt/homebrew/bin/fish\n");
        assert_eq!(d.rename("shell", "command"), 1);
        assert_eq!(d.get_raw("command"), Some("/opt/homebrew/bin/fish"));
        assert_eq!(d.render(), "command = /opt/homebrew/bin/fish\n");
    }

    #[test]
    fn repeatable_keys_keep_order_and_position() {
        let mut d = Document::parse("a = 1\nenv = FOO=bar\nenv = BAZ=qux\nb = 2\n");
        assert_eq!(d.get_all_raw("env"), ["FOO=bar", "BAZ=qux"]);
        d.set_all_raw("env", &["ONE=1".into(), "TWO=2".into(), "THREE=3".into()]);
        assert_eq!(d.get_all_raw("env"), ["ONE=1", "TWO=2", "THREE=3"]);
        assert!(d.render().starts_with("a = 1\nenv = ONE=1\n"));
        assert!(d.render().ends_with("b = 2\n"));
    }

    #[test]
    fn includes_are_collected_in_order() {
        let d = Document::parse("include = ~/.config/otty/personal.toml\ninclude = ~/d/otty/work.toml\n");
        assert_eq!(d.includes(), ["~/.config/otty/personal.toml", "~/d/otty/work.toml"]);
    }

    #[test]
    fn unknown_lines_are_preserved() {
        let d = Document::parse("[section]\nkey = value\n");
        assert_eq!(d.render(), "[section]\nkey = value\n");
        assert_eq!(d.get_raw("key"), Some("value"));
    }

    #[test]
    fn a_file_without_a_trailing_newline_is_not_grown() {
        let d = Document::parse("theme = Nord");
        assert_eq!(d.render(), "theme = Nord");
    }

    #[test]
    fn writing_is_atomic_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        let mut d = Document::parse("");
        d.set_raw("theme", "Paper");
        d.write_atomic(&path).unwrap();
        assert_eq!(Document::read(&path).unwrap().get_raw("theme"), Some("Paper"));
    }
}
