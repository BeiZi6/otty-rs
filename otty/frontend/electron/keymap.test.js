const test = require('node:test');
const assert = require('node:assert/strict');
const { normalize, eventChord, buildKeymap, lookup, accelerator } = require('./keymap');

const key = (code, mods = {}) => ({ code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });

test('cmd is the command key on macOS and Ctrl elsewhere', () => {
  assert.equal(normalize('cmd+shift+p', true), 'shift+cmd+p');
  assert.equal(normalize('cmd+shift+p', false), 'ctrl+shift+p');
  assert.equal(normalize('cmd+opt+d', false), 'ctrl+alt+d');
  assert.equal(normalize('cmd+ctrl+f', false), 'ctrl+alt+f');
  assert.equal(normalize('ctrl+shift+space', false), 'ctrl+shift+space');
});

test('events become chords by physical key', () => {
  assert.equal(eventChord(key('KeyP', { ctrlKey: true, shiftKey: true })), 'ctrl+shift+p');
  assert.equal(eventChord(key('BracketLeft', { metaKey: true, shiftKey: true })), 'shift+cmd+left_bracket');
  assert.equal(eventChord(key('Digit9', { ctrlKey: true })), 'ctrl+9');
  assert.equal(eventChord(key('ControlLeft', { ctrlKey: true })), '');
});

test('a bound chord runs its action and an unbound one misses', () => {
  const map = buildKeymap([
    { trigger: 'cmd+t', action: 'new_tab' },
    { trigger: 'cmd+shift+enter', action: 'zoom_split' },
  ], false);
  assert.equal(lookup(map, eventChord(key('KeyT', { ctrlKey: true }))), 'new_tab');
  assert.equal(lookup(map, eventChord(key('Enter', { ctrlKey: true, shiftKey: true }))), 'zoom_split');
  assert.equal(lookup(map, eventChord(key('KeyT'))), null);
  assert.equal(lookup(map, ''), null);
});

test('shell control keys reach the pane unless copying a selection', () => {
  const map = buildKeymap([
    { trigger: 'cmd+c', action: 'copy' },
    { trigger: 'cmd+d', action: 'split_right' },
    { trigger: 'cmd+shift+d', action: 'split_down' },
  ], false);
  assert.equal(lookup(map, 'ctrl+c', false), null);
  assert.equal(lookup(map, 'ctrl+c', true), 'copy');
  assert.equal(lookup(map, 'ctrl+d', true), null);
  assert.equal(lookup(map, 'ctrl+shift+d', false), 'split_down');
  const mac = buildKeymap([{ trigger: 'cmd+c', action: 'copy' }], true);
  assert.equal(lookup(mac, 'cmd+c', false), 'copy');
});

test('the first binding of a chord wins', () => {
  const map = buildKeymap([
    { trigger: 'cmd+ctrl+up', action: 'focus_pane_up' },
    { trigger: 'cmd+opt+up', action: 'scroll_up' },
  ], false);
  assert.equal(map.get('ctrl+alt+up'), 'focus_pane_up');
});

test('triggers print as Electron accelerators', () => {
  assert.equal(accelerator('cmd+shift+p', false), 'Ctrl+Shift+P');
  assert.equal(accelerator('cmd+comma', true), 'Cmd+,');
  assert.equal(accelerator('cmd+shift+left_bracket', false), 'Ctrl+Shift+[');
  assert.equal(accelerator('cmd+page_up', false), 'Ctrl+PageUp');
  assert.equal(accelerator('cmd+no_such_key', false), undefined);
});
