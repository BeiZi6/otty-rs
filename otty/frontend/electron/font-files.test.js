const test = require('node:test');
const assert = require('node:assert/strict');
const { describeFont } = require('./font-files');

test('licensed faces are recognised by file name', () => {
  assert.deepEqual(describeFont('StyreneB-Regular.otf'), {
    file: 'StyreneB-Regular.otf', family: 'Styrene B', weight: 400, style: 'normal',
  });
  assert.deepEqual(describeFont('Styrene A Medium Italic.woff2'), {
    file: 'Styrene A Medium Italic.woff2', family: 'Styrene A', weight: 500, style: 'italic',
  });
  assert.equal(describeFont('TiemposText-Semibold.ttf').weight, 600);
  assert.equal(describeFont('tiempos-text-bold.otf').weight, 700);
  assert.equal(describeFont('JetBrainsMono.ttf'), null);
  assert.equal(describeFont('StyreneB-Regular.txt'), null);
});
