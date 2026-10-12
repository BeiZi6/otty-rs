# Architecture

A single Cargo workspace, `E:\study_serve\otty-re\otty`, version `1.5.4`, edition 2021, `rust-version
= "1.80"`. Fourteen members; `default-members = ["packages/cli"]`. Line and test counts below are the
**2026-10-06 14:21** snapshot — `cargo build --workspace` = `Finished`, `cargo test --workspace` =
520 passed / 0 failed (see the README's *Build status*, including the note that this tree is under
active parallel development). The dependency edges are read from the per-crate `Cargo.toml` files and
are stable.

Since that snapshot: `packages/bridge` (`otty-bridge`) was added as the backend of the Electron
frontend in `frontend/electron/` — the maintained GUI, whose contract is
`frontend/electron/PROTOCOL.md` — and the Slint shell (`packages/shell`) was removed.

## The crates

| Crate | Source dir | Lines | Tests | Owns | Depends on |
|---|---|---:|---:|---|---|
| `otty-core` | `packages/core` | 1 724 | 18 | The shared vocabulary every other crate agrees on: `Color` (hex, CSS names, `rgb()`/`rgba()`, the xterm-256 cube), `NamedColor`, geometry (`Point`, `Size`, `Rect`, `Insets`, `CellSize`), ids (`WindowId`, `TabId`, `PaneId`, `SessionId`, `AgentId`), the key model (`Key`, `NamedKey`, `Modifiers`, `Chord`, `Trigger`, `KeyEvent`), `Platform`, the config/app-support directory resolution, `~`/`$VAR` path expansion, `VERSION` and `GIT_HASH`. | `serde`, `thiserror`. Nothing internal. |
| `otty-config` | `packages/config` | 11 085 | 119 | The 248-entry key registry (`keys.rs`, generated; `spec.rs`, the types and lookups), the lenient line-preserving `key = value` reader/writer (`toml_lite.rs`), value parsing/rendering (`value.rs`), layered loading and in-place writes with save scopes (`lib.rs`), themes (`theme.rs`), the semantic colour model (`palette.rs`), import/export for iTerm2/kitty/Alacritty/Ghostty (`import.rs`, `export.rs`), `tab-rule`/`details-view`/`quick-access` rules (`rules.rs`), the keymap (`keybind.rs`), the action registry (`actions.rs`). | `otty-core`, `serde`, `serde_json`, `toml`, `toml_edit`, `thiserror`, `regex`, `dirs`, `log`, `aho-corasick`, `glob`. |
| `otty-term` | `packages/term` | 4 980 | 74 | The VT engine. `attrs.rs` (cell, `ColorSpec`, SGR `Flags`, `Pen`), `grid.rs` (rows, scrollback, display offset, reflow arithmetic), `screen.rs` (cursor, modes, tab stops, charsets, primary/alternate buffers), `perform.rs` (the `vte::Perform` dispatch, one arm per sequence in the reference), `input.rs` (key/paste encoding: legacy, `modifyOtherKeys`, Kitty keyboard), `events.rs` (the boundary — see below), `lib.rs` (`AnsiPalette`, the `process()` convenience entry point). | `otty-core`, `vte`, `unicode-width`, `unicode-segmentation`, `unicode-bidi`, `log`. |
| `otty-pty` | `packages/pty` | 1 026 | 18 | The local terminal session: `portable-pty` spawn, login-shell argv, the environment Otty exports (`TERM`, `TERM_PROGRAM`, `OTTY`, `OTTY_PANE_ID`, …), shell-integration injection without editing the user's dotfiles, `ResizeGate` (coalescing a drag into one `SIGWINCH`, honouring `live-resize-sigwinch-delay-ms`), `SessionLog` with column-preserving redaction, `working-directory` resolution. | `otty-core`, `portable-pty`, `thiserror`, `log`, `base64`, `anyhow`; `libc` on Unix. |
| `otty-state` | `packages/state` | 811 | 16 | The SQLite store: session records for restore, frecency-ranked directories (with a 30-day half-life), learned per-directory commands, agent conversations with their resume/fork commands, and versioned migrations that refuse a database newer than the code. | `otty-core`, `rusqlite` (bundled), `serde`, `serde_json`, `jiff`, `thiserror`, `log`. |
| `otty-ipc` | `packages/ipc` | 787 | 17 | The control channel between the CLI, agent hooks and the running app: one listening socket per user (Unix domain socket; named pipe on Windows), one request per connection, one newline-terminated JSON object each way, request/response types for window, tab, pane and agent operations, and the two privilege gates (`ipc-allow-send-keys`, `ipc-allow-sensitive-sessions`) enforced server-side. | `otty-core`, `serde`, `serde_json`, `thiserror`, `log`; `libc` on Unix. |
| `otty-ssh` | `packages/ssh` | 811 | 25 | SSH integration: scanning `ssh` argv for a host, resolving `~/.ssh/config`, and installing/launching the remote helper. | `otty-core`, `otty-config`, `serde`, `thiserror`, `regex`, `log`. |
| `otty-agent` | `packages/agent` | 1 081 | 23 | Coding-agent integrations: parsing hook payloads, the lifecycle state machine (processing / idle / awaiting input), turn and badge decisions, agent history and the resume/fork commands. | `otty-core`, `otty-config`, `otty-state`, `otty-ipc`, `serde`, `serde_json`, `thiserror`, `log`, `jiff`. |
| `otty-autocomplete` | `packages/autocomplete` | 1 454 | 28 | The Fig-compatible spec database, inline suggestion ranking, the candidate panel's candidate model, and on-device learning. | `otty-core`, `otty-config`, `otty-state`, `rusqlite`, `serde`, `serde_json`, `regex`, `log`. |
| `otty-render` | `packages/render` | 737 | 26 | The GPU renderer: `backend.rs` (the `wgpu` device/queue/pipeline setup), `frame.rs` (the frame model), `lib.rs`. Declares `wgpu`, `bytemuck`, `pollster`, `image`. Newly written and in flux. | `otty-core`, `otty-term`, `otty-fonts`, `wgpu`, `bytemuck`, `pollster`, `image`, `log`. |
| `otty-fonts` | `packages/fonts` | 4 076 | 60 | Font discovery, the type stack, shaping and glyph rasterisation: `catalog.rs` (font discovery), `faces.rs` (face selection and the resolved stack), `stack.rs` (the fallback chain), `metrics.rs` (cell metrics), `ligatures.rs`, `blending.rs`, `theme.rs`, `error.rs`, `test_support.rs`. Declares `fontdb`, `swash`. Newly written and in flux. | `otty-core`, `fontdb`, `swash`, `thiserror`, `log`, `serde`. |
| `otty-ui` | `packages/ui` | 1 895 | — | The application shell — windows, tabs, panes, the Details panel, Settings; the consumer of `otty-term`'s events. `model.rs` (workspace model), `layout.rs` (split-tree layout), `session.rs` (session lifecycle), `lib.rs`. Its test binary hangs in a headless run, so its tests are not counted. Declares `winit`, `wgpu` (via `otty-render`), `arboard`, `rfd`, `image`, `notify`, `crossbeam-channel`. | `otty-core`, `otty-config`, `otty-term`, `otty-pty`, `otty-state`, `otty-ipc`, `otty-fonts`, `otty-agent`, `otty-autocomplete`, `otty-ssh`, `otty-render`, `winit`, `pollster`, `arboard`, `rfd`, `image`, `serde`, `serde_json`, `jiff`, `notify`, `log`, `crossbeam-channel`, `which`, `walkdir`, `regex`, `unicode-segmentation`. |
| `otty-cli` (bin) | `packages/cli` | 2 716 | — | The `otty` binary: `main.rs`, `local.rs` (everything that works with no app running), `ipc.rs` (everything that needs the app), `output.rs`. With no subcommand it launches the GUI; `config`, `font`, `theme`, `keybind`, `import`/`export`, `ssh`, `learn`, `features`, `completions`, `jump` and the `watch:<agent>` / `state:<agent>` verbs work stand-alone, while `window`/`tab`/`pane` go over `otty-ipc`. Exit codes are part of the interface and live in `output::exit` (3 = app not running, 4 = no such pane, 13 = a gate refused it). Untested. | `otty-core`, `otty-config`, `otty-term`, `otty-state`, `otty-ipc`, `otty-fonts`, `otty-agent`, `otty-autocomplete`, `otty-ssh`, `clap`, `clap_complete`, `serde`, `serde_json`, `toml_edit`, `rusqlite`, `qrcode`, `regex`, `anyhow`, `log`, `env_logger`, `which`, `url`, `walkdir`, `tempfile`, `unicode-width`. |
| `otty-helper` (bin) | `packages/helper` | 1 250 | 24 | The POSIX helper uploaded to remote hosts for stats and file transfer: `main.rs`, `host.rs`, `protocol.rs`, `lib.rs`. Declares `otty-core`, `serde`, `serde_json`, `anyhow`, `log`. Newly written and in flux. | `otty-core`, `serde`, `serde_json`, `anyhow`, `log`. |

## Layering

The rule, exactly: **`otty-core` depends on nothing in the workspace; `otty-config`, `otty-term`,
`otty-pty`, `otty-state`, `otty-ipc` and `otty-fonts` depend only on `otty-core`; everything else
depends on those.**

```text
                                 otty-core
                    ┌──────────┬──────┬──────┬──────┬─────┬────────┐
                    │          │      │      │      │     │        │
              otty-config  otty-term  otty-pty  otty-state  otty-ipc  otty-fonts
                    │          │      │      │      │     │        │
                    ├──────────┴──────┴──────┴──────┴─────┴────────┤
                    ▼                                              ▼
              otty-ssh, otty-agent, otty-autocomplete        otty-render
                    └──────────────────┬───────────────────────────┘
                                       ▼
                                   otty-ui
                                       │
                                     otty-cli
```

Concretely, the edges that exist: `otty-config → core`; `otty-term → core`; `otty-pty → core`;
`otty-state → core`; `otty-ipc → core`; `otty-fonts → core`; `otty-ssh → {core, config}`;
`otty-agent → {core, config, state, ipc}`; `otty-autocomplete → {core, config, state}`;
`otty-render → {core, term, fonts}`; `otty-ui → {core, config, term, pty, state, ipc, fonts, agent,
autocomplete, ssh, render}`; `otty-cli → {core, config, term, state, ipc, fonts, agent,
autocomplete, ssh}`. Note what the graph does **not** contain: no cycle, and no crate that exists
only to be depended on by one consumer.

Two deliberate properties of that shape:

- **`otty-term` does not depend on `otty-config`.** The engine must be drivable from a test with no
  configuration layer, so the palette it resolves colours against is injected (`AnsiPalette`
  implements the `PaletteLookup` trait) rather than read. Same for fonts and for the OS: `otty-term`
  links no platform crate.
- **`otty-ui` is the only crate that binds a windowing system.** `winit`, `wgpu` (through
  `otty-render`), the clipboard (`arboard`), the file dialogs (`rfd`) and the file watcher
  (`notify`) all appear exactly once in the whole graph.

## Platform-specific code

Almost none of it exists yet, and that is worth stating precisely rather than by category. At the
snapshot above the entire workspace's platform-conditional code is **two `cfg(unix)` / `cfg(not(unix))`
pairs and a handful of arms, all in one crate** — `otty-pty`:

| Location | Conditional | Why |
|---|---|---|
| `packages/pty/src/lib.rs:351` | `#[cfg(unix)] fn foreground_processes(pid: u32)` | Foreground-process discovery for the AppleScript `busy` / `processes` properties and the sudo/ssh session guards. Portable implementation is `ps -o pid=,ppid=,tty=,comm= -ax`, filtered by **matching the child's controlling tty**, not by assuming a child-of-child relationship. |
| `packages/pty/src/lib.rs:244` / `:260`, `:291` / `:296`, `:304` / `:308` | three `cfg(unix)` / `cfg(not(unix))` pairs inside `is_busy` and `processes` | The non-Unix arms return `false` / an empty vector. So on Windows those two answers are *known to be unavailable*, and say so. |
| `packages/pty/src/shell.rs:196` / `:204` | `cfg(unix)` / `cfg(not(unix))` | The POSIX shell-integration environment and its non-Unix counterpart. |

Everything else that is platform-aware is **data, not `cfg`**:

| Concern | How the three platforms are served | State |
|---|---|---|
| Where config/state lives | `otty-core::config_dir()` / `app_support_dir()` branch on `cfg!(target_os = ...)` inside `const fn`/plain `fn`, with `OTTY_CONFIG_DIR` and `OTTY_APP_SUPPORT_DIR` overriding. macOS and Linux share the XDG layout; Windows uses `%APPDATA%` / `%LOCALAPPDATA%`. | Implemented. |
| Window-transparency vocabulary | `theme::Material` holds **both** families as distinct values — macOS `vibrancy-thin/regular/thick`, `glass`; Windows `acrylic-thin`, `acrylic`, `mica`, `mica-alt` — and `Material::for_platform()` translates across via a documented mapping (`vibrancy-thin ↔ acrylic-thin`, `vibrancy-regular ↔ mica`, `vibrancy-thick ↔ mica-alt`, `glass ↔ acrylic`) instead of dropping the material. Linux has no vocabulary, so it reads `[window]` only and maps to `none`. | Implemented, tested (`theme::tests::materials_map_across_platforms`). |
| Theme sections per OS | `[window]` is the fallback every platform reads; `[mac]` and `[windows]` override it per OS, including a chrome-column-only `[mac.sidebar]` / `[windows.sidebar]`. | Implemented. |
| Default theme pair | `theme::default_theme_names(platform)` → Windows opens on `one-light`/`acrylic-dark` (the two themes with the Windows chrome treatment); macOS and Linux open on `paper`/`nord`. | Implemented, tested. |
| IPC transport | Unix domain socket on POSIX, named pipe on Windows. The security property differs by platform and is documented in `packages/ipc/src/lib.rs`: a POSIX socket is protected by the directory mode, a Windows named pipe by its ACL (same-or-higher integrity level). | Specified in the module docstring; the Unix transport is the implemented path. |
| The renderer | `otty-render` exists (737 lines, 26 tests): a `wgpu` backend and a frame model. It has **never drawn a frame** — nothing constructs a surface, and no shader file exists in the tree. On macOS it must also carry the *reason* the original's Metal renderer existed: one GPU layer per pane, so a pane can be lifted out into the PiP window or a torn-off tab without re-rendering the rest. | Partial: compiles and unit-tests; no verified rendering. |
| `otty-helper` | Cross-compiled for `aarch64`/`x86_64` × `apple-darwin`/`linux-musl` — the four triples the shipped bundle ships. Linux uses `musl` so the upload is statically linked. | Implemented as Rust (1 250 lines, 24 tests); not cross-compiled and not verified against a remote host. `docs/BUILDING.md` records the targets. |

The practical consequence: adding Windows support is mostly a matter of *writing* the renderer and
the platform shell, not of untangling `cfg` spaghetti. The one place that is not true is
`foreground_processes`, which needs a real Windows implementation (a `NtQueryInformationProcess`-style
walk or a `WMI` query) before the sudo/ssh guards, the `busy` AppleScript property and the
foreground-command tab rules can work there.

## The event boundary between the engine and the app

**The rule: the engine reports, the app decides.** `otty-term` never touches the OS. It links no
platform library, opens no file, and writes nothing to stdout. Anything with an effect outside the
grid turns into an `Event` value that the caller handles.

The contract is stated in `packages/term/src/events.rs`:

> The engine never touches the OS. A sequence that has a side effect outside the grid — a
> notification, a clipboard write, a theme change — turns into an `Event` instead, and the caller
> decides whether the user's privileges allow it (`privilege-notification-shell`, `clipboard-read`, …).

That sentence is why the privilege gates live in one place. The engine cannot enforce
`clipboard-read = ask`, because enforcing it means showing a dialog, and the engine has no concept of
a dialog — so it emits `ClipboardLoad { selection }` and lets `otty-ui` decide.

The full surface, grouped by what it is for:

| Event | Emitted by | The app must decide |
|---|---|---|
| `Bell`, `Enquiry` | C0 `BEL`; `ENQ` | Whether the bell is audible, visual or silent (`bell`), and what `enquiry-response` returns. |
| `Title(String)`, `IconTitle(String)` | `OSC 0`/`2`, `OSC 1` | Whether the program may set the title at all (`title-from-osc`) and how the tab label is derived (rules vs. a pinned Name/Prefix). |
| `WorkingDirectory(String)` | `OSC 7` | Which pane's cwd to update, and whether to register a frecency visit. |
| `ClipboardStore { selection, data }`, `ClipboardLoad { selection }` | `OSC 52` | `clipboard-write` / `clipboard-read` (`allow` / `ask` / `deny`), and the `privilege-clipboard-shell` gate. A `?` payload becomes `ClipboardLoad` specifically because answering means *reading* the user's clipboard. |
| `Hyperlink { uri, id }` | `OSC 8` | Whether to underline and what ⌘-click does; the href is never resolved by the engine. |
| `Notify(Notification)` | `OSC 9`, `OSC 777`, `OSC 99` | `privilege-notification-shell`, urgency handling, whether to coalesce by `id`, and whether `silent` suppresses presentation. |
| `Progress(Progress)` | `OSC 9;4` | The tab badge, the Dock icon and `otty watch`'s exit code. |
| `PaletteSet` / `PaletteQuery` / `PaletteReset`, `SpecialColorSet` / `…Query` / `…Reset` | `OSC 4`, `OSC 5`, `OSC 10`–`12`, `17`, `19`, `104`, `105`, `110`–`119` | Whether a program may recolour the terminal, and what a query answers; resets restore the *theme's* value, which the engine does not have. |
| `MouseShape(String)` | `OSC 22` | Mapping the CSS cursor name to a native pointer. |
| `PromptMark(PromptMark)`, `CommandFinished { exit_code }` | `OSC 133` | Building the Outline / Jump To index, the tab badges and the `Current` filter of Open Quickly. |
| `WindowReport(WindowReport)`, `WindowOp(WindowOp)`, `ReportTitle` | `XTWINOPS` | Acting on the window at all — and refusing to, which is the common case. |
| `ColorStack(ColorStackOp)` | `XTPUSHCOLORS` / `XTPOPCOLORS` / `XTREPORTCOLORS` | Keeping the stack, and what a report contains. |
| `ScreenAlignmentDisplay` | `DECALN` | Nothing but redrawing; it exists so a geometry check has a defined screen. |
| `RequestSetting(String)` | DCS `$q` (DECRQSS) | Looking the setting up and answering on the pty. |
| `Image(ImageEvent)` | `OSC 1337` (iTerm2), the Kitty graphics protocol, sixel | Decoding, sizing, the atlas and its eviction, and whether the image is drawn at all. Note the image is handed over **undecoded** so a huge base64 blob never blocks the parser. |
| `Resume(ResumeEvent)` | `OSC 88` | Persisting and replaying session state. The engine parses the verbs and surfaces them; it persists nothing. |
| `Agent(AgentEvent)` | `OSC 26` | Everything: the engine keeps the verb and its arguments and hands them on. |

Three properties of this boundary are load-bearing:

1. **It is the test seam.** `otty-term`'s 74 tests drive bytes through a `vte::Parser` and assert on
   the grid and the emitted events. Nothing in the engine's test suite needs a window, a pty or a
   filesystem, which is why the crate's tests run headless while `otty-ui`'s hang.
2. **It is the privilege seam.** Every gate whose name starts with `privilege-` maps onto an event
   above, and each is enforced at the consumer.
3. **It is where "the app decides" is literal.** The engine has no `Event` variant for "deny this";
   it emits and continues. A denied clipboard write is simply a `ClipboardStore` the app does not
   perform.

The engine also defines the two convenience entry points a host needs: `process(&mut Terminal, &mut
Parser, &[u8])` for one-shot feeding, and the note in `lib.rs` that the *app* must keep one `Parser`
per pane so partial sequences survive across reads.

## Differences from the original

The original's shape, recovered in `docs/RESTORATION.md` §4, was three layers: a **Swift/AppKit
shell** (stripped, no symbols recovered), a **Rust static library** (`otty_core` as the engine,
`otty_ffi` as the bridge, plus `otty_config`, `otty_autocomplete`, `otty_state`, `otty_anywhere`,
`otty_sync_*`, `otty_agent_history`, `otty_markdown`, `otty_ssh_config`, `otty_remote_shim`), and
**resources compiled in** with `include_str!`/`include_bytes!`. Rendering was **Metal**
(`Grid.metallib` ships in the bundle, byte-identical across architectures). The host-side code was
reachable from Rust only through `otty_ffi`, whose surviving trait impls (`EngineBackend as
Backend`, `HostShim as SyncHost`, plus a `sync_ffi_mock::MockBackend`) show a real FFI seam with a
compiled-in mock of the host side.

This rebuild is **Rust end to end** and has no FFI seam. The differences that matter:

| Concern | Original | This rebuild | Why |
|---|---|---|---|
| App shell | Swift + AppKit | Rust (`otty-ui` on `winit`) | The Swift source is gone and its symbols are stripped, so nothing about it could be reproduced faithfully. A Rust shell is the only one that can be built from what was recovered. |
| Rendering | Metal, one GPU layer per pane, shaders precompiled to `Grid.metallib` | `wgpu` (`otty-render`) | One renderer for three platforms. `wgpu` maps to **Metal on macOS, DirectX 12 on Windows and Vulkan on Linux** — so on macOS it lands on the same API family the original used, and on the other two it reaches the APIs the product's own documentation says are planned (`spec/FEATURES.md` records that a `[conflict]` exists between the Performance page marking DirectX 12 "planned" and the Unicode/Text Styles page stating it without that qualifier). |
| Shaders | Precompiled `.metallib` in the bundle | wgpu shader modules | The `Grid.metallib` bytes are Metal IR; they are not loadable by `wgpu` and are not portable. Nothing in this repository loads or reproduces them, and `docs/BUILDING.md` records what takes their place in the bundle layout. |
| Bridge | `otty_ffi` — a trait-typed FFI boundary with a host mock | None | With one language on both sides there is no boundary to bridge. This is the largest structural divergence, and it means the original's `HostShim`/`SyncHost` design is documented rather than reproduced. |
| Engine internals | `alacritty_terminal` (41 symbols) + `vte` (9) | Hand-written `otty-term` on `vte` alone | Otty's *observable* behaviour is specified better by its own 106-row sequence table (`spec/vt-sequences.md`) than by Alacritty's implementation, and the event boundary above is not something `alacritty_terminal` offers. `vte` is kept because it is a parser state machine, not a terminal. |
| Config | Rust (`otty_config`, 247 symbols) | Rust (`otty-config`), same registry provenance | The one layer that is a genuine continuation rather than a reimplementation: the same generated key list, the same theme bytes. |
| Networking | `tokio` + `hyper` + `reqwest` + `rustls` + `ring` + `ed25519_dalek` (Data Sync, Remote Control, agent transport) | Not implemented | Nothing in this rebuild opens a network connection. The dependency set is deliberately not carried forward, because carrying it without the features would be dead weight. |
| Persistence | `rusqlite` (46) | `rusqlite` (bundled) | Same choice, same reason: a single-file local store with SQL. |
| AppleScript / accessibility | Present (`Otty.sdef`, `NSAppleScriptEnabled`, `NSAccessibility`) | Not implemented | Both are macOS-only surfaces whose behaviour depends on the Swift shell. The dictionary was recovered (5 580 bytes) and the documented contract is in `spec/FEATURES.md` §7; the implementation is not started. |

The per-feature delta (status, DMG evidence, docs contract, suggested crate) lives in
[`docs/GAPS.md`](GAPS.md). Close one `GAP-*` id per commit.

The single most important thing to carry forward from the original is not the code layout but the
**trait boundary that `otty_ffi` encoded**: a session is an abstraction over "a pty on this machine"
or "a stream on another", the sync engine takes a backend it can mock, and logging funnels through
one sink. `otty-pty`'s module docstring and `otty-ipc`'s transport split preserve that idea in Rust
(`Pty` is a local session; the transport is swappable; `otty-state` is the single sink) — the
boundary is kept, the FFI is not.
