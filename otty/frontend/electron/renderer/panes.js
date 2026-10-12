// The active tab's split tree: one canvas per pane, dividers, selection and
// file drops. The tree itself is the backend's; this only lays it out.
const terminalSurface = document.querySelector('#terminal-surface');
const paneOverlay = document.querySelector('#slot-pane-overlay');
const geometry = globalThis.OttyPaneGeometry;
const paneViews = new Map();
const dividerEls = [];
// {pane, from:[col,row], to:[col,row]} — drawn here, read back with `pane.select`.
let selection = null;

const percent = (value) => `${value * 100}%`;

function place(el, rect) {
  Object.assign(el.style, { left: percent(rect.x), top: percent(rect.y), width: percent(rect.w), height: percent(rect.h) });
}

function focusedPane() {
  return activeTab()?.focused ?? null;
}

function paintFrame(frame) {
  const view = paneViews.get(frame.pane);
  if (!view || !Number.isInteger(frame.width) || !Number.isInteger(frame.height)) return;
  const bytes = Uint8Array.fromBase64(frame.pixels);
  if (bytes.length !== frame.width * frame.height * 4) {
    showNotice('Rust engine returned a frame with an invalid pixel size.');
    return;
  }
  if (view.canvas.width !== frame.width || view.canvas.height !== frame.height) {
    view.canvas.width = frame.width;
    view.canvas.height = frame.height;
  }
  view.context.putImageData(new ImageData(new Uint8ClampedArray(bytes.buffer), frame.width, frame.height), 0, 0);
}

// ponytail: cells are taken as canvas ÷ grid, ignoring chrome.pane_padding, and
// the marks do not scroll with output; move selection into the backend's frame
// when it grows a selection model.
function drawSelection() {
  for (const view of paneViews.values()) view.marks.replaceChildren();
  const view = selection && paneViews.get(selection.pane);
  const grid = selection && state.panes[selection.pane]?.grid;
  if (!view || !grid) return;
  for (const span of geometry.selectionSpans(selection.from, selection.to, grid.cols)) {
    const mark = document.createElement('div');
    place(mark, {
      x: span.from / grid.cols,
      y: span.row / grid.rows,
      w: (span.to - span.from + 1) / grid.cols,
      h: 1 / grid.rows,
    });
    view.marks.append(mark);
  }
}

function clearSelection() {
  if (!selection) return;
  selection = null;
  drawSelection();
}

async function selectedText(pane = selection?.pane) {
  if (!selection || selection.pane !== pane) return '';
  const { from, to } = selection;
  return (await request('pane.select', { pane, from, to })).text || '';
}

function bindPaneMouse(view, id) {
  let anchor = null;
  const cell = (event) => {
    const box = view.el.getBoundingClientRect();
    const grid = state.panes[id]?.grid || { cols: 1, rows: 1 };
    return geometry.cellAt(event.clientX - box.left, event.clientY - box.top, box.width, box.height, grid);
  };
  view.el.addEventListener('pointerdown', (event) => {
    if (focusedPane() !== id) request('pane.focus', { pane: id }).catch(report);
    focusTerminal();
    if (event.button !== 0) return;
    clearSelection();
    anchor = cell(event);
    view.el.setPointerCapture(event.pointerId);
  });
  view.el.addEventListener('pointermove', (event) => {
    if (!anchor) return;
    const to = cell(event);
    if (!selection && to[0] === anchor[0] && to[1] === anchor[1]) return;
    selection = { pane: id, from: anchor, to };
    drawSelection();
  });
  view.el.addEventListener('pointerup', () => {
    anchor = null;
  });
  // Three lines a notch into scrollback; the backend stops at both ends.
  view.el.addEventListener('wheel', (event) => {
    const delta = Math.round(-event.deltaY / 33) || -Math.sign(event.deltaY);
    if (!delta) return;
    clearSelection();
    request('pane.scroll', { pane: id, delta }).catch(report);
  }, { passive: true });
  view.el.addEventListener('contextmenu', async (event) => {
    event.preventDefault();
    const [col, row] = cell(event);
    try {
      const text = await selectedText(id);
      showMenu(await request('pane.context', { pane: id, col, row, selection: text || undefined }));
    } catch (error) {
      report(error);
    }
  });
}

function createPaneView(id) {
  const el = document.createElement('div');
  el.className = 'pane';
  el.dataset.pane = id;
  const canvas = document.createElement('canvas');
  canvas.setAttribute('aria-label', 'Terminal output');
  const marks = document.createElement('div');
  marks.className = 'pane-selection';
  el.append(canvas, marks);
  const view = { el, canvas, marks, context: canvas.getContext('2d', { alpha: false }), size: '', timer: 0 };
  // Each pane reports its own canvas in physical px (PROTOCOL.md `resize`).
  view.observer = new ResizeObserver(([entry]) => {
    const scale = window.devicePixelRatio || 1;
    const width = Math.round(entry.contentRect.width * scale);
    const height = Math.round(entry.contentRect.height * scale);
    clearTimeout(view.timer);
    view.timer = setTimeout(() => {
      const size = `${width}x${height}`;
      if (width < 1 || height < 1 || size === view.size) return;
      view.size = size;
      window.otty.send({ type: 'resize', pane: id, width, height, scale });
    }, 45);
  });
  view.observer.observe(el);
  bindPaneMouse(view, id);
  terminalSurface.prepend(el);
  paneViews.set(id, view);
  return view;
}

function createDivider() {
  const el = document.createElement('div');
  el.setAttribute('role', 'separator');
  let sent = 0;
  const setRatio = (ratio) => {
    if (ratio === sent) return;
    sent = ratio;
    request('pane.ratio', { pane: el.divider.pane, with: el.divider.with, ratio }).catch(report);
  };
  el.addEventListener('pointerdown', (event) => {
    event.preventDefault();
    sent = 0;
    el.setPointerCapture(event.pointerId);
  });
  el.addEventListener('pointermove', (event) => {
    if (!el.hasPointerCapture(event.pointerId)) return;
    const box = terminalSurface.getBoundingClientRect();
    setRatio(geometry.ratioAt(el.divider, (event.clientX - box.left) / box.width, (event.clientY - box.top) / box.height));
  });
  el.addEventListener('dblclick', () => {
    sent = 0;
    setRatio(50);
  });
  terminalSurface.insertBefore(el, paneOverlay);
  dividerEls.push(el);
  return el;
}

function renderPanes() {
  const tab = activeTab();
  const tree = !tab ? null : tab.zoomed ? { pane: tab.zoomed } : tab.tree;
  const { panes, dividers } = geometry.layout(tree);
  const live = new Set(panes.map((rect) => rect.pane));
  // Frames only come for the active tab, so other tabs keep no canvas.
  for (const [id, view] of paneViews) {
    if (live.has(id)) continue;
    view.observer.disconnect();
    clearTimeout(view.timer);
    view.el.remove();
    paneViews.delete(id);
  }
  place(paneOverlay, { x: 0, y: 0, w: 1, h: 1 });
  for (const rect of panes) {
    const view = paneViews.get(rect.pane) || createPaneView(rect.pane);
    const focused = rect.pane === tab.focused;
    place(view.el, rect);
    view.el.classList.toggle('focused', focused);
    if (focused) place(paneOverlay, rect);
  }
  dividers.forEach((divider, index) => {
    const el = dividerEls[index] || createDivider();
    el.divider = divider;
    el.hidden = false;
    el.className = `split-divider ${divider.horizontal ? 'horizontal' : 'vertical'}`;
    el.setAttribute('aria-orientation', divider.horizontal ? 'vertical' : 'horizontal');
    place(el, divider.horizontal
      ? { x: divider.at, y: divider.rect.y, w: 0, h: divider.rect.h }
      : { x: divider.rect.x, y: divider.at, w: divider.rect.w, h: 0 });
  });
  for (const el of dividerEls.slice(dividers.length)) el.hidden = true;
  if (selection && !live.has(selection.pane)) selection = null;
  drawSelection();
}

function quotePath(target) {
  return platform === 'windows' ? `"${target}"` : `'${target.replaceAll("'", "'\\''")}'`;
}

// ponytail: GAP-S14's four coloured edge zones collapse to one rule — a folder
// opens a tab there, a file's path is typed into the pane under the pointer.
// Add the zones when file/folder panes (GAP-S08) give the blue ones a target.
const hasFiles = (event) => event.dataTransfer?.types.includes('Files');
window.addEventListener('dragover', (event) => {
  if (!hasFiles(event)) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = 'copy';
  appShell.classList.add('file-drag');
});
window.addEventListener('dragleave', (event) => {
  if (!event.relatedTarget) appShell.classList.remove('file-drag');
});
window.addEventListener('drop', async (event) => {
  if (!hasFiles(event)) return;
  event.preventDefault();
  appShell.classList.remove('file-drag');
  const pane = event.target.closest?.('.pane')?.dataset.pane || focusedPane();
  const dropped = [...event.dataTransfer.files].map((file) => window.otty.pathForFile(file)).filter(Boolean);
  const files = [];
  for (const target of dropped) {
    if (await window.otty.isDirectory(target)) request('tab.new', { cwd: target }).catch(report);
    else files.push(quotePath(target));
  }
  if (files.length && pane) window.otty.send({ type: 'input', pane, text: `${files.join(' ')} ` });
});

// Clicking a pane must not take focus from the IME textarea.
terminalSurface.addEventListener('mousedown', (event) => {
  if (event.target.closest('.pane')) event.preventDefault();
});
