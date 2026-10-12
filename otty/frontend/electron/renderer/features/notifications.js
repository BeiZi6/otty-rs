// Toasts for backend `notify` / `error` events, and for an agent that starts
// waiting on the user while the window is in the background.
OttyFeatures.register('notifications', (ctx) => {
  const { h } = globalThis.OttyForm;
  const t = ctx.t;
  let root = null;
  let stack = null;
  let agents = new Map();

  function toast({ title, body, pane, error }) {
    if (!root) {
      // Mounted only while something shows, so an idle stack covers nothing.
      root = ctx.mount('overlay');
      stack = h('div', { class: 'notifications', role: 'region', 'aria-label': t('notifications.title', 'Notifications') });
      root.append(stack);
    }
    const dismiss = () => {
      clearTimeout(timer);
      item.remove();
      if (root && !stack.children.length) {
        root.remove();
        root = null;
      }
    };
    const item = h('div', { class: `notifications-toast${error ? ' notifications-error' : ''}`, role: error ? 'alert' : 'status' },
      h('button', {
        type: 'button',
        onclick: () => {
          dismiss();
          if (pane) ctx.request('pane.focus', { pane }).catch(() => {});
          ctx.focusTerminal();
        },
      }, h('strong', {}, title || 'Otty'), body && h('span', {}, body)));
    const timer = setTimeout(dismiss, error ? 8000 : 5000);
    while (stack.children.length >= 5) stack.firstChild.remove();
    stack.append(item);
  }

  ctx.on('notify', (event) => toast(event));
  ctx.on('error', (event) => toast({ title: t('notifications.error', 'Error'), body: event.message, error: true }));
  ctx.on('state', (state) => {
    const next = new Map();
    for (const [id, pane] of Object.entries(state.panes || {})) {
      const now = pane.agent && pane.agent.state;
      next.set(id, now);
      if (now === 'awaiting' && agents.get(id) !== 'awaiting' && !document.hasFocus()) {
        toast({ title: pane.agent.name, body: t('notifications.agent_awaiting', 'Waiting for your input'), pane: id });
      }
    }
    agents = next;
  });
});
