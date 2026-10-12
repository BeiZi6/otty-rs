const { contextBridge, ipcRenderer, webUtils } = require('electron');

contextBridge.exposeInMainWorld('otty', {
  // → {ok:true,result} | {ok:false,error}; the renderer core turns it into a Promise<result>.
  request: (method, params) => ipcRenderer.invoke('bridge:request', method, params),
  // input | paste | resize, addressed to a pane.
  send: (command) => ipcRenderer.send('bridge:send', command),
  onEvent: (listener) => subscribe('bridge:event', listener),
  onAction: (listener) => subscribe('action', listener),
  features: () => ipcRenderer.invoke('features:list'),
  readClipboard: () => ipcRenderer.invoke('clipboard:read'),
  writeClipboard: (text) => ipcRenderer.send('clipboard:write', text),
  popupMenu: (items) => ipcRenderer.send('menu:popup', items),
  appMenu: () => ipcRenderer.send('menu:app'),
  windowAction: (action) => ipcRenderer.send('window:action', action),
  openProject: () => ipcRenderer.invoke('projects:open'),
  saveFile: (name) => ipcRenderer.invoke('file:save-path', name),
  pathForFile: (file) => webUtils.getPathForFile(file),
  isDirectory: (target) => ipcRenderer.invoke('path:is-directory', target),
});

function subscribe(channel, listener) {
  const handler = (_event, value) => listener(value);
  ipcRenderer.on(channel, handler);
  return () => ipcRenderer.removeListener(channel, handler);
}
