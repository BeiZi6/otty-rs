// Pairs each `request` written to the bridge with its `response` line.
// Replies are plain `{ok,result}` / `{ok:false,error}` so they survive IPC.
function createCorrelator(write) {
  const pending = new Map();
  let nextId = 1;

  function request(method, params = {}) {
    return new Promise((resolve) => {
      const id = nextId;
      nextId += 1;
      pending.set(id, resolve);
      write({ type: 'request', id, method, params });
    });
  }

  function settle(response) {
    const resolve = pending.get(response.id);
    if (!resolve) return false;
    pending.delete(response.id);
    resolve(response.ok
      ? { ok: true, result: response.result ?? null }
      : { ok: false, error: String(response.error || 'Request failed.') });
    return true;
  }

  function failAll(error) {
    for (const resolve of pending.values()) resolve({ ok: false, error });
    pending.clear();
  }

  return { request, settle, failAll };
}

module.exports = { createCorrelator };
