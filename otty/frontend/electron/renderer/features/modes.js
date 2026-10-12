// Hint / Vi / Mark: a pill over the pane and a key tap. The backend owns the
// mode logic; every key goes to it as `mode.key`.
OttyFeatures.register('modes', (ctx) => {
  const { h, keys, keyName, actionFor } = OttyList;
  const t = ctx.t;
  const mac = ctx.platform === 'macos';
  const EXIT = { vi: 'toggle_vi_mode', hint: 'toggle_hint_mode', mark: 'toggle_mark_mode' };
  const OWN = [...Object.values(EXIT), 'toggle_vi_mode_help'];
  const HELP = [
    ['h j k l', 'Left / down / up / right (arrows too)'],
    ['w b e', 'Word forward / back / end'],
    ['0 $ ^', 'Line start / end / first non-blank'],
    ['H M L', 'Top / middle / bottom of screen'],
    ['gg G', 'Top / bottom of scrollback'],
    ['⌃u ⌃d', 'Half page up / down'],
    ['⌃b ⌃f', 'Full page up / down'],
    ['5j', 'Any motion takes a count'],
    ['v V ⌃v', 'Select by character / line / block'],
    ['o', 'Jump to the other end of the selection'],
    ['y ↩', 'Copy the selection and exit'],
    ['/ ?', 'Search forward / backward'],
    ['n N', 'Next / previous match'],
    ['f', 'Hint Mode'],
    ['esc q', 'Exit'],
  ];

  let mode = 'shell';
  let pill = null;
  let note;
  let help = null;
  let leaving = false;

  function say(text) {
    if (pill) note.textContent = text || '';
  }

  function run(id) {
    return Promise.resolve().then(() => ctx.runAction(id)).catch((err) => say(err.message));
  }

  // The backend cannot take the key (`mode.key` not built): leave the mode
  // rather than hold the keyboard hostage. Once per entry — a second toggle would re-enter.
  function leave(err) {
    say(err.message);
    if (leaving || !EXIT[mode]) return;
    leaving = true;
    run(EXIT[mode]);
  }

  const onKey = keys((event) => {
    if (['Shift', 'Control', 'Alt', 'Meta'].includes(event.key)) return;
    event.preventDefault();
    const own = actionFor(ctx.keymap(), event, mac);
    if (OWN.includes(own)) run(own);
    // `pending` is the count typed so far ("5" of `5j`), shown in the pill.
    else ctx.request('mode.key', { key: keyName(event) }).then((result) => say(result && result.pending), leave);
  });

  function sync(state) {
    const next = (state && state.mode) || 'shell';
    if (next === mode) return; // only transitions touch captureKeys: an open overlay may hold it
    mode = next;
    leaving = false;
    if (pill) pill.remove();
    pill = null;
    if (mode === 'shell') return ctx.captureKeys(null);
    const exit = t('modes.exit', 'Exit mode');
    pill = ctx.mount('pane-overlay');
    note = h('span', { class: 'modes-note' });
    pill.append(h('div', { class: 'modes-pill', role: 'status' },
      h('strong', {}, t(`modes.${mode}`, mode.toUpperCase())), note,
      h('button', { class: 'modes-close', type: 'button', title: exit, 'aria-label': exit, onclick: () => run(EXIT[mode] || EXIT.vi) }, '×')));
    ctx.captureKeys(onKey);
  }

  function toggleHelp() {
    if (help) {
      help.remove();
      help = null;
      return;
    }
    const label = t('modes.help', 'Vi Mode keys');
    const close = t('modes.help.close', 'Close');
    help = ctx.mount('pane-overlay');
    help.append(h('div', { class: 'modes-help', role: 'region', 'aria-label': label },
      h('button', { class: 'modes-close', type: 'button', title: close, 'aria-label': close, onclick: toggleHelp }, '×'),
      h('dl', {}, HELP.map(([key, text], n) => [h('dt', {}, h('kbd', {}, key)), h('dd', {}, t(`modes.help.${n}`, text))]))));
  }

  ctx.registerAction('toggle_vi_mode_help', toggleHelp);
  ctx.on('state', sync);
  sync(ctx.state());
});
