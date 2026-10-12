const test = require('node:test');
const assert = require('node:assert/strict');
const {
  orderGroups, controlType, eventChord, normalizeTrigger, matchBinding,
  debounce, joinPath, bytesOf, hexDump, errorText,
} = require('./renderer/features/_form');

test('groups follow the original Settings order, unknown ones keep their place after', () => {
  assert.deepEqual(
    orderGroups(['Window', 'Autocomplete', 'Keybindings', 'Font', 'Terminal Colors',
      'Shell & Environment', 'Font', 'Mouse & Input', 'Cursor', 'Theme']),
    ['Font', 'Cursor', 'Shell & Environment', 'Window', 'Theme', 'Terminal Colors',
      'Keybindings', 'Autocomplete', 'Mouse & Input'],
  );
});

test('control type comes from the schema type, enum values win over a string type', () => {
  assert.equal(controlType({ type: 'bool' }), 'bool');
  assert.equal(controlType({ type: 'enum', values: ['a', 'b'] }), 'enum');
  assert.equal(controlType({ type: 'str', values: ['a'] }), 'enum');
  assert.equal(controlType({ type: 'enum', values: [] }), 'text');
  assert.equal(controlType({ type: 'int' }), 'number');
  assert.equal(controlType({ type: 'number' }), 'number');
  assert.equal(controlType({ type: 'color' }), 'color');
  assert.equal(controlType({ type: 'repeated' }), 'list');
  assert.equal(controlType({ type: 'str' }), 'text');
  assert.equal(controlType({}), 'text');
});

test('a KeyboardEvent becomes a canonical chord', () => {
  const key = (code, mods = {}) => ({ code, key: '', ...mods });
  assert.equal(eventChord(key('KeyT', { ctrlKey: true, shiftKey: true }), 'windows'), 'cmd+shift+t');
  assert.equal(eventChord(key('KeyT', { metaKey: true }), 'macos'), 'cmd+t');
  assert.equal(eventChord(key('KeyF', { metaKey: true, ctrlKey: true }), 'macos'), 'cmd+ctrl+f');
  assert.equal(eventChord(key('KeyD', { ctrlKey: true, altKey: true, shiftKey: true }), 'linux'), 'cmd+opt+shift+d');
  assert.equal(eventChord(key('Comma', { ctrlKey: true }), 'windows'), 'cmd+comma');
  assert.equal(eventChord(key('BracketLeft', { ctrlKey: true, shiftKey: true }), 'windows'), 'cmd+shift+left_bracket');
  assert.equal(eventChord(key('ArrowUp', { altKey: true }), 'windows'), 'opt+up');
  assert.equal(eventChord(key('F5'), 'windows'), 'f5');
  assert.equal(eventChord(key('Delete'), 'windows'), 'forward_delete');
  assert.equal(eventChord({ code: 'IntlBackslash', key: '<', ctrlKey: true }, 'windows'), 'cmd+<');
  assert.equal(eventChord({ code: 'ControlLeft', key: 'Control', ctrlKey: true }, 'windows'), '');
  assert.equal(eventChord({ code: 'ShiftLeft', key: 'Shift', shiftKey: true }, 'macos'), '');
});

test('typed triggers normalise to the backend spelling', () => {
  assert.equal(normalizeTrigger('Shift + Command+T'), 'cmd+shift+t');
  assert.equal(normalizeTrigger('alt+ctrl+x'), 'ctrl+opt+x');
  assert.equal(normalizeTrigger('cmd++'), 'cmd++');
  assert.equal(normalizeTrigger('cmd+k>cmd+s'), 'cmd+k>cmd+s');
  assert.equal(normalizeTrigger('cmd+'), 'cmd');
});

test('key bindings filter by action name or by chord', () => {
  const row = { id: 'new_tab', title: 'New Tab', triggers: ['cmd+t'] };
  assert.ok(matchBinding(row, ''));
  assert.ok(matchBinding(row, 'new t'));
  assert.ok(matchBinding(row, 'new_tab'));
  assert.ok(matchBinding(row, 'cmd+t'));
  assert.ok(matchBinding(row, 'Command + T'));
  assert.ok(matchBinding(row, 'cmd+'));
  assert.ok(!matchBinding(row, 'cmd+w'));
  assert.ok(!matchBinding({ id: 'copy', title: 'Copy', triggers: [] }, 'cmd+t'));
  assert.ok(matchBinding({ id: 'split_up', title: 'Split Up', triggers: ['cmd+opt+shift+d'] }, 'shift+alt+cmd+d'));
});

test('debounce runs once with the last arguments', async () => {
  const calls = [];
  const run = debounce((value) => calls.push(value), 5);
  run(1); run(2); run(3);
  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.deepEqual(calls, [3]);
});

test('paths join with the separator the directory already uses', () => {
  assert.equal(joinPath('/work/otty', 'a.rs'), '/work/otty/a.rs');
  assert.equal(joinPath('/', 'etc'), '/etc');
  assert.equal(joinPath('E:\\work', 'a.rs'), 'E:\\work\\a.rs');
  assert.equal(joinPath('E:/work/', 'a.rs'), 'E:/work/a.rs');
});

test('binary bodies decode from base64 or raw chars and dump as hex', () => {
  assert.deepEqual([...bytesOf(Buffer.from([0, 255, 65]).toString('base64'))], [0, 255, 65]);
  assert.deepEqual([...bytesOf('\u0000\u00ff!')], [0, 255, 33]);
  const dump = hexDump(Uint8Array.from({ length: 18 }, (_, i) => 0x40 + i)).split('\n');
  assert.equal(dump[0], '00000000  40 41 42 43 44 45 46 47 48 49 4a 4b 4c 4d 4e 4f  @ABCDEFGHIJKLMNO');
  assert.equal(dump[1], `00000010  50 51${' '.repeat(42)}  PQ`);
  assert.equal(hexDump(new Uint8Array()), '');
});

test('"not implemented" reads as a quiet message, other errors pass through', () => {
  const t = (key, fallback) => fallback;
  assert.equal(errorText(new Error('not implemented'), t), 'Not available yet');
  assert.equal(errorText(new Error('no such key'), t), 'no such key');
});
