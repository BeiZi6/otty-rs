// Stand-in for `otty-bridge`: protocol v2 over stdio with an in-memory model
// and solid-colour frames. Selected by OTTY_ENGINE_MOCK=1; PROTOCOL.md is the
// contract. No shell runs here — input is swallowed.
const readline = require('node:readline');
const path = require('node:path');
const { leaves, clampRatio } = require('./pane-geometry');
const { moveTab, activeAfterClose } = require('./tab-order');

const home = process.env.OTTY_CWD || process.cwd();
const scale = Number(process.env.OTTY_SCALE_FACTOR) || 1;
const COLOURS = [[24, 28, 38], [30, 36, 30], [40, 30, 34], [34, 32, 44], [28, 38, 40]];

// One table feeds menu.list, keymap.list, palette.list and action.run.
const ACTIONS = [
  ['File', 'new_window', 'New Window', 'cmd+n'],
  ['File', 'new_tab', 'New Tab', 'cmd+t'],
  ['File', 'duplicate_tab', 'Duplicate Tab', null],
  ['File', 'close_tab', 'Close Tab', 'cmd+w'],
  ['File', 'close_window', 'Close Window', 'cmd+shift+w'],
  ['File', 'reopen_closed_tab', 'Reopen Closed Tab', 'cmd+shift+t'],
  ['File', 'quit', 'Quit', null],
  ['Edit', 'copy', 'Copy', 'cmd+c'],
  ['Edit', 'paste', 'Paste', 'cmd+v'],
  ['View', 'command_palette', 'Command Palette', 'cmd+shift+p'],
  ['View', 'open_quickly', 'Open Quickly', 'cmd+shift+o'],
  ['View', 'open_settings', 'Settings', 'cmd+comma'],
  ['View', 'find', 'Find', 'cmd+f'],
  ['View', 'toggle_tabs_panel', 'Toggle Tabs Panel', 'cmd+shift+l'],
  ['View', 'toggle_details_panel', 'Toggle Details Panel', 'cmd+shift+r'],
  ['View', 'split_right', 'Split Right', 'cmd+d'],
  ['View', 'split_left', 'Split Left', 'cmd+opt+d'],
  ['View', 'split_down', 'Split Down', 'cmd+shift+d'],
  ['View', 'split_up', 'Split Up', 'cmd+opt+shift+d'],
  ['View', 'zoom_split', 'Zoom Split', 'cmd+shift+enter'],
  ['View', 'equalize_splits', 'Equalize Splits', 'cmd+ctrl+equal'],
  ['View', 'toggle_full_screen', 'Toggle Full Screen', 'cmd+ctrl+f'],
  ['Shell', 'rename_tab', 'Rename Tab...', null],
  ['Shell', 'clear_badge', 'Clear Badge', null],
  ['Window', 'minimize', 'Minimize', 'cmd+m'],
  ['Window', 'next_tab', 'Next Tab', 'cmd+shift+right_bracket'],
  ['Window', 'previous_tab', 'Previous Tab', 'cmd+shift+left_bracket'],
  ['Window', 'focus_next_pane', 'Focus Next Pane', 'cmd+right_bracket'],
  ['Window', 'focus_previous_pane', 'Focus Previous Pane', 'cmd+left_bracket'],
  ['Window', 'move_tab_to_new_window', 'Move Tab to New Window', null],
  ['Window', 'tabs_insert_divider', 'Tabs: Insert Divider', null],
  ['Window', 'tabs_remove_dividers', 'Tabs: Remove All Dividers', null],
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => ['Window', `goto_tab_${n}`, `Go to Tab ${n}`, `cmd+${n}`]),
];

const model = { tabs: [], panes: {}, activeTab: null, dividers: [], closed: [] };
const sizes = new Map();
let nextTab = 1;
let nextPane = 1;
let lastState = '';

function emit(event) {
  process.stdout.write(`${JSON.stringify(event)}\n`);
}

function newPane(cwd) {
  const id = `pane-${nextPane}`;
  nextPane += 1;
  model.panes[id] = { kind: 'terminal', title: 'mock', cwd, agent: null, grid: { cols: 80, rows: 24 } };
  return id;
}

function newTab(cwd = home, name = '') {
  const pane = newPane(cwd);
  const tab = { id: `tab-${nextTab}`, name, cwd, badge: null, focused: pane, zoomed: null, tree: { pane } };
  nextTab += 1;
  model.tabs.push(tab);
  model.activeTab = tab.id;
  return { tab: tab.id, pane };
}

const activeTab = () => model.tabs.find((tab) => tab.id === model.activeTab);
const tabOf = (pane) => model.tabs.find((tab) => leaves(tab.tree).includes(pane));

function need(found, what) {
  if (!found) throw new Error(`no such ${what}`);
  return found;
}

function tabById(id) {
  return need(model.tabs.find((tab) => tab.id === id), 'tab');
}

// `pane` omitted = the focused pane.
function paneId(pane) {
  return need(pane ? model.panes[pane] && pane : activeTab()?.focused, 'pane');
}

function replaceLeaf(tree, pane, make) {
  if (tree.pane) return tree.pane === pane ? make(tree) : tree;
  return { ...tree, first: replaceLeaf(tree.first, pane, make), second: replaceLeaf(tree.second, pane, make) };
}

// Closing a pane folds its parent split; the sibling takes the space.
function removeLeaf(tree, pane) {
  if (tree.pane) return tree.pane === pane ? null : tree;
  const first = removeLeaf(tree.first, pane);
  const second = removeLeaf(tree.second, pane);
  return first && second ? { ...tree, first, second } : first || second;
}

function setRatio(tree, pane, ratio) {
  if (tree.pane) return false;
  if (tree.first.pane === pane || tree.second.pane === pane) {
    tree.ratio = ratio;
    return true;
  }
  return setRatio(tree.first, pane, ratio) || setRatio(tree.second, pane, ratio);
}

function equalize(tree) {
  if (tree.pane) return;
  tree.ratio = 50;
  equalize(tree.first);
  equalize(tree.second);
}

function closeTab(id) {
  const tab = tabById(id);
  const order = model.tabs.map((item) => item.id);
  for (const pane of leaves(tab.tree)) delete model.panes[pane];
  model.closed.push({ cwd: tab.cwd, name: tab.name });
  model.closed.splice(0, model.closed.length - 12);
  model.tabs = model.tabs.filter((item) => item !== tab);
  if (!model.tabs.length) process.exit(0);
  const remaining = model.tabs.map((item) => item.id);
  if (model.activeTab === id) model.activeTab = activeAfterClose(order, id, remaining);
  return null;
}

function stepTab(direction) {
  const index = model.tabs.findIndex((tab) => tab.id === model.activeTab);
  model.activeTab = model.tabs[(index + direction + model.tabs.length) % model.tabs.length].id;
}

function stepPane(direction) {
  const tab = activeTab();
  const panes = leaves(tab.tree);
  tab.focused = panes[(panes.indexOf(tab.focused) + direction + panes.length) % panes.length];
}

const methods = {
  'action.run': ({ id }) => {
    const run = actions[id];
    if (run) run();
    return { handled: Boolean(run) };
  },
  'tab.new': ({ cwd }) => newTab(cwd || activeTab()?.cwd),
  'tab.activate': ({ tab }) => {
    model.activeTab = tabById(tab).id;
    return null;
  },
  'tab.close': ({ tab }) => closeTab(tab),
  'tab.move': ({ tab, target, after }) => {
    const order = moveTab(model.tabs.map((item) => item.id), tab, target, after === true);
    model.tabs.sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));
    return null;
  },
  'tab.rename': ({ tab, title }) => {
    tabById(tab).name = String(title || '');
    return null;
  },
  'tab.reopen': () => {
    const last = model.closed.pop();
    return last ? newTab(last.cwd, last.name) : null;
  },
  'pane.split': ({ pane, direction }) => {
    const target = paneId(pane);
    const tab = tabOf(target);
    const created = newPane(model.panes[target].cwd);
    const before = direction === 'left' || direction === 'up';
    tab.tree = replaceLeaf(tab.tree, target, (leaf) => ({
      axis: direction === 'left' || direction === 'right' ? 'horizontal' : 'vertical',
      ratio: 50,
      first: before ? { pane: created } : leaf,
      second: before ? leaf : { pane: created },
    }));
    tab.focused = created;
    tab.zoomed = null;
    return { pane: created };
  },
  'pane.focus': ({ pane }) => {
    const tab = need(tabOf(pane), 'pane');
    tab.focused = pane;
    model.activeTab = tab.id;
    return null;
  },
  'pane.close': ({ pane }) => {
    const target = paneId(pane);
    const tab = tabOf(target);
    const panes = leaves(tab.tree);
    if (panes.length === 1) return closeTab(tab.id);
    tab.tree = removeLeaf(tab.tree, target);
    delete model.panes[target];
    sizes.delete(target);
    if (tab.zoomed === target) tab.zoomed = null;
    if (tab.focused === target) tab.focused = leaves(tab.tree)[0];
    return null;
  },
  'pane.zoom': ({ pane }) => {
    const target = paneId(pane);
    const tab = tabOf(target);
    tab.zoomed = tab.zoomed === target || leaves(tab.tree).length === 1 ? null : target;
    tab.focused = target;
    return null;
  },
  'pane.ratio': ({ pane, ratio }) => {
    setRatio(need(tabOf(pane), 'pane').tree, pane, clampRatio(ratio));
    return null;
  },
  'pane.text': ({ pane }) => ({ rows: [`mock ${paneId(pane)}`] }),
  'pane.select': ({ pane, from, to }) => ({ text: `${paneId(pane)} ${from}..${to}` }),
  'pane.context': ({ selection }) => [
    ...(selection ? [{ label: 'Copy', action: 'copy' }] : []),
    { label: 'Paste', action: 'paste' },
    { label: 'Split Right', action: 'split_right' },
    { label: 'Split Down', action: 'split_down' },
    { label: 'Close', action: 'close_tab' },
  ],
  'palette.list': ({ query }) => ACTIONS
    .filter(([, id, title]) => `${id} ${title}`.toLowerCase().includes(String(query || '').toLowerCase()))
    .map(([, id, title, shortcut]) => ({ id, title, scope: 'global', shortcut })),
  'keymap.list': () => ACTIONS.filter((row) => row[3]).map(([, action, , trigger]) => ({ trigger, action })),
  'menu.list': () => [...new Set(ACTIONS.map((row) => row[0]))].map((title) => ({
    title,
    items: ACTIONS.filter((row) => row[0] === title).map(([, action, label, shortcut]) => ({ label, action, shortcut })),
  })),
  'i18n.strings': () => ({ language: 'en', strings: {} }),
};

const actions = {
  new_tab: () => newTab(activeTab()?.cwd),
  duplicate_tab: () => newTab(activeTab().cwd, activeTab().name),
  // ⌘W closes the focused pane first, the tab when it is the last one.
  close_tab: () => methods['pane.close']({}),
  reopen_closed_tab: () => methods['tab.reopen']({}),
  next_tab: () => stepTab(1),
  previous_tab: () => stepTab(-1),
  focus_next_pane: () => stepPane(1),
  focus_previous_pane: () => stepPane(-1),
  zoom_split: () => methods['pane.zoom']({}),
  equalize_splits: () => equalize(activeTab().tree),
  clear_badge: () => { activeTab().badge = null; },
  tabs_insert_divider: () => {
    const index = model.tabs.indexOf(activeTab());
    if (!model.dividers.includes(index)) model.dividers.push(index);
  },
  tabs_remove_dividers: () => { model.dividers = []; },
  quit: () => emit({ type: 'quit' }),
};
for (const direction of ['right', 'left', 'down', 'up']) {
  actions[`split_${direction}`] = () => methods['pane.split']({ direction });
}
for (let n = 1; n <= 9; n += 1) {
  actions[`goto_tab_${n}`] = () => {
    const tab = n === 9 ? model.tabs.at(-1) : model.tabs[n - 1];
    if (tab) model.activeTab = tab.id;
  };
}

function emitState() {
  const state = {
    type: 'state',
    activeTab: model.activeTab,
    tabs: model.tabs.map((tab) => ({
      id: tab.id,
      title: tab.name || `mock ${path.basename(tab.cwd)}`,
      cwd: model.panes[tab.focused].cwd,
      project: tab.cwd,
      icon: 'terminal',
      badge: tab.badge,
      focused: tab.focused,
      zoomed: tab.zoomed,
      tree: tab.tree,
    })),
    panes: model.panes,
    tabGroup: 'none',
    dividers: model.dividers,
    mode: 'shell',
  };
  const line = JSON.stringify(state);
  if (line === lastState) return;
  lastState = line;
  emit(state);
}

function emitFrame(pane) {
  const size = sizes.get(pane);
  if (!size || !activeTab() || !leaves(activeTab().tree).includes(pane)) return;
  const colour = COLOURS[Number(pane.split('-')[1]) % COLOURS.length];
  const pixels = Buffer.alloc(size.width * size.height * 4, Buffer.from([...colour, 255]));
  emit({ type: 'frame', pane, ...size, pixels: pixels.toString('base64') });
}

function onCommand(command) {
  if (command.type === 'close') process.exit(0);
  if (command.type === 'resize' && model.panes[command.pane]) {
    const { pane, width, height } = command;
    sizes.set(pane, { width, height });
    model.panes[pane].grid = {
      cols: Math.max(1, Math.floor(width / (8 * scale))),
      rows: Math.max(1, Math.floor(height / (17 * scale))),
    };
    emitFrame(pane);
  } else if (command.type === 'request') {
    const { id, method, params } = command;
    try {
      const run = methods[method];
      if (!run) throw new Error('not implemented');
      emit({ type: 'response', id, ok: true, result: run(params || {}) ?? null });
    } catch (error) {
      emit({ type: 'response', id, ok: false, error: error.message });
    }
  }
  emitState();
}

// A demo layout so the chrome has something to draw: a split tab, a badge,
// and a second project.
const first = newTab(home);
methods['pane.split']({ pane: first.pane, direction: 'right' });
newTab(home).tab;
model.tabs[1].badge = 'running';
newTab(path.dirname(home));
model.tabs[2].badge = 'completed';
model.activeTab = first.tab;

emit({ type: 'ready', version: 'mock', chrome: {}, layout: 'sidebar-left', sidebar_visible: true,
  platform: { win32: 'windows', darwin: 'macos' }[process.platform] || 'linux' });
emitState();

readline.createInterface({ input: process.stdin })
  .on('line', (line) => {
    try {
      onCommand(JSON.parse(line));
    } catch (error) {
      emit({ type: 'error', message: `mock bridge: ${error.message}` });
    }
  })
  .on('close', () => process.exit(0));
