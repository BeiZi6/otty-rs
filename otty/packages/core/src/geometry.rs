//! Geometry in device-independent pixels and terminal cells.

use serde::{Deserialize, Serialize};

/// A point in logical pixels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { origin: Point::new(x, y), size: Size::new(width, height) }
    }

    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.origin.x && p.x < self.right() && p.y >= self.origin.y && p.y < self.bottom()
    }

    pub fn inset(&self, by: Insets) -> Rect {
        Rect::new(
            self.origin.x + by.left,
            self.origin.y + by.top,
            (self.size.width - by.left - by.right).max(0.0),
            (self.size.height - by.top - by.bottom).max(0.0),
        )
    }
}

/// Per-edge padding, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Insets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Insets {
    pub const fn uniform(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }

    pub const fn symmetric(x: f32, y: f32) -> Self {
        Self { top: y, right: x, bottom: y, left: x }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

/// One cell of the terminal grid, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CellSize {
    pub width: f32,
    pub height: f32,
    /// Distance from the top of the cell to the text baseline.
    pub baseline: f32,
}

impl Default for CellSize {
    fn default() -> Self {
        Self { width: 8.0, height: 17.0, baseline: 13.0 }
    }
}

impl CellSize {
    /// How many whole cells fit in `size` after `padding` is removed.
    pub fn grid_dimensions(&self, size: Size, padding: Insets) -> (u16, u16) {
        let w = (size.width - padding.horizontal()).max(0.0);
        let h = (size.height - padding.vertical()).max(0.0);
        let cols = (w / self.width).floor().max(1.0) as u32;
        let rows = (h / self.height).floor().max(1.0) as u32;
        (cols.min(u16::MAX as u32) as u16, rows.min(u16::MAX as u32) as u16)
    }
}

/// The pixel box a cell occupies inside a grid with the given origin.
pub fn cell_rect(origin: Point, cell: CellSize, col: u16, row: u16) -> Rect {
    Rect::new(
        origin.x + col as f32 * cell.width,
        origin.y + row as f32 * cell.height,
        cell.width,
        cell.height,
    )
}

/// Interpret an `adjust-cell-height` value the way Ghostty does: a bare number
/// or `Npx` is an absolute delta in points, `N%` is a multiplier of the natural
/// cell height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellHeightAdjust {
    /// Add (or remove) this many logical points.
    Points(f32),
    /// Scale the natural height by `1 + percent / 100`.
    Percent(f32),
}

impl CellHeightAdjust {
    /// Parse `Npx`, `Npx` with a leading `+`/`-`, a bare number, or `N%`.
    pub fn parse(input: &str) -> Result<Self, String> {
        let s = input.trim();
        if s.is_empty() {
            return Err("empty".into());
        }
        if let Some(p) = s.strip_suffix('%') {
            let n: f32 = p.trim().parse().map_err(|_| format!("bad percent: {s}"))?;
            return Ok(Self::Percent(n));
        }
        let n = s.strip_suffix("px").unwrap_or(s).trim();
        let v: f32 = n.parse().map_err(|_| format!("bad point delta: {s}"))?;
        Ok(Self::Points(v))
    }

    /// Apply the delta to a natural cell height.
    pub fn apply(self, natural: f32) -> f32 {
        match self {
            Self::Points(d) => (natural + d).max(1.0),
            Self::Percent(p) => (natural * (1.0 + p / 100.0)).max(1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_dimensions_respect_padding() {
        let cell = CellSize { width: 10.0, height: 20.0, baseline: 15.0 };
        assert_eq!(cell.grid_dimensions(Size::new(100.0, 100.0), Insets::default()), (10, 5));
        assert_eq!(
            cell.grid_dimensions(Size::new(100.0, 100.0), Insets::uniform(5.0)),
            (9, 4)
        );
    }

    #[test]
    fn grid_never_collapses_to_zero() {
        let cell = CellSize::default();
        assert_eq!(cell.grid_dimensions(Size::new(1.0, 1.0), Insets::default()), (1, 1));
    }

    #[test]
    fn cell_height_adjust_parses_both_spellings() {
        assert_eq!(CellHeightAdjust::parse("2px").unwrap(), CellHeightAdjust::Points(2.0));
        assert_eq!(CellHeightAdjust::parse("-3").unwrap(), CellHeightAdjust::Points(-3.0));
        assert_eq!(CellHeightAdjust::parse("20%").unwrap(), CellHeightAdjust::Percent(20.0));
        assert!((CellHeightAdjust::parse("20%").unwrap().apply(17.0) - 20.4).abs() < 1e-4);
        assert_eq!(CellHeightAdjust::parse("2px").unwrap().apply(17.0), 19.0);
        assert!(CellHeightAdjust::parse("nope").is_err());
    }
}
