// Viewer for the panes the core draws no canvas for: file, folder and URL.
// The pane carries `path` (file, folder) or `url`.
OttyFeatures.register('fileview', (ctx) => {
  const F = globalThis.OttyForm;
  const { h } = F;
  const t = ctx.t;
  // ponytail: one plain <pre> per file (the backend refuses files over 2 MiB);
  // virtualise rows and colour by `language` when that is not enough.
  const HEX_LIMIT = 64 * 1024;
  let root = null;
  let shown = '';

  const message = (text) => h('p', { class: 'fileview-msg', role: 'status' }, text);
  const quiet = (box) => (error) => box.replaceChildren(message(F.errorText(error, t)));
  const open = (kind, value, note) => ctx.request('quickly.open', { kind, value })
    .catch((error) => { note.textContent = F.errorText(error, t); });

  const views = {
    file(body, path) {
      ctx.request('file.read', { path }).then((file) => {
        const binary = file.kind === 'binary';
        // The backend sends no bytes for binary files today; dump them if it ever does.
        if (binary && !file.body) {
          body.replaceChildren(message(t('fileview.binary', 'Binary file — no preview available.')));
          return;
        }
        const bytes = binary ? F.bytesOf(file.body) : null;
        const text = binary ? F.hexDump(bytes.subarray(0, HEX_LIMIT)) : String(file.body ?? '');
        const lines = text.split('\n').length;
        body.replaceChildren(h('div', {},
          binary && bytes.length > HEX_LIMIT && message(t('fileview.truncated', 'Showing the beginning of a large file.')),
          h('div', { class: 'fileview-code' },
            !binary && h('pre', { class: 'fileview-gutter', 'aria-hidden': 'true' },
              Array.from({ length: lines }, (_, index) => index + 1).join('\n')),
            h('pre', { class: 'fileview-text', tabIndex: 0, 'aria-label': path }, text))));
      }, quiet(body));
    },

    folder(body, path) {
      const note = h('p', { class: 'fileview-msg', role: 'status' });
      const list = h('ul', { class: 'fileview-list' });
      let entries = [];
      const label = t('settings.data.filter', 'Filter');
      const paint = () => {
        const query = filter.value.trim().toLowerCase();
        const hits = entries.filter((entry) => entry.name.toLowerCase().includes(query));
        list.replaceChildren(...hits.map((entry) => h('li', {},
          h('button', { type: 'button', onclick: () => open('path', F.joinPath(path, entry.name), note) },
            entry.name + (entry.dir ? '/' : '')))));
        note.textContent = hits.length ? '' : t('open_quickly.no_results', 'No results');
      };
      const filter = h('input', {
        class: 'fileview-filter', type: 'search', placeholder: label, 'aria-label': label, spellcheck: false,
        oninput: paint,
        onkeydown: (event) => { if (event.key === 'Escape') ctx.focusTerminal(); },
      });
      ctx.request('details.files', { path }).then((result) => {
        entries = result || [];
        body.replaceChildren(filter, list, note);
        paint();
      }, quiet(body));
    },

    url(body, url) {
      const note = h('p', { class: 'fileview-msg', role: 'status' });
      body.replaceChildren(
        message(t('fileview.url_notice', 'Web pages open in your browser.')),
        h('button', { type: 'button', class: 'fileview-button', onclick: () => open('url', url, note) },
          t('open_quickly.action.open_in_browser', 'Open in Browser')),
        note,
      );
    },
  };

  function sync(state) {
    const tab = state && (state.tabs || []).find((item) => item.id === state.activeTab);
    const id = (tab && tab.focused) || ctx.focusedPane();
    const pane = state && state.panes && state.panes[id];
    const target = pane && (pane.path || pane.url || pane.title);
    const key = pane && views[pane.kind] && target ? `${id}|${pane.kind}|${target}` : '';
    if (key === shown) return;
    shown = key;
    if (root) root.remove();
    root = null;
    if (!key) return;
    const body = h('div', { class: 'fileview-body' }, message(t('settings.data.loading', 'Loading…')));
    root = ctx.mount('pane-overlay');
    root.append(h('section', { class: 'fileview', 'aria-label': target },
      h('header', { class: 'fileview-path' }, target), body));
    views[pane.kind](body, target);
  }

  ctx.on('state', sync);
  sync(ctx.state());
});
