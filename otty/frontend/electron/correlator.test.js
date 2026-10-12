const test = require('node:test');
const assert = require('node:assert/strict');
const { createCorrelator } = require('./correlator');

test('responses settle their own request, in any order', async () => {
  const written = [];
  const requests = createCorrelator((command) => written.push(command));
  const first = requests.request('tab.new');
  const second = requests.request('pane.split', { direction: 'right' });
  assert.deepEqual(written, [
    { type: 'request', id: 1, method: 'tab.new', params: {} },
    { type: 'request', id: 2, method: 'pane.split', params: { direction: 'right' } },
  ]);
  assert.equal(requests.settle({ id: 2, ok: false, error: 'not implemented' }), true);
  assert.equal(requests.settle({ id: 1, ok: true, result: { tab: 'tab-2' } }), true);
  assert.deepEqual(await first, { ok: true, result: { tab: 'tab-2' } });
  assert.deepEqual(await second, { ok: false, error: 'not implemented' });
  assert.equal(requests.settle({ id: 1, ok: true }), false);
});

test('a dead bridge fails everything still waiting', async () => {
  const requests = createCorrelator(() => {});
  const waiting = requests.request('menu.list');
  requests.failAll('Rust engine exited.');
  assert.deepEqual(await waiting, { ok: false, error: 'Rust engine exited.' });
});
