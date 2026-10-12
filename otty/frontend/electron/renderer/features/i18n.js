// Localization: fetch the string table for the configured `language` and
// publish it where the core's ctx.t reads it.
OttyFeatures.register('i18n', (ctx) => {
  const load = () => ctx.request('i18n.strings', {}).then((table) => {
    globalThis.OttyStrings = { language: table.language, strings: table.strings || {} };
    window.dispatchEvent(new CustomEvent('otty-strings'));
  }, () => { /* no table: ctx.t keeps answering with its English fallbacks */ });

  ctx.on('config', (event) => {
    if (event.key === 'language') load();
  });
  load();
});
