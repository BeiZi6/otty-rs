// Open Quickly (⌘⇧O): one fuzzy field over nine filters; the backend owns the sources.
OttyFeatures.register('quickly', (ctx) => {
  const { h, marked, fuzzy, pinFirst, shortcutLabel } = OttyList;
  const t = ctx.t;
  const mac = ctx.platform === 'macos';

  // [filter, label, key] — the chords only exist while the overlay is open.
  const FILTERS = [
    ['all', 'All', '0'], ['opened', 'Opened', 'w'], ['recent', 'Recent', 'r'], ['folders', 'Folders', 'z'], ['ssh', 'SSH', 's'],
    ['agents', 'Agents', 'g'], ['current', 'Current', 'j'], ['recipes', 'Recipes', 'e'], ['files', 'Files', 'p'],
  ];
  const OPENERS = { open_quickly: 'all', open_files_quickly: 'files', open_recent: 'recent', open_agent: 'agents', open_folder: 'folders' };
  // ⌘K sub-list. `target` rides along on quickly.open; "copy" stays local.
  // `fork` is an agent conversation's: a second one, started from it.
  const ACTIONS = [['', 'Open'], ['tab', 'Open in New Tab'], ['split-right', 'Open in Split Right'], ['fork', 'Fork in New Tab'], ['copy', 'Copy Path']];

  let filter = 'all';
  let target = null; // the row whose Actions list is up
  let chips = null;

  function paint() {
    if (chips) Array.from(chips.children).forEach((chip, n) => chip.setAttribute('aria-pressed', String(FILTERS[n][0] === filter)));
  }

  function setFilter(next) {
    filter = next;
    target = null;
    paint();
    if (list.isOpen()) list.refresh();
    else list.open();
  }

  function act(row, how) {
    if (how === 'copy') return navigator.clipboard.writeText(String(row.value));
    return ctx.request('quickly.open', how ? { kind: row.kind, value: row.value, target: how } : { kind: row.kind, value: row.value });
  }

  const list = OttyList.picker(ctx, {
    name: 'quickly',
    label: t('quickly.label', 'Open Quickly'),
    actions: Object.keys(OPENERS),
    placeholder: () => (target ? `${t('quickly.actions', 'Actions')}: ${target.title}` : t('quickly.placeholder', 'Open Quickly')),
    load(query) {
      if (target) {
        return ACTIONS.filter(([how]) => how !== 'fork' || target.kind === 'agent')
          .map(([how, label]) => ({ how, title: t(`quickly.action.${how || 'open'}`, label) })).filter((row) => fuzzy(row.title, query));
      }
      return ctx.request('quickly.query', { filter, query }).then((rows) => (
        // A typed path that exists comes back as a `path` row: pin it.
        /^[/~]/.test(query) ? pinFirst(rows || [], (row) => row.kind === 'path') : rows
      ));
    },
    row: (item, query) => [
      h('span', { class: 'quickly-title' }, marked(item.title, query, 'quickly-hit')),
      item.subtitle && h('span', { class: 'quickly-subtitle' }, item.subtitle),
      item.kind && h('span', { class: 'quickly-kind' }, t(`quickly.kind.${item.kind}`, item.kind)),
    ],
    run(item) {
      const row = target || item;
      list.exec(() => act(row, item.how));
    },
    key(event, item) {
      if (event.key === 'Escape' && target) {
        target = null;
        list.open();
        return true;
      }
      if (!(mac ? event.metaKey : event.ctrlKey) || event.shiftKey || event.altKey) return false;
      const key = event.key.toLowerCase();
      const hit = FILTERS.find((entry) => entry[2] === key);
      if (hit) setFilter(hit[0]);
      else if (key === 'k' && item && !target) {
        target = item;
        list.open();
      } else return false;
      return true;
    },
    head() {
      chips = h('div', { class: 'quickly-filters', role: 'group', 'aria-label': t('quickly.filters', 'Filters') },
        FILTERS.map(([id, label, key]) => h('button', {
          class: 'quickly-chip', type: 'button', tabindex: '-1',
          onmousedown: (event) => { event.preventDefault(); setFilter(id); },
        }, t(`quickly.filter.${id}`, label), h('kbd', { class: 'quickly-key' }, shortcutLabel(`cmd+${key}`, mac)))));
      paint();
      return chips;
    },
  });

  for (const [id, preset] of Object.entries(OPENERS)) {
    ctx.registerAction(id, () => {
      if (list.isOpen() && filter === preset && !target) list.close();
      else setFilter(preset);
    });
  }
});
