function createImeInput(textarea, send) {
  let composing = false;

  function flush() {
    const text = textarea.value;
    textarea.value = '';
    if (text) send(text);
  }

  textarea.addEventListener('compositionstart', () => {
    composing = true;
  });
  textarea.addEventListener('compositionend', () => {
    composing = false;
    setTimeout(flush, 0);
  });
  textarea.addEventListener('input', (event) => {
    if (composing || event.isComposing) return;
    flush();
  });

  return { isComposing: () => composing };
}

const OttyImeInput = { createImeInput };
globalThis.OttyImeInput = OttyImeInput;
if (typeof module !== 'undefined') module.exports = OttyImeInput;
