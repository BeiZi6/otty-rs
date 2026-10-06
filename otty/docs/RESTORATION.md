# Restoration — how Otty 1.5.4 was reconstructed without its source

A reproducible method, in the order it was actually run. Every step names the tool that did the work
and the command that re-runs it. Paths are absolute and were current when this document was written.

## 0. What was available, and what was not

| | Detail |
|---|---|
| **Available** | Two shipped macOS installers: `E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app` (Apple Silicon) and `E:\study_serve\otty-re\dmg\x64\Otty (Intel)\Otty.app`. Both are Otty 1.5.4, `io.appmakes.otty`. |
| **Available** | The vendor documentation site, `https://docs.otty.sh` — a VitePress site whose `sitemap.xml` lists 132 pages. |
| **Missing** | The source. No Swift, no Rust, no build files, no repository. Nothing in this document was read from source, because there is none. |
| **Missing** | Any symbol table for the Swift majority of the app: the shipped Swift is stripped, so the Swift side was reconstructed from its *resource* surface (localisation catalogues, the settings UI bundle, `Info.plist`) rather than from code. |
| **Consequence** | Everything recovered is either (a) a resource the build embedded verbatim, (b) a symbol the Rust static library left unmangled, or (c) a behavioural statement the documentation makes. Where a fact could only be inferred, it is marked as inferred. |

The reconstruction had to answer three questions in order: *what is in the binary* (inventory), *what
does the code do* (symbols and resources), *what was it supposed to do* (documentation). Steps 1–7
answer the first two; steps 8–9 answer the third.

## 1. Reading the DMGs

A `.dmg` is an HFS+ (or APFS) filesystem image. There is no mounting step on a Linux or Windows host,
so the images were expanded with **7-Zip**, which understands both the UDIF wrapper and HFS+.

```sh
7z x "otty-arm64.dmg" -o"E:\study_serve\otty-re\dmg\arm64"
7z x "otty-intel.dmg" -o"E:\study_serve\otty-re\dmg\x64"
```

Two artifacts of the extraction are worth knowing about, because they look like corruption and are
not:

- Each volume carries HFS+ private-directory scaffolding — `.fseventsd/`, `.HFS+ Private Directory
  Data_/`, `[HFS+ Private Data]/` — which is filesystem bookkeeping, not app content.
- `Contents\_CodeSignature\CodeResources` extracts as a 1779-byte file, but the code-directory digest
  itself lives in the file's extended attributes, which 7-Zip does not carry. The signature is
  therefore **not** recoverable from this extraction, and nothing in the reconstruction depends on it.

The next step depends on step 1 having produced a readable Mach-O. To check:

```sh
python E:\study_serve\otty-re\tools\macho.py ^
  "E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\MacOS\Otty" ^
  "E:\study_serve\otty-re\dmg\x64\Otty (Intel)\Otty.app\Contents\MacOS\Otty"
```

`tools/macho.py` is a 6.3 KB stand-alone Mach-O/FAT inspector: slice architectures, `LC_LOAD_DYLIB`
and `LC_RPATH` lists, `LC_BUILD_VERSION` (platform, minimum OS, SDK), the `LC_UUID`, and
`LC_SOURCE_VERSION`. It accepts any number of file paths and prints one report per file. This is what
established that the shipped core is a **stripped Rust `staticlib` linked into a Swift app** — not a
Rust *binary*, which is why `strings` on it yields Rust symbols but no `_main` for a Rust target.

## 2. The app bundle inventory

Everything the product ships lives under `Contents\`. The arm64 and Intel bundles are identical in
resource layout; only the four executable sizes differ.

| Path (relative to `Contents\`) | arm64 | x64 | What it revealed |
|---|---:|---:|---|
| `MacOS\Otty` | 39 154 624 | 42 976 864 | The app. A stripped Mach-O carrying a Swift/AppKit shell plus the Rust core linked in as a static library. Both bundles contain the Rust symbol set and the `include_str!` resource text. |
| `MacOS\otty-cli` | 4 917 968 | 5 706 480 | The `otty` command-line tool, a separate Rust **binary**. Its symbol set is small and clean (`otty_config` 163 symbols, `clap_builder` 132, `regex_automata` 114, `qrcode` 37) and is what proved the CLI is Rust end to end. |
| `MacOS\rg` | 4 010 304 | 4 440 480 | A bundled ripgrep, for Search in Files. Not Otty code. |
| `MacOS\otty-sleep-helper` | 123 584 | 85 968 | The `caffeinate`-equivalent helper, plus `Library\LaunchDaemons\io.appmakes.otty.sleep-helper.plist` to register it. |
| `Frameworks\libswiftCompatibilitySpan.dylib` | 73 520 | 73 520 | A Swift runtime compatibility shim; tells you the Swift toolchain used was new enough to need back-deployment of `Span`. |
| `Resources\Grid.metallib` | 68 810 | 68 810 | Compiled Metal shaders. Byte-identical between the two bundles — this is the single clearest evidence that the renderer is **Swift + Metal**, not Rust. |
| `Resources\settings-ui.html` | 1 498 025 | 1 498 025 | The Settings window, shipped as one Vite-built bundle. Byte-identical between architectures. |
| `Resources\en.json` / `zh.json` | 165 213 / 166 634 | same | The localisation catalogues — in practice a complete feature index. |
| `Resources\completions-client.db` | 11 821 056 | same | The Fig-compatible autocomplete spec database, prebuilt. |
| `Resources\syntaxes.bin` | 644 632 | same | The bundled syntect grammar dump. |
| `Resources\Assets.car` | 1 389 688 | same | Compiled asset catalogue (app icon and UI imagery). |
| `Resources\Otty.sdef` | 5 580 | same | The AppleScript dictionary. |
| `Resources\mermaid.min.js` | 3 572 661 | same | Used by the file-pane Markdown preview. |
| `Resources\CREDITS.md` | 8 799 | same | Every bundled font, icon set and Rust crate with its licence — an authoritative third-party dependency list, and the reason this reconstruction knows what it may and may not redistribute. |
| `Resources\fonts\` | 7 files | same | JetBrains Mono (regular + italic), Office Code Pro (4 weights/styles), Symbols Nerd Font Mono. |
| `Resources\editors\` | 3 files | same | `editors\README.md`, `editors\generate.py`, `editors\vim\{ftdetect,syntax}\otty.vim`, `editors\bat\otty.sublime-syntax`. See step 6. |
| `Resources\shell-integration\` | 6 files | same | `otty-integration.{bash,zsh,fish}`, `zsh\.zshenv`, `fish\vendor_conf.d\otty-shell-integration.fish`. |
| `Resources\helper\` | 4 files | same | Four `otty-helper` builds, one per cross-target. See step 3 and `docs/BUILDING.md`. |
| `Resources\agent-integration\` | 11 files | same | One hook per coding agent: Claude, Codex, Cursor, Copilot, Grok, Kimi, OpenCode, Antigravity, CodeBuddy, Pi, Reasonix. |
| `Resources\terminfo\` | 5 files | same | `61\alacritty`, `61\alacritty-direct`, `61\alacritty+common`, `78\xterm-ghostty` — so Otty *builds on* Alacritty's and Ghostty's terminfo rather than shipping a fresh `xterm-otty`. |
| `Resources\tab-icons\` / `view-icons\` | 62 + 62 | same | Icon sets, all SVG. |
| `Contents\Info.plist` | — | — | 88 keys. `CFBundleShortVersionString`/`CFBundleVersion` = `1.5.4`, `LSMinimumSystemVersion` = `14.0`, `NSAppleScriptEnabled`, `OSAScriptingDefinition`, 5 exported document types, 3 URL schemes, and a custom `OttyGitHash` key that `otty-core::GIT_HASH` mirrors. |

`E:\study_serve\otty-re\manifest_arm64.txt` (188 lines) records the arm64 file inventory with sizes;
it is the flattened form of this table.

**What was carried across, and how that was checked.** Every non-derived resource in that table was
copied out of the bundle into `resources/`, and the copy was then verified file by file against the
bundle by SHA-256:

| Directory under `resources/` | Files | Bytes | Identical to the shipped bundle |
|---|---:|---:|---|
| `fonts/` | 7 | 3 263 808 | 7 / 7 |
| `terminfo/` | 5 | 16 084 | 5 / 5 |
| `agent-integration/` | 11 | 36 885 | 11 / 11 |
| `shell-integration/` | 5 | 144 988 | 5 / 5 |
| `editors/` | 5 | 27 730 | 5 / 5 |
| `tab-icons/` | 62 | 44 860 | 62 / 62 |
| `view-icons/` | 62 | 101 144 | 62 / 62 |
| `view-sandbox/` | 3 | 1 823 | 3 / 3 |
| root: `CREDITS.md`, `dotfiles-catalog.json`, `more-from-us.json`, `Otty.sdef` | 4 | 44 619 | 4 / 4 |
| **subtotal** | **164** | **3 681 941** | **164 identical, 0 differing, 0 absent** |
| `themes/` | 25 | 39 416 | recovered, with the one documented deviation per file (step 7) |

(`resources/README.md`, 3 149 bytes, is written for this repository, not recovered — so `resources/`
holds 190 files: 164 recovered bytes, 25 themes, and that one index. The SHA-256 comparison above was
run over the 164.)

All seven fonts are redistributable under their own licences (SIL OFL 1.1 for JetBrains Mono and
Office Code Pro, MIT for Symbols Nerd Font, per the bundle's `CREDITS.md`), which is why they could
be carried across. The *derived* resources — `completions-client.db`, `syntaxes.bin`, `Assets.car`,
`settings-ui.html`, `Grid.metallib`, `otty.icns`, `mermaid.min.js`, `en.json`, `zh.json` — were read
and analysed (steps 8–10) but are **not** in the repository; `docs/BUILDING.md` lists each with its
size and what its absence costs.

## 3. The four `otty-helper` cross-targets

`Resources\helper\` holds the same helper built four times, and the directory names are the
`rustc` target triples verbatim:

| Directory | Size (arm64 bundle) | Size (x64 bundle) |
|---|---:|---:|
| `aarch64-apple-darwin\otty-helper` | 519 376 | 519 376 |
| `x86_64-apple-darwin\otty-helper` | 551 744 | 551 744 |
| `aarch64-unknown-linux-musl\otty-helper` | 573 056 | 573 056 |
| `x86_64-unknown-linux-musl\otty-helper` | 635 360 | 635 360 |

Two facts fall out of this, and both are load-bearing for `docs/BUILDING.md`:

1. **The helper is shipped for the remote host, not the local one.** A macOS build carries Linux
   builds, because the helper is uploaded to whatever machine you `ssh` into. The Linux targets are
   `musl`, not `gnu` — statically linked, so the upload works on any distribution.
2. **Both macOS bundles carry byte-identical helper trees.** The helper is architecture-independent
   of its packager, so the build extracts `CFBundleShortVersionString` once and cross-compiles all
   four from the same source revision.

## 4. Symbol demangling: the Rust crate and module tree

The Rust core was linked into the app as a static library, and a static library does **not** get its
symbols stripped the way the final executable's own symbols do. `strings` therefore recovered
thousands of legacy-mangled (`_ZN…17h…E`) Rust symbols from the shipped binaries.

```sh
python E:\study_serve\otty-re\tools\strings_analyze.py ^
  "E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\MacOS\Otty" ^
  "E:\study_serve\otty-re\analysis\strings_main.txt"

python E:\study_serve\otty-re\tools\strings_analyze.py ^
  "E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\MacOS\otty-cli" ^
  "E:\study_serve\otty-re\analysis\strings_cli.txt"

python E:\study_serve\otty-re\tools\demangle.py "E:\study_serve\otty-re\analysis\strings_main.txt"
python E:\study_serve\otty-re\tools\demangle.py "E:\study_serve\otty-re\analysis\strings_cli.txt"
```

`tools/strings_analyze.py` extracts ASCII runs of ≥ 6 bytes and sorts them into categories by regex:
`rust-marker`, `swift-marker`, `objc-class`, `toml-key`, `config-path`, `osc`, `vt-name`, `uri`,
`appkit`, `bundle-id`, `cmdline`, `font-name`, `agent`. Its output is the `strings_*.txt` family —
`strings_main.txt` is 4.46 MB / 12 518 legacy symbols, `strings_cli.txt` 649 KB / 3 664 symbols.

`tools/demangle.py` decodes the `_ZN` grammar (length-prefixed path segments plus the `$…$` escape
table), strips the trailing `17h<16 hex>E` hash, and emits the module tree it implies. Its output is
`analysis\modules_main.txt` (928 KB, 5 072 distinct modules) and `analysis\modules_cli.txt` (145 KB,
720 modules).

**What the trees told us.** The crate list, ordered by symbol count, is a dependency lock-file that
nobody had to write. From `modules_main.txt`, the first tier:

```
otty_config 247 · otty_sync_dotfiles 187 · otty_core 180 · otty_autocomplete 122
otty_anywhere 107 · otty_agent_history 50 · otty_sync_transport 45 · otty_state 33
otty_ssh_config 23 · otty_markdown 11 · otty_remote_shim 8 · otty_ffi 5
```

and the third-party tier that implies the architecture:

```
tokio 399 · rustls 221 · ring 189 · jiff 182 · toml_edit 167 · image 165 · moxcms 156
core_graphics 145 · socket2 140 · hyper_util 107 · icu_locale_core 100 · core_text 87
core_foundation 55 · rusqlite 46 · syntect 54 · pulldown_cmark 54 · serial2 27 · argon2 17
ed25519_dalek 12 · curve25519_dalek 86 · alacritty_terminal 41 · vte 9 · portable_pty 10
```

Three structural facts follow, and they shaped every later decision:

- **The core is async and networked.** `tokio` + `hyper` + `reqwest` + `rustls` + `ring` +
  `socket2` are not what a local terminal needs. They are what **Data Sync**, **Remote Control** and
  the agent integrations need. `curve25519_dalek` + `ed25519_dalek` + `argon2` say the transport is
  authenticated and the local store is encrypted.
- **The tray/settings/remote surfaces are separate Rust crates, not Swift.** `otty_anywhere` (the
  remote-control protocol), `otty_sync_dotfiles` (settings/recipe sync, with its own split-and-merge
  node type), `otty_sync_transport`, `otty_remote_shim`, `otty_ssh_config`, `otty_agent_history`,
  `otty_markdown` — every one of these is a Rust crate reachable from the core.
- **`otty_ffi` is the bridge, and it is a trait boundary.** The surviving trait impls are the
  clearest statement of the architecture in the whole binary:

  ```
  <otty_ffi::sync_ffi::EngineBackend as otty_ffi::sync_ffi::Backend>           29 symbols
  <otty_ffi::sync_ffi::HostShim as otty_sync_transport::host::SyncHost>        20
  <otty_ffi::sync_ffi_mock::MockBackend as otty_ffi::sync_ffi::Backend>        12
  <otty_ffi::file_logger::CombinedLogger as log::Log>                           3
  <otty_core::pty::Pty as otty_core::session_transport::SessionTransport>        3
  <otty_core::pty::lazy::LazyPty as otty_core::session_transport::SessionTransport> 5
  <otty_core::host_stream::HostStreamTransport as otty_core::session_transport::SessionTransport> 3
  <otty_core::pty_writer::PtySink as std::io::Write>                             2
  <otty_core::sixel_scanner::SixelScanner as core::default::Default>             1
  ```

  `EngineBackend` is implemented by both a real `sync_ffi` backend and a `sync_ffi_mock` — a mock of
  the *host* side, compiled into the release build, which only makes sense if the host is on the
  other side of an FFI boundary. `otty_core::session_transport::SessionTransport` has three
  implementations (a live `Pty`, a `LazyPty`, and a `HostStreamTransport`), so "a session" was
  already an abstraction over "a pty here" and "a stream over there". `log::Log` is implemented by
  `CombinedLogger`, so logging was funnelled through one Rust sink back into the Swift logger.

**The recovered crate inventory, stated plainly.** Swift/AppKit shell (stripped, no symbols) → Rust
static library (`otty_ffi` as the bridge, `otty_core` as the engine) → resources compiled in with
`include_str!`/`include_bytes!`. This rebuild does not reproduce the split; see
`docs/ARCHITECTURE.md`.

`tools/macho.py` also matters here, because the demangling only works if you point it at the right
file. The Swift shell's own symbols are stripped, which is why the classification counts in
`strings_main.txt` skew so heavily Rust — absence of Swift symbols is an artifact of stripping, not
evidence that the Swift side was small.

## 5. The dependency list, and what it implies

The `CREDITS.md` inside the bundle and the `modules_*.txt` crate lists agree, which is a useful
cross-check: `CREDITS.md` says what the vendor believed it shipped, the symbols say what the linker
actually pulled in. `alacritty_terminal` appears in both, as does `vte`, `portable_pty`, `syntect`,
`rusqlite`, `image`, `core-graphics`, `core-text`, `core-foundation`. The rebuild's root `Cargo.toml`
selects its dependency set from that intersection.

Two recoveries constrain the rebuild directly:

- **`alacritty_terminal 41` + `vte 9`.** Otty's grid and escape parsing were built on the Alacritty
  terminal crate. The rebuild keeps `vte` in `otty-term` but does not take `alacritty_terminal`,
  because the visible behaviour is specified better by Otty's own sequence table (step 8) than by
  Alacritty's implementation.
- **`moxcms 156` + `image 165`.** Colour management is first-class, not incidental: the theme model
  has materials, tints, opacity and cross-platform translation (`docs/ARCHITECTURE.md`), and inline
  images go through a real colour pipeline.

## 6. Config keys, recovered from the app's own generated syntax file

This is the single highest-value recovery in the whole project, because it is a list the *app
generates from its own schema*. Otty ships an editor-integration generator
(`Resources\editors\generate.py`, 9 088 bytes) and its output for Vim:

```
Resources\editors\vim\syntax\otty.vim
```

Inside that syntax file, two generated `syn match` lines carry the complete key vocabulary as a
`\%(…\)` alternation group: `ottyKnownKey` and `ottyEnum`. Extracting them is exact — no guessing, no
fuzzy matching, no inference from documentation prose.

```sh
python E:\study_serve\otty-re\tools\extract_keys.py
```

`tools/extract_keys.py` reads the two lines, splits on `\|`, sorts and de-duplicates, and writes:

- `analysis\config_keys.txt` — **236 keys**
- `analysis\enum_values.txt` — **77 enum literals**

(The script hard-codes the shipped-bundle path and the output paths; it asserts > 150 keys so a
malformed read fails loudly rather than writing a short list.)

The 236-key list is not the same as the registry the rebuild ships, and the difference is the point:

| | Count | Explanation |
|---|---:|---|
| Keys in `config_keys.txt` | 236 | The literal alternation group in the generated syntax file. |
| …of which are legacy spellings | 4 | `font-antialiased`, `font-smooth`, `ligatures`, `shell`. The syntax highlighter knows them because user configs contain them; the runtime maps them to canonical names. |
| Canonical keys the syntax file does not spell out | 16 | `palette-0` … `palette-15` — generated per-index by the app, so they are pattern-matched rather than enumerated in the alternation. |
| **Registry shipped by this rebuild** | **248** | 236 − 4 + 16 = 248, and `packages/config/src/keys.rs` holds exactly 248 `KeySpec` literals, all distinct. |

The canonical registry itself (`spec/config-schema.json`, 111 KB / 3 588 lines) merges three sources,
named in its own `generated_from` field: `reference__configuration.txt`, `reference__config-file-format.txt`,
and `config_keys.txt`. It carries, per key, `section`, `kind`, `default`, `default_note`, `aliases`
(one-directional: a legacy spelling points at its canonical key, never the reverse), `platform`,
`value_shape` and a prose `description` quoted from the documentation. That file is the input to:

```sh
python E:\study_serve\otty-re\tools\gen_keys.py
```

`tools/gen_keys.py` emits `packages/config/src/keys.rs` (126 KB of data). The same mechanism generates
the keybinding action registry from the shipped localisation catalogue rather than from documentation:

```sh
python E:\study_serve\otty-re\tools\gen_actions.py
```

`tools/gen_actions.py` reads `settings.key_bindings.actions` out of `Resources\en.json` and emits
`packages/config/src/actions.rs`, grouping ids by namespace (`general`, `window`, `tabs`, `pane`, …).
The action ids are therefore verbatim product strings, not paraphrase.

For completeness: `tools/canon_spelling.py`, `tools/fix_tests.py` and `tools/fix_round2.py` are the
one-off repair passes that followed — bringing the keymap and the tests onto the shipped key map's
canonical chord spelling (`cmd+alt+d` → `cmd+opt+d`, `cmd+[` → `cmd+left_bracket`,
`cmd+ctrl+arrow-up` → `cmd+ctrl+up`), and correcting assertions written before that spelling was
adopted. They are kept because they document *what changed*, not because they are re-run.

## 7. Theme recovery from in-binary `include_str!` text

The 25 built-in themes are compiled into the shipped binary by `include_str!`, so the binary carries
the original text byte for byte.

```sh
python E:\study_serve\otty-re\tools\extract_embedded.py
```

`tools/extract_embedded.py` walks the 39 MB Mach-O looking for printable runs of ≥ 160 bytes that
contain at least two newlines and are > 90 % printable, then classifies each run as `theme-toml`,
`json`, `script`, `config`, `doc` or `text` and writes it to
`analysis\embedded\<offset>-<kind>.txt`, plus `_index.tsv` (`offset`, `length`, `kind`, `file`).
Result: **384 runs** — 191 `doc`, 142 `text`, 30 `toml`, 20 `config`, 1 index. Because every run keeps
its byte offset, any reconstructed text can be checked back against the binary at an address.

The classifier targets theme TOML by looking for `[terminal]`, `[meta]` or `[colors` in the run. Two
things make the byte-exactness verifiable rather than asserted:

- The strings are present **twice** in the arm64 binary. A direct byte search finds theme text at
  offset `0x17C286F` and again at `0x1C3CF69` — the `include_str!` copy inside the Rust static
  library, and a second copy the linker or a resource-embedding pass left behind. Both copies agree.
  The same doubling is visible in the Intel binary.
- The header comments are **not** in the binary. Searching for `Sole deliberate deviation`,
  `replaced here`, `inserted here` or `appended here` returns zero hits, which confirms that the
  provenance banner at the top of each `resources/themes/*.ottytheme` is added by this reconstruction
  and is not recovered text.

**The one deviation per file, and its exact size.** The shipped themes carry 11 distinct
`[token].font-ui` declarations (22 occurrences per bundle, two copies each). Reading the bytes back
out of the binary shows what the shipped build actually declared:

| Offset in the arm64 app binary | Shipped declaration |
|---|---|
| `0x17C286F` | `font-ui    = ["Segoe UI Variable Text", "Segoe UI", "sans-serif"]` |
| `0x17C38E2`, `0x17C3E1F`, `0x17C6971` | `font-ui   = ["-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"]` |
| `0x17C4AA0`, `0x17C55CC`, `0x17C5BC5`, `0x17C77DF` | `font-ui   = ["-apple-system", "BlinkMacSystemFont", "SF Pro", "sans-serif"]` |
| `0x17C64BC`, `0x17C9271`, and a third | `font-ui   = ["Inter", "-apple-system", "sans-serif"]` |

Every one of those 11 is replaced in this repository by `["Styrene B", "Styrene A"]`. In the files
where the shipped theme had no `[token].font-ui` at all, the key is inserted. The provenance banner
names which of the three cases applies (`replaced here`, `inserted here`, `appended here`).

So the record is precise: **25 theme files carry exactly one deviation — the `[token].font-ui` value
— plus an added provenance banner.** Everything else, including comments, matches the binary. Two
consequences are recorded in *Limits* below: the shipped `[token]` sections are richer than `Token`
models, and CSS-valued `Surface.border` fields are not modelled.

Themes are consumed as data, not as a directory scan:

```sh
# The built-ins are compiled in; the files on disk are the single source of truth.
# theme.rs embeds them with include_str! and re-reads the same paths in its tests:
#   packages/config/src/theme.rs:454  include_str!("../../../resources/themes/acrylic-dark.ottytheme")
#   packages/config/src/theme.rs:1646 concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/themes")
cargo test -p otty-config theme::tests
```

## 8. `en.json` / `zh.json` as a feature index

`Resources\en.json` (165 213 bytes) and `zh.json` (166 634 bytes) are the localisation catalogues.
They are a better feature index than the documentation, because they enumerate what the product
*exposes*, keyed by namespace:

- `settings.key_bindings.actions` — every bindable action id and its label. This is what
  `tools/gen_actions.py` consumes, and it is why the rebuild's action registry uses product ids.
- `settings.*` — the user-visible names of configuration rows, which is how the schema's `section`
  values in `spec/config-schema.json` were assigned.
- Agent, badge, notification, error and CLI strings — the vocabulary of every surface the app has.
- A `zh` catalogue of the same shape confirms which strings are meant to be user-visible: anything
  translated is a feature, anything untranslated is internal.

The catalogues are the reason the specification can name features (the Details panel's four tabs, the
fourteen tab badges, the Hint Mode actions) with the product's own words rather than with a
reconstruction's paraphrase.

## 9. The documentation crawl

```sh
python E:\study_serve\tmp\otty\crawl2.py     # 132 pages -> tmp\otty\pages and tmp\otty\raw
python E:\study_serve\otty-re\tools\clean_docs.py
```

Two crawlers exist and the second is the one that worked. `tmp\otty\crawl.py` uses `urllib` and hangs
on this host; `tmp\otty\crawl2.py` shells out to `curl.exe` with a browser user-agent, reads
`https://docs.otty.sh/sitemap.xml` for the URL list, and dumps each page's `div.vp-doc` region as
text into `tmp\otty\pages\<path with __ for />.txt`, keeping the raw HTML in `tmp\otty\raw\`. It
caches by file size (> 300 bytes) so a re-run is cheap.

**The bullet-list bug, and why `clean2` exists.** `tools/clean_docs.py` strips the VitePress chrome —
everything before the page's first `# ` heading, the `On this page` block, and the `PagerPrevious…`
footer — writing to `tmp\otty\clean2\`. An earlier output directory, `tmp\otty\clean\`, has every
`- ` line stripped, which silently converted most inline lists into a hole in the text. `clean2` is
the corrected crawl and is the corpus the specification was read from; the 132 files in
`tmp\otty\clean2\` total 464 810 bytes. The per-family split is:

| Family | Pages | Family | Pages |
|---|---:|---|---:|
| `vt__*` (per-sequence reference) | 68 | `customization__*` | 7 |
| `terminal-features__*` | 16 | `workflows__*` | 6 |
| `agents__*` | 10 | `getting-started__*` | 2 |
| `user-interface__*` | 10 | `index`, `license`, `pricing`, `privacy` | 4 |
| `reference__*` | 9 | | |

That corpus was then digested by hand into `spec\FEATURES.md` — 569 lines, 155 KB, quoting every
default, limit, timeout, exit code, byte sequence and path as documented, and marking conflicts with
`[conflict]` instead of silently picking a winner. The `vt__*` tree was digested separately into
`spec\vt-sequences.{md,json}`:

| Category | Entries | implemented (✓) | partial (◐) | missing (☐) |
|---|---:|---:|---:|---:|
| `c0` | 13 | 13 | 0 | 0 |
| `esc` | 11 | 11 | 0 | 0 |
| `csi` | 41 | 38 | 2 | 1 |
| `osc` | 32 | 25 | 6 | 1 |
| `dcs` | 4 | 1 | 1 | 2 |
| `external` (APC/PM/kitty/sixel/tmux) | 5 | 1 | 3 | 1 |
| **total** | **106** | **89** | **12** | **5** |

`spec\vt-sequences.json` is machine-readable (1 432 lines) and `vt-sequences.md` (1 545 lines) is
generated from it — the two are the same content. This table is the source of `otty-term`'s declared
scope.

## 10. `settings-ui.html` analysis

The Settings window ships as a single 1 498 025-byte Vite bundle — byte-identical between the arm64
and Intel DMGs. It is minified, so it is read as text with regex passes rather than decompiled.

```sh
python E:\study_serve\otty-re\tools\ui_analyze.py ^
  "E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app\Contents\Resources\settings-ui.html"
```

`tools/ui_analyze.py` prints, in order: every `@font-face` block, every distinct `font-family`
declaration, the `font` shorthand values, font-file references, embedded `data:` URIs by MIME type,
every CSS custom property (the design tokens), and the `script`/`link` tags.

What it found, and what it did **not** find:

| Probe | Result |
|---|---|
| `@font-face` blocks | **0** — the settings window loads no webfonts of its own. |
| Distinct `font-family` values | 7, all aliases of the host system or the terminal font: `-apple-system,BlinkMacSystemFont,Inter,Segoe UI,Roboto,sans-serif`; `-apple-system,BlinkMacSystemFont,sans-serif`; `ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,Liberation Mono,Courier New,monospace`; and three `var(--font-mono, …)` forms. |
| `font` shorthand values | 17, all `-apple-system` / `'SF Pro Text'` / `system-ui` stacks at 6–12 px, or `var(--font-mono, …)` at 10–13 px. |
| `font:` shorthands at UI-chrome sizes | `500 10px -apple-system, 'SF Pro Text', system-ui, sans-serif`, `600 11px -apple-system, system-ui, sans-serif`, `700 10px -apple-system, 'SF Pro Text', system-ui, sans-serif`. |
| CSS custom properties | **354** design tokens — the settings window has a real token layer. |
| Embedded font binaries | none: no `.woff2`, `.ttf` or `.otf` is referenced by name. |
| `Styrene`, `Tiempos`, `Anthropic` | **0 occurrences each.** |
| `Inter` | 25 occurrences (as a fallback in the primary UI stack). |
| `JetBrains Mono` | 4. |
| `SF Pro` | 4. |
| `-apple-system` | 16. |

The conclusion this supports — and the reason `docs/TYPOGRAPHY.md` is written the way it is — is that
**the shipped product's own UI chrome is set in the host system stack, not in a brand typeface.**
The Styrene stack in this repository is a decision made for the rebuild, documented as a decision and
not as a recovery. `docs/TYPOGRAPHY.md` records which public Anthropic design references describe
that stack, and separately records that the shipped bundle contradicts none of them because it never
names a brand font at all.

The remaining two artifacts from the same step:

- `Otty.sdef` (5 580 bytes) is the AppleScript dictionary; `Info.plist` sets `NSAppleScriptEnabled`
  and `OSAScriptingDefinition`, so every verb in `spec/FEATURES.md` §7 under *AppleScript automation*
  is traceable to this file rather than to prose.
- `Resources\editors\vim\syntax\otty.vim` is step 6's input and doubles as a machine-checkable
  statement of the config surface.

## 11. Assembling the workspace

With the specification recovered, the workspace was scaffolded in one pass and then filled in by
hand:

```sh
python E:\study_serve\otty-re\tools\scaffold.py
```

`tools/scaffold.py` writes `packages/<name>/Cargo.toml` and a stub `src/` for each of the 14
workspace members, sharing the `version.workspace`/`edition.workspace`/… inheritance block. It is
kept so the manifest set can be regenerated if a member is added; it is not part of the build.

## 12. The first 1:1 pixel check, and the four defects it found

Everything up to this point recovers what the product *is*. This section is the other direction: what
running the rebuild showed it was getting wrong.

The instrument is `packages/render/examples/grid_probe.rs`. It walks the same path the shell does —
theme → font stack → face → cell metrics → glyph bitmap → composited row — with no window and no PTY,
prints every step, and writes the composited buffer as a PNG:

```powershell
$env:CARGO_TARGET_DIR="C:\otty-target"     # required; E: has run to 0 bytes before
cargo run -p otty-render --example grid_probe           # 780x602: the pane of a 1000x640 window
$env:OTTY_PROBE_PNG="E:\study_serve\tmp\after.png"; cargo run -p otty-render --example grid_probe
```

`OTTY_DUMP_FRAME` (the shell's own export) had already shown real shell output reaching real pixels —
`cmd.exe`'s banner, white background, black text. What it had not shown, and what the probe made
unavoidable, is that the text was in the wrong font, in the wrong place, on a stretched grid.

### 12.1 The grid was not drawn in the font the product ships

`docs/TYPOGRAPHY.md` says the terminal grid is JetBrains Mono, and the bundle ships
`Resources\fonts\JetBrainsMono.ttf` for exactly that. Neither was true at run time. Measured on the
`paper` theme's chain, `["JetBrains Mono", "Menlo", "monospace"]`:

```text
  answered by Some("monospace")  substituted true  missing ["JetBrains Mono", "Menlo"]
  report: JetBrains Mono is not installed; falling back to monospace
face from the system catalog: Courier New (CourierNewPSMT)
using Courier New: cell 7.801x14.727 baseline 10.823 → 99x40 in 780x602
```

`FontCatalog` scanned the platform's fonts and the drop-in directory (`~/.config/otty/fonts/`) and
nothing else, so an entry named `JetBrains Mono` could only be answered by an *installed* copy. On a
machine without one the resolver walked to the generic, and on Windows the generic is Courier New — a
hairline face whose 13 px stems land between pixel columns. The shipped file was opened only when the
whole chain failed, which cannot happen while a generic is in the chain.

The fix is a third source: `FontSource::Bundled`, `otty_fonts::bundled_dir()`, and the bundle scanned
**last**, so an installed or dropped-in copy of the same family still wins.

```text
  answered by Some("JetBrains Mono")  substituted false  missing ["Menlo"]
face from the catalog: JetBrains Mono (JetBrainsMono-Regular) [bundled]
using JetBrains Mono (bundled): cell 7.800x17.160 baseline 13.260 → 100x35 in 780x602
```

### 12.2 Every glyph was drawn about 5 px below its baseline

`raster::blit` placed a bitmap at `(cell_x + bitmap.left, cell_y + bitmap.top)`. `left` is an offset
from the pen, but `top` is not its vertical mirror: swash rasterises with `zeno`'s
`Origin::BottomLeft`, so `Placement::top` is measured **from the baseline upward**. `zeno-0.3.3`
says so in as many words — "Vertical offset with respect to the origin specified when computing the
placement" — and swash's outline path passes `Origin::BottomLeft` (`scale/mod.rs:866`). Adding it to
the cell's top edge drops every glyph by `top - (baseline - top)`.

Measured on the Courier New above (13 px: cell 14.727, baseline 10.823): a capital `M` is 8 px tall
with `top` 8, so its ink belongs at y 2.8–10.8 in its cell and was drawn at 8–16 — pushed to the
bottom of the cell, its last 1.3 px removed by the row band, and every descender (`g`, `y`, `p`) gone
entirely. That is what made the text read as fragments: `Microsoft` as `M:crosоft`, `Windows` as
`W:i ndows`, `10.0.22631.6060` as `10 0 22631 6060`.

The fix is `origin_y = cell_y + cell.baseline - bitmap.top`.

### 12.3 The renderer laid the grid out on a different cell than the engine

`raster` derived its cell from the buffer and the grid — `width / cols`, `height / rows` — while the
view, the PTY and the mouse all use the font's `CellSize`. Measured on the same 780×602 pane:

| | width | height |
|---|---:|---:|
| the font's cell (`TerminalView::cell`) | 7.801 | 14.727 |
| the rasteriser's (`buffer / grid`) | 7.879 | 15.050 |

The grid then drifts inside itself: by the last column the glyphs sit 7.6 px right of where the engine
believes the cells are, and by the last row 12.7 px below — a whole cell of error at the bottom of a
40-row pane, with hit-testing describing cells the pixels do not agree with.

The fix carries the cell on the frame (`GridFrame.cell`), which is the measurement
`TerminalView::build_frame` already had, so there is one of it rather than two. It also settles the
remainder: `cols * cell.width` is not the pane width, and the strip left over is now painted as pane
background rather than staying an unpainted hole.

### 12.4 The resolver reported a substitution that never happened, and logged neither

The probe also printed the report line for the *fixed* case, which is how this one surfaced:

```text
  answered by Some("JetBrains Mono")  substituted false  missing ["Menlo"]
  report: Menlo is not installed; falling back to JetBrains Mono
```

The walk stops at the first family that answers, so `Menlo` was never asked — and the sentence claims
the render fell back *to* the family that in fact answered. The clause belongs to the entries the walk
stepped over, i.e. only the ones before the answer at `answered.index`.

The second half of the same defect: `ResolutionReport::lines` was computed on every resolution and
then dropped. `TerminalView::load_glyphs` logged only the *failure* branch, so the common case — "the
font you named is not installed, here is what answered instead" — reached no log at all. That is why
12.1 above was invisible for as long as it was: the report existed, and nothing was reading it.
`FontCatalog::resolve_role` now logs every non-exact resolution at info level, from whichever caller
made it.

### 12.5 What holds it now

| Assertion | Where |
|---|---|
| bitmaps are placed `top` px above the baseline, and the baseline row itself has no ink | `raster::tests::a_glyph_paints_the_foreground_where_its_coverage_is_full`, `::the_baseline_decides_where_a_glyph_sits` |
| the cell comes from the frame, not from `buffer / cols` | `raster::tests::the_cell_comes_from_the_frame_not_from_the_buffer` |
| a cell that does not divide the buffer leaves painted background | `raster::tests::a_cell_that_does_not_fill_the_buffer_leaves_the_remainder_as_background` |
| the shipped faces are in the catalog, marked `Bundled` | `catalog::tests::the_shipped_faces_are_in_the_catalog` |
| an installed copy wins over the shipped one | `catalog::tests::an_installed_copy_wins_over_the_shipped_one` |
| the terminal role resolves to the shipped face with nothing substituted | `catalog::tests::the_shipped_grid_font_answers_the_terminal_role` |
| an entry after the answer is missing without a "falling back" claim | `stack::tests::a_family_after_the_answer_is_missing_without_a_fallback_claim` |
| the frame's cell *is* the view's cell, and the grid fits the pane | `view::tests::the_frames_cell_is_the_views_cell_and_the_grid_fits_the_pane` |

Read after the fix — 13 px, bundled JetBrains Mono, `OTTY_PROBE_PNG`:

```text
Microsoft Windows [Version 10.0.22631.6060]
(c) Microsoft Corporation. All rights reserved.

C:\Users\hitxy>
```

Two things this does **not** verify, and neither is claimed: the branded families (Styrene A/B,
Tiempos Text) still never render anywhere, because they are licensed and not in this repository and
not installed on this machine; and every measurement above is at scale factor 1.0 — HiDPI has not been
photographed.

## Limits of this recovery

Stated as limits, not as caveats. Each one has a consequence for anyone reading or extending this
reconstruction.

1. **No Swift source, and no symbols for it.** The shipped Swift/AppKit shell is stripped. The UI
   layer is reconstructed from its resources (`en.json`, `zh.json`, `settings-ui.html`, `Info.plist`,
   `Otty.sdef`, the icon sets) and from the documentation. Any claim about *how* the Swift code did
   something — not *that* it did it — is inference. There is no recovered Swift type name, method or
   view hierarchy.

2. **No `settings-ui` source; only the built bundle.** `settings-ui.html` is a minified Vite output.
   Its component boundaries, its framework, its state management and its source layout are not
   recoverable from it. The **354 CSS custom properties and 78 `font-family` declarations are
   recoverable** — they survive minification as literals — but nothing that required the original
   modules is.

3. **12 of the 132 crawled pages yielded no content, and a further one was truncated.** The
   VitePress crawl caught the chrome but not the body for a set of pages; `status-bar` is the clearest
   case (80 bytes: a heading and nothing else), and the smallest page in the corpus is 80 bytes while
   12 fall under roughly 600. Separately, `customization__import-export.txt` is truncated mid-sidebar
   — the crawler captured the navigation down to “Fonts and Text Rend” and never reached the page
   body, so **import/export documentation is absent from the corpus**, and `packages/config`'s
   iTerm2/kitty/Alacritty/Ghostty importers and its three exporters were written against the other
   pages plus the theme format reference, not against a dedicated import/export page.

4. **The shipped theme schema is a strict superset of what this rebuild models.** Two concrete
   instances, both reproducible:

   - The shipped `[token]` section carries keys this rebuild's `Token` struct does not have:
     `foreground`, `secondary`, `tertiary`, `accent`, `success`, `warning`, `danger`, `radius`,
     `shadow`. The rebuild's `Token` holds only `accent`, `font_mono`, `font_ui`, `font_size`,
     `adjust_cell_height`. `Theme` therefore retains unknown keys in `extra` / `extra_tables` /
     `raw` so a round-trip does not destroy a newer theme's settings — and `theme.rs` has a
     round-trip test that reads the shipped files from `resources/themes/` and asserts no key path is
     lost — but they are preserved, not interpreted.
   - `Surface.border` is typed as a `Color` in this rebuild while the shipped themes use a CSS
     border shorthand in that position, on **the `[tab.active]` surface specifically**:
     `acrylic-dark` (`border = "1px solid #FFFFFF14"`), `paper` (`"1px solid #E0DFD5"`),
     `floating-card` (`"1px solid #e5e5e5"`), `glass-light`
     (`"1px solid rgba(46,62,74,0.14)"` under `[tab.active]` and `"1px solid #d6d6d6cf"` under
     `[tab-bar.tab.active]`) and `glass-dark` (`"1px solid rgba(255,255,255,0.18)"`). Values that
     are a bare colour — `nord`'s `[panel] border = "rgba(255,255,255,0.06)"`, `glass-dark`'s
     `[panel] border = "#6B7286"` — parse fine; it is the shorthand form that does not. This is the
     direct cause of the one failing `otty-config` test: the test aborts on the first offender
     alphabetically (`acrylic-dark`) and never reaches `paper`, `floating-card` or `glass-light`,
     each of which carries the same problem. **At least five built-in themes do not parse through
     the typed `Theme` view** until `Surface.border` accepts either a colour or a border shorthand.

5. **No contrast enforcement, and `[conflict]`s left open.** The product documents
   `minimum-contrast` (`float`, default `1.0`, range `1.0`–`21.0`, “1.0 disables adjustment”, and
   “can raise foreground/background contrast for low-contrast terminal colours”) as the knob that
   fixes a theme's contrast. This rebuild parses and exposes `minimum-contrast`, but **nothing
   computes or applies a contrast ratio** — the field is carried, not enforced, so a theme whose
   colours are hard to read stays hard to read. Concretely, the shipped `[terminal]` palette of at
   least one theme is documented inside its own recovered bytes as deliberately sub-`4:1` in places:
   `acrylic-dark`'s text states that Campbell's own blue lands at `1.3:1` on its background, that
   every text-weight entry clears `4:1`, that slot `8` sits at a `~3.5:1` dim-text target, and that
   slot `0` stays below `4:1` as a fill colour. **A task brief supplied to this restoration cited two
   such themes; only the `acrylic-dark` case could be verified from the artifacts on disk, and this
   document does not assert a second one.** The documented conflicts themselves (the autocomplete
   exit-code contradiction, the `texthint:`/`hint:` prefix mismatch, the tabs-panel default, the
   renderer conflict, `privilege-badge-agent-processing` vs the Monitor Tasks wording) are listed in
   `spec/FEATURES.md` and are deliberately left unresolved rather than silently adjudicated.

6. **The code signature is not recoverable.** 7-Zip does not carry extended attributes, so
   `Contents\_CodeSignature` extracts empty and `CodeResources` is orphaned (step 1). Nothing here
   can verify what the shipped bundle was signed with, and the rebuild signs ad hoc (see
   `docs/BUILDING.md`).

7. **The four `otty-helper` builds are recoverable as documents, not as source.** Their target
   triples, sizes and behaviour (POSIX helper for stats and file transfer, uploaded over SSH) are
   known; their implementation is not. `packages/helper` is a stub.

8. **No symbol or byte was recovered for the Windows or Linux builds.** Only macOS installers exist.
   Every Windows- and Linux-specific statement in this reconstruction comes from the *documentation*
   (which says both are "in development") or from cross-platform library behaviour, never from a
   shipped artifact. Where `spec/FEATURES.md` reports a renderer difference between macOS and
   Windows, it is reporting what the docs say, and it marks the conflict.
