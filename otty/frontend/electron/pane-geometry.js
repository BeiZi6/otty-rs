(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OttyPaneGeometry = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  function clampRatio(value) {
    const ratio = Number(value);
    return Math.min(90, Math.max(10, Number.isFinite(ratio) ? Math.round(ratio) : 50));
  }

  function leaves(tree) {
    if (!tree) return [];
    return tree.pane ? [tree.pane] : [...leaves(tree.first), ...leaves(tree.second)];
  }

  // Split tree → pane rects and divider lines, as fractions of the tab area.
  // A divider's `pane` is what `pane.ratio` takes: a leaf directly under that
  // split, or null when both children are splits themselves.
  function layout(tree, rect = { x: 0, y: 0, w: 1, h: 1 }, out = { panes: [], dividers: [] }) {
    if (!tree) return out;
    if (tree.pane) {
      out.panes.push({ pane: tree.pane, ...rect });
      return out;
    }
    const share = clampRatio(tree.ratio) / 100;
    const horizontal = tree.axis === 'horizontal';
    const first = horizontal ? { ...rect, w: rect.w * share } : { ...rect, h: rect.h * share };
    const second = horizontal
      ? { ...rect, x: rect.x + first.w, w: rect.w - first.w }
      : { ...rect, y: rect.y + first.h, h: rect.h - first.h };
    out.dividers.push({
      horizontal,
      rect,
      at: horizontal ? second.x : second.y,
      pane: tree.first.pane || tree.second.pane || null,
    });
    layout(tree.first, first, out);
    layout(tree.second, second, out);
    return out;
  }

  // The ratio a divider takes when dragged to (x, y), fractions of the tab area.
  function ratioAt(divider, x, y) {
    const { rect } = divider;
    const share = divider.horizontal ? (x - rect.x) / rect.w : (y - rect.y) / rect.h;
    return clampRatio(share * 100);
  }

  function cellAt(x, y, width, height, grid) {
    const clamp = (value, count) => Math.min(count - 1, Math.max(0, Math.floor(value)));
    return [clamp((x / width) * grid.cols, grid.cols), clamp((y / height) * grid.rows, grid.rows)];
  }

  // A stream selection as one inclusive column span per row, top to bottom.
  function selectionSpans(from, to, cols) {
    const [start, end] = from[1] < to[1] || (from[1] === to[1] && from[0] <= to[0]) ? [from, to] : [to, from];
    const spans = [];
    for (let row = start[1]; row <= end[1]; row += 1) {
      spans.push({ row, from: row === start[1] ? start[0] : 0, to: row === end[1] ? end[0] : cols - 1 });
    }
    return spans;
  }

  return { clampRatio, leaves, layout, ratioAt, cellAt, selectionSpans };
});
