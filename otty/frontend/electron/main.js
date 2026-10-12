const { app, BrowserWindow, Menu, Notification, clipboard, dialog, ipcMain, screen, shell } = require('electron');
const { spawn } = require('node:child_process');
const readline = require('node:readline');
const path = require('node:path');
const fs = require('node:fs');
const { pathToFileURL } = require('node:url');
const { createCorrelator } = require('./correlator');
const { accelerator } = require('./keymap');
const { describeFont } = require('./font-files');

// One bridge per window (PROTOCOL.md "Transport"); the backend owns tabs and panes.
const sessions = new Map();
const workspace = path.resolve(__dirname, '..', '..');
const mac = process.platform === 'darwin';
const mock = process.env.OTTY_ENGINE_MOCK === '1';

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

function listLicensedFonts() {
  try {
    return fs.readdirSync(path.join(workspace, 'resources', 'fonts', 'licensed')).map(describeFont).filter(Boolean);
  } catch {
    return [];
  }
}

// Adding a file to renderer/features/ is the whole registration.
function listFeatures() {
  let names = [];
  try {
    names = fs.readdirSync(path.join(__dirname, 'renderer', 'features')).sort();
  } catch {
    // No features directory yet.
  }
  return {
    js: names.filter((name) => name.endsWith('.js') && !name.endsWith('.test.js')),
    css: names.filter((name) => name.endsWith('.css')),
    fonts: listLicensedFonts(),
  };
}

function send(state, channel, payload) {
  if (state.window.isDestroyed()) return;
  if (!state.loaded) state.queue.push([channel, payload]);
  else state.window.webContents.send(channel, payload);
}

function write(state, command) {
  const bridge = state.bridge;
  if (!bridge || bridge.killed || !bridge.stdin.writable) return;
  bridge.stdin.write(`${JSON.stringify(command)}\n`);
}

function fail(state, message) {
  send(state, 'bridge:event', { type: 'error', message });
}

function openExternal(url) {
  try {
    const target = new URL(url);
    if (['http:', 'https:', 'file:'].includes(target.protocol)) shell.openExternal(target.href);
  } catch {
    // Not a URL; nothing to open.
  }
}

function runAction(window, action) {
  const target = window || BrowserWindow.getFocusedWindow();
  if (target && !target.isDestroyed()) target.webContents.send('action', action);
}

// Menu rows are action ids; the renderer's runAction decides who handles one.
// Accelerators are shown, not registered — the renderer keymap owns the keys.
async function refreshMenu(state) {
  const reply = await state.requests.request('menu.list');
  if (!reply.ok || !Array.isArray(reply.result)) return;
  const template = reply.result.map((menu) => ({
    label: String(menu.title),
    submenu: (menu.items || []).map((item) => ({
      label: String(item.label),
      accelerator: item.shortcut ? accelerator(item.shortcut, mac) : undefined,
      registerAccelerator: false,
      click: (_item, window) => runAction(window, item.action),
    })),
  }));
  if (mac) template.unshift({ role: 'appMenu' });
  try {
    Menu.setApplicationMenu(Menu.buildFromTemplate(template));
  } catch (error) {
    console.error(`[otty] menu: ${error.message}`);
  }
}

function onBridgeEvent(state, event) {
  if (event.type === 'response') {
    state.requests.settle(event);
    return;
  }
  if (event.type === 'clipboard' && typeof event.text === 'string') clipboard.writeText(event.text);
  else if (event.type === 'open') openExternal(event.url);
  else if (event.type === 'notify' && Notification.isSupported()) {
    const notification = new Notification({ title: String(event.title || 'Otty'), body: String(event.body || '') });
    notification.on('click', () => state.window.isDestroyed() || state.window.show());
    notification.show();
  } else if (event.type === 'quit') {
    app.quit();
    return;
  } else if (event.type === 'ready' || event.type === 'config') refreshMenu(state);
  send(state, 'bridge:event', event);
}

function startBridge(state, cwd) {
  const binary = mock ? process.execPath : enginePath();
  if (!binary) {
    fail(state, 'Rust engine not found. Run `npm run build:engine` first.');
    return;
  }
  const scale = screen.getDisplayMatching(state.window.getBounds()).scaleFactor;
  const env = { ...process.env, OTTY_SCALE_FACTOR: String(scale), OTTY_CWD: cwd };
  if (mock) env.ELECTRON_RUN_AS_NODE = '1';
  let bridge;
  try {
    bridge = spawn(binary, mock ? [path.join(__dirname, 'mock-bridge.js')] : [], {
      cwd,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
  } catch (error) {
    fail(state, error.message);
    return;
  }
  state.bridge = bridge;
  readline.createInterface({ input: bridge.stdout }).on('line', (line) => {
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      fail(state, 'Rust engine sent an invalid event.');
      return;
    }
    onBridgeEvent(state, event);
  });
  bridge.stdin.on('error', () => {});
  bridge.stderr.setEncoding('utf8');
  bridge.stderr.on('data', (data) => console.error(`[otty-bridge:${state.window.id}] ${data.trimEnd()}`));
  bridge.once('error', (error) => fail(state, error.message));
  bridge.once('exit', (code) => {
    state.requests.failAll('Rust engine exited.');
    if (state.closing || state.window.isDestroyed()) return;
    // A clean exit is the backend closing its last tab.
    if (code === 0) state.window.close();
    else fail(state, `Rust engine exited (${code ?? 'signal'}).`);
  });
}

function stopBridge(state) {
  state.closing = true;
  write(state, { type: 'close' });
  setTimeout(() => state.bridge?.kill(), 300);
}

function createWindow(cwd) {
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
  const state = { window, bridge: null, loaded: false, closing: false, queue: [] };
  state.requests = createCorrelator((command) => write(state, command));
  sessions.set(window.id, state);

  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event, url) => {
    if (url !== pathToFileURL(path.join(__dirname, 'renderer', 'index.html')).href) {
      event.preventDefault();
    }
  });
  if (process.env.OTTY_DEBUG === '1') {
    window.webContents.on('console-message', (details) => {
      console.log(`[renderer:${details.level}] ${details.message} (${path.basename(details.sourceId || '')}:${details.lineNumber})`);
    });
  }
  window.webContents.once('did-finish-load', () => {
    state.loaded = true;
    for (const [channel, payload] of state.queue.splice(0)) window.webContents.send(channel, payload);
    window.show();
  });
  window.on('maximize', () => send(state, 'bridge:event', { type: 'window-state', maximized: true }));
  window.on('unmaximize', () => send(state, 'bridge:event', { type: 'window-state', maximized: false }));
  window.on('closed', () => {
    stopBridge(state);
    sessions.delete(window.id);
  });
  window.loadFile(path.join(__dirname, 'renderer', 'index.html'));
  startBridge(state, cwd || process.env.OTTY_CWD || app.getPath('home'));

  // Dev only: OTTY_SHOT=<png> captures the window after OTTY_SHOT_MS and quits;
  // OTTY_SHOT_EVAL runs a renderer snippet first, to stage what gets captured.
  if (process.env.OTTY_SHOT && !app.isPackaged) {
    setTimeout(async () => {
      try {
        if (process.env.OTTY_SHOT_EVAL) {
          console.log('[shot]', JSON.stringify(await window.webContents.executeJavaScript(process.env.OTTY_SHOT_EVAL)));
        }
        fs.writeFileSync(process.env.OTTY_SHOT, (await window.webContents.capturePage()).toPNG());
      } catch (error) {
        console.error(`[shot] ${error.message}`);
      }
      app.quit();
    }, Number(process.env.OTTY_SHOT_MS) || 5000);
  }
}

function sessionFor(event) {
  const window = BrowserWindow.fromWebContents(event.sender);
  return window && sessions.get(window.id);
}

ipcMain.handle('bridge:request', (event, method, params) => {
  const state = sessionFor(event);
  if (!state || typeof method !== 'string') return { ok: false, error: 'Bad request.' };
  if (!state.bridge || state.bridge.exitCode !== null) return { ok: false, error: 'Rust engine is not running.' };
  return state.requests.request(method, params && typeof params === 'object' ? params : {});
});

ipcMain.on('bridge:send', (event, command) => {
  const state = sessionFor(event);
  if (!state || !command || typeof command.pane !== 'string') return;
  const { type, pane, text, width, height } = command;
  if (type === 'input' || type === 'paste') {
    // Key text is short; a paste may be a file's worth.
    if (typeof text !== 'string' || text.length > (type === 'input' ? 8192 : 1 << 20)) return;
    write(state, { type, pane, text });
  } else if (type === 'resize') {
    if (!Number.isInteger(width) || !Number.isInteger(height)) return;
    if (width < 1 || height < 1 || width > 8192 || height > 8192) return;
    write(state, { type, pane, width, height });
  }
});

ipcMain.handle('features:list', () => listFeatures());
ipcMain.handle('clipboard:read', () => clipboard.readText());
ipcMain.on('clipboard:write', (_event, text) => {
  if (typeof text === 'string') clipboard.writeText(text);
});

// Context menus are native; a click comes back as an action id like the app menu's.
ipcMain.on('menu:popup', (event, items) => {
  const window = BrowserWindow.fromWebContents(event.sender);
  if (!window || !Array.isArray(items) || !items.length) return;
  Menu.buildFromTemplate(items.map((item) => (item && typeof item.action === 'string'
    ? { label: String(item.label), click: () => runAction(window, item.action) }
    : { type: 'separator' }))).popup({ window });
});

// The frameless window has no menu bar, so the titlebar button pops the app menu.
ipcMain.on('menu:app', (event) => {
  const window = BrowserWindow.fromWebContents(event.sender);
  if (window) Menu.getApplicationMenu()?.popup({ window });
});

ipcMain.handle('projects:open', async (event) => {
  const state = sessionFor(event);
  if (!state) return null;
  const result = await dialog.showOpenDialog(state.window, {
    title: 'Open project',
    buttonLabel: 'Open',
    properties: ['openDirectory'],
  });
  return result.canceled ? null : result.filePaths[0] || null;
});

ipcMain.handle('path:is-directory', (_event, target) => {
  try {
    return fs.statSync(target).isDirectory();
  } catch {
    return false;
  }
});

ipcMain.on('window:new', (_event, cwd) => createWindow(typeof cwd === 'string' && cwd ? cwd : undefined));

ipcMain.on('window:action', (event, action) => {
  const window = BrowserWindow.fromWebContents(event.sender);
  if (!window) return;
  if (action === 'minimize') window.minimize();
  if (action === 'toggle-maximize') window.isMaximized() ? window.unmaximize() : window.maximize();
  if (action === 'toggle-full-screen') window.setFullScreen(!window.isFullScreen());
  if (action === 'close') window.close();
});

app.whenReady().then(() => {
  // Electron's default menu would claim Ctrl+W and Ctrl+R before ours arrives.
  Menu.setApplicationMenu(null);
  createWindow();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', () => {
  if (!mac) app.quit();
});

app.on('before-quit', () => {
  for (const state of sessions.values()) stopBridge(state);
});
