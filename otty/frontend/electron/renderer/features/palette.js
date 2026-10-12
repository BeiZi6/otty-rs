// Command Palette (⌘⇧P): every action the backend lists, run through ctx.runAction.
OttyFeatures.register('palette', (ctx) => {
  const { h, marked, fuzzy, shortcutLabel } = OttyList;
  const t = ctx.t;
  const mac = ctx.platform === 'macos';

  // "Split View…", "Fork…" and "Branch…" pick a destination before they run.
  const WHERE = ['Right', 'Left', 'Up', 'Down', 'New Tab', 'New Window'];
  const FORK = ['split_right', 'split_left', 'split_up', 'split_down', 'new_tab', 'new_window'].map((id) => `agent_fork_${id}`);
  const STAGED = {
    split_view: ['split_right', 'split_left', 'split_up', 'split_down', 'new_tab', 'new_window'],
    agent_fork: FORK,
    agent_branch: FORK,
  };
  let stage = null; // { title, ids } while the destination picker is up

  function keyFor(id) {
    const hit = (ctx.keymap() || []).find((binding) => binding.action === id);
    return hit && hit.trigger;
  }

  const list = OttyList.picker(ctx, {
    name: 'palette',
    label: t('palette.label', 'Command Palette'),
    actions: ['command_palette'],
    placeholder: () => (stage ? stage.title : t('palette.placeholder', 'Type a command')),
    load(query) {
      if (!stage) return ctx.request('palette.list', { query });
      return stage.ids
        .map((id, n) => ({ id, title: t(`palette.where.${n}`, WHERE[n]), shortcut: keyFor(id) }))
        .filter((row) => fuzzy(row.title, query));
    },
    row: (item, query) => [
      h('span', { class: 'palette-title' }, marked(item.title, query, 'palette-hit')),
      item.scope && h('span', { class: 'palette-scope' }, t(`palette.scope.${item.scope}`, item.scope)),
      item.shortcut && h('kbd', { class: 'palette-key' }, shortcutLabel(item.shortcut, mac)),
    ],
    run(item, event) {
      if (!stage && STAGED[item.id]) {
        stage = { title: item.title, ids: STAGED[item.id] };
        return list.open();
      }
      // ⌘↩ (Ctrl+↩ off-mac) runs and keeps the palette open.
      list.exec(() => Promise.resolve(ctx.runAction(item.id)).then((result) => {
        if (result && result.handled === false) throw new Error(t('palette.unhandled', 'Nothing handles this action yet'));
      }), mac ? event.metaKey : event.ctrlKey);
    },
    key(event) {
      if (event.key !== 'Escape' || !stage) return false;
      stage = null;
      list.open();
      return true;
    },
  });

  ctx.registerAction('command_palette', () => {
    stage = null;
    if (list.isOpen()) list.close();
    else list.open();
  });
});
