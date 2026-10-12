// Composer (⌘⇧E) and the Prompt Queue bar (⌘⇧M): a multi-line draft handed to the focused pane.
OttyFeatures.register('composer', (ctx) => {
  const { h, keys, actionFor } = OttyList;
  const t = ctx.t;
  const mac = ctx.platform === 'macos';
  const OWN = ['compose_overlay', 'paste_continue_composer', 'prompt_queue_add', 'prompt_queue_clear'];

  let host = null;
  let area;
  let note;
  let chip;
  let count;
  let draft = ''; // survives Esc
  let queueing = false; // opened as the Prompt Queue bar: every send is queued
  // ponytail: counts this window's queued sends since the last clear; the backend
  // drains the queue without telling us. Put the length in `state` to make it exact.
  let queued = 0;

  function say(text) {
    if (host) note.textContent = text || '';
  }

  function paint() {
    if (!host) return;
    chip.hidden = !queued;
    count.textContent = `${queued} ${t('composer.queued', 'queued')}`;
    const label = queueing ? t('composer.queue', 'Add to prompt queue') : t('composer.label', 'Composer');
    area.placeholder = label;
    area.setAttribute('aria-label', label);
  }

  function send() {
    const text = area.value;
    if (!text.trim()) return;
    ctx.request('composer.send', queueing ? { text, queue: true } : { text }).then((result) => {
      if (host) area.value = '';
      draft = '';
      if (!(result && result.queued)) return close();
      queued += 1;
      paint();
      say('');
    }, (err) => say(err.message));
  }

  function clearQueue() {
    ctx.request('action.run', { id: 'prompt_queue_clear' }).then(
      () => { queued = 0; paint(); },
      (err) => { open(queueing); say(err.message); },
    );
  }

  const onKey = keys((event) => {
    const own = actionFor(ctx.keymap(), event, mac);
    if (event.key === 'Escape') close();
    else if (event.key === 'Enter' && !event.shiftKey) send(); // ⇧↩ stays a newline
    else if (OWN.includes(own)) ctx.runAction(own);
    else return;
    event.preventDefault();
  }, () => host);

  function open(queue, extra = '') {
    queueing = queue;
    if (!host) {
      host = ctx.mount('pane-overlay');
      area = h('textarea', { class: 'composer-input', rows: '3', spellcheck: 'false' });
      area.value = draft;
      note = h('span', { class: 'composer-note', role: 'status' });
      count = h('span', {});
      const clear = t('composer.clear', 'Clear queue');
      chip = h('span', { class: 'composer-chip' }, count,
        h('button', { class: 'composer-clear', type: 'button', title: clear, 'aria-label': clear, onclick: clearQueue }, '×'));
      host.append(h('div', {
        class: 'composer', role: 'dialog', 'aria-label': t('composer.label', 'Composer'),
        onkeydown: (event) => { onKey(event); event.stopPropagation(); },
        onfocusin: () => ctx.captureKeys(onKey),
      }, area, h('div', { class: 'composer-foot' }, chip, note,
        h('span', { class: 'composer-hint' }, t('composer.hint', '↩ send · ⇧↩ newline · esc close')))));
    }
    area.value += extra;
    paint();
    ctx.captureKeys(onKey);
    area.focus();
  }

  function close() {
    if (!host) return;
    draft = area.value;
    host.remove();
    host = null;
    ctx.captureKeys(null);
    ctx.focusTerminal();
  }

  function toggle(queue) {
    if (host && queueing === queue) close();
    else open(queue);
  }

  ctx.registerAction('compose_overlay', () => toggle(false));
  ctx.registerAction('prompt_queue_add', () => toggle(true));
  ctx.registerAction('prompt_queue_clear', clearQueue);
  ctx.registerAction('paste_continue_composer', () => navigator.clipboard.readText().then(
    (text) => open(false, text),
    (err) => { open(false); say(err.message); },
  ));
});
