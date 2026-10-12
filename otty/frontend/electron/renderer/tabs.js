// Projects sidebar and the titlebar strip of the current project's sessions,
// drawn from the backend's `state`. app.js (loaded last) owns `state`,
// `request`, `runAction`, `showMenu`, `report` and `t`.
const appShell = document.querySelector('.app-shell');
const tabList = document.querySelector('#tab-list');
const sidebarTabs = document.querySelector('#sidebar-tabs');
const projects = globalThis.OttyProjects;
const lastTabByFolder = new Map();
const BADGES = ['running', 'completed', 'error'];

let folders = [];
let draggedTabId = null;
let renaming = false;
let panelVisible = true;
let sidebarLayout = true;

function activeTab() {
  return state?.tabs.find((tab) => tab.id === state.activeTab) || null;
}

function activateTab(id) {
  if (id !== state?.activeTab) request('tab.activate', { tab: id }).catch(report);
}

function setLayout(layout, visible) {
  panelVisible = visible !== false;
  sidebarLayout = (layout || 'sidebar-left').startsWith('sidebar');
  appShell.classList.toggle('layout-sidebar', sidebarLayout);
  appShell.classList.toggle('layout-tabs', !sidebarLayout);
  appShell.classList.toggle('tabs-panel-hidden', !panelVisible);
  const label = panelVisible ? 'Hide projects panel' : 'Show projects panel';
  const toggle = document.querySelector('#sidebar-toggle');
  toggle.setAttribute('aria-label', label);
  toggle.setAttribute('aria-expanded', String(panelVisible));
  toggle.title = label;
}

function chord(n, shift = false) {
  const primary = platform === 'macos' ? '⌘' : 'Ctrl';
  return shift ? `${primary}+Shift+${n}` : `${primary}+${n}`;
}

function sessionTabs() {
  const tabs = state?.tabs || [];
  if (!sidebarLayout) return tabs;
  const folder = projects.currentFolder(folders, state?.activeTab);
  return folder ? folder.tabs : tabs;
}

function activateFolder(folder) {
  if (!folder || !folder.tabs.length) return;
  const last = lastTabByFolder.get(folder.key);
  activateTab((folder.tabs.find((item) => item.id === last) || folder.tabs[0]).id);
}

function bindTabDrag(item, tab) {
  item.draggable = true;
  item.addEventListener('dragstart', (event) => {
    draggedTabId = tab.id;
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', tab.id);
    item.classList.add('dragging');
  });
  item.addEventListener('dragend', () => {
    draggedTabId = null;
    renderTabs();
  });
  item.addEventListener('dragover', (event) => {
    if (draggedTabId && draggedTabId !== tab.id) {
      event.preventDefault();
      item.classList.add('drop-target');
    }
  });
  item.addEventListener('dragleave', () => item.classList.remove('drop-target'));
  item.addEventListener('drop', (event) => {
    if (!draggedTabId) return;
    event.preventDefault();
    event.stopPropagation();
    const rect = item.getBoundingClientRect();
    const after = event.clientX >= rect.left + rect.width / 2;
    request('tab.move', { tab: draggedTabId, target: tab.id, after }).catch(report);
  });
}

function tabIconSrc(name) {
  const stem = /^[a-z0-9-]+$/.test(name || '') ? name : 'terminal';
  return `../../../resources/tab-icons/${stem}.svg`;
}

// null | "running" | "completed" | "error" | {progress}
function badgeElement(badge) {
  const progress = badge && typeof badge === 'object' ? Number(badge.progress) : null;
  if (progress === null && !BADGES.includes(badge)) return null;
  const el = document.createElement('span');
  el.className = `tab-badge ${progress === null ? badge : 'progress'}`;
  el.setAttribute('role', 'img');
  el.setAttribute('aria-label', progress === null ? badge : `${progress}%`);
  if (progress !== null) el.style.setProperty('--progress', String(progress));
  return el;
}

function startRename(tabId) {
  const tab = state?.tabs.find((item) => item.id === tabId);
  const item = tabList.querySelector(`[data-tab-id="${CSS.escape(tabId)}"]`);
  if (!tab || !item || renaming) return;
  renaming = true;
  item.draggable = false;
  const input = document.createElement('input');
  input.className = 'tab-rename';
  input.value = tab.title || '';
  input.setAttribute('aria-label', 'Tab title');
  item.querySelector('.tab-title').replaceWith(input);
  input.select();
  const finish = (commit) => {
    if (!renaming) return;
    renaming = false;
    // An empty title clears the user title (PROTOCOL.md `tab.rename`).
    if (commit) request('tab.rename', { tab: tabId, title: input.value.trim() }).catch(report);
    renderTabs();
    focusTerminal();
  };
  input.addEventListener('keydown', (event) => {
    event.stopPropagation();
    if (event.key === 'Enter') finish(true);
    if (event.key === 'Escape') finish(false);
  });
  input.addEventListener('blur', () => finish(true));
}

function tabMenu(tab) {
  showMenu([
    { label: t('rename_tab', 'Rename Tab...'), run: () => startRename(tab.id) },
    { label: t('close_tab', 'Close Tab'), run: () => request('tab.close', { tab: tab.id }) },
    { label: t('move_tab_to_new_window', 'Move Tab to New Window'), run: () => moveTabToNewWindow(tab) },
    null,
    // Divider actions act on the active tab, so the hit becomes it first.
    { label: t('tabs_insert_divider', 'Insert Divider'), run: () => request('tab.activate', { tab: tab.id }).then(() => runAction('tabs_insert_divider')) },
    ...(state.dividers?.length ? [{ label: t('tabs_remove_dividers', 'Remove All Dividers'), action: 'tabs_remove_dividers' }] : []),
  ]);
}

function createTabItem(tab, index, total) {
  const active = tab.id === state.activeTab;
  const item = document.createElement('div');
  item.className = 'tab';
  item.classList.toggle('active', active);
  item.setAttribute('role', 'tab');
  item.setAttribute('aria-selected', String(active));
  const shortcut = projects.shortcutLabel(index, total);
  const title = tab.title || 'Terminal';
  item.setAttribute('aria-label', shortcut ? `${title} (${chord(shortcut)})` : title);
  item.title = shortcut ? `${title} · ${chord(shortcut)}` : title;
  item.tabIndex = active ? 0 : -1;
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

  const badge = badgeElement(tab.badge);
  if (badge) item.append(badge);

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

  item.addEventListener('click', () => activateTab(tab.id));
  item.addEventListener('dblclick', () => startRename(tab.id));
  item.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    tabMenu(tab);
  });
  item.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      activateTab(tab.id);
    }
  });
  close.addEventListener('click', (event) => {
    event.stopPropagation();
    request('tab.close', { tab: tab.id }).catch(report);
  });
  bindTabDrag(item, tab);
  return item;
}

function createFolderItem(folder, index, total) {
  const item = document.createElement('div');
  const selected = folder.tabs.some((tab) => tab.id === state.activeTab);
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
  icon.src = tabIconSrc(selected ? 'folder-open' : 'folder');
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

  // The loudest badge among the folder's sessions.
  const badge = badgeElement(['error', 'running', 'completed']
    .find((kind) => folder.tabs.some((tab) => tab.badge === kind)));
  if (badge) item.append(badge);

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
  // A redraw would drop the drag or the rename box; both redraw when they end.
  if (!state || draggedTabId || renaming) return;
  folders = projects.groupTabs(state.tabs);
  const activeFolder = projects.currentFolder(folders, state.activeTab);
  if (activeFolder && state.activeTab) lastTabByFolder.set(activeFolder.key, state.activeTab);

  const sessions = sessionTabs();
  const topItems = document.createDocumentFragment();
  sessions.forEach((tab, index) => {
    topItems.append(createTabItem(tab, index, sessions.length));
    // `dividers` holds indexes into the whole tab list.
    if (state.dividers?.includes(state.tabs.indexOf(tab))) {
      const divider = document.createElement('span');
      divider.className = 'tab-divider';
      divider.setAttribute('role', 'separator');
      topItems.append(divider);
    }
  });
  tabList.replaceChildren(topItems);

  const sideItems = document.createDocumentFragment();
  folders.forEach((folder, index) => sideItems.append(createFolderItem(folder, index, folders.length)));
  sidebarTabs.replaceChildren(sideItems);
}
