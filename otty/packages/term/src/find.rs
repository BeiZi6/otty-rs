//! Find in the terminal grid, including scrollback.

use crate::grid::{Grid, Row};

/// One match: grid line and column (char index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindHit {
    pub line: usize,
    pub col: usize,
}

pub fn row_text(row: &Row) -> String {
    row.cells.iter().map(|c| c.ch).collect::<String>().trim_end().to_string()
}

/// Next match after `after` (inclusive-exclusive: search starts at col+1).
/// Wraps to the top of scrollback.
pub fn find_next(lines: &[String], query: &str, after: Option<FindHit>) -> Option<FindHit> {
    let q = query.trim();
    if q.is_empty() || lines.is_empty() {
        return None;
    }
    let q = q.to_lowercase();
    let start_line = after.map(|h| h.line).unwrap_or(0);
    let start_col = after.map(|h| h.col + 1).unwrap_or(0);
    for i in start_line..lines.len() {
        if let Some(hit) = scan(&lines[i], &q, i, if i == start_line { start_col } else { 0 }) {
            return Some(hit);
        }
    }
    for i in 0..=start_line.min(lines.len() - 1) {
        let from = 0;
        if let Some(hit) = scan(&lines[i], &q, i, from) {
            if after.is_some_and(|h| h.line == i && h.col == hit.col) {
                continue;
            }
            return Some(hit);
        }
    }
    None
}

fn scan(line: &str, q: &str, line_i: usize, from: usize) -> Option<FindHit> {
    let lower = line.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if from >= chars.len() {
        return None;
    }
    let rest: String = chars[from..].iter().collect();
    rest.find(q).map(|byte_off| {
        let col = from + rest[..byte_off].chars().count();
        FindHit { line: line_i, col }
    })
}

pub fn grid_lines(grid: &Grid) -> Vec<String> {
    (0..grid.total_lines())
        .filter_map(|i| grid.line(i).map(row_text))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Grid;

    #[test]
    fn find_next_jumps_in_scrollback_and_wraps() {
        let lines = vec!["alpha foo".into(), "bar".into(), "foo baz".into()];
        let first = find_next(&lines, "foo", None).unwrap();
        assert_eq!(first, FindHit { line: 0, col: 6 });
        let second = find_next(&lines, "foo", Some(first)).unwrap();
        assert_eq!(second, FindHit { line: 2, col: 0 });
        let wrap = find_next(&lines, "foo", Some(second)).unwrap();
        assert_eq!(wrap, first);
        assert!(find_next(&lines, "nope", None).is_none());
    }

    #[test]
    fn grid_lines_include_scrollback() {
        let g = Grid::new(8, 2, 10);
        let lines = grid_lines(&g);
        assert_eq!(lines.len(), 2);
    }
}
