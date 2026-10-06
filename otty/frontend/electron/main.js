const { app, BrowserWindow, ipcMain, screen, dialog } = require('electron');
const { spawn } = require('node:child_process');
const readline = require('node:readline');
const path = require('node:path');
const fs = require('node:fs');
const { pathToFileURL } = require('node:url');
const { moveTab, activeAfterClose } = require('./tab-order');
const { findProjectRoot } = require('./project-groups');

const sessions = new Map();
const workspace = path.resolve(__dirname, '..', '..');

function enginePath() {
  const name = process.platform === 'win32' ? 'otty-bridge.exe' : 'otty-bridge';
  const candidates = [
    process.env.OTTY_ENGINE,
    path.join(process.resourcesPath, name),
    path.join(process.resourcesPath, 'otty', name),
    path.join(workspace, 'target', 'debug', name),
    path.join(workspace, 'target', 'release', name),
  ].filter(Boolean);
  return candidates.find((candidate) => fs.existsSync(candidate));
}

function activeTab(state) {
  return state.tabs.get(state.activeTabId);
}

function send(state, channel, payload) {
  if (state.window.isDestroyed()) return;
  if (!state.loaded) state.queue.push([channel, payload]);
  else state.window.webContents.send(channel, payload);
}

function projectOf(cwd) {
  if (!cwd) return '';
  return findProjectRoot(
    cwd,
    (dir) => {
      try {
        return fs.existsSync(path.join(dir, '.git'));
      } catch {
        return false;
      }
    },
    path.dirname,
  );
}

function tabSummary(tab) {
  return {
    id: tab.id,
    title: tab.title,
    cwd: tab.cwd,
    status: tab.status,
    project: projectOf(tab.cwd),
    icon: tab.icon || 'terminal',
  };
}

function sendTabsState(state) {
  send(state, 'tabs:state', {
    tabs: state.tabOrder.map((id) => tabSummary(state.tabs.get(id))).filter(Boolean),
    activeTabId: state.activeTabId,
    layout: state.layout,
    panelVisible: state.panelVisible,
  });
}

function writeCommand(tab, command) {
  if (!tab?.engine || tab.engine.killed || !tab.engine.stdin.writable) return;
  tab.engine.stdin.write(`${JSON.stringify(command)}\n`);
}

function publishTabEvent(state, tab, event) {
  if (event.type === 'ready') {
    tab.title = event.title || tab.title;
    tab.cwd = event.cwd || tab.cwd;
    tab.icon = event.icon || tab.icon || 'terminal';
    tab.status = 'connected';
    if (!state.layoutInitialized) {
      state.layout = event.layout || state.layout;
      if (!state.panelUserChanged && typeof event.sidebar_visible === 'boolean') {
        state.panelVisible = event.sidebar_visible;
      }
      state.layoutInitialized = true;
    }
    sendTabsState(state);
  } else if (event.type === 'title') {
    tab.title = event.value || 'Terminal';
    sendTabsState(state);
  } else if (event.type === 'cwd') {
    tab.cwd = event.value || tab.cwd;
    sendTabsState(state);
  } else if (event.type === 'icon') {
    tab.icon = event.value || 'terminal';
    sendTabsState(state);
  } else if (event.type === 'error') {
    tab.status = 'error';
    sendTabsState(state);
  }

  if (event.type === 'frame' && tab.id !== state.activeTabId) return;
  send(state, 'engine:event', { tabId: tab.id, ...event });
}

function startEngine(state, tab) {
  const binary = enginePath();
  if (!binary) {
    publishTabEvent(state, tab, { type: 'error', message: 'Rust engine not found. Run `npm run build:engine` first.' });
    return;
  }

  const scale = screen.getDisplayMatching(state.window.getBounds()).scaleFactor;
  let engine;
  try {
    engine = spawn(binary, [], {
      cwd: tab.cwd,
      env: { ...process.env, OTTY_SCALE_FACTOR: String(scale) },
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
  } catch (error) {
    publishTabEvent(state, tab, { type: 'error', message: error.message });
    return;
  }
  tab.engine = engine;
  const lines = readline.createInterface({ input: engine.stdout });
  lines.on('line', (line) => {
    try {
      const event = JSON.parse(line);
      if (event.type === 'ready' && tab.pendingSize) {
        writeCommand(tab, { type: 'resize', ...tab.pendingSize });
        tab.pendingSize = null;
      }
      publishTabEvent(state, tab, event);
    } catch {
      publishTabEvent(state, tab, { type: 'error', message: 'Rust engine sent an invalid event.' });
    }
  });
  engine.stderr.setEncoding('utf8');
  engine.stderr.on('data', (data) => console.error(`[otty-engine:${tab.id}] ${data.trimEnd()}`));
  engine.once('error', (error) => publishTabEvent(state, tab, { type: 'error', message: error.message }));
  engine.once('exit', (code) => {
    if (!tab.closing && state.tabs.has(tab.id) && code !== 0) {
      publishTabEvent(state, tab, { type: 'error', message: `Rust engine exited (${code ?? 'signal'}).` });
    }
  });
}

function activateTab(state, tabId) {
  const tab = state.tabs.get(tabId);
  if (!tab) return;
  state.activeTabId = tabId;
  sendTabsState(state);
  send(state, 'tabs:activated', tabSummary(tab));
}

function createTab(state, { cwd, title = 'Terminal' } = {}) {
  const current = activeTab(state);
  const tab = {
    id: `tab-${state.nextTabId++}`,
    title,
    cwd: cwd || current?.cwd || process.env.OTTY_CWD || app.getPath('home'),
    status: 'starting',
    icon: 'terminal',
    engine: null,
    closing: false,
    pendingSize: null,
  };
  state.tabs.set(tab.id, tab);
  state.tabOrder.push(tab.id);
  state.activeTabId = tab.id;
  sendTabsState(state);
  send(state, 'tabs:activated', tabSummary(tab));
  startEngine(state, tab);
}

function stopTab(tab) {
  if (!tab) return;
  tab.closing = true;
  writeCommand(tab, { type: 'close' });
  setTimeout(() => tab.engine?.kill(), 300);
}

function closeTab(state, tabId) {
  const tab = state.tabs.get(tabId);
  if (!tab) return;
  if (state.tabs.size === 1) {
    state.window.close();
    return;
  }
  const previousOrder = [...state.tabOrder];
  const wasActive = tabId === state.activeTabId;
  state.closedTabs.push({ cwd: tab.cwd, title: tab.title });
  stopTab(tab);
  state.tabs.delete(tabId);
  state.tabOrder = state.tabOrder.filter((id) => id !== tabId);
  if (wasActive) state.activeTabId = activeAfterClose(previousOrder, tabId, state.tabOrder);
  sendTabsState(state);
  const next = activeTab(state);
  if (next) {
    send(state, 'tabs:activated', tabSummary(next));
  }
}

function createWindow() {
  const window = new BrowserWindow({
    width: 1180,
    height: 760,
    minWidth: 720,
    minHeight: 480,
    backgroundColor: '#111315',
    title: 'Otty',
    frame: false,
    show: false,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      preload: path.join(__dirname, 'preload.js'),
    },
  });
  const state = {
    window,
    tabs: new Map(),
    tabOrder: [],
    activeTabId: null,
    closedTabs: [],
    nextTabId: 1,
    loaded: false,
    closing: false,
    queue: [],
    layout: 'sidebar-left',
    panelVisible: true,
    panelUserChanged: false,
    layoutInitialized: false,
  };
  sessions.set(window.id, state);

  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event, url) => {
    if (url !== pathToFileURL(path.join(__dirname, 'renderer', 'index.html')).href) {
      event.preventDefault();
    }
  });
  window.webContents.once('did-finish-load', () => {
    state.loaded = true;
    for (const [channel, payload] of state.queue.splice(0)) window.webContents.send(channel, payload);
    window.show();
    createTab(state);
  });
  window.on('maximize', () => send(state, 'engine:event', { type: 'window-state', maximized: true }));
  window.on('unmaximize', () => send(state, 'engine:event', { type: 'window-state', maximized: false }));
  window.on('closed', () => {
    state.closing = true;
    for (const tab of state.tabs.values()) stopTab(tab);
    sessions.delete(window.id);
  });
  window.loadFile(path.join(__dirname, 'renderer', 'index.html'));
}

function sessionFor(event) {
  const window = BrowserWindow.fromWebContents(event.sender);
  return window && sessions.get(window.id);
}

ipcMain.on('engine:input', (event, payload) => {
  const state = sessionFor(event);
  if (!state || !payload || payload.tabId !== state.activeTabId) return;
  if (typeof payload.text !== 'string' || payload.text.length > 8192) return;
  writeCommand(activeTab(state), { type: 'input', text: payload.text });
});

ipcMain.on('engine:resize', (event, payload) => {
  const state = sessionFor(event);
  if (!state || !payload || payload.tabId !== state.activeTabId) return;
  const { width, height } = payload;
  if (!Number.isInteger(width) || !Number.isInteger(height)) return;
  if (width < 1 || height < 1 || width > 8192 || height > 8192) return;
  const tab = activeTab(state);
  if (!tab) return;
  if (!tab.engine) tab.pendingSize = { width, height };
  writeCommand(tab, { type: 'resize', width, height });
});

ipcMain.on('tabs:new', (event) => {
  const state = sessionFor(event);
  if (state) createTab(state);
});

ipcMain.handle('projects:open', async (event) => {
  const state = sessionFor(event);
  if (!state) return;
  const result = await dialog.showOpenDialog(state.window, {
    title: 'Open project',
    buttonLabel: 'Open',
    defaultPath: activeTab(state)?.cwd || app.getPath('home'),
    properties: ['openDirectory'],
  });
  const cwd = result.filePaths && result.filePaths[0];
  if (result.canceled || !cwd) return;
  createTab(state, { cwd });
});

ipcMain.on('tabs:activate', (event, tabId) => {
  const state = sessionFor(event);
  if (state && typeof tabId === 'string') activateTab(state, tabId);
});

ipcMain.on('tabs:close', (event, tabId) => {
  const state = sessionFor(event);
  if (state && typeof tabId === 'string') closeTab(state, tabId);
});

ipcMain.on('tabs:reopen', (event) => {
  const state = sessionFor(event);
  const lastClosed = state?.closedTabs.pop();
  if (state && lastClosed) createTab(state, lastClosed);
});

ipcMain.on('tabs:move', (event, payload) => {
  const state = sessionFor(event);
  if (!state || !payload || typeof payload.tabId !== 'string' || typeof payload.targetId !== 'string') return;
  state.tabOrder = moveTab(state.tabOrder, payload.tabId, payload.targetId, payload.after === true);
  sendTabsState(state);
  const tab = activeTab(state);
  if (tab) send(state, 'tabs:activated', tabSummary(tab));
});

ipcMain.on('tabs:toggle-panel', (event) => {
  const state = sessionFor(event);
  if (!state) return;
  state.panelVisible = !state.panelVisible;
  state.panelUserChanged = true;
  sendTabsState(state);
  const tab = activeTab(state);
  if (tab) send(state, 'tabs:activated', tabSummary(tab));
});

ipcMain.on('window:action', (event, action) => {
  const window = BrowserWindow.fromWebContents(event.sender);
  if (!window) return;
  if (action === 'minimize') window.minimize();
  if (action === 'toggle-maximize') window.isMaximized() ? window.unmaximize() : window.maximize();
  if (action === 'close') window.close();
});

app.whenReady().then(() => {
  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

app.on('before-quit', () => {
  for (const state of sessions.values()) {
    state.closing = true;
    for (const tab of state.tabs.values()) stopTab(tab);
  }
});
