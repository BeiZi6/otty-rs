# Electron ↔ Rust protocol (v2)

The contract between the JS frontend (`frontend/electron/`) and the Rust backend
(`packages/bridge`, binary `otty-bridge`). The frontend draws chrome and owns no
terminal logic; the backend owns the model (`otty_ui::Dispatcher`), the sessions,
the VT engine and the pixels. If the two sides disagree, this file wins — change
it in the same commit as the code.

## Transport

Electron main spawns **one `otty-bridge` per BrowserWindow** and talks JSON
lines over stdio: one JSON object per line, UTF-8, both ways. stderr is logs.
Env: `OTTY_SCALE_FACTOR` (device pixel ratio), `OTTY_CWD` (first tab's cwd).

## Frontend → backend

```jsonc
{"type":"input",  "pane":"pane-1", "text":"ls\n"}      // key text, same encoding TerminalView::handle_key takes
{"type":"paste",  "pane":"pane-1", "text":"…"}         // bracketed when the app asked for it
{"type":"resize", "pane":"pane-1", "width":800, "height":600}   // that pane's canvas, physical px
{"type":"request","id":7, "method":"tab.new", "params":{}}
{"type":"close"}
```

Every `request` gets exactly one
`{"type":"response","id":7,"ok":true,"result":…}` or
`{"type":"response","id":7,"ok":false,"error":"message"}`.
An unknown method is `ok:false`, never a crash.

### Methods

Ids are slugs: `tab-2`, `pane-3`. `pane` omitted = the focused pane.

| method | params | result |
|---|---|---|
| `action.run` | `{id}` — any id in `otty_config::actions::ACTIONS` | `{handled:bool}` |
| `tab.new` | `{cwd?}` | `{tab,pane}` |
| `tab.activate` / `tab.close` | `{tab}` | `null` |
| `tab.move` | `{tab,target,after:bool}` | `null` |
| `tab.rename` | `{tab,title}` (empty title clears) | `null` |
| `tab.reopen` | `{}` — last closed, stack of 12 | `{tab,pane}` or `null` |
| `pane.split` | `{pane?,direction:"right"\|"left"\|"up"\|"down"}` | `{pane}` |
| `pane.focus` / `pane.close` / `pane.zoom` | `{pane}` | `null` |
| `pane.ratio` | `{pane,with?,ratio}` — first child's share 10–90 of the split that separates `pane` from `with` (omitted: the split holding `pane` directly) | `null` |
| `pane.text` | `{pane?,lines?}` | `{rows:[string]}` visible grid |
| `pane.scroll` | `{pane?,line}` grid line to the top of the viewport, or `{pane?,delta}` lines back into history (negative: towards live output) | `null` |
| `pane.select` | `{pane?,from:[col,row],to:[col,row]}` cells | `{text}` |
| `pane.context` | `{pane?,col,row,selection?}` | `[{label,action}]` |
| `find.next` | `{pane?,query,from?:{line,col},backwards?:bool}` | `{line,col}` or `null` |
| `palette.list` | `{query}` | `[{id,title,scope,shortcut}]` |
| `quickly.query` | `{filter,query}` filter ∈ all, opened, recent, folders, ssh, agents, current, recipes, files | `[{title,subtitle,kind,value}]` |
| `quickly.open` | `{kind,value}` | `null` |
| `keymap.list` | `{}` | `[{trigger,action}]` trigger like `cmd+shift+p` |
| `menu.list` | `{}` | `[{title,items:[{label,action,shortcut}]}]` |
| `settings.schema` | `{}` | `[{key,type,default,doc,group,values?}]` |
| `settings.get` | `{key}` | `{value,origin}` origin = file the value comes from, or `null` |
| `settings.set` | `{key,value,scope:"auto"\|"local"\|"shared"}` | `null` (then a `config` event) |
| `settings.reset` | `{key}` | `null` |
| `theme.list` | `{}` | `[{name,dark:bool}]` |
| `theme.set` | `{name}` | `null` (then a `ready`-shaped `chrome` event) |
| `details.info` | `{pane?}` | `{cwd,processes:[string],ports:[number],agent}` — `ports` are the TCP ports the pane's process tree listens on |
| `details.outline` | `{pane?}` | `[{line,title,kind}]` OSC 133 commands, oldest first; `line` is a grid line, as `pane.scroll` and `find.next` count them |
| `details.git` | `{pane?}` | `{root,branch,changes:[{path,status}]}` or `null` |
| `details.files` | `{path}` | `[{name,dir:bool}]` |
| `file.read` | `{path}` | `{kind:"text"\|"binary",body,language?}` |
| `composer.send` | `{pane?,text}` | `{queued:bool}` |
| `i18n.strings` | `{}` | `{language,strings:{key:value}}` |

A method the backend has not built yet answers `ok:false,"error":"not implemented"`;
the frontend must show that plainly and keep working.

## Backend → frontend

```jsonc
{"type":"ready", "version":"1.5.4", "chrome":{…}, "layout":"sidebar-left",
 "sidebar_visible":true, "platform":"windows"}
{"type":"state", …}                      // full snapshot, sent after every model change
{"type":"frame", "pane":"pane-1", "width":800, "height":600, "pixels":"<base64 RGBA>"}
{"type":"chrome", "chrome":{…}}          // theme changed
{"type":"config", "key":"font-size"}     // a setting changed
{"type":"bell", "pane":"pane-1"}
{"type":"notify", "title":"…", "body":"…", "pane":"pane-1"}
{"type":"clipboard", "text":"…"}         // the frontend writes it to the OS clipboard
{"type":"open", "url":"https://…"}       // the frontend opens it externally
{"type":"error", "message":"…"}
{"type":"quit"}
```

Frames are only sent for panes of the active tab. `chrome` is
`otty_render::theme::Chrome` serialised as today (colours are `0xRRGGBBAA` numbers).

### `state`

```jsonc
{
  "type": "state",
  "activeTab": "tab-1",
  "tabs": [{
    "id": "tab-1",
    "title": "zsh",            // prefix + user title or shell title
    "cwd": "/work/otty",       // focused pane's cwd
    "project": "/work/otty",   // git root of cwd, or ""
    "icon": "terminal",        // stem in resources/tab-icons/
    "badge": null,             // null | "running" | "completed" | "error" | {"progress":40}
    "focused": "pane-1",
    "zoomed": null,            // pane id filling the tab, or null
    "tree": {"axis":"horizontal","ratio":50,
             "first":{"pane":"pane-1"}, "second":{"pane":"pane-2"}}
  }],
  "panes": {
    "pane-1": {"kind":"terminal", "title":"zsh", "cwd":"/work/otty",
               "agent":null,   // null | {"name":"claude","state":"processing"}
               "grid":{"cols":100,"rows":32}}
  },
  "tabGroup": "none",          // none | project | date
  "dividers": [],              // tab indexes a divider sits after
  "mode": "shell"              // shell | hint | vi | mark
}
```

`axis:"horizontal"` = side by side. A leaf is `{"pane":id}`.

## Renderer feature contract

Features are classic scripts in `renderer/features/<name>.js` (+ optional
`<name>.css`), loaded automatically — adding a file is the whole registration:

```js
OttyFeatures.register('palette', (ctx) => { /* … */ });
```

`ctx`:

| member | meaning |
|---|---|
| `request(method, params)` | → `Promise<result>`; rejects with `Error(message)` on `ok:false` |
| `on(type, fn)` | backend events by `type` (`state`, `chrome`, `config`, `notify`, …) |
| `state()` | the latest `state` snapshot (or `null` before the first) |
| `focusedPane()` | pane id or `null` |
| `registerAction(id, fn)` | this feature owns action `id` in the UI (e.g. `command_palette`) |
| `runAction(id)` | the local handler if one is registered, else `action.run` on the backend |
| `keymap()` | `[{trigger,action}]`, refreshed on `config` |
| `mount(slot)` | → a fresh `HTMLElement` in `overlay` (centred, above everything), `right-panel`, `pane-overlay` (over the focused pane), `titlebar-right` or `sidebar-foot` |
| `captureKeys(fn \| null)` | while set, keydown goes to `fn(event)` instead of the terminal; return `true` if consumed |
| `focusTerminal()` | give input back to the focused pane |
| `t(key, fallback)` | localised string |
| `platform` | `"windows"`, `"macos"` or `"linux"` |

Keyboard: the core looks every keydown up in `keymap()` first; a hit runs
`runAction`, a miss goes to the pane. `cmd` in a trigger means ⌘ on macOS and
Ctrl+Shift-free "primary" (`Ctrl`) elsewhere — except where that would steal a
shell control key (`ctrl+c`, `ctrl+d`, …), which always go to the pane unless
there is a selection for `copy`.

CSS variables the core defines for features: `--window-bg --titlebar-bg
--titlebar-fg --tab-bg --tab-fg --tab-active-bg --tab-active-fg --tab-hover-bg
--tab-indicator --sidebar-bg --sidebar-fg --sidebar-border --divider
--overlay-bg --overlay-fg --overlay-border --overlay-shadow --accent --muted
--radius --font-ui --font-serif --font-mono` (chrome, long-form prose, code — the roles of `docs/TYPOGRAPHY.md`). Features style only their own classes, prefixed
with the feature name (`.palette-…`).

## Ceilings

- ponytail: frames are whole-pane RGBA in base64 over stdio. Fine at one or two
  panes; move to a binary pipe or dirty rows when a profile says so.
- One bridge per window, so a tab cannot be dragged between windows yet.
