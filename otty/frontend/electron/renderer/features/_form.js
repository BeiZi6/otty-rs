// Helpers shared by the feature modules. Nothing here touches the DOM at load.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OttyForm = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  // The original Settings order (GAPS S03); a group ranks by the first word it contains.
  const GROUP_ORDER = ['font', 'cursor', 'shell', 'vt', 'scrollback', 'restore', 'window',
    'transparency', 'theme', 'color', 'keybind', 'key binding', 'agent', 'recipe'];

  function groupRank(group) {
    const name = String(group).toLowerCase();
    const index = GROUP_ORDER.findIndex((word) => name.includes(word));
    return index < 0 ? GROUP_ORDER.length : index;
  }

  // Unique groups, known ones first in the original order, the rest as given.
  function orderGroups(groups) {
    return [...new Set(groups)]
      .map((group, index) => ({ group, index, rank: groupRank(group) }))
      .sort((a, b) => a.rank - b.rank || a.index - b.index)
      .map((item) => item.group);
  }

  function controlType(spec) {
    const type = String((spec && spec.type) || '').toLowerCase();
    if (type === 'bool' || type === 'boolean') return 'bool';
    if (spec && Array.isArray(spec.values) && spec.values.length) return 'enum';
    if (['number', 'int', 'integer', 'float'].includes(type)) return 'number';
    if (type === 'color') return 'color';
    if (['list', 'repeated', 'pairs', 'rule'].includes(type)) return 'list';
    return 'text';
  }

  // Spellings follow otty_core::key (`Modifiers::canonical`, `NamedKey::name`).
  const MODS = ['cmd', 'ctrl', 'opt', 'shift', 'hyper', 'meta'];
  const MOD_ALIAS = {
    control: 'ctrl', alt: 'opt', option: 'opt', command: 'cmd', super: 'cmd', cmdorctrl: 'cmd',
  };
  const CODE_KEYS = {
    Enter: 'enter', NumpadEnter: 'enter', Tab: 'tab', Backspace: 'backspace', Escape: 'escape',
    Space: 'space', Delete: 'forward_delete', Insert: 'insert', Home: 'home', End: 'end',
    PageUp: 'page_up', PageDown: 'page_down', ArrowUp: 'up', ArrowDown: 'down',
    ArrowLeft: 'left', ArrowRight: 'right', Comma: 'comma', Period: 'period', Slash: 'slash',
    Backslash: 'backslash', Semicolon: 'semicolon', Quote: 'quote', Backquote: 'grave',
    Minus: 'minus', Equal: 'equal', BracketLeft: 'left_bracket', BracketRight: 'right_bracket',
  };

  // ponytail: keys are read by position (event.code), so a non-QWERTY layout
  // binds the QWERTY letter; switch to event.key when someone needs it.
  function keyName(event) {
    const code = event.code || '';
    const simple = /^(?:Key([A-Z])|Digit(\d)|(F\d{1,2}))$/.exec(code);
    if (simple) return (simple[1] || simple[2] || simple[3]).toLowerCase();
    if (CODE_KEYS[code]) return CODE_KEYS[code];
    const key = event.key || '';
    return key.length === 1 && key !== ' ' ? key.toLowerCase() : '';
  }

  // KeyboardEvent → `cmd+shift+t`; '' while only modifiers are down. `cmd` is
  // ⌘ on macOS and Ctrl elsewhere, as in PROTOCOL.md.
  function eventChord(event, platform) {
    const key = keyName(event);
    if (!key) return '';
    const mac = platform === 'macos';
    const mods = [];
    if (mac ? event.metaKey : event.ctrlKey) mods.push('cmd');
    if (mac && event.ctrlKey) mods.push('ctrl');
    if (event.altKey) mods.push('opt');
    if (event.shiftKey) mods.push('shift');
    return [...mods, key].join('+');
  }

  // Typed text → the canonical trigger: `Shift + Command+T` → `cmd+shift+t`.
  function normalizeTrigger(text) {
    return String(text).toLowerCase().replace(/\s+/g, '').split('>').map((chord) => {
      const mods = new Set();
      let key = '';
      for (const part of chord.split('+')) {
        const name = MOD_ALIAS[part] || part;
        if (MODS.includes(name)) mods.add(name);
        else key = name || (chord.endsWith('++') ? '+' : key);
      }
      return [...MODS.filter((mod) => mods.has(mod)), key].filter(Boolean).join('+');
    }).join('>');
  }

  // A key-binding row matches by action name / id, or by a (partial) chord.
  function matchBinding(row, query) {
    const text = String(query || '').toLowerCase().trim();
    if (!text) return true;
    if (`${row.title || ''}\n${row.id || ''}`.toLowerCase().includes(text)) return true;
    const chord = normalizeTrigger(text);
    const loose = text.replace(/\s+/g, '');
    return (row.triggers || []).some((trigger) => trigger.includes(loose) || (chord && trigger.includes(chord)));
  }

  function debounce(fn, ms) {
    let timer;
    return (...args) => {
      clearTimeout(timer);
      timer = setTimeout(() => fn(...args), ms);
    };
  }

  function joinPath(dir, name) {
    const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
    return dir.endsWith(sep) ? dir + name : dir + sep + name;
  }

  // A binary `file.read` body: base64 if it decodes, else one byte per char.
  function bytesOf(body) {
    let raw = String(body);
    try { raw = atob(raw); } catch { /* not base64 */ }
    return Uint8Array.from(raw, (char) => char.charCodeAt(0) & 0xff);
  }

  function hexDump(bytes) {
    const lines = [];
    for (let at = 0; at < bytes.length; at += 16) {
      const row = [...bytes.slice(at, at + 16)];
      const hex = row.map((byte) => byte.toString(16).padStart(2, '0')).join(' ');
      const text = row.map((byte) => (byte >= 0x20 && byte < 0x7f ? String.fromCharCode(byte) : '.')).join('');
      lines.push(`${at.toString(16).padStart(8, '0')}  ${hex.padEnd(47)}  ${text}`);
    }
    return lines.join('\n');
  }

  // "not implemented" is an expected answer while the backend grows.
  function errorText(error, t) {
    const message = String((error && error.message) || error || '');
    return message === 'not implemented' ? t('common.not_implemented', 'Not available yet') : message;
  }

  // h('button', {class, onclick, 'aria-label'}, ...children)
  function h(tag, props, ...children) {
    const node = document.createElement(tag);
    for (const [name, value] of Object.entries(props || {})) {
      if (name === 'class') node.className = value;
      else if (name.startsWith('on')) node.addEventListener(name.slice(2), value);
      else if (name in node) node[name] = value;
      else node.setAttribute(name, value);
    }
    node.append(...children.flat(Infinity).filter((child) => child != null && child !== false));
    return node;
  }

  return {
    orderGroups, controlType, eventChord, normalizeTrigger, matchBinding,
    debounce, joinPath, bytesOf, hexDump, errorText, h,
  };
});
