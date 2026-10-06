const assert = require('node:assert/strict');
const test = require('node:test');
const { createImeInput } = require('./ime-input');

function createTextarea() {
  const listeners = new Map();
  return {
    value: '',
    addEventListener(type, listener) {
      const callbacks = listeners.get(type) || [];
      callbacks.push(listener);
      listeners.set(type, callbacks);
    },
    dispatch(type, event = {}) {
      for (const listener of listeners.get(type) || []) listener(event);
    },
  };
}

function tick() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

test('sends only committed IME text when input events happen during composition', async () => {
  const textarea = createTextarea();
  const sent = [];
  const ime = createImeInput(textarea, (text) => sent.push(text));

  textarea.dispatch('compositionstart');
  textarea.value = 'n';
  textarea.dispatch('input', { isComposing: true });
  textarea.value = 'ni';
  textarea.dispatch('input', { isComposing: true });
  textarea.value = '你';
  textarea.dispatch('compositionend');
  await tick();

  assert.deepEqual(sent, ['你']);
  assert.equal(textarea.value, '');
  assert.equal(ime.isComposing(), false);
});

test('does not send composition text twice when the final input follows compositionend', async () => {
  const textarea = createTextarea();
  const sent = [];
  createImeInput(textarea, (text) => sent.push(text));

  textarea.dispatch('compositionstart');
  textarea.value = '你';
  textarea.dispatch('compositionend');
  textarea.dispatch('input', { isComposing: false });
  await tick();

  assert.deepEqual(sent, ['你']);
});
