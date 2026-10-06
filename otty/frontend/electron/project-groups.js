(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OttyProjects = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  function normalizePath(value) {
    if (!value) return '';
    return String(value).replace(/\\/g, '/').replace(/\/+$/, '');
  }

  function folderKey(tab) {
    return normalizePath(tab && (tab.project || tab.cwd)) || '__starting__';
  }

  function folderLabel(key) {
    if (!key || key === '__starting__') return 'Starting';
    const parts = key.split('/').filter(Boolean);
    return parts[parts.length - 1] || key;
  }

  function groupTabs(tabs) {
    const folders = [];
    const byKey = new Map();
    for (const tab of tabs || []) {
      const key = folderKey(tab);
      let folder = byKey.get(key);
      if (!folder) {
        folder = {
          key,
          label: folderLabel(key),
          path: key === '__starting__' ? '' : tab.project || tab.cwd || '',
          tabs: [],
        };
        byKey.set(key, folder);
        folders.push(folder);
      }
      folder.tabs.push(tab);
    }
    return folders;
  }

  function currentFolder(folders, activeTabId) {
    return (folders || []).find((folder) => folder.tabs.some((tab) => tab.id === activeTabId))
      || (folders && folders[0])
      || null;
  }

  function shortcutIndex(key, length) {
    if (!/^[1-9]$/.test(String(key)) || length < 1) return -1;
    const n = Number(key);
    if (n === 9) return length - 1;
    return n - 1 < length ? n - 1 : -1;
  }

  function itemAtShortcut(list, key) {
    const index = shortcutIndex(key, (list || []).length);
    return index < 0 ? null : list[index];
  }

  function shortcutLabel(index, length) {
    if (index < 8) return String(index + 1);
    if (length >= 9 && index === length - 1) return '9';
    return '';
  }

  function relativeItem(list, currentId, direction, idKey = 'id') {
    if (!list || !list.length) return null;
    const index = list.findIndex((item) => item[idKey] === currentId);
    const from = index < 0 ? 0 : index;
    return list[(from + direction + list.length) % list.length];
  }

  function findProjectRoot(cwd, hasGit, dirname) {
    if (!cwd) return '';
    let dir = cwd;
    for (let i = 0; i < 64; i += 1) {
      if (hasGit(dir)) return dir;
      const parent = dirname(dir);
      if (!parent || parent === dir) return cwd;
      dir = parent;
    }
    return cwd;
  }

  return {
    folderKey,
    folderLabel,
    groupTabs,
    currentFolder,
    shortcutIndex,
    itemAtShortcut,
    shortcutLabel,
    relativeItem,
    findProjectRoot,
  };
});
