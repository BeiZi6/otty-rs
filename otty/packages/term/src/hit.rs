//! Hit-testing for the context menu: paths in a line, and selected grid text.

use crate::grid::Grid;

/// The path token covering `col` in `line`, if it looks like a filesystem path.
///
/// Absolute (`/usr/...`), tilde (`~/...`), and relative (`./`, `../`) forms,
/// plus `file:line` / `file:line:col` suffixes. URLs are not paths.
pub fn path_at(line: &str, col: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let col = col.min(chars.len() - 1);
    if !is_path_char(chars[col]) {
        return None;
    }
    let mut start = col;
    while start > 0 && is_path_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = col + 1;
    while end < chars.len() && is_path_char(chars[end]) {
        end += 1;
    }
    let token: String = chars[start..end].iter().collect();
    let path = strip_line_col(&token);
    looks_like_path(path).then(|| path.to_string())
}

fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '/' | '\\' | '.' | '_' | '-' | '~' | ':' | '+')
}

fn strip_line_col(token: &str) -> &str {
    let mut s = token;
    // Trailing `:42` or `:42:5`.
    if let Some((head, tail)) = s.rsplit_once(':') {
        if tail.chars().all(|c| c.is_ascii_digit()) {
            s = head;
            if let Some((head, tail)) = s.rsplit_once(':') {
                if tail.chars().all(|c| c.is_ascii_digit()) {
                    s = head;
                }
            }
        }
    }
    s
}

fn looks_like_path(s: &str) -> bool {
    if s.is_empty() || s.starts_with("http://") || s.starts_with("https://") {
        return false;
    }
    s.starts_with('/')
        || s.starts_with("~/")
        || s.starts_with("./")
        || s.starts_with("../")
        || (s.contains('/') && s.contains('.'))
}

/// Text covered by a screen-cell selection, ordered, newline-separated.
pub fn selected_text(grid: &Grid, a: (u16, u16), b: (u16, u16)) -> String {
    let (a, b) = order(a, b);
    let (c0, r0) = a;
    let (c1, r1) = b;
    if r0 == r1 {
        return slice_row(grid, r0, c0, c1.saturating_add(1));
    }
    let mut out = slice_row(grid, r0, c0, u16::MAX);
    for r in r0 + 1..r1 {
        out.push('\n');
        out.push_str(&slice_row(grid, r, 0, u16::MAX));
    }
    out.push('\n');
    out.push_str(&slice_row(grid, r1, 0, c1.saturating_add(1)));
    out
}

fn order(a: (u16, u16), b: (u16, u16)) -> ((u16, u16), (u16, u16)) {
    // Row first, then column.
    if (a.1, a.0) <= (b.1, b.0) {
        (a, b)
    } else {
        (b, a)
    }
}

fn slice_row(grid: &Grid, row: u16, from: u16, to: u16) -> String {
    let Some(r) = grid.row(row) else { return String::new() };
    let to = (to as usize).min(r.cells.len());
    let from = (from as usize).min(to);
    r.cells[from..to]
        .iter()
        .filter(|c| !c.is_wide_spacer())
        .map(|c| c.ch)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::{TermConfig, Terminal};
    use crate::Parser;

    fn feed(bytes: &[u8]) -> Terminal {
        let mut t = Terminal::new(80, 8, TermConfig::default());
        Parser::new().advance(&mut t, bytes);
        t
    }

    #[test]
    fn path_at_finds_absolute_tilde_and_relative() {
        assert_eq!(
            path_at("see /Users/me/src/lib.rs now", 10),
            Some("/Users/me/src/lib.rs".into())
        );
        assert_eq!(
            path_at("cd ~/project/file.swift", 8),
            Some("~/project/file.swift".into())
        );
        assert_eq!(
            path_at("open ./src/lib.rs:42", 8),
            Some("./src/lib.rs".into())
        );
        assert_eq!(path_at("hello world", 3), None);
        assert_eq!(path_at("https://example.com/x", 4), None);
    }

    #[test]
    fn selected_text_copies_a_range_from_the_grid() {
        let t = feed(b"hello path\n");
        let text = selected_text(t.grid(), (0, 0), (4, 0));
        assert_eq!(text, "hello");
        let path = path_at(&t.grid().row(0).unwrap().text(), 7);
        assert_eq!(path, None); // "path" has no slash
    }
}
