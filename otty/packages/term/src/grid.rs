//! The grid: rows of cells, scrollback, and the viewport arithmetic on top.
//!
//! Two coordinate systems meet here and must not be confused:
//!
//! * **grid lines** — 0-based, counted from the top of *scrollback*, so line
//!   numbers survive scrolling. This is what `Outline`, `Jump To`, and the
//!   scrollback pager report.
//! * **screen lines** — 0-based, counted from the top of the *viewport*, which
//!   is what every CSI cursor command uses.
//!
//! [`Grid::display_offset`] converts between them: `screen_row == grid_line -
//! (scrollback_len - display_offset)`.

use std::collections::VecDeque;

use crate::attrs::Cell;

/// A row of cells plus the reflow metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
    /// How many columns of the row's right edge are spares left by a wide
    /// character that will not fit.
    pub trailing_spacer: u8,
    /// The row continues onto the next one (a soft wrap, not a newline).
    pub wrapped: bool,
    /// The row was produced by a line feed, not a wrap.
    pub is_first_in_command: bool,
}

impl Row {
    pub fn blank(cols: u16) -> Self {
        Self {
            cells: vec![Cell::blank(); cols as usize],
            trailing_spacer: 0,
            wrapped: false,
            is_first_in_command: false,
        }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn get(&self, col: u16) -> Option<&Cell> {
        self.cells.get(col as usize)
    }

    pub fn get_mut(&mut self, col: u16) -> Option<&mut Cell> {
        self.cells.get_mut(col as usize)
    }

    /// Resize horizontally, padding with blanks on the right.
    pub fn resize(&mut self, cols: u16) {
        self.cells.resize(cols as usize, Cell::blank());
    }

    /// The cells that carry content, trimmed of trailing blanks — what
    /// `otty pane capture` returns.
    pub fn trimmed(&self) -> &[Cell] {
        let end = self
            .cells
            .iter()
            .rposition(|c| !c.is_blank())
            .map_or(0, |i| i + 1);
        &self.cells[..end]
    }

    /// The row as text.
    ///
    /// A wide glyph occupies two cells — its own and a spacer — so the spacer is
    /// dropped here. Without that, captured text reads `日 本 語` instead of
    /// `日本語`, and every consumer of `otty pane capture` inherits the damage.
    pub fn text(&self) -> String {
        self.trimmed()
            .iter()
            .filter(|c| !c.is_wide_spacer())
            .map(|c| c.ch)
            .collect()
    }

    /// Whether every cell is blank.
    pub fn is_blank(&self) -> bool {
        self.cells.iter().all(Cell::is_blank)
    }
}

/// The scrolling region, as `DECSTBM` sets it. Rows are screen-relative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollRegion {
    pub top: u16,
    pub bottom: u16,
}

impl ScrollRegion {
    pub const fn full(rows: u16) -> Self {
        Self { top: 0, bottom: rows.saturating_sub(1) }
    }

    pub const fn contains(self, row: u16) -> bool {
        row >= self.top && row <= self.bottom
    }

    pub const fn len(self) -> u16 {
        self.bottom - self.top + 1
    }
}

/// One of the two grids a terminal keeps: the primary and the alternate screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    rows: VecDeque<Row>,
    /// Rows that have scrolled above the viewport.
    scrollback: VecDeque<Row>,
    scrollback_limit: usize,
    pub cols: u16,
    pub screen_lines: u16,
    /// How far the viewport is scrolled back from the live output.
    pub display_offset: usize,
    pub scroll_region: ScrollRegion,
}

impl Grid {
    /// A grid of `cols` × `screen_lines` with `scrollback_limit` rows of
    /// history. `scrollback-lines` is the config key; the alternate screen is
    /// created with a limit of 0.
    pub fn new(cols: u16, screen_lines: u16, scrollback_limit: usize) -> Self {
        Self {
            rows: (0..screen_lines).map(|_| Row::blank(cols)).collect(),
            scrollback: VecDeque::new(),
            scrollback_limit,
            cols,
            screen_lines,
            display_offset: 0,
            scroll_region: ScrollRegion::full(screen_lines),
        }
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// The grid line index of screen row 0.
    pub fn history_offset(&self) -> usize {
        self.scrollback.len()
    }

    /// Total lines addressable, scrollback included.
    pub fn total_lines(&self) -> usize {
        self.scrollback.len() + self.rows.len()
    }

    /// The row at a *screen* index.
    pub fn row(&self, screen_row: u16) -> Option<&Row> {
        self.rows.get(screen_row as usize)
    }

    pub fn row_mut(&mut self, screen_row: u16) -> Option<&mut Row> {
        self.rows.get_mut(screen_row as usize)
    }

    /// The row at a *grid* index, scrollback included.
    pub fn line(&self, grid_line: usize) -> Option<&Row> {
        if grid_line < self.scrollback.len() {
            self.scrollback.get(grid_line)
        } else {
            self.rows.get(grid_line - self.scrollback.len())
        }
    }

    /// Screen rows, top to bottom.
    pub fn rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter()
    }

    /// Scrollback plus screen, oldest first — what `history` returns over
    /// AppleScript and the CLI.
    pub fn all_lines(&self) -> impl Iterator<Item = &Row> {
        self.scrollback.iter().chain(self.rows.iter())
    }

    /// Push a blank row in at the bottom, moving the top row of the region out.
    ///
    /// A scroll of the *whole* screen leaves the outgoing row in scrollback; a
    /// scroll inside a `DECSTBM` region discards it, because that is what keeps
    /// a status line from polluting history.
    pub fn scroll_up(&mut self, count: u16, region: ScrollRegion) {
        for _ in 0..count {
            let outgoing = self.rows.remove(region.top as usize);
            if region.top == 0 && region.bottom == self.screen_lines - 1 {
                if let Some(row) = outgoing {
                    self.push_scrollback(row);
                }
            }
            self.rows.insert(region.bottom as usize, Row::blank(self.cols));
        }
    }

    /// Push a blank row in at the top of the region, dropping the bottom one.
    pub fn scroll_down(&mut self, count: u16, region: ScrollRegion) {
        for _ in 0..count {
            self.rows.remove(region.bottom as usize);
            self.rows.insert(region.top as usize, Row::blank(self.cols));
        }
    }

    fn push_scrollback(&mut self, row: Row) {
        if self.scrollback_limit == 0 {
            return;
        }
        // A soft-wrapped line is one logical line: pushing its last row keeps
        // the pair together, which is what reflow and copy expect.
        self.scrollback.push_back(row);
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    /// Drop everything above the viewport — `Clear Scrollback Above`.
    pub fn clear_scrollback(&mut self) {
        self.scrollback.clear();
        self.display_offset = 0;
    }

    /// Move every visible row into scrollback, oldest first, leaving the
    /// viewport blank — what `ED 22` (the kitty scroll-then-erase) asks for, so
    /// the user can still scroll back to what was on screen.
    pub fn push_viewport_to_scrollback(&mut self) {
        let rows: Vec<Row> = self.rows.drain(..).collect();
        for row in rows {
            self.push_scrollback(row);
        }
        self.rows = (0..self.screen_lines).map(|_| Row::blank(self.cols)).collect();
        self.display_offset = self.display_offset.min(self.scrollback.len());
    }

    /// Wipe the viewport without touching scrollback — `ED 2`.
    pub fn clear_screen(&mut self) {
        for r in &mut self.rows {
            *r = Row::blank(self.cols);
        }
    }

    /// Resize, reflowing the content the way the docs describe: text above the
    /// viewport is preserved, and a narrower grid rewraps the soft-wrapped
    /// lines instead of truncating them.
    pub fn resize(&mut self, cols: u16, screen_lines: u16) {
        if cols == self.cols && screen_lines == self.screen_lines {
            return;
        }
        let old_cols = self.cols;
        self.cols = cols;
        self.screen_lines = screen_lines;

        if cols == old_cols {
            // Same width: only the number of visible rows changes.
            while self.rows.len() < screen_lines as usize {
                // Pull a scrollback row back in if there is one, else pad.
                if let Some(r) = self.scrollback.pop_back() {
                    self.rows.push_front(r);
                } else {
                    self.rows.push_back(Row::blank(cols));
                }
            }
            while self.rows.len() > screen_lines as usize {
                let mut r = self.rows.pop_front().unwrap_or_else(|| Row::blank(cols));
                r.resize(cols);
                self.push_scrollback(r);
            }
            self.scroll_region = ScrollRegion::full(screen_lines);
            self.display_offset = self.display_offset.min(self.scrollback.len());
            return;
        }

        // Width changed: rebuild the whole buffer by unwrapping to logical
        // lines, then rewrapping at the new width.
        let mut logical: Vec<Vec<Cell>> = Vec::new();
        let mut current: Vec<Cell> = Vec::new();
        let all: Vec<Row> = self.scrollback.iter().cloned().chain(self.rows.iter().cloned()).collect();
        for row in all {
            let mut cells = row.cells.clone();
            cells.truncate(old_cols as usize);
            current.extend(cells);
            if !row.wrapped {
                logical.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            logical.push(current);
        }

        let mut rebuilt: Vec<Row> = Vec::new();
        let mut carry: Vec<Cell> = Vec::new();
        for line in logical {
            let mut plain: Vec<Cell> = line;
            // Trim the blank padding the old width introduced.
            while plain.last().is_some_and(Cell::is_blank) {
                plain.pop();
            }
            if plain.is_empty() {
                rebuilt.push(Row::blank(cols));
                continue;
            }
            if !carry.is_empty() {
                let mut merged = std::mem::take(&mut carry);
                merged.extend(plain);
                plain = merged;
            }
            for chunk in plain.chunks(cols as usize) {
                let mut r = Row::blank(cols);
                let n = chunk.len();
                for (i, c) in chunk.iter().enumerate() {
                    r.cells[i] = *c;
                }
                r.wrapped = n == cols as usize;
                rebuilt.push(r);
            }
        }

        // Keep the last `screen_lines` rows visible, the rest in scrollback.
        let split = rebuilt.len().saturating_sub(screen_lines as usize);
        let screen: Vec<Row> = rebuilt.split_off(split);
        let mut screen = screen;
        while screen.len() < screen_lines as usize {
            screen.push(Row::blank(cols));
        }
        self.rows = screen.into();
        self.scrollback = rebuilt.into();
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
        self.scroll_region = ScrollRegion::full(screen_lines);
        self.display_offset = self.display_offset.min(self.scrollback.len());
    }

    /// Scroll the viewport by `delta` lines; a positive `delta` scrolls back
    /// into history, which is the direction a scroll-wheel-up or a
    /// `PageUp`-style binding uses.
    ///
    /// Returns whether the viewport moved, clamped at the two ends (live output
    /// and the oldest retained line).
    pub fn scroll_display(&mut self, delta: i32) -> bool {
        let max = self.scrollback.len();
        let old = self.display_offset;
        let next = (self.display_offset as i64 + delta as i64).clamp(0, max as i64) as usize;
        self.display_offset = next;
        self.display_offset != old
    }

    pub fn scroll_to_bottom(&mut self) {
        self.display_offset = 0;
    }

    /// The screen row that the live output's *last* row occupies.
    pub fn cursor_screen_row(&self) -> u16 {
        self.screen_lines.saturating_sub(1)
    }

    /// Convert a viewport row to a grid line, honouring the current scroll.
    pub fn screen_to_line(&self, screen_row: u16) -> usize {
        self.scrollback.len() + screen_row as usize - self.display_offset
    }

    /// Convert a grid line to a viewport row, if it is on screen.
    pub fn line_to_screen(&self, grid_line: usize) -> Option<u16> {
        let screen = grid_line as i64 - (self.scrollback.len() as i64 - self.display_offset as i64);
        if (0..self.screen_lines as i64).contains(&screen) {
            Some(screen as u16)
        } else {
            None
        }
    }

    /// Drop rows that duplicate what is directly above them.
    ///
    /// Backs `dedup-repaint-scrollback`: some full-screen TUIs (Claude Code's
    /// classic renderer among them) leave stale copies in scrollback when they
    /// repaint a region taller than the window. Legitimate repeats — a status
    /// line after every command — are left alone, so only an exact duplicate of
    /// the immediately preceding row is dropped, and only once.
    pub fn dedup_scrollback(&mut self) -> usize {
        let mut removed = 0;
        let mut out: VecDeque<Row> = VecDeque::with_capacity(self.scrollback.len());
        // "Only once": after a duplicate is dropped, the next identical row is
        // kept, so a real repeat (the same status line after each command)
        // survives while a stale repaint copy does not.
        let mut dropped_this_row = false;
        for row in self.scrollback.drain(..) {
            let duplicate = out.back().is_some_and(|prev| *prev == row) && !row.is_blank();
            if duplicate && !dropped_this_row {
                removed += 1;
                dropped_this_row = true;
                continue;
            }
            dropped_this_row = false;
            out.push_back(row);
        }
        self.scrollback = out;
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_grid_is_blank_and_has_the_requested_shape() {
        let g = Grid::new(80, 24, 1000);
        assert_eq!(g.rows().count(), 24);
        assert!(g.rows().all(Row::is_blank));
        assert_eq!(g.scrollback_len(), 0);
        assert_eq!(g.scroll_region, ScrollRegion { top: 0, bottom: 23 });
    }

    #[test]
    fn scrolling_pushes_the_top_row_into_scrollback() {
        let mut g = Grid::new(10, 3, 10);
        g.row_mut(0).unwrap().cells[0].ch = 'a';
        g.scroll_up(1, g.scroll_region);
        assert_eq!(g.scrollback_len(), 1);
        assert_eq!(g.line(0).unwrap().cells[0].ch, 'a');
        assert!(g.row(0).unwrap().is_blank());
    }

    #[test]
    fn a_scroll_region_that_is_not_the_whole_screen_does_not_touch_scrollback() {
        let mut g = Grid::new(10, 5, 10);
        g.scroll_region = ScrollRegion { top: 1, bottom: 3 };
        g.row_mut(1).unwrap().cells[0].ch = 'x';
        g.scroll_up(1, g.scroll_region);
        assert_eq!(g.scrollback_len(), 0);
        assert_eq!(g.row(0).unwrap().cells[0].ch, ' ');
        assert!(g.row(3).unwrap().is_blank());
    }

    #[test]
    fn scrollback_is_capped() {
        let mut g = Grid::new(4, 2, 3);
        for i in 0..10u8 {
            g.row_mut(0).unwrap().cells[0].ch = (b'a' + i) as char;
            g.scroll_up(1, g.scroll_region);
        }
        assert_eq!(g.scrollback_len(), 3);
        // Ten rows were pushed and the cap is three, so the newest survive:
        // h, i and j — the oldest were evicted first.
        assert_eq!(g.line(0).unwrap().cells[0].ch, 'h');
    }

    #[test]
    fn screen_and_grid_coordinates_line_up() {
        let mut g = Grid::new(4, 3, 10);
        for _ in 0..5 {
            g.scroll_up(1, g.scroll_region);
        }
        assert_eq!(g.history_offset(), 5);
        assert_eq!(g.screen_to_line(0), 5);
        assert_eq!(g.line_to_screen(5), Some(0));
        assert_eq!(g.line_to_screen(4), None);
        // Scrolling back two lines shifts the mapping.
        assert!(g.scroll_display(2));
        assert_eq!(g.screen_to_line(0), 3);
        assert_eq!(g.line_to_screen(3), Some(0));
    }

    #[test]
    fn display_offset_clamps_to_the_scrollback() {
        let mut g = Grid::new(4, 3, 10);
        g.scroll_up(2, g.scroll_region);
        // A positive delta scrolls back, and the offset stops at the oldest
        // retained line rather than running past it.
        assert!(g.scroll_display(5));
        assert_eq!(g.display_offset, 2);
        assert!(g.scroll_display(-10));
        assert_eq!(g.display_offset, 0);
        g.scroll_to_bottom();
        assert_eq!(g.display_offset, 0);
    }

    #[test]
    fn narrowing_the_grid_rewraps_instead_of_truncating() {
        let mut g = Grid::new(10, 3, 100);
        let text = "abcdefghijklmnopqrst";
        for (i, c) in text.chars().enumerate() {
            g.row_mut(0).unwrap().cells[i % 10].ch = c;
            if i % 10 == 9 {
                g.row_mut(0).unwrap().wrapped = true;
                g.scroll_up(1, g.scroll_region);
            }
        }
        g.resize(5, 3);
        let rebuilt: String = g.all_lines().map(Row::text).collect::<Vec<_>>().join("");
        assert!(rebuilt.contains("abcdefghij"), "reflow lost content: {rebuilt}");
    }

    #[test]
    fn widening_keeps_the_visible_rows_and_the_bottom_line() {
        let mut g = Grid::new(5, 2, 100);
        g.row_mut(1).unwrap().cells[0].ch = 'z';
        g.resize(20, 2);
        assert_eq!(g.rows().count(), 2);
        assert_eq!(g.row(1).unwrap().cells[0].ch, 'z');
    }

    #[test]
    fn growing_the_viewport_pulls_rows_back_out_of_scrollback() {
        let mut g = Grid::new(4, 2, 100);
        g.row_mut(0).unwrap().cells[0].ch = 'a';
        g.scroll_up(1, g.scroll_region);
        assert_eq!(g.scrollback_len(), 1);
        g.resize(4, 3);
        assert_eq!(g.rows().count(), 3);
        assert_eq!(g.scrollback_len(), 0);
    }

    #[test]
    fn duplicate_repaint_rows_are_dropped_but_real_repeats_are_kept() {
        // The rows have to be wide enough for the longest sample line.
        let mut g = Grid::new(8, 2, 100);
        let push = |g: &mut Grid, s: &str| {
            let mut r = Row::blank(8);
            for (i, c) in s.chars().enumerate() {
                r.cells[i].ch = c;
            }
            g.scrollback.push_back(r);
        };
        push(&mut g, "building");
        push(&mut g, "building"); // a stale repaint
        push(&mut g, "done");
        push(&mut g, "done"); // a legitimate repeat after a command
        push(&mut g, "done");
        let removed = g.dedup_scrollback();
        assert_eq!(removed, 2);
        let text: Vec<String> = g.all_lines().map(Row::text).collect();
        assert_eq!(text[0], "building");
        assert_eq!(text[1], "done");
    }

    #[test]
    fn clearing_scrollback_leaves_the_viewport_alone() {
        let mut g = Grid::new(4, 2, 10);
        g.row_mut(0).unwrap().cells[0].ch = 'k';
        g.scroll_up(1, g.scroll_region);
        g.clear_scrollback();
        assert_eq!(g.scrollback_len(), 0);
        assert_eq!(g.display_offset, 0);
    }

    #[test]
    fn clearing_the_screen_leaves_scrollback_alone() {
        let mut g = Grid::new(4, 2, 10);
        g.scroll_up(1, g.scroll_region);
        let before = g.scrollback_len();
        g.clear_screen();
        assert_eq!(g.scrollback_len(), before);
        assert!(g.rows().all(Row::is_blank));
    }

    #[test]
    fn a_row_trims_its_trailing_blanks_for_capture() {
        let mut r = Row::blank(10);
        r.cells[0].ch = 'h';
        r.cells[1].ch = 'i';
        assert_eq!(r.text(), "hi");
        assert_eq!(r.trimmed().len(), 2);
    }
}
