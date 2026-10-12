const test = require('node:test');
const assert = require('node:assert/strict');
const { clampRatio, leaves, layout, ratioAt, cellAt, selectionSpans } = require('./pane-geometry');

const leaf = (pane) => ({ pane });
// pane-1 | (pane-2 over pane-3)
const tree = {
  axis: 'horizontal',
  ratio: 25,
  first: leaf('pane-1'),
  second: { axis: 'vertical', ratio: 50, first: leaf('pane-2'), second: leaf('pane-3') },
};

test('a single leaf fills the tab', () => {
  assert.deepEqual(layout(leaf('pane-1')), { panes: [{ pane: 'pane-1', x: 0, y: 0, w: 1, h: 1 }], dividers: [] });
  assert.deepEqual(layout(null), { panes: [], dividers: [] });
});

test('nested splits lay out by axis and ratio', () => {
  const { panes, dividers } = layout(tree);
  assert.deepEqual(panes, [
    { pane: 'pane-1', x: 0, y: 0, w: 0.25, h: 1 },
    { pane: 'pane-2', x: 0.25, y: 0, w: 0.75, h: 0.5 },
    { pane: 'pane-3', x: 0.25, y: 0.5, w: 0.75, h: 0.5 },
  ]);
  assert.equal(dividers.length, 2);
  assert.deepEqual(dividers[0], { horizontal: true, rect: { x: 0, y: 0, w: 1, h: 1 }, at: 0.25, pane: 'pane-1', with: 'pane-2' });
  assert.deepEqual(dividers[1], { horizontal: false, rect: { x: 0.25, y: 0, w: 0.75, h: 1 }, at: 0.5, pane: 'pane-2', with: 'pane-3' });
  assert.deepEqual(leaves(tree), ['pane-1', 'pane-2', 'pane-3']);
});

test('a divider between two splits is addressed by a leaf on each side', () => {
  const other = { axis: 'horizontal', ratio: 50, first: leaf('pane-4'), second: leaf('pane-5') };
  const [root] = layout({ axis: 'vertical', ratio: 50, first: tree, second: other }).dividers;
  assert.deepEqual([root.pane, root.with], ['pane-1', 'pane-4']);
});

test('dragging a divider gives a ratio inside its own split, clamped 10 to 90', () => {
  const [outer, inner] = layout(tree).dividers;
  assert.equal(ratioAt(outer, 0.4, 0.9), 40);
  assert.equal(ratioAt(inner, 0.9, 0.75), 75);
  assert.equal(ratioAt(outer, 0.01, 0), 10);
  assert.equal(ratioAt(outer, 2, 0), 90);
  assert.equal(clampRatio('nope'), 50);
});

test('pixels map to cells and stay on the grid', () => {
  const grid = { cols: 80, rows: 24 };
  assert.deepEqual(cellAt(0, 0, 800, 480, grid), [0, 0]);
  assert.deepEqual(cellAt(15, 45, 800, 480, grid), [1, 2]);
  assert.deepEqual(cellAt(900, -5, 800, 480, grid), [79, 0]);
});

test('a selection covers whole rows between its ends, either drag direction', () => {
  const spans = [
    { row: 1, from: 5, to: 9 },
    { row: 2, from: 0, to: 9 },
    { row: 3, from: 0, to: 2 },
  ];
  assert.deepEqual(selectionSpans([5, 1], [2, 3], 10), spans);
  assert.deepEqual(selectionSpans([2, 3], [5, 1], 10), spans);
  assert.deepEqual(selectionSpans([7, 4], [3, 4], 10), [{ row: 4, from: 3, to: 7 }]);
});
