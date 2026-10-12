const test = require('node:test');
const assert = require('node:assert/strict');
const {
  fuzzy, segments, move, debounce, pinFirst, trigger, chord, actionFor, shortcutLabel, keyName,
} = require('./renderer/features/_list');

test('fuzzy prefers the contiguous hit, falls back to a subsequence', () => {
  assert.deepEqual(fuzzy('Split Right', 'right'), [6, 7, 8, 9, 10]);
  assert.deepEqual(fuzzy('Split Right', 'sr'), [0, 6]);
  assert.deepEqual(fuzzy('Split Right', 'sp r'), [0, 1, 6]);
  assert.deepEqual(fuzzy('anything', ''), []);
  assert.equal(fuzzy('Split Right', 'xz'), null);
});

test('segments cut the text into matched and unmatched runs', () => {
  assert.deepEqual(segments('New Tab', 'tab'), [['New ', false], ['Tab', true]]);
  assert.deepEqual(segments('New Tab', 'nt'), [['N', true], ['ew ', false], ['T', true], ['ab', false]]);
  assert.deepEqual(segments('New Tab', 'zzz'), [['New Tab', false]]);
  assert.deepEqual(segments('New Tab', ''), [['New Tab', false]]);
});

test('move wraps on arrows, clamps on pages, ignores other keys', () => {
  assert.equal(move(4, 5, 'ArrowDown'), 0);
  assert.equal(move(0, 5, 'ArrowUp'), 4);
  assert.equal(move(1, 30, 'PageDown'), 11);
  assert.equal(move(25, 30, 'PageDown'), 29);
  assert.equal(move(3, 30, 'PageUp'), 0);
  assert.equal(move(3, 5, 'Home'), 0);
  assert.equal(move(3, 5, 'End'), 4);
  assert.equal(move(3, 5, 'a'), null);
  assert.equal(move(0, 0, 'ArrowDown'), -1);
});

test('debounce runs once with the last arguments and can be cancelled', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const calls = [];
  const run = debounce((value) => calls.push(value), 100);
  run('a');
  run('b');
  t.mock.timers.tick(99);
  assert.deepEqual(calls, []);
  t.mock.timers.tick(1);
  assert.deepEqual(calls, ['b']);
  run('c');
  run.cancel();
  t.mock.timers.tick(200);
  assert.deepEqual(calls, ['b']);
});

test('pinFirst lifts the first matching row and keeps the rest in order', () => {
  const rows = [{ kind: 'folder', n: 1 }, { kind: 'path', n: 2 }, { kind: 'file', n: 3 }];
  assert.deepEqual(pinFirst(rows, (row) => row.kind === 'path').map((row) => row.n), [2, 1, 3]);
  assert.equal(pinFirst(rows, (row) => row.kind === 'ssh'), rows);
});

test('a keydown matches its keymap trigger; cmd is Ctrl off macOS', () => {
  const key = (code, k, mods) => ({ code, key: k, ...mods });
  assert.equal(chord(key('KeyP', 'P', { ctrlKey: true, shiftKey: true }), false), trigger('cmd+shift+p', false));
  assert.equal(chord(key('KeyP', 'P', { metaKey: true, shiftKey: true }), true), trigger('cmd+shift+p', true));
  assert.notEqual(chord(key('KeyP', 'P', { ctrlKey: true, shiftKey: true }), true), trigger('cmd+shift+p', true));
  assert.notEqual(chord(key('KeyP', 'P', { metaKey: true, shiftKey: true }), false), trigger('cmd+shift+p', false));
  // ⌥D types "∂" on macOS; the physical key still matches.
  assert.equal(chord(key('KeyD', '∂', { metaKey: true, altKey: true }), true), trigger('cmd+opt+d', true));
  assert.equal(chord(key('BracketLeft', '{', { ctrlKey: true, shiftKey: true }), false), trigger('cmd+shift+left_bracket', false));
  assert.equal(chord(key('Space', ' ', { ctrlKey: true, shiftKey: true }), false), trigger('ctrl+shift+space', false));
  assert.equal(chord(key('Enter', 'Enter', { ctrlKey: true, shiftKey: true }), false), trigger('cmd+shift+enter', false));

  const keymap = [{ trigger: 'cmd+g', action: 'find_next' }, { trigger: 'cmd+shift+g', action: 'find_previous' }];
  assert.equal(actionFor(keymap, key('KeyG', 'g', { ctrlKey: true }), false), 'find_next');
  assert.equal(actionFor(keymap, key('KeyG', 'G', { ctrlKey: true, shiftKey: true }), false), 'find_previous');
  assert.equal(actionFor(keymap, key('KeyG', 'g', {}), false), undefined);
  assert.equal(actionFor(undefined, key('KeyG', 'g', {}), false), undefined);
});

test('shortcutLabel renders glyphs on macOS and words elsewhere', () => {
  assert.equal(shortcutLabel('cmd+shift+p', true), '⌘⇧P');
  assert.equal(shortcutLabel('cmd+opt+d', true), '⌘⌥D');
  assert.equal(shortcutLabel('cmd+shift+p', false), 'Ctrl+Shift+P');
  assert.equal(shortcutLabel('cmd+shift+left_bracket', false), 'Ctrl+Shift+Left Bracket');
  assert.equal(shortcutLabel('', false), '');
});

test('keyName keeps the typed character and names held modifiers', () => {
  assert.equal(keyName({ key: 'G', shiftKey: true }), 'G');
  assert.equal(keyName({ key: 'u', ctrlKey: true }), 'ctrl+u');
  assert.equal(keyName({ key: 'ArrowUp', shiftKey: true }), 'shift+ArrowUp');
  assert.equal(keyName({ key: 'Escape' }), 'Escape');
});
