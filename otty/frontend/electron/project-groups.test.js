const test = require('node:test');
const assert = require('node:assert/strict');
const {
  groupTabs,
  currentFolder,
  itemAtShortcut,
  shortcutLabel,
  relativeItem,
  findProjectRoot,
  folderKey,
} = require('./project-groups');

function tab(id, cwd, project) {
  return { id, title: id, cwd, project, status: 'connected' };
}

function posixDirname(value) {
  if (value === '/' || /^[A-Za-z]:\/?$/.test(value)) return value;
  const cut = value.replace(/\/+$/, '').lastIndexOf('/');
  if (cut <= 0) return '/';
  const parent = value.slice(0, cut);
  return /^[A-Za-z]:$/.test(parent) ? `${parent}/` : parent;
}

test('groups sessions by project root, then cwd, preserving order', () => {
  const folders = groupTabs([
    tab('a', 'E:/work/otty-re/packages/ui', 'E:/work/otty-re'),
    tab('b', 'E:/work/jev', 'E:/work/jev'),
    tab('c', 'E:/work/otty-re', 'E:/work/otty-re'),
    tab('d', 'E:/work/jev/src', 'E:/work/jev'),
  ]);
  assert.deepEqual(folders.map((folder) => folder.label), ['otty-re', 'jev']);
  assert.deepEqual(folders[0].tabs.map((item) => item.id), ['a', 'c']);
  assert.deepEqual(folders[1].tabs.map((item) => item.id), ['b', 'd']);
});

test('tabs without a cwd sit in Starting until the engine reports one', () => {
  const folders = groupTabs([tab('boot'), tab('home', 'C:/Users/hitxy')]);
  assert.equal(folders[0].label, 'Starting');
  assert.equal(folders[0].key, '__starting__');
  assert.equal(folders[1].label, 'hitxy');
});

test('current folder follows the active session', () => {
  const folders = groupTabs([
    tab('a', '/a'),
    tab('b', '/b'),
  ]);
  assert.equal(currentFolder(folders, 'b').label, 'b');
  assert.equal(currentFolder(folders, 'missing').label, 'a');
  assert.equal(currentFolder([], 'a'), null);
});

test('Ctrl+1 is the first session of the current project, not the global tab index', () => {
  const folders = groupTabs([
    tab('a1', '/work/otty-re'),
    tab('a2', '/work/otty-re'),
    tab('b1', '/work/jev'),
    tab('b2', '/work/jev'),
  ]);
  const jev = currentFolder(folders, 'b2');
  assert.equal(jev.label, 'jev');
  assert.equal(itemAtShortcut(jev.tabs, '1').id, 'b1');
  assert.equal(itemAtShortcut(jev.tabs, '2').id, 'b2');
  assert.equal(itemAtShortcut(folders, '1').label, 'otty-re');
  assert.equal(itemAtShortcut(folders, '2').label, 'jev');
});

test('Ctrl+1..8 pick that session, Ctrl+9 picks the last in the current folder', () => {
  const sessions = [tab('s1', '/p'), tab('s2', '/p'), tab('s3', '/p')];
  assert.equal(itemAtShortcut(sessions, '1').id, 's1');
  assert.equal(itemAtShortcut(sessions, '3').id, 's3');
  assert.equal(itemAtShortcut(sessions, '4'), null);
  assert.equal(itemAtShortcut(sessions, '9').id, 's3');
  assert.equal(itemAtShortcut([], '1'), null);
});

test('shortcut labels match Chrome: 1-8, then 9 on the last item', () => {
  assert.equal(shortcutLabel(0, 3), '1');
  assert.equal(shortcutLabel(2, 3), '3');
  assert.equal(shortcutLabel(7, 10), '8');
  assert.equal(shortcutLabel(8, 10), '');
  assert.equal(shortcutLabel(9, 10), '9');
  assert.equal(shortcutLabel(8, 9), '9');
});

test('relative wrap stays inside the current folder', () => {
  const sessions = [tab('s1', '/p'), tab('s2', '/p'), tab('s3', '/p')];
  assert.equal(relativeItem(sessions, 's1', 1).id, 's2');
  assert.equal(relativeItem(sessions, 's3', 1).id, 's1');
  assert.equal(relativeItem(sessions, 's1', -1).id, 's3');
});

test('walks up to the git root and otherwise keeps cwd', () => {
  const git = new Set(['/work/otty-re', 'E:/work/otty-re']);
  const hasGit = (dir) => git.has(dir);
  assert.equal(
    findProjectRoot('/work/otty-re/packages/ui', hasGit, posixDirname),
    '/work/otty-re',
  );
  assert.equal(
    findProjectRoot('E:/work/otty-re/frontend/electron', hasGit, posixDirname),
    'E:/work/otty-re',
  );
  assert.equal(findProjectRoot('/tmp/scratch', hasGit, posixDirname), '/tmp/scratch');
  assert.equal(findProjectRoot('', hasGit, posixDirname), '');
});

test('folderKey prefers the git project over a nested cwd', () => {
  assert.equal(
    folderKey(tab('a', 'E:/work/otty-re/packages/ui', 'E:/work/otty-re')),
    'E:/work/otty-re',
  );
});
