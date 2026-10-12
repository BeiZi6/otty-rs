const { app, BrowserWindow, Menu, Notification, clipboard, dialog, ipcMain, screen, shell } = require('electron');
const { spawn } = require('node:child_process');
const readline = require('node:readline');
const path = require('node:path');
const fs = require('node:fs');
const { pathToFileURL } = require('node:url');
const { createCorrelator } = require('./correlator');
const { accelerator } = require('./keymap');
const { describeFont } = require('./font-files');

// One bridge for the app (PROTOCOL.md "Transport"). The backend owns windows,
// tabs and panes; `sessions` holds a BrowserWindow for each model window
// (`id`: `window-1`, …; null until the backend has claimed it).
const sessions = new Map();
let bridge = null;
let quitting = false;
// The last `ready`, replayed to every window that opens after it.
let ready = null;
const requests = createCorrelator(write);
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

// An event that names a window goes to it; the rest go to every window.
function deliver(event) {
  for (const state of sessions.values()) {
    if (!event.window || event.window === state.id) send(state, 'bridge:event', event);
  }
}

function write(command) {
  if (!bridge || bridge.killed || !bridge.stdin.writable) return;
  bridge.stdin.write(`${JSON.stringify(command)}\n`);
}

// Shown in every window, or in one opened to show it.
function fail(message) {
  if (!sessions.size && !quitting) createWindow();
  deliver({ type: 'error', message });
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
async function refreshMenu() {
  const reply = await requests.request('menu.list');
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

// The model opened a window: a BrowserWindow nobody has claimed takes it
// (the one put up at launch), else a new one, where a torn-off tab was dropped.
function openWindow({ window: id, x, y }) {
  const free = [...sessions.values()].find((state) => state.id === null);
  const state = free || createWindow(Number.isFinite(x) && Number.isFinite(y) ? { x, y } : undefined);
  state.id = id;
}

function onBridgeEvent(event) {
  if (event.type === 'response') {
    requests.settle(event);
    return;
  }
  if (event.type === 'clipboard' && typeof event.text === 'string') clipboard.writeText(event.text);
  else if (event.type === 'open') openExternal(event.url);
  else if (event.type === 'notify' && Notification.isSupported()) {
    const notification = new Notification({ title: String(event.title || 'Otty'), body: String(event.body || '') });
    notification.on('click', () => (BrowserWindow.getFocusedWindow() || BrowserWindow.getAllWindows()[0])?.show());
    notification.show();
  } else if (event.type === 'quit') {
    // On macOS the app outlives its last window; `activate` starts the bridge again.
    if (!mac || sessions.size) app.quit();
    return;
  } else if (event.type === 'window-open') {
    openWindow(event);
    return;
  } else if (event.type === 'window-close') {
    const state = [...sessions.values()].find((item) => item.id === event.window);
    if (state) {
      // Forgotten first, so `closed` does not report back what the model already knows.
      sessions.delete(state.key);
      state.window.close();
    }
    return;
  } else if (event.type === 'ready') {
    ready = event;
    refreshMenu();
  } else if (event.type === 'config') refreshMenu();
  else if (event.type === 'chrome' && ready) ready = { ...ready, chrome: event.chrome };
  deliver(event);
}

function startBridge() {
  const cwd = process.env.OTTY_CWD || app.getPath('home');
  const binary = mock ? process.execPath : enginePath();
  if (!binary) {
    fail('Rust engine not found. Run `npm run build:engine` first.');
    return;
  }
  // The primary display's scale, until a pane reports its own with `resize`.
  const env = { ...process.env, OTTY_SCALE_FACTOR: String(screen.getPrimaryDisplay().scaleFactor), OTTY_CWD: cwd };
  if (mock) env.ELECTRON_RUN_AS_NODE = '1';
  let child;
  try {
    child = spawn(binary, mock ? [path.join(__dirname, 'mock-bridge.js')] : [], {
      cwd,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
  } catch (error) {
    fail(error.message);
    return;
  }
  bridge = child;
  readline.createInterface({ input: child.stdout }).on('line', (line) => {
    let event;
    try {
      event = JSON.parse(line);
    } catch {
      fail('Rust engine sent an invalid event.');
      return;
    }
    onBridgeEvent(event);
  });
  child.stdin.on('error', () => {});
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (data) => console.error(`[otty-bridge] ${data.trimEnd()}`));
  child.once('error', (error) => fail(error.message));
  child.once('exit', (code) => {
    requests.failAll('Rust engine exited.');
    if (bridge === child) bridge = null;
    ready = null;
    if (quitting) return;
    // A clean exit is the backend closing its last window.
    if (code !== 0) fail(`Rust engine exited (${code ?? 'signal'}).`);
    else if (!mac) app.quit();
  });
}

function stopBridge() {
  quitting = true;
  write({ type: 'close' });
  const child = bridge;
  setTimeout(() => child?.kill(), 300);
}

// `at`: the screen point a torn-off tab was dropped at; its titlebar goes there.
function createWindow(at) {
  const window = new BrowserWindow({
    ...(at ? { x: Math.round(at.x - 120), y: Math.round(at.y - 16) } : {}),
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
  const state = { window, key: window.id, id: null, loaded: false, queue: [] };
  sessions.set(state.key, state);
  if (ready) send(state, 'bridge:event', ready);

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
    // Still known here means the user closed it: its tabs and shells go too.
    if (sessions.delete(state.key) && state.id && !quitting) write({ type: 'close', window: state.id });
  });
  window.loadFile(path.join(__dirname, 'renderer', 'index.html'));
  return state;
}

// Dev only: OTTY_SHOT=<png> captures every window after OTTY_SHOT_MS and quits
// (the first to that path, the others to <name>-2.png, …); OTTY_SHOT_EVAL runs
// a renderer snippet in the first window beforehand, to stage what is captured.
function scheduleShot() {
  if (!process.env.OTTY_SHOT || app.isPackaged) return;
  const windows = () => BrowserWindow.getAllWindows().sort((a, b) => a.id - b.id);
  setTimeout(async () => {
    try {
      if (process.env.OTTY_SHOT_EVAL) {
        console.log('[shot]', JSON.stringify(await windows()[0].webContents.executeJavaScript(process.env.OTTY_SHOT_EVAL)));
      }
      for (const [index, window] of windows().entries()) {
        const file = index ? process.env.OTTY_SHOT.replace(/(\.\w+)?$/, `-${index + 1}$1`) : process.env.OTTY_SHOT;
        fs.writeFileSync(file, (await window.webContents.capturePage()).toPNG());
      }
    } catch (error) {
      console.error(`[shot] ${error.message}`);
    }
    app.quit();
  }, Number(process.env.OTTY_SHOT_MS) || 5000);
}

function sessionFor(event) {
  const window = BrowserWindow.fromWebContents(event.sender);
  return window && sessions.get(window.id);
}

ipcMain.handle('bridge:request', (event, method, params) => {
  const state = sessionFor(event);
  if (!state || typeof method !== 'string') return { ok: false, error: 'Bad request.' };
  if (!bridge || bridge.exitCode !== null) return { ok: false, error: 'Rust engine is not running.' };
  // The asking window rides along; the backend acts on that one.
  return requests.request(method, params && typeof params === 'object' ? params : {}, state.id ? { window: state.id } : {});
});

ipcMain.on('bridge:send', (event, command) => {
  const state = sessionFor(event);
  if (!state || !command || typeof command.pane !== 'string') return;
  const { type, pane, text, width, height } = command;
  if (type === 'input' || type === 'paste') {
    // Key text is short; a paste may be a file's worth.
    if (typeof text !== 'string' || text.length > (type === 'input' ? 8192 : 1 << 20)) return;
    write({ type, pane, text });
  } else if (type === 'resize') {
    if (!Number.isInteger(width) || !Number.isInteger(height)) return;
    if (width < 1 || height < 1 || width > 8192 || height > 8192) return;
    const scale = Number.isFinite(command.scale) && command.scale > 0 ? command.scale : undefined;
    write({ type, pane, width, height, scale });
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
  // The first window is up before the engine is: startup shows something, and
  // an engine that will not start has somewhere to say so.
  createWindow();
  startBridge();
  scheduleShot();
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length) return;
    createWindow();
    if (bridge) requests.request('action.run', { id: 'new_window' });
    else startBridge();
  });
});

app.on('window-all-closed', () => {
  if (!mac) app.quit();
});

app.on('before-quit', stopBridge);
