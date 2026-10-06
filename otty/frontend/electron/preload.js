const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('otty', {
  input: (tabId, text) => ipcRenderer.send('engine:input', { tabId, text }),
  resize: (tabId, width, height) => ipcRenderer.send('engine:resize', { tabId, width, height }),
  windowAction: (action) => ipcRenderer.send('window:action', action),
  newTab: () => ipcRenderer.send('tabs:new'),
  openProject: () => ipcRenderer.invoke('projects:open'),
  activateTab: (tabId) => ipcRenderer.send('tabs:activate', tabId),
  closeTab: (tabId) => ipcRenderer.send('tabs:close', tabId),
  reopenTab: () => ipcRenderer.send('tabs:reopen'),
  moveTab: (tabId, targetId, after) => ipcRenderer.send('tabs:move', { tabId, targetId, after }),
  toggleTabsPanel: () => ipcRenderer.send('tabs:toggle-panel'),
  onEngineEvent: (listener) => subscribe('engine:event', listener),
  onTabsState: (listener) => subscribe('tabs:state', listener),
  onTabActivated: (listener) => subscribe('tabs:activated', listener),
});

function subscribe(channel, listener) {
  const handler = (_event, value) => listener(value);
  ipcRenderer.on(channel, handler);
  return () => ipcRenderer.removeListener(channel, handler);
}
