//! Turning the split tree into rectangles.
//!
//! Pure arithmetic on the model, so every rule here is a test rather than a
//! screenshot. The renderer asks for the rectangles and draws them; the mouse
//! handler asks for the dividers and hit-tests them.
//!
//! # The two rules that are easy to get subtly wrong
//!
//! **The ratio is a share of what is left after the dividers, not of the whole.**
//! In a 1000 px viewport with a 6 px divider, a 50 % split gives each pane 497 px,
//! not 500 — and if you compute the second pane as `width - first` you get 503,
//! which drifts by the divider width on every nested split. The second pane is
//! therefore computed from the divider's right edge, never by subtraction from the
//! parent.
//!
//! **Round, don't truncate.** A pane at 33 % of 100 px is 33.0; at 33 % of 101 px
//! it is 33.33, and truncating loses a pixel per split per level. The result is
//! that a grid of panes does not tile its parent exactly and a hairline of
//! background shows through. Rounding is what keeps `sum(panes) + dividers ==
//! viewport` true, and that identity is a test.

use std::collections::BTreeMap;

use otty_core::geometry::{Insets, Rect, Size};

use crate::model::{Axis, Node};
use otty_core::PaneId;

/// How the dividers between panes are sized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutOptions {
    /// The width of the gutter between two panes.
    pub divider: f32,
    /// Padding inside each pane, before its grid starts.
    pub pane_padding: Insets,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            // Matches the window's own divider: wide enough to grab, narrow
            // enough not to read as a gap.
            divider: 6.0,
            pane_padding: Insets::uniform(0.0),
        }
    }
}

/// Where every pane goes, plus the dividers to hit-test.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    rects: BTreeMap<PaneId, Rect>,
    dividers: Vec<Divider>,
}

/// A draggable gutter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Divider {
    pub rect: Rect,
    pub axis: Axis,
    /// The grid position of the split this divider belongs to, so a drag can be
    /// routed back to `Node::set_ratio_for` without searching the tree again.
    pub first_pane: PaneId,
    pub second_pane: PaneId,
}

impl Layout {
    /// The rectangle for a pane, if it is on screen.
    pub fn rect(&self, pane: PaneId) -> Option<Rect> {
        self.rects.get(&pane).copied()
    }

    /// Every pane's rectangle, in layout order.
    pub fn rects(&self) -> &BTreeMap<PaneId, Rect> {
        &self.rects
    }

    pub fn dividers(&self) -> &[Divider] {
        &self.dividers
    }

    /// The pane under a point, if any. Later panes win, which matters only for
    /// the impossible case of two panes overlapping — it makes the function total
    /// rather than returning whichever the map happened to yield first.
    pub fn pane_at(&self, x: f32, y: f32) -> Option<PaneId> {
        self.rects
            .iter()
            .filter(|(_, r)| r.contains(otty_core::Point::new(x, y)))
            .map(|(id, _)| *id)
            .next_back()
    }

    /// The divider under a point, if any. Checked before panes in a drag handler,
    /// because a divider sits *between* two panes and hit-testing the panes first
    /// would make it ungrabbable.
    pub fn divider_at(&self, x: f32, y: f32) -> Option<Divider> {
        self.dividers
            .iter()
            .find(|d| d.rect.contains(otty_core::Point::new(x, y)))
            .copied()
    }

    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rects.len()
    }
}

/// Lay a split tree out inside `viewport`.
///
/// Zoomed panes are handled by the caller: a zoomed tab lays out its single pane
/// in the whole viewport and never reaches this function with a tree.
pub fn layout(tree: &Node, viewport: Rect, options: &LayoutOptions) -> Layout {
    let mut out = Layout::default();
    place(tree, viewport, options, &mut out);
    out
}

fn place(node: &Node, area: Rect, options: &LayoutOptions, out: &mut Layout) {
    match node {
        Node::Leaf(id) => {
            out.rects.insert(*id, area.inset(options.pane_padding));
        }
        Node::Split { axis, ratio, first, second } => {
            let (first_area, divider_area, second_area) = divide(area, *axis, *ratio, options.divider);
            match divider_area {
                Some(divider) => {
                    let first_pane = first.panes()[0];
                    let second_pane = second.panes()[0];
                    out.dividers.push(Divider {
                        rect: divider,
                        axis: *axis,
                        first_pane,
                        second_pane,
                    });
                }
                None => {}
            }
            place(first, first_area, options, out);
            place(second, second_area, options, out);
        }
    }
}

/// Split `area` into first, divider and second.
///
/// Returns `None` for the divider when the two panes are touching anyway —
/// which happens when the area is too small for a gutter, and is better than
/// emitting a divider whose width is negative.
fn divide(area: Rect, axis: Axis, ratio: u8, divider: f32) -> (Rect, Option<Rect>, Rect) {
    let extent = match axis {
        Axis::Horizontal => area.size.width,
        Axis::Vertical => area.size.height,
    };
    // The gutter may never take more than half the extent. Clamping it to the
    // full extent instead would leave two panes of zero width and a divider
    // filling the area — every pixel spent on chrome and none on text.
    let gutter = divider.max(0.0).min(extent * 0.5);
    let usable = (extent - gutter).max(0.0);
    let first_extent = (usable * ratio as f32 / 100.0).round();
    // The second pane is measured from the gutter's far edge. Subtracting the
    // first from the parent would accumulate the gutter's width on every level.
    let second_extent = (extent - gutter - first_extent).max(0.0);

    match axis {
        Axis::Horizontal => (
            Rect::new(area.origin.x, area.origin.y, first_extent, area.size.height),
            (gutter > 0.0).then(|| Rect::new(area.origin.x + first_extent, area.origin.y, gutter, area.size.height)),
            Rect::new(area.origin.x + first_extent + gutter, area.origin.y, second_extent, area.size.height),
        ),
        Axis::Vertical => (
            Rect::new(area.origin.x, area.origin.y, area.size.width, first_extent),
            (gutter > 0.0).then(|| Rect::new(area.origin.x, area.origin.y + first_extent, area.size.width, gutter)),
            Rect::new(area.origin.x, area.origin.y + first_extent + gutter, area.size.width, second_extent),
        ),
    }
}

/// Lay out a single pane filling the viewport — a zoomed tab, or a tab that was
/// never split.
pub fn layout_single(pane: PaneId, viewport: Rect, options: &LayoutOptions) -> Layout {
    let mut out = Layout::default();
    out.rects.insert(pane, viewport.inset(options.pane_padding));
    out
}

/// The grid each pane can hold, given a cell size.
///
/// The padding is applied by `layout` already, so the rectangle passed here is
/// the text area; the insets are zeroed rather than applied twice.
pub fn grid_dimensions(rect: Rect, cell: otty_core::CellSize) -> (u16, u16) {
    cell.grid_dimensions(Size::new(rect.size.width, rect.size.height), Insets::uniform(0.0))
}

/// Convert a pixel position inside a pane back to a cell coordinate.
///
/// Returns `None` outside the pane, so a click that lands on a divider does not
/// become a click on the last column of the pane beside it.
pub fn cell_at(rect: Rect, cell: otty_core::CellSize, x: f32, y: f32) -> Option<(u16, u16)> {
    if !rect.contains(otty_core::Point::new(x, y)) {
        return None;
    }
    if cell.width <= 0.0 || cell.height <= 0.0 {
        return None;
    }
    let col = ((x - rect.origin.x) / cell.width).floor().max(0.0) as u16;
    let row = ((y - rect.origin.y) / cell.height).floor().max(0.0) as u16;
    Some((col, row))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SplitDirection;
    use otty_core::CellSize;

    fn p(n: u64) -> PaneId {
        PaneId::new(n)
    }

    fn cell() -> CellSize {
        CellSize { width: 8.0, height: 18.0, baseline: 14.0 }
    }

    fn viewport() -> Rect {
        Rect::new(0.0, 0.0, 1000.0, 600.0)
    }

    fn opts() -> LayoutOptions {
        LayoutOptions { divider: 6.0, pane_padding: Insets::uniform(0.0) }
    }

    /// The identity every layout must satisfy.
    fn assert_tiles(layout: &Layout, area: Rect) {
        let total: f32 = layout.rects().values().map(|r| r.size.width * r.size.height).sum::<f32>()
            + layout.dividers().iter().map(|d| d.rect.size.width * d.rect.size.height).sum::<f32>();
        let expected = area.size.width * area.size.height;
        assert!(
            (total - expected).abs() < 0.5,
            "panes + dividers = {total}, viewport = {expected}"
        );
    }

    #[test]
    fn a_single_leaf_fills_the_viewport() {
        let l = layout(&Node::Leaf(p(1)), viewport(), &opts());
        assert_eq!(l.len(), 1);
        assert_eq!(l.rect(p(1)), Some(viewport()));
        assert!(l.dividers().is_empty());
        assert_tiles(&l, viewport());
    }

    #[test]
    fn a_horizontal_split_shares_what_is_left_after_the_divider() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        let l = layout(&tree, viewport(), &opts());
        // 1000 - 6 = 994 usable; half is 497 each.
        assert_eq!(l.rect(p(1)).unwrap().size.width, 497.0);
        assert_eq!(l.rect(p(2)).unwrap().size.width, 497.0);
        assert_eq!(l.dividers().len(), 1);
        let d = l.dividers()[0];
        // The divider sits exactly between them and is its own width.
        assert_eq!(d.rect, Rect::new(497.0, 0.0, 6.0, 600.0));
        assert_eq!(d.axis, Axis::Horizontal);
        assert_tiles(&l, viewport());
        // Nothing is lost: the right pane starts where the divider ends.
        assert_eq!(l.rect(p(1)).unwrap().right(), d.rect.origin.x);
        assert_eq!(d.rect.right(), l.rect(p(2)).unwrap().origin.x);
    }

    #[test]
    fn a_vertical_split_shares_the_height() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Down, 25);
        let l = layout(&tree, viewport(), &opts());
        // 600 - 6 = 594; 25% is 148.5, rounded to 149.
        assert_eq!(l.rect(p(1)).unwrap().size.height, 149.0);
        // The second takes the remainder from the gutter's edge, not 594-149.
        assert_eq!(l.rect(p(2)).unwrap().size.height, 600.0 - 6.0 - 149.0);
        assert_eq!(l.dividers()[0].axis, Axis::Vertical);
        assert_tiles(&l, viewport());
    }

    #[test]
    fn a_left_split_puts_the_new_pane_first_and_tiles_exactly() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Left, 30);
        let l = layout(&tree, viewport(), &opts());
        // Layout order follows the tree, so p(2) is on the left.
        assert_eq!(l.rect(p(2)).unwrap().origin.x, 0.0);
        assert!(l.rect(p(1)).unwrap().origin.x > l.rect(p(2)).unwrap().origin.x);
        assert_tiles(&l, viewport());
    }

    #[test]
    fn nested_splits_tile_the_parent_exactly() {
        // The bug this guards: computing the second pane by subtracting the first
        // from the parent loses the divider width at every level.
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        tree.split(p(2), p(3), SplitDirection::Down, 50);
        tree.split(p(1), p(4), SplitDirection::Down, 33);
        let l = layout(&tree, viewport(), &opts());
        assert_eq!(l.len(), 4);
        assert_eq!(l.dividers().len(), 3);
        assert_tiles(&l, viewport());
    }

    #[test]
    fn four_levels_of_nesting_do_not_drift() {
        let mut tree = Node::Leaf(p(1));
        let mut current = p(1);
        for i in 2..=5u64 {
            tree.split(current, p(i), SplitDirection::Right, 50);
            current = p(i);
        }
        let l = layout(&tree, viewport(), &opts());
        assert_eq!(l.len(), 5);
        // Each level halves what is left after its own divider. The last pane is
        // the remainder, so the sum must still be exact.
        assert_tiles(&l, viewport());
        let sum: f32 = l.rects().values().map(|r| r.size.width).sum();
        let gutters: f32 = l.dividers().iter().map(|d| d.rect.size.width).sum();
        assert!((sum + gutters - 1000.0).abs() < 0.5, "sum {sum} + gutters {gutters}");
    }

    #[test]
    fn rounding_does_not_lose_a_pixel_on_an_awkward_width() {
        // 101 px, 4 px divider, 33%: usable 97, 33% is 32.01 -> 32.
        let tree = {
            let mut t = Node::Leaf(p(1));
            t.split(p(1), p(2), SplitDirection::Right, 33);
            t
        };
        let area = Rect::new(0.0, 0.0, 101.0, 50.0);
        let l = layout(&tree, area, &opts());
        assert_eq!(l.rect(p(1)).unwrap().size.width + l.rect(p(2)).unwrap().size.width + 6.0, 101.0);
        assert_tiles(&l, area);
    }

    #[test]
    fn a_viewport_too_small_for_a_gutter_shrinks_the_gutter_rather_than_the_panes() {
        let tree = {
            let mut t = Node::Leaf(p(1));
            t.split(p(1), p(2), SplitDirection::Right, 50);
            t
        };
        let area = Rect::new(0.0, 0.0, 4.0, 10.0);
        let l = layout(&tree, area, &opts());
        // A 6 px gutter in a 4 px area must not take everything: it is clamped
        // to half, so each pane keeps a pixel and the divider keeps two.
        assert_eq!(l.dividers().len(), 1);
        assert_eq!(l.dividers()[0].rect.size.width, 2.0);
        assert_eq!(l.rect(p(1)).unwrap().size.width, 1.0);
        assert_eq!(l.rect(p(2)).unwrap().size.width, 1.0);
        assert!(l.rects().values().all(|r| r.size.width >= 0.0 && r.size.height >= 0.0));
        assert_tiles(&l, area);
    }

    #[test]
    fn a_zero_width_area_has_no_room_for_a_divider_at_all() {
        let tree = {
            let mut t = Node::Leaf(p(1));
            t.split(p(1), p(2), SplitDirection::Right, 50);
            t
        };
        let area = Rect::new(0.0, 0.0, 0.0, 10.0);
        let l = layout(&tree, area, &opts());
        assert_eq!(l.divider_at(0.0, 0.0), None);
        assert!(l.rects().values().all(|r| r.size.width == 0.0));
    }

    #[test]
    fn pane_padding_is_applied_once_inside_each_pane() {
        let o = LayoutOptions { divider: 6.0, pane_padding: Insets::uniform(4.0) };
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        let l = layout(&tree, viewport(), &o);
        let a = l.rect(p(1)).unwrap();
        assert_eq!(a.origin.x, 4.0);
        assert_eq!(a.origin.y, 4.0);
        assert_eq!(a.size.width, 497.0 - 8.0);
        assert_eq!(a.size.height, 600.0 - 8.0);
        // A single pane keeps the padding too.
        let l = layout(&Node::Leaf(p(1)), viewport(), &o);
        assert_eq!(l.rect(p(1)).unwrap(), Rect::new(4.0, 4.0, 992.0, 592.0));
    }

    #[test]
    fn hit_testing_finds_the_pane_and_respects_the_divider() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        let l = layout(&tree, viewport(), &opts());
        assert_eq!(l.pane_at(100.0, 300.0), Some(p(1)));
        assert_eq!(l.pane_at(900.0, 300.0), Some(p(2)));
        // Inside the gutter there is no pane.
        assert_eq!(l.pane_at(500.0, 300.0), None);
        // Outside the viewport there is none either.
        assert_eq!(l.pane_at(1200.0, 300.0), None);
        assert_eq!(l.pane_at(-1.0, 300.0), None);
    }

    #[test]
    fn the_divider_is_grabbable_where_the_pane_test_says_no() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        let l = layout(&tree, viewport(), &opts());
        let d = l.divider_at(500.0, 300.0).expect("the gutter must be grabbable");
        assert_eq!(d.axis, Axis::Horizontal);
        assert_eq!(d.first_pane, p(1));
        assert_eq!(d.second_pane, p(2));
        // Just outside the gutter is a pane, not a divider.
        assert!(l.divider_at(400.0, 300.0).is_none());
    }

    #[test]
    fn a_dividers_two_sides_name_the_panes_it_separates() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        tree.split(p(2), p(3), SplitDirection::Down, 50);
        let l = layout(&tree, viewport(), &opts());
        let outer = l.dividers().iter().find(|d| d.axis == Axis::Horizontal).unwrap();
        assert_eq!((outer.first_pane, outer.second_pane), (p(1), p(2)));
        let inner = l.dividers().iter().find(|d| d.axis == Axis::Vertical).unwrap();
        // The inner divider's first child is p(2): the split it belongs to is the
        // one that p(2) and p(3) are the children of.
        assert_eq!(inner.first_pane, p(2));
        assert_eq!(inner.second_pane, p(3));
    }

    #[test]
    fn a_zoomed_pane_gets_the_whole_viewport() {
        let l = layout_single(p(7), viewport(), &opts());
        assert_eq!(l.len(), 1);
        assert_eq!(l.rect(p(7)), Some(viewport()));
        assert!(l.dividers().is_empty());
    }

    #[test]
    fn the_grid_dimensions_follow_the_pane_rectangle() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        let l = layout(&tree, viewport(), &opts());
        let (cols, rows) = grid_dimensions(l.rect(p(1)).unwrap(), cell());
        // 497 / 8 = 62 (floor), 600 / 18 = 33.
        assert_eq!((cols, rows), (62, 33));
    }

    #[test]
    fn a_pane_too_small_for_one_cell_still_reports_one_cell() {
        let rect = Rect::new(0.0, 0.0, 4.0, 9.0);
        let (cols, rows) = grid_dimensions(rect, cell());
        // `CellSize::grid_dimensions` clamps to at least one cell in each axis,
        // which is the right answer for a terminal: a pane that exists is never
        // a 0x0 grid, it is a 1x1 grid showing one character.
        assert_eq!((cols, rows), (1, 1));
    }

    #[test]
    fn a_click_maps_back_to_the_cell_it_landed_on() {
        let rect = Rect::new(10.0, 20.0, 800.0, 600.0);
        let c = cell();
        assert_eq!(cell_at(rect, c, 10.0, 20.0), Some((0, 0)));
        assert_eq!(cell_at(rect, c, 17.9, 20.0), Some((0, 0)));
        assert_eq!(cell_at(rect, c, 18.0, 20.0), Some((1, 0)));
        assert_eq!(cell_at(rect, c, 10.0, 38.0), Some((0, 1)));
        // Outside the pane there is no cell. `Rect::contains` is half-open, so
        // the right edge itself belongs to whatever comes next — on a split
        // that is the divider, and a click there must not become a click on the
        // pane's last column.
        assert_eq!(cell_at(rect, c, 810.0, 20.0), None);
        assert_eq!(cell_at(rect, c, 9.0, 20.0), None);
        // The last cell inside is column 99 (10 + 99*8 = 802, and 802..810).
        assert_eq!(cell_at(rect, c, 809.9, 20.0), Some((99, 0)));
    }

    #[test]
    fn a_zero_sized_cell_does_not_divide_by_zero() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(cell_at(rect, CellSize { width: 0.0, height: 0.0, baseline: 0.0 }, 50.0, 50.0), None);
    }

    #[test]
    fn every_pane_in_a_deep_tree_is_reachable_by_a_point_inside_it() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        tree.split(p(2), p(3), SplitDirection::Down, 50);
        tree.split(p(1), p(4), SplitDirection::Down, 60);
        let l = layout(&tree, viewport(), &opts());
        for (id, rect) in l.rects() {
            let centre = (rect.origin.x + rect.size.width / 2.0, rect.origin.y + rect.size.height / 2.0);
            assert_eq!(l.pane_at(centre.0, centre.1), Some(*id), "centre of {id:?}");
        }
    }
}
