// Renderer core: the bridge bus, the feature contract (PROTOCOL.md "Renderer
// feature contract"), keyboard routing and chrome. tabs.js and panes.js draw.
const ime = document.querySelector('#ime-capture');
const notice = document.querySelector('#notice');
const SLOTS = ['overlay', 'right-panel', 'pane-overlay', 'titlebar-right', 'sidebar-foot'];

let platform = /Mac/.test(navigator.platform) ? 'macos' : /Win/.test(navigator.platform) ? 'windows' : 'linux';
let state = null;
let keymapList = [];
let keymapIndex = new Map();
let noticeTimer;
let nextMenuRun = 1;
const listeners = new Map();
const actions = new Map();
const menuRuns = new Map();
// captureKeys handlers, most recent last.
const keyCaptures = [];

function showNotice(message, sticky = false) {
  notice.textContent = message;
  notice.classList.remove('hidden');
  clearTimeout(noticeTimer);
  if (!sticky) noticeTimer = setTimeout(() => notice.classList.add('hidden'), 5000);
}

function report(error) {
  showNotice(error.message);
}

async function request(method, params = {}) {
  const reply = await window.otty.request(method, params);
  if (!reply.ok) throw new Error(reply.error);
  return reply.result;
}

function on(type, fn) {
  if (!listeners.has(type)) listeners.set(type, new Set());
  listeners.get(type).add(fn);
  return () => listeners.get(type).delete(fn);
}

function emit(event) {
  for (const fn of listeners.get(event.type) || []) {
    try {
      fn(event);
    } catch (error) {
      console.error(`[on:${event.type}]`, error);
    }
  }
}

// The local handler if one is registered, else the backend; rejects on failure.
function runAction(id) {
  const local = actions.get(id);
  return Promise.resolve().then(() => (local ? local() : request('action.run', { id })));
}

// runAction for the core's own keys, menus and buttons: a failure — "not
// implemented" included — is shown plainly.
function act(id) {
  return runAction(id).catch((error) => showNotice(`${id}: ${error.message}`));
}

// Native context menu. Items are `{label,action}` (as `pane.context` returns),
// `{label,run}` for a UI-only row, or null for a separator.
function showMenu(items) {
  menuRuns.clear();
  window.otty.popupMenu((items || []).map((item) => {
    if (!item || !item.run) return item;
    const action = `menu:${nextMenuRun}`;
    nextMenuRun += 1;
    menuRuns.set(action, item.run);
    return { label: item.label, action };
  }));
}

function mount(slot) {
  if (!SLOTS.includes(slot)) throw new Error(`Unknown mount slot: ${slot}`);
  const el = document.createElement('div');
  el.className = `mount mount-${slot}`;
  document.querySelector(`#slot-${slot}`).append(el);
  return el;
}

function focusTerminal() {
  ime.focus({ preventScroll: true });
}

// The i18n feature installs the table.
function t(key, fallback) {
  return globalThis.OttyStrings?.strings?.[key] ?? fallback;
}

const ctx = Object.freeze({
  request,
  on,
  state: () => state,
  focusedPane,
  registerAction: (id, fn) => void actions.set(id, fn),
  runAction,
  keymap: () => keymapList.slice(),
  mount,
  // A stack: fn pushes (or moves itself back to the top), null pops.
  captureKeys: (fn) => {
    if (typeof fn !== 'function') return void keyCaptures.pop();
    const at = keyCaptures.indexOf(fn);
    if (at >= 0) keyCaptures.splice(at, 1);
    keyCaptures.push(fn);
  },
  focusTerminal,
  t,
  get platform() {
    return platform;
  },
});

// One feature throwing must not break the others.
globalThis.OttyFeatures = {
  register(name, fn) {
    try {
      fn(ctx);
    } catch (error) {
      console.error(`[feature:${name}]`, error);
    }
  },
};

async function loadFeatures() {
  const { js, css } = await window.otty.features();
  for (const name of css) {
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = `features/${name}`;
    document.head.append(link);
  }
  await Promise.all(js.map((name) => new Promise((resolve) => {
    const script = document.createElement('script');
    script.src = `features/${name}`;
    script.async = false;
    script.onload = resolve;
    script.onerror = resolve;
    document.body.append(script);
  })));
}

async function refreshKeymap() {
  keymapList = await request('keymap.list');
  keymapIndex = OttyKeymap.buildKeymap(keymapList, platform === 'macos');
}

function color(value) {
  const hex = Number(value).toString(16).padStart(8, '0').slice(-8);
  const [red, green, blue, alpha] = [0, 2, 4, 6].map((at) => Number.parseInt(hex.slice(at, at + 2), 16));
  return alpha === 255 ? `#${hex.slice(0, 6)}` : `rgba(${red}, ${green}, ${blue}, ${(alpha / 255).toFixed(3)})`;
}

function luma(value) {
  if (value == null || !Number.isFinite(Number(value))) return 0;
  const hex = Number(value).toString(16).padStart(8, '0').slice(-8);
  const [red, green, blue] = [0, 2, 4].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255);
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

// A colour the theme leaves out falls back to the default in styles.css.
function applyChrome(chrome = {}) {
  const style = document.documentElement.style;
  const colors = {
    '--window-bg': 'window_background',
    '--terminal-bg': 'window_background',
    '--titlebar-bg': 'titlebar_background',
    '--titlebar-fg': 'titlebar_foreground',
    '--tab-bg': 'tab_background',
    '--tab-fg': 'tab_foreground',
    '--tab-active-bg': 'tab_active_background',
    '--tab-active-fg': 'tab_active_foreground',
    '--tab-hover-bg': 'tab_hover_background',
    '--tab-indicator': 'tab_indicator',
    '--accent': 'tab_indicator',
    '--sidebar-bg': 'sidebar_background',
    '--sidebar-fg': 'sidebar_foreground',
    '--sidebar-border': 'sidebar_border',
    '--divider': 'divider',
    '--overlay-bg': 'panel_background',
    '--overlay-fg': 'panel_foreground',
    '--overlay-border': 'panel_border',
    '--selection-bg': 'selection_background',
  };
  const lengths = {
    '--sidebar-width': 'sidebar_width',
    '--titlebar-height': 'titlebar_height',
    '--tab-height': 'tab_height',
    '--tab-radius': 'tab_radius',
    '--window-radius': 'window_radius',
    '--radius': 'panel_radius',
  };
  for (const [variable, field] of Object.entries(colors)) {
    if (chrome[field] == null) style.removeProperty(variable);
    else style.setProperty(variable, color(chrome[field]));
  }
  for (const [variable, field] of Object.entries(lengths)) {
    if (Number.isFinite(chrome[field])) style.setProperty(variable, `${chrome[field]}px`);
    else style.removeProperty(variable);
  }
  const text = chrome.tab_active_foreground ?? chrome.titlebar_foreground;
  const lightText = text == null || luma(text) > 0.55;
  style.setProperty('--icon-invert', lightText ? '1' : '0');
  style.setProperty('color-scheme', lightText ? 'dark' : 'light');
}

function dispatch(event) {
  if (event.type === 'ready') {
    platform = event.platform || platform;
    keymapIndex = OttyKeymap.buildKeymap(keymapList, platform === 'macos');
    applyChrome(event.chrome);
    setLayout(event.layout, event.sidebar_visible);
    notice.classList.add('hidden');
  } else if (event.type === 'state') {
    state = event;
    document.title = `${activeTab()?.title || 'Terminal'} — Otty`;
    renderTabs();
    renderPanes();
  } else if (event.type === 'frame') {
    paintFrame(event);
  } else if (event.type === 'chrome') {
    applyChrome(event.chrome);
  } else if (event.type === 'config') {
    refreshKeymap().catch(report);
  } else if (event.type === 'error') {
    showNotice(event.message, true);
  } else if (event.type === 'window-state') {
    document.querySelector('[data-window-action="toggle-maximize"]')
      .setAttribute('aria-label', event.maximized ? 'Restore' : 'Maximize');
  }
  emit(event);
}

// Keyboard: capture handler, then keymap, then the focused pane.

const namedKeys = new Map([
  ['Enter', '\n'], ['Backspace', '\b'], ['Tab', '\t'], ['Escape', '\x1b'],
  ['ArrowUp', ''], ['ArrowDown', ''], ['ArrowLeft', ''], ['ArrowRight', ''],
  ['Home', ''], ['End', ''], ['PageUp', ''], ['PageDown', ''], ['Delete', ''],
]);
for (let index = 1; index <= 12; index += 1) {
  namedKeys.set(`F${index}`, String.fromCodePoint(0xF704 + index - 1));
}

function controlCharacter(key) {
  if (key === ' ') return '\0';
  if (key === '?') return '\x7f';
  const code = key.toUpperCase().charCodeAt(0);
  return key.length === 1 && code >= 0x40 && code <= 0x5f ? String.fromCharCode(code & 0x1f) : null;
}

// Key text as TerminalView::handle_key takes it, or null when the key is
// plain text (which arrives through the IME textarea instead).
function encodeKey(event) {
  if (event.metaKey) return null;
  // Ctrl+Alt is AltGr on Windows layouts; that is text, not a control key.
  if (event.ctrlKey && !event.altKey) return controlCharacter(event.key);
  const named = namedKeys.get(event.key);
  if (named !== undefined) return named;
  if (event.altKey && !event.ctrlKey && event.key.length === 1) return `\x1b${event.key}`;
  return null;
}

function input(text) {
  const pane = focusedPane();
  if (!pane) return;
  clearSelection();
  window.otty.send({ type: 'input', pane, text });
}

// Project switching is this design's own, so it has no action id to bind:
// primary+Shift+digit picks a project, primary+Alt+[ / ] steps through them.
function projectShortcut(event) {
  const primary = platform === 'macos' ? event.metaKey : event.ctrlKey;
  if (!primary) return false;
  const current = projects.currentFolder(folders, state?.activeTab);
  const digit = /^Digit([1-9])$/.exec(event.code)?.[1];
  let folder = null;
  if (digit && event.shiftKey && !event.altKey) folder = projects.itemAtShortcut(folders, digit);
  else if (event.altKey && event.code === 'BracketLeft') folder = projects.relativeItem(folders, current?.key, -1, 'key');
  else if (event.altKey && event.code === 'BracketRight') folder = projects.relativeItem(folders, current?.key, 1, 'key');
  if (folder) activateFolder(folder);
  return Boolean(folder);
}

function onKeyDown(event) {
  const consume = () => {
    event.preventDefault();
    event.stopPropagation();
  };
  // Capture handlers get first refusal, newest first; one that takes the key
  // also owns its default action.
  for (const capture of keyCaptures.toReversed()) {
    try {
      if (capture(event) === true) return;
    } catch (error) {
      console.error('[captureKeys]', error);
    }
  }
  const inTerminal = event.target === ime || event.target === document.body;
  // A feature's own text field edits natively.
  if (!inTerminal && event.target.matches?.('input, textarea, select, [contenteditable]')) return;
  if (imeInput.isComposing() || event.isComposing || event.key === 'Process') return;
  const chord = event.getModifierState('AltGraph') ? '' : OttyKeymap.eventChord(event);
  const action = OttyKeymap.lookup(keymapIndex, chord, Boolean(selection));
  if (action) {
    consume();
    act(action);
    return;
  }
  if (projectShortcut(event)) {
    consume();
    return;
  }
  if (!inTerminal) return;
  focusTerminal();
  const text = encodeKey(event);
  if (text !== null) {
    consume();
    input(text);
  }
}

const imeInput = OttyImeInput.createImeInput(ime, input);
ime.addEventListener('paste', (event) => {
  event.preventDefault();
  paste(event.clipboardData?.getData('text/plain'));
});

function paste(text) {
  const pane = focusedPane();
  if (text && pane) window.otty.send({ type: 'paste', pane, text });
}

// Actions the UI owns. A feature may take any of them over with registerAction.

function stepSession(direction) {
  const next = projects.relativeItem(sessionTabs(), state?.activeTab, direction);
  if (next) activateTab(next.id);
}

// ponytail: one bridge per window, so the session cannot follow the tab
// (PROTOCOL.md "Ceilings"); this opens a window at the tab's cwd and leaves the
// tab where it is. Move the pane tree once the backend can hand a tab over.
function moveTabToNewWindow(tab) {
  if (tab) window.otty.newWindow(tab.cwd);
}

actions.set('copy', async () => {
  const text = await selectedText();
  if (text) window.otty.writeClipboard(text);
  else await request('action.run', { id: 'copy' });
});
actions.set('paste', async () => paste(await window.otty.readClipboard()));
actions.set('new_window', () => window.otty.newWindow());
actions.set('close_window', () => window.otty.windowAction('close'));
actions.set('minimize', () => window.otty.windowAction('minimize'));
actions.set('toggle_full_screen', () => window.otty.windowAction('toggle-full-screen'));
actions.set('toggle_tabs_panel', () => setLayout(sidebarLayout ? 'sidebar-left' : 'tabs', !panelVisible));
actions.set('rename_tab', () => startRename(state?.activeTab));
actions.set('move_tab_to_new_window', () => moveTabToNewWindow(activeTab()));
// The strip shows the current project's sessions, so tab stepping stays inside it.
actions.set('next_tab', () => stepSession(1));
actions.set('previous_tab', () => stepSession(-1));
for (let n = 1; n <= 9; n += 1) {
  actions.set(`goto_tab_${n}`, () => {
    const tab = projects.itemAtShortcut(sessionTabs(), String(n));
    if (tab) activateTab(tab.id);
  });
}

window.addEventListener('keydown', onKeyDown, true);
window.addEventListener('focus', () => {
  if (document.activeElement === document.body) focusTerminal();
});
for (const button of document.querySelectorAll('[data-window-action]')) {
  button.addEventListener('click', () => window.otty.windowAction(button.dataset.windowAction));
}
document.querySelector('#app-menu').addEventListener('click', () => window.otty.appMenu());
document.querySelector('#new-tab-top').addEventListener('click', () => act('new_tab'));
document.querySelector('#new-tab-side').addEventListener('click', async () => {
  const cwd = await window.otty.openProject();
  if (cwd) request('tab.new', { cwd }).catch(report);
});
document.querySelector('#sidebar-toggle').addEventListener('click', () => act('toggle_tabs_panel'));
document.querySelector('#titlebar').addEventListener('dblclick', (event) => {
  if (!event.target.closest('button, .tab, .mount')) window.otty.windowAction('toggle-maximize');
});

const panelResizer = document.querySelector('#right-panel-resizer');
panelResizer.addEventListener('pointerdown', (event) => {
  event.preventDefault();
  panelResizer.setPointerCapture(event.pointerId);
});
panelResizer.addEventListener('pointermove', (event) => {
  if (!panelResizer.hasPointerCapture(event.pointerId)) return;
  document.documentElement.style.setProperty('--right-panel-width', `${window.innerWidth - event.clientX}px`);
});

window.otty.onAction((id) => {
  const run = menuRuns.get(id);
  if (run) Promise.resolve().then(run).catch(report);
  else act(id);
});

// Events wait until the features are in, so each one sees `ready` and the
// first `state`.
const backlog = [];
let booted = false;
window.otty.onEvent((event) => (booted ? dispatch(event) : backlog.push(event)));

(async () => {
  await refreshKeymap().catch(() => {});
  await loadFeatures().catch((error) => console.error('[features]', error));
  booted = true;
  for (const event of backlog.splice(0)) dispatch(event);
  focusTerminal();
})();
