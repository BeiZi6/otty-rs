const test = require('node:test');
const assert = require('node:assert/strict');
const { moveTab, activeAfterClose } = require('./tab-order');

test('drag reorders before or after a target without changing other tabs', () => {
  assert.deepEqual(moveTab(['a', 'b', 'c'], 'c', 'a'), ['c', 'a', 'b']);
  assert.deepEqual(moveTab(['a', 'b', 'c'], 'a', 'b', true), ['b', 'a', 'c']);
});

test('dragging onto itself or an unknown tab leaves order unchanged', () => {
  assert.deepEqual(moveTab(['a', 'b'], 'a', 'a'), ['a', 'b']);
  assert.deepEqual(moveTab(['a', 'b'], 'x', 'b'), ['a', 'b']);
});

test('closing a tab selects the tab now occupying its slot, or the last tab', () => {
  assert.equal(activeAfterClose(['a', 'b', 'c'], 'b', ['a', 'c']), 'c');
  assert.equal(activeAfterClose(['a', 'b', 'c'], 'c', ['a', 'b']), 'b');
  assert.equal(activeAfterClose(['a'], 'a', []), null);
});
