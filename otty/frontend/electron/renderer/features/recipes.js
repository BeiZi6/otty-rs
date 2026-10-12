// Recipes: Save (⌘S), Save As — an exported .ottyrecipe — (⇧⌘S), and the
// question an untrusted recipe file raises before any of its commands run.
// Opening is Open Quickly's Recipes filter, or a .ottyrecipe dropped on the window.
OttyFeatures.register('recipes', (ctx) => {
  const { h, errorText } = globalThis.OttyForm;
  const t = ctx.t;
  const swallow = () => true;
  let dialog = null;

  function close() {
    if (!dialog) return;
    dialog.host.remove();
    dialog = null;
    ctx.captureKeys(null);
    ctx.focusTerminal();
  }

  // A native modal dialog: it keeps focus inside and Esc closes it. While it
  // is up the terminal and the keymap get no keys.
  function show(title, body, buttons) {
    close();
    const host = ctx.mount('overlay');
    dialog = h('dialog', { class: 'recipes', onclose: close },
      h('h2', { class: 'recipes-title' }, title), body, h('div', { class: 'recipes-buttons' }, buttons));
    dialog.host = host;
    host.append(dialog);
    ctx.captureKeys(swallow);
    dialog.showModal();
  }

  const button = (label, onclick, primary) => h('button', {
    type: primary ? 'submit' : 'button', class: `recipes-button${primary ? ' primary' : ''}`, onclick,
  }, label);
  const field = (label, control) => h('label', { class: 'recipes-field' }, h('span', {}, label), control);
  const options = (rows) => rows.map(([value, key, label]) => h('option', { value }, t(key, label)));

  function saveForm(exporting) {
    const name = h('input', { type: 'text', required: true, autocomplete: 'off', spellcheck: false });
    const scope = h('select', {}, options([
      ['window', 'recipes.scope.window', 'Current Window'],
      ['tab', 'recipes.scope.tab', 'Current Tab'],
      ['commands', 'recipes.scope.commands', 'Commands'],
    ]));
    // ponytail: Include Scrollback is not offered; the backend keeps layout
    // and commands only.
    const content = h('select', {}, options([
      ['layout', 'recipes.content.layout', 'Layout Only'],
      ['commands', 'recipes.content.commands', 'Include Commands (replay on open)'],
    ]));
    // One command a line: delete a line to leave it out, edit it to change it.
    const commands = h('textarea', { rows: 6, spellcheck: false });
    const portable = h('input', { type: 'checkbox' });
    const note = h('p', { class: 'recipes-note', role: 'status' });
    const contentField = field(t('recipes.content', 'Content'), content);
    const commandsField = field(t('recipes.commands', 'Commands'), commands);
    const portableField = h('label', { class: 'recipes-check' }, portable, t('recipes.portable', 'Make paths portable'));

    function sync() {
      const only = scope.value === 'commands';
      contentField.hidden = only;
      commandsField.hidden = !only;
      portableField.hidden = only || !exporting;
      if (!only || commands.value) return;
      ctx.request('details.outline', {}).then((rows) => {
        commands.value = (rows || []).slice(-10).map((row) => row.title).join('\n');
      }, () => {});
    }
    scope.addEventListener('change', sync);

    async function save(event) {
      event.preventDefault();
      const params = { name: name.value.trim(), scope: scope.value };
      if (scope.value === 'commands') params.commands = commands.value.split('\n').map((line) => line.trim()).filter(Boolean);
      else params.content = content.value;
      try {
        if (exporting) {
          const path = await window.otty.saveFile(`${params.name}.ottyrecipe`);
          if (!path) return;
          Object.assign(params, { path, portable: portable.checked });
        }
        await ctx.request('recipe.save', params);
        close();
      } catch (error) {
        note.textContent = errorText(error, t);
      }
    }

    const title = exporting ? t('recipes.export', 'Export Recipe') : t('recipes.save', 'Save Recipe');
    show(title, h('form', { class: 'recipes-form', id: 'recipes-form', onsubmit: save },
      field(t('recipes.name', 'Name'), name), field(t('recipes.scope', 'Scope'), scope),
      contentField, commandsField, portableField, note,
    ), [
      button(t('common.cancel', 'Cancel'), close),
      h('button', { type: 'submit', form: 'recipes-form', class: 'recipes-button primary' },
        exporting ? t('recipes.export_button', 'Export…') : t('common.save', 'Save')),
    ]);
    sync();
    name.focus();
  }

  ctx.registerAction('save_recipe', () => saveForm(false));
  ctx.registerAction('save_recipe_as', () => saveForm(true));

  // The backend opened nothing: this file is not from the library, has not
  // been trusted, and would run what is listed.
  ctx.on('recipe-trust', (asked) => {
    const answer = (trust) => {
      close();
      ctx.request('recipe.open', { path: asked.path, trust }).catch((error) => {
        show(asked.name, h('p', { class: 'recipes-note', role: 'alert' }, errorText(error, t)), [button(t('common.close', 'Close'), close, true)]);
      });
    };
    show(`${t('recipes.trust.title', 'Run the commands in this recipe?')}`, [
      h('p', {}, h('strong', {}, asked.name), ' — ', h('code', { class: 'recipes-path' }, asked.path)),
      h('p', {}, t('recipes.trust.body', 'Opening it will type these commands into new panes. Continue only if you trust where the file came from.')),
      h('ul', { class: 'recipes-commands' }, (asked.commands || []).map((row) => h('li', {},
        h('span', { class: 'recipes-where' }, row.tab ? `${t('recipes.tab', 'Tab')} ${row.tab} · ${t('recipes.pane', 'Pane')} ${row.pane}` : t('recipes.this_pane', 'This pane')),
        h('code', {}, row.command)))),
    ], [
      button(t('common.cancel', 'Cancel'), close),
      button(t('recipes.trust.once', 'Run Once'), () => answer('once')),
      button(t('recipes.trust.always', 'Always Trust'), () => answer('always'), true),
    ]);
  });
});
