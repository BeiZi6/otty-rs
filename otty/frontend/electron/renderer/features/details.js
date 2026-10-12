// Details panel: Info / Outline / Git / Files for whichever pane has focus.
OttyFeatures.register('details', (ctx) => {
  const F = globalThis.OttyForm;
  const { h } = F;
  const t = ctx.t;
  // id, view-icon stem, en.json key, fallback
  const TABS = [
    ['info', 'info', 'menu.details_info', 'Info'],
    ['outline', 'list', 'menu.details_outline', 'Outline'],
    ['git', 'git-branch', 'menu.details_git', 'Git'],
    ['files', 'folder', 'menu.details_files', 'Files'],
  ];
  let root = null;
  let panel = null;
  let buttons = [];
  let tab = 'info';
  let followed = '';
  let serial = 0;

  // Icons are masks so they take the text colour of whatever theme is active.
  function icon(stem) {
    const node = h('span', { class: 'details-icon', 'aria-hidden': 'true' });
    const url = `url("../../../resources/view-icons/${stem}.svg")`;
    node.style.setProperty('-webkit-mask-image', url);
    node.style.setProperty('mask-image', url);
    return node;
  }

  const message = (text) => h('p', { class: 'details-msg', role: 'status' }, text);
  const heading = (text) => h('h3', { class: 'details-heading' }, text);
  const paneParams = () => (ctx.focusedPane() ? { pane: ctx.focusedPane() } : {});
  const run = (id) => {
    try { Promise.resolve(ctx.runAction(id)).catch(() => {}); } catch { /* nothing to run it */ }
  };

  const views = {
    async info() {
      const info = await ctx.request('details.info', paneParams());
      const agent = typeof info.agent === 'string' ? { name: info.agent } : info.agent;
      const processes = info.processes || [];
      const ports = info.ports || [];
      const note = h('span', { class: 'details-msg', role: 'status' });
      return [
        heading(t('settings.general.working_directory_group', 'Working Directory')),
        h('code', { class: 'details-path' }, info.cwd || '—'),
        heading(t('details.processes', 'Processes')),
        processes.length
          ? h('ul', { class: 'details-list' }, processes.map((name) => h('li', {}, name)))
          : message('—'),
        // The backend reports no ports yet; the row appears once it does.
        ports.length > 0 && [
          heading(t('menu.ports', 'Ports')),
          h('div', { class: 'details-row' }, ports.map((port) => h('button', {
            type: 'button',
            class: 'details-chip',
            onclick: () => ctx.request('quickly.open', { kind: 'url', value: `http://localhost:${port}` })
              .catch((error) => { note.textContent = F.errorText(error, t); }),
          }, `:${port}`)), note),
        ],
        heading(t('menu.agent', 'Agent')),
        agent
          ? h('div', { class: 'details-row' },
            h('strong', {}, agent.name || ''),
            agent.state && h('span', { class: `details-state details-state-${agent.state}` }, agent.state),
            // Plan quota, when the backend reports one (percent used; 75 warns, 90 is critical).
            Number.isFinite(agent.quota) && h('meter', {
              min: 0, max: 100, low: 75, high: 90, optimum: 0, value: agent.quota, 'aria-label': `${agent.quota}%`,
            }))
          : message(t('menu.not_running', 'Not Running')),
        heading(t('menu.open_in', 'Open in')),
        h('div', { class: 'details-row' }, [
          ctx.platform === 'macos'
            ? ['reveal_in_finder', 'menu.reveal_in_finder', 'Reveal in Finder']
            : ['reveal_in_finder', 'details.reveal_in_explorer', 'Reveal in File Explorer'],
          ['open_with_default_app', 'settings.key_bindings.actions.open_with_default_app', 'Open with Default App'],
          ['copy_path', 'menu.copy_path', 'Copy Path'],
        ].map(([id, key, fallback]) => h('button', { type: 'button', class: 'details-chip', onclick: () => run(id) }, t(key, fallback)))),
      ];
    },

    async outline() {
      const rows = await ctx.request('details.outline', paneParams());
      if (!rows || !rows.length) return [message(t('menu.no_line_info', 'No Line Info'))];
      return [h('ul', { class: 'details-list' }, rows.map((row) => h('li', {}, h('button', {
        type: 'button',
        class: `details-item details-kind-${row.kind || 'command'}`,
        // The protocol has no scroll-to-line; a find from that line is the closest read-only jump.
        onclick: () => ctx.request('find.next', { ...paneParams(), query: row.title, from: { line: row.line, col: 0 } }).catch(() => {}),
      }, h('span', { class: 'details-line' }, String(row.line)), row.title))))];
    },

    async git() {
      const git = await ctx.request('details.git', paneParams());
      if (!git) return [message(t('menu.not_a_git_repo', 'Not a Git Repository'))];
      const changes = git.changes || [];
      return [
        h('div', { class: 'details-row' }, icon('git-branch'), h('strong', {}, git.branch || '—')),
        h('code', { class: 'details-path' }, git.root || ''),
        changes.length
          ? h('ul', { class: 'details-list' }, changes.map((change) => h('li', { class: 'details-change' },
            h('span', { class: 'details-status', title: change.status }, String(change.status || '?').trim().charAt(0).toUpperCase() || '?'),
            change.path)))
          : message(t('details.no_changes', 'No changes')),
      ];
    },

    async files() {
      const state = ctx.state();
      const pane = state && state.panes && state.panes[ctx.focusedPane()];
      if (!pane || !pane.cwd) return [message(t('menu.no_active_tab', 'No Active Tab'))];
      const note = message('');
      const tree = h('ul', {
        class: 'details-tree', role: 'tree', 'aria-label': pane.cwd,
        onclick: (event) => {
          const item = event.target.closest('[role=treeitem]');
          if (item) { focusItem(tree, item); activate(item, note); }
        },
        onkeydown: (event) => treeKey(tree, event, note),
      });
      fill(tree, pane.cwd);
      return [tree, note];
    },
  };

  // Files tree: folders load on first expand.
  function fill(list, path) {
    ctx.request('details.files', { path }).then((entries) => {
      const sorted = [...(entries || [])].sort((a, b) => b.dir - a.dir || a.name.localeCompare(b.name));
      list.replaceChildren(...sorted.map((entry) => {
        const item = h('li', { role: 'treeitem', tabIndex: -1, class: 'details-node' },
          h('span', { class: 'details-label' }, icon(entry.dir ? 'folder' : 'file'), entry.name));
        item.dataset.path = F.joinPath(path, entry.name);
        if (entry.dir) item.setAttribute('aria-expanded', 'false');
        return item;
      }));
      if (!sorted.length) list.append(h('li', { class: 'details-msg', role: 'none' }, '—'));
      else if (list.getAttribute('role') === 'tree') list.firstChild.tabIndex = 0;
    }, (error) => list.replaceChildren(h('li', { class: 'details-msg', role: 'none' }, F.errorText(error, t))));
  }

  // expand: true / false for folders, undefined = toggle a folder or open a file.
  function activate(item, note, expand) {
    if (!item.hasAttribute('aria-expanded')) {
      if (expand === undefined) {
        ctx.request('quickly.open', { kind: 'path', value: item.dataset.path })
          .then(() => { note.textContent = ''; }, (error) => { note.textContent = F.errorText(error, t); });
      }
      return;
    }
    const open = expand === undefined ? item.getAttribute('aria-expanded') !== 'true' : expand;
    let group = item.querySelector(':scope > ul');
    if (open && !group) {
      group = h('ul', { role: 'group' });
      item.append(group);
      fill(group, item.dataset.path);
    }
    if (group) group.hidden = !open;
    item.setAttribute('aria-expanded', String(open));
  }

  function focusItem(tree, item) {
    for (const node of tree.querySelectorAll('[tabindex="0"]')) node.tabIndex = -1;
    item.tabIndex = 0;
    item.focus();
  }

  function treeKey(tree, event, note) {
    const item = event.target.closest('[role=treeitem]');
    if (!item) return;
    const visible = [...tree.querySelectorAll('[role=treeitem]')].filter((node) => !node.closest('ul[hidden]'));
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (step) {
      const next = visible[visible.indexOf(item) + step];
      if (next) focusItem(tree, next);
    } else if (event.key === 'ArrowRight') activate(item, note, true);
    else if (event.key === 'ArrowLeft') activate(item, note, false);
    else if (event.key === 'Enter' || event.key === ' ') activate(item, note);
    else return;
    event.preventDefault();
    event.stopPropagation();
  }

  function load() {
    if (!root) return;
    const mine = ++serial;
    const show = (nodes) => {
      if (root && mine === serial) panel.replaceChildren(...nodes.flat().filter(Boolean));
    };
    views[tab]().then(show, (error) => show([message(F.errorText(error, t))]));
  }
  const reload = F.debounce(load, 120);

  function paintTabs() {
    buttons.forEach((button, index) => {
      const selected = TABS[index][0] === tab;
      button.setAttribute('aria-selected', String(selected));
      button.tabIndex = selected ? 0 : -1;
    });
    panel.setAttribute('aria-labelledby', `details-tab-${tab}`);
  }

  function select(id, focus) {
    tab = id;
    if (!root) return show();
    paintTabs();
    if (focus) buttons[TABS.findIndex((item) => item[0] === id)].focus();
    panel.replaceChildren(message(t('settings.data.loading', 'Loading…')));
    load();
  }

  function show() {
    root = ctx.mount('right-panel');
    panel = h('div', { class: 'details-panel', id: 'details-panel', role: 'tabpanel' });
    buttons = TABS.map(([id, stem, key, fallback]) => h('button', {
      type: 'button', role: 'tab', id: `details-tab-${id}`, class: 'details-tab',
      title: t(key, fallback), 'aria-label': t(key, fallback), 'aria-controls': 'details-panel',
      onclick: () => select(id),
    }, icon(stem)));
    root.append(h('aside', {
      class: 'details', 'aria-label': t('details.title', 'Details'),
      // Not modal, so no captureKeys: shortcuts keep working with focus in here.
      onkeydown: (event) => { if (event.key === 'Escape') ctx.focusTerminal(); },
    }, h('div', {
      class: 'details-tabs', role: 'tablist', 'aria-label': t('details.title', 'Details'),
      onkeydown: (event) => {
        const step = { ArrowRight: 1, ArrowLeft: -1 }[event.key];
        if (!step) return;
        const at = TABS.findIndex((item) => item[0] === tab);
        select(TABS[(at + step + TABS.length) % TABS.length][0], true);
        event.preventDefault();
        event.stopPropagation();
      },
    }, buttons), panel));
    select(tab);
  }

  function hide() {
    root.remove();
    root = null;
    ctx.focusTerminal();
  }

  ctx.registerAction('toggle_details_panel', () => (root ? hide() : show()));
  for (const [id] of TABS) ctx.registerAction(`details_tab_${id}`, () => select(id));

  // Follow the focused pane: refetch when it, its cwd or (for Info) its agent state changes.
  ctx.on('state', (state) => {
    if (!root) return;
    const active = (state.tabs || []).find((item) => item.id === state.activeTab);
    const id = (active && active.focused) || ctx.focusedPane();
    const pane = (state.panes || {})[id] || {};
    const key = [id, pane.cwd, tab === 'info' && pane.agent && pane.agent.state].join('|');
    if (key === followed) return;
    followed = key;
    reload();
  });
  window.addEventListener('otty-strings', () => {
    if (!root) return;
    root.remove();
    show();
  });
});
