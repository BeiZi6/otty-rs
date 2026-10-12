(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OttyKeymap = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  const MODS = ['ctrl', 'alt', 'shift', 'cmd'];
  const ALIAS = { opt: 'alt', option: 'alt', control: 'ctrl', command: 'cmd', super: 'cmd', meta: 'cmd' };
  // Readline and job-control keys. Off macOS `cmd` folds onto Ctrl, and these
  // must still reach the shell (PROTOCOL.md "Keyboard").
  const SHELL_KEYS = new Set('acdeklruz');

  const CODES = {
    Comma: 'comma', Period: 'period', Slash: 'slash', Backslash: 'backslash', Semicolon: 'semicolon',
    Quote: 'quote', Backquote: 'grave', BracketLeft: 'left_bracket', BracketRight: 'right_bracket',
    Equal: 'equal', Minus: 'minus', Space: 'space', Enter: 'enter', NumpadEnter: 'enter', Tab: 'tab',
    Escape: 'escape', Backspace: 'backspace', Delete: 'forward_delete', Insert: 'insert',
    ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    PageUp: 'page_up', PageDown: 'page_down', Home: 'home', End: 'end',
  };
  const ACCELERATOR_KEYS = {
    comma: ',', period: '.', slash: '/', backslash: '\\', semicolon: ';', quote: "'", grave: '`',
    left_bracket: '[', right_bracket: ']', equal: '=', minus: '-', space: 'Space', enter: 'Enter',
    tab: 'Tab', escape: 'Esc', backspace: 'Backspace', forward_delete: 'Delete', insert: 'Insert',
    up: 'Up', down: 'Down', left: 'Left', right: 'Right',
    page_up: 'PageUp', page_down: 'PageDown', home: 'Home', end: 'End',
  };
  const ACCELERATOR_MODS = { ctrl: 'Ctrl', alt: 'Alt', shift: 'Shift', cmd: 'Cmd' };

  // `cmd+shift+p` → canonical `ctrl+shift+p` (or `shift+cmd+p` on macOS).
  // Off macOS a trigger holding both cmd and ctrl becomes ctrl+alt.
  function normalize(trigger, mac) {
    const parts = String(trigger).toLowerCase().split('+').map((part) => ALIAS[part] || part);
    const key = parts.pop();
    const mods = new Set(parts);
    if (!mac && mods.delete('cmd')) mods.add(mods.has('ctrl') ? 'alt' : 'ctrl');
    return [...MODS.filter((mod) => mods.has(mod)), key].join('+');
  }

  // Physical key, so a chord does not move with the keyboard layout.
  function keyName(event) {
    const code = event.code || '';
    const single = /^(?:Key|Digit)(.)$/.exec(code);
    if (single) return single[1].toLowerCase();
    if (/^F\d+$/.test(code)) return code.toLowerCase();
    return CODES[code] || '';
  }

  function eventChord(event) {
    const key = keyName(event);
    if (!key) return '';
    const held = { ctrl: event.ctrlKey, alt: event.altKey, shift: event.shiftKey, cmd: event.metaKey };
    return [...MODS.filter((mod) => held[mod]), key].join('+');
  }

  // First binding of a chord wins, as in the backend's lookup.
  function buildKeymap(list, mac) {
    const map = new Map();
    for (const { trigger, action } of list || []) {
      const chord = normalize(trigger, mac);
      if (!map.has(chord)) map.set(chord, action);
    }
    return map;
  }

  function lookup(map, chord, hasSelection) {
    const action = chord ? map.get(chord) : undefined;
    if (!action) return null;
    const control = /^ctrl\+([a-z])$/.exec(chord);
    if (control && SHELL_KEYS.has(control[1]) && !(action === 'copy' && hasSelection)) return null;
    return action;
  }

  // Electron accelerator for display in the native menu, or undefined.
  function accelerator(trigger, mac) {
    const parts = normalize(trigger, mac).split('+');
    const key = parts.pop();
    const name = ACCELERATOR_KEYS[key] || (/^(?:[a-z0-9]|f\d{1,2})$/.test(key) ? key.toUpperCase() : '');
    if (!name || parts.some((mod) => !ACCELERATOR_MODS[mod])) return undefined;
    return [...parts.map((mod) => ACCELERATOR_MODS[mod]), name].join('+');
  }

  return { normalize, eventChord, buildKeymap, lookup, accelerator };
});
