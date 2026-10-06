const appShell = document.querySelector('.app-shell');
const terminalSurface = document.querySelector('#terminal-surface');
const canvas = document.querySelector('#terminal-grid');
const context = canvas.getContext('2d', { alpha: false });
const ime = document.querySelector('#ime-capture');
const imeInput = OttyImeInput.createImeInput(ime, (text) => {
  if (activeTabId) window.otty.input(activeTabId, text);
});
const errorBox = document.querySelector('#engine-error');
const tabList = document.querySelector('#tab-list');
const sidebarTabs = document.querySelector('#sidebar-tabs');
const resizeObserver = new ResizeObserver(scheduleResize);
const isMacOS = /Mac|iPhone|iPad/.test(`${navigator.platform} ${navigator.userAgent}`);

let resizeTimer;
let lastResize = '';
const projects = globalThis.OttyProjects;
let tabs = [];
let folders = [];
let activeTabId = null;
let panelVisible = true;
let draggedTabId = null;
let sidebarLayout = true;
const lastTabByFolder = new Map();

function setText(selector, value) {
  document.querySelector(selector).textContent = value;
}

function color(value, fallback) {
  if (value == null) return fallback;
  const hex = Number(value).toString(16).padStart(8, '0').slice(-8);
  const red = Number.parseInt(hex.slice(0, 2), 16);
  const green = Number.parseInt(hex.slice(2, 4), 16);
  const blue = Number.parseInt(hex.slice(4, 6), 16);
  const alpha = Number.parseInt(hex.slice(6, 8), 16) / 255;
  return alpha >= 0.999 ? `#${hex.slice(0, 6)}` : `rgba(${red}, ${green}, ${blue}, ${alpha.toFixed(3)})`;
}

function luma(value) {
  if (value == null || !Number.isFinite(Number(value))) return 0;
  const hex = Number(value).toString(16).padStart(8, '0').slice(-8);
  const red = Number.parseInt(hex.slice(0, 2), 16) / 255;
  const green = Number.parseInt(hex.slice(2, 4), 16) / 255;
  const blue = Number.parseInt(hex.slice(4, 6), 16) / 255;
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function applyChrome(chrome = {}) {
  const style = document.documentElement.style;
  const fields = [
    ['window_background', '--window-bg', '#1c1c1e'],
    ['titlebar_background', '--titlebar-bg', '#1c1c1e'],
    ['titlebar_foreground', '--titlebar-fg', '#eeeeee'],
    ['tabbar_background', '--tabbar-bg', '#1c1c1e'],
    ['tab_background', '--tab-bg', '#252527'],
    ['tab_foreground', '--tab-fg', '#99999a'],
    ['tab_active_background', '--tab-active-bg', '#303033'],
    ['tab_active_foreground', '--tab-active-fg', '#eeeeee'],
    ['tab_hover_background', '--tab-hover-bg', '#38383b'],
    ['tab_indicator', '--tab-indicator', '#0a84ff'],
    ['sidebar_background', '--sidebar-bg', '#29292b'],
    ['sidebar_foreground', '--sidebar-fg', '#d5d5d6'],
    ['sidebar_border', '--sidebar-border', 'rgba(255,255,255,.08)'],
    ['divider', '--divider', 'rgba(255,255,255,.12)'],
  ];
  for (const [field, variable, fallback] of fields) {
    style.setProperty(variable, color(chrome[field], fallback));
  }
  style.setProperty('--terminal-bg', color(chrome.window_background, '#111315'));
  const lightText = luma(chrome.tab_active_foreground ?? chrome.titlebar_foreground) > 0.55;
  style.setProperty('--icon-invert', lightText ? '1' : '0');
  style.setProperty('color-scheme', lightText ? 'dark' : 'light');
  if (Number.isFinite(chrome.sidebar_width)) style.setProperty('--sidebar-width', `${chrome.sidebar_width}px`);
  if (Number.isFinite(chrome.titlebar_height)) style.setProperty('--titlebar-height', `${chrome.titlebar_height}px`);
  if (Number.isFinite(chrome.tab_height)) style.setProperty('--tab-height', `${chrome.tab_height}px`);
  if (Number.isFinite(chrome.tab_radius)) style.setProperty('--tab-radius', `${chrome.tab_radius}px`);
  if (Number.isFinite(chrome.window_radius)) style.setProperty('--window-radius', `${chrome.window_radius}px`);
}

function setTitle(title) {
  const value = title || 'Terminal';
  setText('#window-title', value);
  document.title = `${value} — Otty`;
}

function setPath(path) {
  if (path) setText('#status-path', path);
}

function setLayout(layout, visible) {
  const layoutType = layout || 'sidebar-left';
  panelVisible = visible !== false;
  sidebarLayout = layoutType.startsWith('sidebar');
  appShell.classList.toggle('layout-sidebar', sidebarLayout);
  appShell.classList.toggle('layout-tabs', !sidebarLayout);
  appShell.classList.toggle('tabs-panel-hidden', !panelVisible);
  const label = panelVisible ? 'Hide projects panel' : 'Show projects panel';
  const toggle = document.querySelector('#sidebar-toggle');
  toggle.setAttribute('aria-label', label);
  toggle.setAttribute('aria-expanded', String(panelVisible));
  toggle.title = label;
  scheduleResize();
}

function chord(n, shift = false) {
  const primary = isMacOS ? '⌘' : 'Ctrl';
  return shift ? `${primary}+Shift+${n}` : `${primary}+${n}`;
}

function sessionTabs() {
  if (!sidebarLayout) return tabs;
  const folder = projects.currentFolder(folders, activeTabId);
  return folder ? folder.tabs : tabs;
}

function activateFolder(folder) {
  if (!folder || !folder.tabs.length) return;
  const last = lastTabByFolder.get(folder.key);
  const tab = folder.tabs.find((item) => item.id === last) || folder.tabs[0];
  if (tab) window.otty.activateTab(tab.id);
}

function bindTabDrag(item, tab, vertical) {
  item.draggable = true;
  item.addEventListener('dragstart', (event) => {
    draggedTabId = tab.id;
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', tab.id);
    item.classList.add('dragging');
  });
  item.addEventListener('dragend', () => {
    draggedTabId = null;
    item.classList.remove('dragging');
    document.querySelectorAll('.drop-target').forEach((node) => node.classList.remove('drop-target'));
  });
  item.addEventListener('dragover', (event) => {
    if (draggedTabId && draggedTabId !== tab.id) {
      event.preventDefault();
      item.classList.add('drop-target');
    }
  });
  item.addEventListener('dragleave', () => item.classList.remove('drop-target'));
  item.addEventListener('drop', (event) => {
    event.preventDefault();
    event.stopPropagation();
    item.classList.remove('drop-target');
    const rect = item.getBoundingClientRect();
    const after = vertical
      ? event.clientY >= rect.top + rect.height / 2
      : event.clientX >= rect.left + rect.width / 2;
    window.otty.moveTab(draggedTabId || event.dataTransfer.getData('text/plain'), tab.id, after);
  });
}

function tabIconSrc(name) {
  const stem = /^[a-z0-9-]+$/.test(name || '') ? name : 'terminal';
  return `../../../resources/tab-icons/${stem}.svg`;
}

function createTabItem(tab, index, total) {
  const item = document.createElement('div');
  item.className = 'tab';
  item.classList.toggle('active', tab.id === activeTabId);
  item.setAttribute('role', 'tab');
  item.setAttribute('aria-selected', String(tab.id === activeTabId));
  const shortcut = projects.shortcutLabel(index, total);
  const title = tab.title || 'Terminal';
  item.setAttribute('aria-label', shortcut ? `${title} (${chord(shortcut)})` : title);
  item.title = shortcut ? `${title} · ${chord(shortcut)}` : title;
  item.tabIndex = tab.id === activeTabId ? 0 : -1;
  item.dataset.tabId = tab.id;

  const icon = document.createElement('img');
  icon.className = 'tab-icon';
  icon.src = tabIconSrc(tab.icon);
  icon.alt = '';
  item.append(icon);

  const copy = document.createElement('span');
  copy.className = 'tab-title';
  copy.textContent = title;
  item.append(copy);

  const close = document.createElement('button');
  close.className = 'tab-close';
  close.type = 'button';
  close.setAttribute('aria-label', `Close ${title}`);
  close.title = 'Close session';
  close.textContent = '×';
  item.append(close);

  const indicator = document.createElement('span');
  indicator.className = 'tab-indicator';
  item.append(indicator);

  item.addEventListener('click', () => window.otty.activateTab(tab.id));
  item.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      window.otty.activateTab(tab.id);
    }
  });
  close.addEventListener('click', (event) => {
    event.stopPropagation();
    window.otty.closeTab(tab.id);
  });
  bindTabDrag(item, tab, false);
  return item;
}

function createFolderItem(folder, index, total) {
  const item = document.createElement('div');
  const selected = folder.tabs.some((tab) => tab.id === activeTabId);
  item.className = 'session-row folder-row';
  item.classList.toggle('selected', selected);
  item.setAttribute('role', 'listitem');
  item.setAttribute('aria-current', selected ? 'true' : 'false');
  const shortcut = projects.shortcutLabel(index, total);
  item.setAttribute('aria-label', `${folder.label}, ${folder.tabs.length} sessions`);
  item.title = shortcut
    ? `${folder.path || folder.label} · ${chord(shortcut, true)}`
    : folder.path || folder.label;
  item.tabIndex = 0;
  item.dataset.folderKey = folder.key;

  const icon = document.createElement('img');
  icon.src = selected
    ? '../../../resources/tab-icons/folder-open.svg'
    : '../../../resources/tab-icons/folder.svg';
  icon.alt = '';
  item.append(icon);

  const copy = document.createElement('span');
  copy.className = 'session-copy';
  const title = document.createElement('span');
  title.className = 'session-title';
  title.textContent = folder.label;
  const pathLabel = document.createElement('span');
  pathLabel.className = 'session-path';
  pathLabel.textContent = folder.path || 'Starting';
  copy.append(title, pathLabel);
  item.append(copy);

  const count = document.createElement('span');
  count.className = 'folder-count';
  count.textContent = String(folder.tabs.length);
  item.append(count);

  const busy = folder.tabs.some((tab) => tab.status === 'error') ? 'error'
    : folder.tabs.some((tab) => tab.status === 'connected') ? 'connected'
      : 'starting';
  const state = document.createElement('span');
  state.className = `session-state ${busy}`;
  state.setAttribute('aria-hidden', 'true');
  item.append(state);

  item.addEventListener('click', () => activateFolder(folder));
  item.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      activateFolder(folder);
    }
  });
  return item;
}

function renderTabs() {
  folders = projects.groupTabs(tabs);
  const activeFolder = projects.currentFolder(folders, activeTabId);
  if (activeFolder && activeTabId) lastTabByFolder.set(activeFolder.key, activeTabId);

  const sessions = sessionTabs();
  const topItems = document.createDocumentFragment();
  sessions.forEach((tab, index) => topItems.append(createTabItem(tab, index, sessions.length)));
  tabList.replaceChildren(topItems);

  const sideItems = document.createDocumentFragment();
  folders.forEach((folder, index) => sideItems.append(createFolderItem(folder, index, folders.length)));
  sidebarTabs.replaceChildren(sideItems);
}

function displayFrame(frame) {
  if (!Number.isInteger(frame.width) || !Number.isInteger(frame.height)) return;
  const raw = atob(frame.pixels);
  if (raw.length !== frame.width * frame.height * 4) {
    errorBox.textContent = 'Rust engine returned a frame with an invalid pixel size.';
    errorBox.classList.remove('hidden');
    return;
  }
  const bytes = new Uint8ClampedArray(raw.length);
  for (let i = 0; i < raw.length; i += 1) bytes[i] = raw.charCodeAt(i);
  if (canvas.width !== frame.width || canvas.height !== frame.height) {
    canvas.width = frame.width;
    canvas.height = frame.height;
  }
  context.putImageData(new ImageData(bytes, frame.width, frame.height), 0, 0);
  setText('#connection-status', 'CONNECTED');
  document.querySelector('.status-led').classList.add('connected');
}

function onTabsState(state) {
  tabs = state.tabs || [];
  activeTabId = state.activeTabId;
  setLayout(state.layout, state.panelVisible);
  renderTabs();
}

function onTabActivated(tab) {
  activeTabId = tab.id;
  renderTabs();
  setTitle(tab.title);
  setPath(tab.cwd);
  errorBox.classList.add('hidden');
  errorBox.textContent = '';
  setText('#connection-status', tab.status === 'connected' ? 'CONNECTED' : 'CONNECTING');
  document.querySelector('.status-led').classList.toggle('connected', tab.status === 'connected');
  canvas.width = 0;
  canvas.height = 0;
  lastResize = '';
  scheduleResize();
  ime.focus();
}

function onEngineEvent(event) {
  if (event.type === 'window-state') {
    document.querySelector('[data-window-action="toggle-maximize"]')
      .setAttribute('aria-label', event.maximized ? 'Restore' : 'Maximize');
    return;
  }
  if (event.tabId !== activeTabId) return;
  if (event.type === 'ready') {
    applyChrome(event.chrome);
    setTitle(event.title);
    setPath(event.cwd);
    if (event.grid) setText('#grid-size', `${event.grid.cols} × ${event.grid.rows}`);
    setText('#connection-status', 'CONNECTED');
    document.querySelector('.status-led').classList.add('connected');
  } else if (event.type === 'frame') {
    displayFrame(event);
  } else if (event.type === 'title') {
    setTitle(event.value);
  } else if (event.type === 'cwd') {
    setPath(event.value);
  } else if (event.type === 'grid') {
    setText('#grid-size', `${event.cols} × ${event.rows}`);
  } else if (event.type === 'error') {
    errorBox.textContent = event.message;
    errorBox.classList.remove('hidden');
    setText('#connection-status', 'ENGINE ERROR');
    document.querySelector('.status-led').classList.remove('connected');
  }
}

function sendResize() {
  if (!activeTabId) return;
  const rect = terminalSurface.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;
  const scale = window.devicePixelRatio || 1;
  const width = Math.max(1, Math.round(rect.width * scale));
  const height = Math.max(1, Math.round(rect.height * scale));
  const key = `${activeTabId}:${width}x${height}`;
  if (key === lastResize) return;
  lastResize = key;
  window.otty.resize(activeTabId, width, height);
}

function scheduleResize() {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(sendResize, 45);
}

const namedKeys = new Map([
  ['Enter', '\n'], ['Backspace', '\b'], ['Tab', '\t'], ['Escape', '\x1b'],
  ['ArrowUp', '\uF700'], ['ArrowDown', '\uF701'], ['ArrowLeft', '\uF702'], ['ArrowRight', '\uF703'],
  ['Home', '\uF729'], ['End', '\uF72B'], ['PageUp', '\uF72C'], ['PageDown', '\uF72D'], ['Delete', '\uF728'],
]);
for (let index = 1; index <= 12; index += 1) {
  namedKeys.set(`F${index}`, String.fromCodePoint(0xF704 + index - 1));
}

function controlCharacter(key) {
  if (key === ' ') return '\0';
  if (key === '?') return '\x7f';
  const code = key.toUpperCase().charCodeAt(0);
  return code >= 0x40 && code <= 0x5f ? String.fromCharCode(code & 0x1f) : null;
}

function handleTabShortcut(event) {
  const primary = isMacOS ? event.metaKey : event.ctrlKey;
  if (!primary || event.isComposing) return;
  const key = event.key.toLowerCase();
  const digit = /^Digit([1-9])$/.exec(event.code)?.[1];
  const sessions = sessionTabs();
  const activeFolder = projects.currentFolder(folders, activeTabId);

  if (key === 't' && event.shiftKey && !event.altKey) window.otty.reopenTab();
  else if (key === 't' && !event.altKey) window.otty.newTab();
  else if (key === 'w' && event.shiftKey && !event.altKey) window.otty.windowAction('close');
  else if (key === 'w' && !event.altKey) window.otty.closeTab(activeTabId);
  else if (key === 'l' && event.shiftKey && !event.altKey) window.otty.toggleTabsPanel();
  else if (event.altKey && (event.code === 'BracketLeft' || event.code === 'ArrowUp')) {
    const next = projects.relativeItem(folders, activeFolder?.key, -1, 'key');
    if (next) activateFolder(next);
    else return;
  } else if (event.altKey && (event.code === 'BracketRight' || event.code === 'ArrowDown')) {
    const next = projects.relativeItem(folders, activeFolder?.key, 1, 'key');
    if (next) activateFolder(next);
    else return;
  } else if (event.shiftKey && event.code === 'BracketLeft') {
    const next = projects.relativeItem(sessions, activeTabId, -1);
    if (next) window.otty.activateTab(next.id);
    else return;
  } else if (event.shiftKey && event.code === 'BracketRight') {
    const next = projects.relativeItem(sessions, activeTabId, 1);
    if (next) window.otty.activateTab(next.id);
    else return;
  } else if (digit && event.shiftKey && !event.altKey) {
    const folder = projects.itemAtShortcut(folders, digit);
    if (folder) activateFolder(folder);
    else return;
  } else if (digit && !event.shiftKey && !event.altKey) {
    const tab = projects.itemAtShortcut(sessions, digit);
    if (tab) window.otty.activateTab(tab.id);
    else return;
  } else return;
  event.preventDefault();
  event.stopPropagation();
}

terminalSurface.addEventListener('pointerdown', (event) => {
  event.preventDefault();
  ime.focus({ preventScroll: true });
});
ime.addEventListener('paste', (event) => {
  event.preventDefault();
  const text = event.clipboardData?.getData('text/plain');
  if (text && activeTabId) window.otty.input(activeTabId, text);
});
ime.addEventListener('keydown', (event) => {
  if (imeInput.isComposing() || event.isComposing || event.key === 'Process') return;
  if (event.metaKey || (event.ctrlKey && !isMacOS)) return;
  if (event.ctrlKey) {
    const control = controlCharacter(event.key);
    if (control !== null) {
      event.preventDefault();
      window.otty.input(activeTabId, control);
    }
    return;
  }
  const keyText = namedKeys.get(event.key);
  if (keyText !== undefined) {
    event.preventDefault();
    window.otty.input(activeTabId, keyText);
    return;
  }
  if (event.altKey && event.key.length === 1) {
    event.preventDefault();
    window.otty.input(activeTabId, `\x1b${event.key}`);
  }
});

window.addEventListener('keydown', handleTabShortcut);
window.addEventListener('focus', () => {
  if (activeTabId) ime.focus({ preventScroll: true });
});
for (const button of document.querySelectorAll('[data-window-action]')) {
  button.addEventListener('click', () => window.otty.windowAction(button.dataset.windowAction));
}
document.querySelector('#new-tab-top').addEventListener('click', () => window.otty.newTab());
document.querySelector('#new-tab-side').addEventListener('click', () => window.otty.newTab());
document.querySelector('#sidebar-toggle').addEventListener('click', () => window.otty.toggleTabsPanel());
document.querySelector('#titlebar').addEventListener('dblclick', (event) => {
  if (!event.target.closest('button')) window.otty.windowAction('toggle-maximize');
});

window.otty.onTabsState(onTabsState);
window.otty.onTabActivated(onTabActivated);
window.otty.onEngineEvent(onEngineEvent);
resizeObserver.observe(terminalSurface);
window.addEventListener('resize', scheduleResize);
terminalSurface.addEventListener('contextmenu', (event) => event.preventDefault());
ime.focus();
