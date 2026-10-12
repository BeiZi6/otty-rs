# Otty — reconstruction

Otty is a terminal emulator for macOS, version 1.5.4, bundle id `io.appmakes.otty`. Its workspace is
three nested levels: a **window** owns the title bar, the tab sidebar (or horizontal tab bar) and all
tabs; a **tab** is one independent terminal session holding a tree of **splits**; a **pane** is the
leaf and the only thing you interact with — a terminal, file, folder or URL pane. Around that core
sit a docked **Details panel** with four shipped tabs (Info, Outline, Git, Files) plus user-defined
Terminal-Program and Web-Page views, a VSCode-style **Command Palette** (⌘⇧P), an Xcode-style **Open
Quickly** over nine filters (⌘⇧O), an OSC 133-driven **Outline / Jump To** index, bundled-ripgrep
**Search in Files**, in-pane **Find** (⌘F) with global ⇧⌘F, hint mode, Vi mode, recipes, session
recovery, and coding-agent awareness. Full surface: [`spec/FEATURES.md`](spec/FEATURES.md) §1.

## What this repository is

A **from-scratch, cross-platform reimplementation** of Otty, reconstructed after the source code was
lost, from two shipped macOS installers plus the vendor documentation site and nothing else. It is
**not** the original product, it does not contain the original source, and it does not ship the
original binaries. Every recovered fact, and the tool that recovered it, is recorded in
[`docs/RESTORATION.md`](docs/RESTORATION.md). The living gap list against the docs and the DMG is [`docs/GAPS.md`](docs/GAPS.md). Nothing here is a decompiled dump — the Rust packages
are written by hand; what was recovered from the shipped bundle is the *specification* (the config
key list, the theme bytes, the VT sequence table, the functional digest), and that is what the
hand-written code is checked against.

## Build status

> **Timestamped snapshot of a moving target.** The tree is under active parallel development: crates
> were rewritten while this document was written, and the observed states ranged from *12 crates
> green* to *`otty-fonts`/`otty-render`/`otty-ui` mid-edit and not compiling* to *the build failing
> because `E:\` was full*. The numbers below come from the last full pass — **2026-10-06 14:21**:
> `cargo build --workspace` = `Finished`, `cargo test --workspace` = **520 passed, 0 failed**. Re-run
> both before relying on any number. Environment constraints also observed: `E:\` repeatedly hit **0
> bytes free** (the `target/` tree reaches ~5.7 GB), failing a fresh build with `磁盘空间不足 (os
> error 112)` / `LNK1140` though the source is fine; and that exhaustion corrupted the host toolchain
> (a duplicate `1.96.0-x86_64-pc-windows-msvc` lost its `rustc`), repaired by deleting the broken
> toolchain so `rustup` falls through to `stable`. Listed test counts are that pass's per-crate
> library suites; the workspace-wide 520 also counts integration and doc-test targets.

| Package | Source | Tests | Notes |
|---|---:|---:|---|
| `otty-core` | 1 724 lines / 5 files | **18** | Colours (hex, CSS names, `rgb()`/`rgba()`), geometry, ids, the key/chord/trigger model, platform paths. |
| `otty-config` | 11 085 lines / 12 files | **119** | The 248-entry key registry, the line-preserving `key = value` reader/writer, layered loading, themes, import from iTerm2/kitty/Alacritty/Ghostty, export to the last three, `tab-rule`/`details-view`/`quick-access` rules, the keymap. |
| `otty-term` | 4 980 lines / 8 files | **74** | The VT engine: grid, scrollback, reflow, attributes, key encoding. See *Maturity* below. |
| `otty-fonts` | 4 076 lines / 10 files | **60** | Font discovery and the type stack: faces, catalog, metrics, ligatures, blending, theme wiring. Newly written and in flux. |
| `otty-pty` | 1 026 lines / 2 files | **18** | PTY spawn, shell discovery, shell-integration injection, resize coalescing, session logging with column-preserving redaction. |
| `otty-helper` | 1 250 lines / 4 files | **24** | The remote helper: host probe and wire protocol. |
| `otty-render` | 737 lines / 3 files | **26** | The `wgpu` render backend and frame model. Newly written and in flux. |
| `otty-state` | 811 lines / 1 file | **16** | SQLite persistence: session history, frecency, learned commands, agent conversations; migrations that refuse a newer database. |
| `otty-ipc` | 787 lines / 1 file | **17** | One socket per user, one request per connection, JSON both ways, with the two privilege gates enforced server-side. |
| `otty-ssh` | 811 lines / 1 file | **25** | `ssh` argv scanning, `ssh_config` resolution, remote-helper install. |
| `otty-agent` | 1 081 lines / 1 file | **23** | Coding-agent hooks, lifecycle state, history and resume. |
| `otty-autocomplete` | 1 454 lines / 2 files | **28** | Fig-compatible spec database, inline suggestions, on-device learning. |
| `otty-ui` | 1 895 lines / 4 files | — | The application shell: `model.rs`, `layout.rs`, `session.rs`. Its test binary hangs headless, so it is **not** covered by the 520. |
| `otty-cli` | 2 716 lines / 4 files | — | The `otty` binary: `main.rs`, `local.rs`, `ipc.rs`, `output.rs`. Untested. |

14 workspace members, 58 Rust files, 34 433 lines of Rust across `packages/`.

### Maturity, stated plainly

- **Verified** — the twelve packages with counts above compile and pass their own unit tests. The
  Electron frontend (`frontend/electron`) is the GUI: it draws the chrome and puts on screen the
  frames `otty-bridge` composites, so the PTY → VT → font rasteriser → RGBA path runs end to end in
  a real window.
- **In flux** — `otty-fonts`, `otty-render` and `otty-ui` were written during this session and moved
  between *compiling*, *not compiling* and *test-hanging* while this document was written. Treat
  their numbers and internals as transient; the current shell verification is recorded above.
- **`otty-term`'s scope**, from its own module docstrings and the sequence table — every C0 control;
  the ESC set (IND, NEL, RI, HTS, DECSC/DECRC, DECKPAM/DECKPNM, DECALN, RIS); the CSI set (cursor
  movement, CUP/CHA/VPA, ED/EL/ECH, ICH/DCH/IL/DL, SU/SD, DECSTBM, DECSLRM, DECSCA with selective
  erase, DECSCUSR, DECSTR, DSR/DECDSR, DA, XTWINOPS, XTPUSHCOLORS/XTPOPCOLORS/XTREPORTCOLORS); the
  OSC set (0/1/2, 4, 5, 7, 8, 9, 9;4, 10–12, 17, 19, 22, 26, 52, 77, 88, 99, 104, 105, 110–119, 133,
  1337); DCS (DECRQSS, sixel); and the DEC private modes. Documented as partial: `OSC 88` resume
  (verbs parsed and surfaced, no state persisted), Kitty graphics (control block parsed, per-action
  matrix not published, so only transmit-and-display is modelled), and the clamping of
  CUU/CUD/DL/SU/SD (clamped to the active region).
- **Still limited** — the native macOS Swift frontend was not recovered. The cross-platform
  frontend in this tree is Electron over `otty-bridge` (`frontend/electron/PROTOCOL.md`); the
  terminal is still the Rust engine and rasteriser, not a second renderer. The earlier Slint shell
  (`packages/shell`) has been removed and is not maintained.

## Build and test

Rust 1.80 or later (workspace `rust-version`; developed against 1.96).

```sh
cargo build --workspace                  # Finished in the 2026-10-06 14:21 pass
cargo test  --workspace                  # 520 passed, 0 failed at that moment
cargo test  -p otty-core -p otty-config    # the two crates this brief singles out: 137 tests
cargo test  -p otty-term -p otty-fonts -p otty-render -p otty-pty -p otty-state \
            -p otty-ipc -p otty-ssh -p otty-agent -p otty-autocomplete -p otty-helper
cargo test  -p otty-ui                    # caution: hangs headless
```

Per-platform packaging, the macOS bundle layout, code signing and the four `otty-helper`
cross-builds: [`docs/BUILDING.md`](docs/BUILDING.md).

## Where the configuration lives

| Scope | Path |
|---|---|
| Shared config (sync between machines) | `~/.config/otty/config.toml` |
| This machine only; its keys win | `~/.config/otty/config.local.toml` |
| Themes | `~/.config/otty/themes/<slug>.ottytheme` |
| Drop-in fonts | `~/.config/otty/fonts/` |
| Caches and state | macOS `~/Library/Application Support/otty`; Windows `%LOCALAPPDATA%\otty`; Linux `$XDG_STATE_HOME/otty` (fallback `~/.local/state/otty`) |
| Runtime override | `OTTY_CONFIG_DIR`, `OTTY_APP_SUPPORT_DIR` |

On Windows the config directory is `%APPDATA%\otty`; macOS and Linux follow the XDG layout so one
dotfile repository keeps working across them. Reading is layered: every `include` first, in order,
then `config.toml`, then `config.local.toml` last and winning. Writing never regenerates the file —
every change edits the file the value already comes from. The registry holds **248 keys**; their
authoritative spelling is [`spec/config-schema.json`](spec/config-schema.json) and
`packages/config/src/keys.rs`.

## Supported platforms

| Platform | Architecture | State |
|---|---|---|
| macOS | arm64 (`aarch64-apple-darwin`), x86_64 (`x86_64-apple-darwin`) | **Target.** Both shipped installers were read. Requires macOS 14 (Sonoma) or later, per the product's installation page and `LSMinimumSystemVersion = 14.0`. |
| Windows | x86_64 (`x86_64-pc-windows-msvc`) | **Target.** No Windows build produced; the only Windows-aware code is `otty-core`'s path helpers and `otty-ipc`'s named-pipe transport. |
| Linux | x86_64 / aarch64 (`*-unknown-linux-musl`) | **Target.** Same caveat. The four `otty-helper` triples show Linux/`musl` is the intended remote-host target. |

The rebuild draws with `wgpu`: Metal on macOS, DirectX 12 on Windows, Vulkan on Linux. The original did not — see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Licence position

Two separate things, and they must not be conflated:

- **This reconstruction** is MIT-licensed (`license = "MIT"` in the root `Cargo.toml`).
- **Otty itself** is proprietary. The shipped [`License Agreement`] at `docs.otty.sh/license` grants a
  personal, non-exclusive, non-transferable right to *use* the app, retains all intellectual-property
  rights in appmakes.io, and forbids redistribution of the Software as your own product. This
  repository redistributes none of its code: no binaries, no Swift, no `Grid.metallib`, no
  `settings-ui.html`, no `Assets.car`. What it does redistribute is the 25 short theme configuration
  texts recovered from a binary it does not ship, plus the third-party fonts and resource files whose
  *own* licences permit it — all seven fonts under `resources/fonts/` are SIL OFL 1.1 or MIT per the
  shipped bundle's `CREDITS.md`, and the 165 recovered resource files were hash-verified against the
  bundle. **Styrene** and **Tiempos**, the commercially licensed families the typography decision
  names, are absent: see [`docs/TYPOGRAPHY.md`](docs/TYPOGRAPHY.md).

If you are appmakes.io and you want this taken down, the answer is yes — but the reconstruction
exists to document a product that no longer has source.

## Documentation

| Document | What it covers |
|---|---|
| [`docs/RESTORATION.md`](docs/RESTORATION.md) | How the reconstruction was done, step by step, with the exact tool and command for each step; and what could not be recovered. |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | The crate graph, the layering, where platform code lives, the engine↔app event boundary, and the divergences from the original. |
| [`docs/TYPOGRAPHY.md`](docs/TYPOGRAPHY.md) | The type stack: Styrene A/B for UI chrome, Tiempos Text for editorial, JetBrains Mono for the grid, and the fallback chain. |
| [`docs/BUILDING.md`](docs/BUILDING.md) | Prerequisites, per-platform builds, the macOS `.app` assembly, code signing, and the four `otty-helper` cross-targets. |
| [`spec/FEATURES.md`](spec/FEATURES.md) | The 569-line functional specification digested from 132 crawled documentation pages, conflicts left unresolved and marked. |
| [`spec/vt-sequences.md`](spec/vt-sequences.md) | All 106 VT sequences with the implementation status the documentation gives each one. |
