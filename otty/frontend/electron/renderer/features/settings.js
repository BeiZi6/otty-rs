// Settings: one overlay over the config key registry (`settings.schema`), with
// the two pages that are not plain keys — themes and key bindings. Every write
// goes through `settings.set`; the backend patches the toml line.
OttyFeatures.register('settings', (ctx) => {
  const F = globalThis.OttyForm;
  const { h } = F;
  const t = ctx.t;
  const LANGUAGES = { system: 'System', english: 'English', chinese: '中文' };
  const SEARCH_LIMIT = 60; // each visible row costs one settings.get
  let root = null;
  let schema = null;
  let schemaError = null;
  let page = null;
  let query = '';
  let scope = 'auto';
  let recorder = null; // fn(event) while a key-binding row waits for a chord
  let body = null;
  let nav = null;
  let heading = null;
  const loaders = new Map(); // visible key → refetch
  const handled = new WeakSet(); // a keydown can arrive via captureKeys and via the DOM

  const isThemes = (group) => /^themes?$/i.test(group);
  const isKeys = (group) => /^key ?bindings$/i.test(group);
  const message = (text) => h('p', { class: 'settings-msg', role: 'status' }, text);
  const quiet = (error) => F.errorText(error, t);
  const loose = (name) => String(name).toLowerCase().replace(/[^a-z0-9]/g, '');

  function groupLabel(group) {
    if (isKeys(group)) return t('settings.categories.key_bindings', 'Key Bindings');
    if (isThemes(group)) return t('settings.appearance.theme', 'Theme');
    return t(`settings.categories.${group.toLowerCase().replace(/[^a-z0-9]+/g, '_')}`, group);
  }

  function groups() {
    return F.orderGroups([...(schema || []).map((spec) => spec.group || 'Other'), 'Theme', 'Keybindings']);
  }

  // One key: label + doc, a control picked by type, where the value comes from, Reset.
  function row(spec) {
    const type = F.controlType(spec);
    const id = `settings-key-${spec.key}`;
    const origin = h('span', { class: 'settings-origin' });
    const note = h('span', { class: 'settings-note', role: 'status' });
    const fail = (error) => { note.textContent = quiet(error); };
    const save = (value) => ctx.request('settings.set', { key: spec.key, value, scope })
      .then(() => { note.textContent = ''; load(); }, fail);
    let control;
    let extra = null;
    let fill = (value) => { control.value = value ?? ''; };
    if (type === 'bool') {
      control = h('input', { id, type: 'checkbox', onchange: () => save(control.checked) });
      fill = (value) => { control.checked = value === true || value === 'true'; };
    } else if (type === 'enum') {
      control = h('select', { id, onchange: () => save(control.value) },
        spec.values.map((value) => h('option', { value }, value)));
    } else if (type === 'number') {
      control = h('input', { id, type: 'number', step: 'any', onchange: () => { if (control.value !== '') save(control.valueAsNumber); } });
    } else if (type === 'list') {
      control = h('textarea', {
        id, rows: 3, spellcheck: false,
        onchange: () => save(control.value.split('\n').map((line) => line.trim()).filter(Boolean)),
      });
      fill = (value) => { control.value = [].concat(value ?? []).join('\n'); };
    } else {
      control = h('input', { id, type: 'text', spellcheck: false, onchange: () => save(control.value.trim()) });
      if (type === 'color') {
        extra = h('input', {
          type: 'color', 'aria-label': `${spec.key} ${t('settings.appearance.color_theme', 'Color')}`,
          onchange: () => { control.value = extra.value; save(extra.value); },
        });
        fill = (value) => {
          control.value = value ?? '';
          if (/^#[0-9a-f]{6}$/i.test(control.value)) extra.value = control.value;
        };
      }
    }
    function load() {
      ctx.request('settings.get', { key: spec.key }).then((result) => {
        fill(result.value ?? spec.default);
        origin.textContent = result.origin || t('common.default', 'Default');
        origin.title = result.origin || '';
      }, (error) => {
        fill(spec.default);
        origin.textContent = quiet(error);
      });
    }
    loaders.set(spec.key, load);
    load();
    const reset = t('common.reset', 'Reset');
    return h('div', { class: 'settings-row' },
      h('div', { class: 'settings-about' },
        h('label', { for: id, class: 'settings-key' }, spec.key),
        spec.doc && h('p', { class: 'settings-doc' }, spec.doc),
        h('p', { class: 'settings-meta' }, origin, note)),
      h('div', { class: 'settings-control' }, extra, control,
        h('button', {
          type: 'button', class: 'settings-button', 'aria-label': `${reset} ${spec.key}`,
          onclick: () => ctx.request('settings.reset', { key: spec.key }).then(() => { note.textContent = ''; load(); }, fail),
        }, reset)));
  }

  function themesView() {
    const grid = h('div', { class: 'settings-themes', role: 'group', 'aria-label': t('settings.appearance.theme', 'Theme') },
      message(t('settings.data.loading', 'Loading…')));
    const note = message('');
    Promise.all([
      ctx.request('theme.list', {}),
      ctx.request('settings.get', { key: 'theme' }).catch(() => ({})),
    ]).then(([themes, saved]) => {
      let current = loose(saved.value ?? '');
      const paint = () => grid.replaceChildren(...themes.map((theme) => h('button', {
        type: 'button',
        class: `settings-theme${theme.dark ? ' settings-theme-dark' : ''}`,
        'aria-pressed': String(loose(theme.name) === current),
        onclick: () => ctx.request('theme.set', { name: theme.name }).then(() => {
          current = loose(theme.name);
          note.textContent = '';
          paint();
        }, (error) => { note.textContent = quiet(error); }),
      }, h('span', { class: 'settings-swatch', 'aria-hidden': 'true' }), theme.name)));
      paint();
    }, (error) => grid.replaceChildren(message(quiet(error))));
    return [grid, note];
  }

  // Every action with its chords. Click a chord (or +) and press keys to bind;
  // a `keybind` line rebinds the chord it names, so moving an action to a new
  // chord also writes `old=unbind`.
  function keysView() {
    const strings = (key, fallback) => t(`settings.key_bindings.${key}`, fallback);
    const list = h('div', { class: 'settings-bindings', role: 'list' });
    const note = message('');
    const filter = h('input', {
      class: 'settings-input', type: 'search', spellcheck: false,
      placeholder: strings('search_placeholder', 'Search key bindings'),
      'aria-label': strings('search_placeholder', 'Search key bindings'),
      oninput: () => paint(),
    });
    let actions = [];
    let keymap = [];
    const title = (id) => (actions.find((action) => action.id === id) || {}).title || id;
    const write = (value) => ctx.request('settings.set', { key: 'keybind', value, scope });
    const fail = (error) => { note.textContent = quiet(error); };

    function record(action, old, button) {
      paint(); // drops any other row's pending recording
      note.textContent = `${title(action)}: ${strings('recording', 'Press a shortcut…')} ${old ? strings('clear', 'Press Backspace to clear') : ''}`;
      recorder = (event) => {
        if (event.key === 'Escape') { recorder = null; note.textContent = ''; return paint(); }
        const clear = event.key === 'Backspace' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey;
        const chord = clear ? '' : F.eventChord(event, ctx.platform);
        if (!clear && !chord) return undefined; // only modifiers so far
        recorder = null;
        if (chord === old || (clear && !old)) { note.textContent = ''; return paint(); }
        const clash = keymap.find((binding) => binding.trigger === chord && binding.action !== action);
        note.textContent = clash ? strings('conflict', 'Conflicts with: {action}').replace('{action}', title(clash.action)) : '';
        return (chord ? write(`${chord}=${action}`) : Promise.resolve())
          .then(() => old && write(`${old}=unbind`))
          .then(load, (error) => { fail(error); paint(); });
      };
      const live = list.querySelector(`[data-bind="${CSS.escape(`${action} ${old}`)}"]`) || button;
      live.textContent = strings('recording', 'Press a shortcut…');
      live.classList.add('settings-recording');
      live.focus();
    }

    function paint() {
      recorder = null;
      const rows = actions
        .map((action) => ({ ...action, triggers: keymap.filter((binding) => binding.action === action.id).map((binding) => binding.trigger) }))
        .filter((item) => F.matchBinding(item, filter.value));
      list.replaceChildren(...rows.map((item) => h('div', { class: 'settings-binding', role: 'listitem' },
        h('span', { class: 'settings-binding-title', title: item.id }, item.title || item.id),
        item.triggers.map((trigger) => [
          h('button', {
            type: 'button', class: 'settings-chord', 'data-bind': `${item.id} ${trigger}`,
            title: strings('rebind_hint', 'Click to rebind'),
            'aria-label': `${item.title || item.id}: ${trigger}. ${strings('rebind_hint', 'Click to rebind')}`,
            onclick: (event) => record(item.id, trigger, event.currentTarget),
          }, trigger),
          h('button', {
            type: 'button', class: 'settings-unbind',
            title: strings('unbind', 'Unbind'),
            'aria-label': `${item.title || item.id}: ${strings('unbind', 'Unbind')} ${trigger}`,
            onclick: () => write(`${trigger}=unbind`).then(load, fail),
          }, '×'),
        ]),
        h('button', {
          type: 'button', class: 'settings-chord settings-add', 'data-bind': `${item.id} `,
          title: strings('add_hint', 'Click to add shortcut'),
          'aria-label': `${item.title || item.id}: ${strings('add_hint', 'Click to add shortcut')}`,
          onclick: (event) => record(item.id, '', event.currentTarget),
        }, '+'))));
      if (!rows.length) {
        list.append(message(filter.value.includes('+')
          ? strings('no_results_keys', 'Nothing is bound to this shortcut')
          : strings('no_results', 'No matching key bindings')));
      }
    }

    function load() {
      Promise.all([
        ctx.request('palette.list', { query: '' }).catch(() => null),
        ctx.request('keymap.list', {}).catch((error) => { fail(error); return ctx.keymap() || []; }),
      ]).then(([palette, bindings]) => {
        keymap = bindings || [];
        // Without the palette the bound actions are still listable, by id.
        actions = palette || [...new Set(keymap.map((binding) => binding.action))].map((id) => ({ id, title: id }));
        paint();
      });
    }
    loaders.set('keybind', load);
    load();
    return [filter, note, list];
  }

  function renderBody() {
    recorder = null;
    loaders.clear();
    for (const button of nav.children) {
      if (button.dataset.group === page && !query) button.setAttribute('aria-current', 'page');
      else button.removeAttribute('aria-current');
    }
    if (!schema && !schemaError) {
      body.replaceChildren(message(t('settings.data.loading', 'Loading…')));
      return;
    }
    // Without a schema the theme and key-binding pages still work.
    const keys = schema || [];
    const text = query.toLowerCase();
    const specs = query
      ? keys.filter((spec) => `${spec.key}\n${spec.doc || ''}`.toLowerCase().includes(text)).slice(0, SEARCH_LIMIT)
      : keys.filter((spec) => (spec.group || 'Other') === page && !(isKeys(page) && spec.key === 'keybind'));
    heading.textContent = query ? `${t('settings.search_placeholder', 'Search')}: ${query}` : groupLabel(page);
    const special = query ? [] : isThemes(page) ? themesView() : isKeys(page) ? keysView() : [];
    const empty = !special.length && !specs.length;
    body.replaceChildren(
      ...(schemaError ? [message(quiet(schemaError))] : empty ? [message(t('open_quickly.no_results', 'No results'))] : []),
      ...special, ...specs.map(row),
    );
    body.scrollTop = 0;
  }

  function onKey(event) {
    // true = "not for the terminal"; typing and Tab still reach the form
    // controls because only the keys handled here are default-prevented.
    if (handled.has(event) || event.isComposing) return true;
    handled.add(event);
    if (recorder) recorder(event);
    else if (event.key === 'Escape') close();
    else return true;
    event.preventDefault();
    event.stopPropagation();
    return true;
  }

  function build() {
    const title = t('settings.title', 'Settings');
    const search = h('input', {
      class: 'settings-input', type: 'search', value: query, spellcheck: false,
      placeholder: t('settings.search_placeholder', 'Search'), 'aria-label': t('settings.search_placeholder', 'Search'),
      oninput: () => { query = search.value.trim(); renderBody(); },
    });
    const fail = (error) => { heading.textContent = quiet(error); };
    const language = h('select', {
      id: 'settings-language',
      onchange: () => ctx.request('settings.set', { key: 'language', value: language.value, scope }).catch(fail),
    }, Object.entries(LANGUAGES).map(([value, name]) => h('option', { value }, t(`settings.general.language_${value}`, name))));
    ctx.request('settings.get', { key: 'language' }).then((result) => { language.value = result.value || 'system'; }, () => {});
    const scopes = h('select', { id: 'settings-scope', onchange: () => { scope = scopes.value; } },
      [['auto', 'settings.scope_auto', 'Auto'], ['local', 'settings.data.state_local', 'Local'], ['shared', 'settings.scope_shared', 'Shared']]
        .map(([value, key, fallback]) => h('option', { value, selected: value === scope }, t(key, fallback))));
    nav = h('div', { class: 'settings-groups' });
    heading = h('h2', { class: 'settings-heading' }, title);
    body = h('div', { class: 'settings-body' });
    root.replaceChildren(h('div', { class: 'settings', role: 'dialog', 'aria-modal': 'true', 'aria-label': title, onkeydown: onKey },
      h('nav', { class: 'settings-nav', 'aria-label': title },
        search, nav,
        h('label', { class: 'settings-field', for: 'settings-language' }, t('settings.general.language', 'Language'), language)),
      h('section', { class: 'settings-main' },
        h('header', { class: 'settings-header' }, heading,
          h('label', { class: 'settings-field', for: 'settings-scope', title: 'config.toml / config.local.toml' },
            t('settings.advanced.config_file', 'Config File'), scopes),
          h('button', { type: 'button', class: 'settings-button', 'aria-label': t('menu.close', 'Close'), onclick: close }, '×')),
        body)));
    renderNav();
    renderBody();
    search.focus();
  }

  function renderNav() {
    const list = groups();
    if (!list.includes(page) && (schema || schemaError)) [page] = list;
    nav.replaceChildren(...list.map((group) => h('button', {
      type: 'button', class: 'settings-group', 'data-group': group,
      onclick: () => {
        page = group;
        query = '';
        root.querySelector('.settings-nav input').value = '';
        renderBody();
      },
    }, groupLabel(group))));
  }

  function open() {
    root = ctx.mount('overlay');
    build();
    // ponytail: no focus trap; Tab can walk out of the dialog. Add one if the
    // overlay slot does not already contain focus.
    ctx.captureKeys(onKey);
    if (schema) return;
    ctx.request('settings.schema', {}).then((result) => { schema = result || []; schemaError = null; }, (error) => { schemaError = error; })
      .then(() => { if (root) { renderNav(); renderBody(); } });
  }

  function close() {
    if (!root) return;
    root.remove();
    root = null;
    recorder = null;
    loaders.clear();
    ctx.captureKeys(null);
    ctx.focusTerminal();
  }

  ctx.registerAction('open_settings', () => (root ? close() : open()));
  ctx.on('config', (event) => {
    const load = root && loaders.get(event.key);
    if (load) load();
  });
  window.addEventListener('otty-strings', () => { if (root) build(); });
});
