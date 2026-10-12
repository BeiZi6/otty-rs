// Find bar (⌘F): steps through the focused pane with find.next, continuing from the last hit.
OttyFeatures.register('find', (ctx) => {
  const { h, keys, debounce, actionFor } = OttyList;
  const t = ctx.t;
  const mac = ctx.platform === 'macos';
  const OWN = ['find', 'find_next', 'find_previous', 'use_selection_for_find'];
  // Sent along with find.next; the backend may ignore them.
  const opts = { case_sensitive: false, whole_word: false, regex: false };

  let host = null;
  let input;
  let info;
  let query = '';
  let last = null; // the hit to continue from
  let serial = 0;

  function say(text) {
    if (host) info.textContent = text;
  }

  function step(backwards) {
    if (!query) return say('');
    const mine = ++serial;
    ctx.request('find.next', { query, from: last || undefined, backwards, ...opts }).then(
      (hit) => {
        if (mine !== serial) return;
        last = hit;
        say(hit ? `${hit.line}:${hit.col}` : t('find.none', 'No results'));
      },
      (err) => { if (mine === serial) say(err.message); },
    );
  }

  function restart() {
    last = null;
    step(false);
  }
  const lazy = debounce(restart, 150);

  const onKey = keys((event) => {
    const own = actionFor(ctx.keymap(), event, mac);
    if (event.key === 'Escape') close();
    else if (event.key === 'Enter') step(event.shiftKey);
    else if (OWN.includes(own)) ctx.runAction(own);
    else return; // typing: ours, but the input's to handle
    event.preventDefault();
  }, () => host);

  function button(cls, text, label, onclick, pressed) {
    return h('button', { class: cls, type: 'button', title: label, 'aria-label': label, 'aria-pressed': pressed, onclick }, text);
  }

  function toggle(key, text, label) {
    return button('find-toggle', text, label, (event) => {
      opts[key] = !opts[key];
      event.currentTarget.setAttribute('aria-pressed', String(opts[key]));
      input.focus();
      restart();
    }, String(opts[key]));
  }

  function open(text) {
    if (text != null) {
      query = text;
      last = null;
    }
    if (!host) {
      host = ctx.mount('pane-overlay');
      input = h('input', {
        class: 'find-input', type: 'search', spellcheck: 'false',
        placeholder: t('find.placeholder', 'Find'), 'aria-label': t('find.label', 'Find'),
        oninput: () => { query = input.value; lazy(); },
      });
      info = h('span', { class: 'find-info', role: 'status' });
      host.append(h('div', {
        class: 'find-bar', role: 'search',
        onkeydown: (event) => { onKey(event); event.stopPropagation(); },
        // Another overlay may have taken the keys since; take them back with the focus.
        onfocusin: () => ctx.captureKeys(onKey),
      },
      input, info,
      toggle('case_sensitive', 'Aa', t('find.case', 'Match Case')),
      toggle('whole_word', 'W', t('find.word', 'Whole Word')),
      toggle('regex', '.*', t('find.regex', 'Regular Expression')),
      button('find-button', '‹', t('find.previous', 'Previous'), () => step(true)),
      button('find-button', '›', t('find.next', 'Next'), () => step(false)),
      button('find-button', '×', t('find.close', 'Close'), close)));
    }
    input.value = query;
    ctx.captureKeys(onKey);
    input.focus();
    input.select();
  }

  function close() {
    if (!host) return;
    serial += 1;
    lazy.cancel();
    host.remove();
    host = null;
    ctx.captureKeys(null);
    ctx.focusTerminal();
  }

  function jump(backwards) {
    if (!host) open();
    step(backwards);
  }

  ctx.registerAction('find', () => open());
  ctx.registerAction('find_next', () => jump(false));
  ctx.registerAction('find_previous', () => jump(true));
  // The selection lives in the backend; it answers the action with the text.
  ctx.registerAction('use_selection_for_find', () => ctx.request('action.run', { id: 'use_selection_for_find' }).then(
    (result) => {
      const text = (result && result.query) || String(getSelection());
      open(text || undefined);
      if (text) step(false);
      else say(t('find.noselection', 'No selection'));
    },
    (err) => { open(); say(err.message); },
  ));
});
