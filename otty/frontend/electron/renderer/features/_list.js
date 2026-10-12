// Shared by the overlay features: fuzzy highlight, list navigation, debounce,
// keymap chords, and the one list overlay behind the palette and Open Quickly.
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OttyList = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  // Indices of `query` in `text`: the contiguous hit when there is one, else a
  // subsequence. `[]` for an empty query, `null` for no match.
  function fuzzy(text, query) {
    const hay = String(text).toLowerCase();
    const needle = String(query || '').toLowerCase();
    const at = hay.indexOf(needle);
    if (at >= 0) return Array.from({ length: needle.length }, (_, i) => at + i);
    const out = [];
    let from = 0;
    for (const ch of needle.replace(/\s/g, '')) {
      from = hay.indexOf(ch, from);
      if (from < 0) return null;
      out.push(from);
      from += ch.length;
    }
    return out;
  }

  // `text` cut into `[piece, matched]` runs.
  function segments(text, query) {
    const value = String(text);
    const hit = new Set(fuzzy(value, query) || []);
    const out = [];
    for (let i = 0; i < value.length; i += 1) {
      const last = out[out.length - 1];
      if (last && last[1] === hit.has(i)) last[0] += value[i];
      else out.push([value[i], hit.has(i)]);
    }
    return out;
  }

  // Next selected index for a navigation key; `null` when `key` is not one.
  function move(index, length, key, page = 10) {
    const next = {
      ArrowDown: (index + 1) % length,
      ArrowUp: (index - 1 + length) % length,
      PageDown: Math.min(index + page, length - 1),
      PageUp: Math.max(index - page, 0),
      Home: 0,
      End: length - 1,
    }[key];
    if (next === undefined) return null;
    return length > 0 ? next : -1;
  }

  function debounce(fn, ms) {
    let timer;
    const run = (...args) => {
      clearTimeout(timer);
      timer = setTimeout(fn, ms, ...args);
    };
    run.cancel = () => clearTimeout(timer);
    return run;
  }

  // The first row matching `pred`, moved to the top.
  function pinFirst(rows, pred) {
    const at = rows.findIndex(pred);
    return at > 0 ? [rows[at], ...rows.slice(0, at), ...rows.slice(at + 1)] : rows;
  }

  const MODS = { cmd: 'cmd', command: 'cmd', super: 'cmd', ctrl: 'ctrl', control: 'ctrl', opt: 'alt', option: 'alt', alt: 'alt', shift: 'shift' };
  const CODES = {
    Comma: 'comma', BracketLeft: 'left_bracket', BracketRight: 'right_bracket', Equal: 'equal', Minus: 'minus',
    Slash: 'slash', Space: 'space', ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right',
    PageUp: 'page_up', PageDown: 'page_down', Delete: 'forward_delete',
  };

  function canonical(mods, key) {
    return [...new Set(mods)].sort().concat(key).join('+');
  }

  // A keymap trigger (`cmd+shift+p`) in the form `chord` produces. `cmd` is ⌘ on macOS, Ctrl elsewhere.
  function trigger(text, mac) {
    const parts = String(text).toLowerCase().split('+');
    const key = parts.pop();
    return canonical(parts.map((part) => (MODS[part] === 'cmd' ? (mac ? 'meta' : 'ctrl') : MODS[part] || part)), key);
  }

  function chord(event, mac) {
    const code = event.code || '';
    const key = /^(Key|Digit).$/.test(code) ? code.slice(-1).toLowerCase() : CODES[code] || String(event.key).toLowerCase();
    const mods = [event.metaKey && 'meta', event.ctrlKey && 'ctrl', event.altKey && 'alt', event.shiftKey && 'shift'];
    return canonical(mods.filter(Boolean), key);
  }

  // The action `keymap` binds to this keydown, if any. Multi-chord triggers never match.
  function actionFor(keymap, event, mac) {
    const want = chord(event, mac);
    const hit = (keymap || []).find((binding) => trigger(binding.trigger, mac) === want);
    return hit ? hit.action : undefined;
  }

  const GLYPHS = { cmd: '⌘', shift: '⇧', alt: '⌥', ctrl: '⌃', enter: '↩', up: '↑', down: '↓', left: '←', right: '→' };

  // `cmd+shift+p` → `⌘⇧P` on macOS, `Ctrl+Shift+P` elsewhere.
  function shortcutLabel(text, mac) {
    const parts = String(text || '').toLowerCase().split('+').map((part) => MODS[part] || part);
    if (mac) return parts.map((part) => GLYPHS[part] || part.toUpperCase()).join('');
    // Off macOS cmd is Ctrl; a trigger holding both becomes Ctrl+Alt, as the
    // core's keymap matches it.
    const both = parts.includes('cmd') && parts.includes('ctrl');
    return parts
      .map((part) => (part === 'cmd' ? 'ctrl' : both && part === 'ctrl' ? 'alt' : part))
      .map((part) => part.replace(/(^|_)(.)/g, (_, gap, ch) => (gap && ' ') + ch.toUpperCase()))
      .join('+');
  }

  // What `mode.key` is sent: `event.key` as typed ("G", "$", "Escape") behind the
  // held ctrl/alt/cmd — and shift, for named keys only ("shift+ArrowUp").
  function keyName(event) {
    const named = event.key.length > 1;
    return [event.ctrlKey && 'ctrl', event.altKey && 'alt', event.metaKey && 'cmd', named && event.shiftKey && 'shift', event.key]
      .filter(Boolean)
      .join('+');
  }

  // --- DOM from here on; nothing below runs at load time. ---

  function h(tag, props, ...kids) {
    const el = document.createElement(tag);
    for (const [name, value] of Object.entries(props || {})) {
      if (name === 'class') el.className = value;
      else if (name.startsWith('on')) el.addEventListener(name.slice(2), value);
      else if (value != null && value !== false) el.setAttribute(name, value);
    }
    el.append(...kids.flat(Infinity).filter((kid) => kid != null && kid !== false && kid !== ''));
    return el;
  }

  // `text` as nodes, the part matching `query` wrapped in <mark class=cls>.
  function marked(text, query, cls) {
    return segments(text, query).map(([piece, hit]) => (hit ? h('mark', { class: cls }, piece) : piece));
  }

  // A keydown handler usable both as the `ctx.captureKeys` callback and as an
  // element listener: each event reaches `fn` once, whichever route delivers it
  // first. `fn` returning `false` declines the key; so does focus being outside
  // `within()`. Keys the IME is composing are left alone.
  const seen = new WeakMap();
  function keys(fn, within) {
    return (event) => {
      if (within) {
        const el = within();
        if (!el || !el.contains(document.activeElement)) return false;
      }
      if (event.isComposing || event.keyCode === 229) return true;
      if (!seen.has(event)) seen.set(event, fn(event) !== false);
      return seen.get(event);
    };
  }

  // Input + listbox in the `overlay` slot. `o`:
  //   name          class prefix: `.name`, `.name-input`, `.name-list`, `.name-row`, `.name-status`
  //   label         dialog name
  //   actions       action ids this feature owns; their keymap chords still work while open
  //   placeholder() input placeholder, re-read on every load
  //   load(query)   → rows or a Promise of them
  //   row(item, query) → nodes for one row
  //   run(item, event) on ↩ or click
  //   key(event, item) → true when the feature consumed the key (optional)
  //   head()        → a node between input and list (optional)
  function picker(ctx, o) {
    const mac = ctx.platform === 'macos';
    let host = null;
    let input;
    let list;
    let status;
    let rows = [];
    let selected = 0;
    let serial = 0;

    function say(text) {
      if (host) status.textContent = text || '';
    }

    function select(index) {
      selected = index;
      Array.from(list.children).forEach((el, n) => el.setAttribute('aria-selected', String(n === index)));
      const el = list.children[index];
      if (!el) return input.removeAttribute('aria-activedescendant');
      input.setAttribute('aria-activedescendant', el.id);
      if (el.scrollIntoView) el.scrollIntoView({ block: 'nearest' });
    }

    function show(found, query, message) {
      rows = found;
      list.replaceChildren(...rows.map((item, n) => h('div', {
        class: `${o.name}-row`,
        role: 'option',
        id: `${o.name}-row-${n}`,
        // mousedown, not click: the input keeps focus.
        onmousedown: (event) => { event.preventDefault(); select(n); o.run(item, event); },
      }, o.row(item, query))));
      select(0);
      say(message);
    }

    function refresh() {
      const query = input.value;
      const mine = ++serial;
      input.placeholder = o.placeholder();
      return Promise.resolve().then(() => o.load(query)).then(
        (found) => {
          if (host && mine === serial) show(found || [], query, found && found.length ? '' : ctx.t('list.empty', 'No results'));
        },
        (err) => { if (host && mine === serial) show([], query, err.message); },
      );
    }
    const lazy = debounce(refresh, 120);

    const onKey = keys((event) => {
      const mods = event.shiftKey || event.altKey || event.ctrlKey || event.metaKey;
      const own = actionFor(ctx.keymap(), event, mac);
      // ponytail: a fixed 10-row page; measure the list if rows ever vary in height.
      const next = mods ? null : move(selected, rows.length, event.key);
      if (o.key && o.key(event, rows[selected])) event.preventDefault();
      else if (event.key === 'Escape') close();
      else if (event.key === 'Tab') event.preventDefault(); // modal: focus stays in the input
      else if (event.key === 'Enter') { event.preventDefault(); if (rows[selected]) o.run(rows[selected], event); }
      else if (o.actions.includes(own)) { event.preventDefault(); ctx.runAction(own); }
      else if (next != null) { event.preventDefault(); select(next); }
    });

    function grab() {
      ctx.captureKeys(onKey);
      input.focus();
    }

    function open(query = '') {
      if (!host) {
        host = ctx.mount('overlay');
        const listId = `${o.name}-list`;
        input = h('input', {
          class: `${o.name}-input`, type: 'text', role: 'combobox', spellcheck: 'false', autocomplete: 'off',
          'aria-label': o.label, 'aria-expanded': 'true', 'aria-controls': listId, 'aria-autocomplete': 'list',
          oninput: lazy,
        });
        list = h('div', { class: `${o.name}-list`, id: listId, role: 'listbox', 'aria-label': o.label });
        status = h('div', { class: `${o.name}-status`, role: 'status' });
        host.append(h('div', {
          class: o.name, role: 'dialog', 'aria-modal': 'true', 'aria-label': o.label,
          onkeydown: (event) => { onKey(event); event.stopPropagation(); },
        }, input, o.head && o.head(), list, status));
        host.addEventListener('mousedown', (event) => { if (event.target === host) close(); });
      }
      input.value = query;
      grab();
      return refresh();
    }

    function close() {
      if (!host) return;
      serial += 1;
      lazy.cancel();
      host.remove();
      host = null;
      rows = [];
      ctx.captureKeys(null);
      ctx.focusTerminal();
    }

    // Run `fn` for a chosen row. The overlay closes first (unless `keep`) so an
    // action that opens another overlay is not clobbered by our close; a
    // rejection brings it back with the message.
    function exec(fn, keep) {
      const query = host ? input.value : '';
      if (!keep) close();
      return Promise.resolve().then(fn).then(
        () => { if (keep && host) grab(); },
        (err) => open(query).then(() => say(err.message)),
      );
    }

    return { open, close, refresh, exec, say, isOpen: () => !!host };
  }

  return { fuzzy, segments, move, debounce, pinFirst, trigger, chord, actionFor, shortcutLabel, keyName, h, marked, keys, picker };
});
