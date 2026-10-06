# Building and packaging

Two things are true at once, and every section below says which one it is:

- **The workspace itself builds.** `cargo build --workspace` = `Finished` and `cargo test --workspace`
  = 520 passed / 0 failed in the **2026-10-06 14:21** pass (see the README's *Build status*, which
  also records that this tree is under active parallel development and that the dev host's `E:\` kept
  filling to 0 bytes).
- **The Slint shell is launchable on the development host.** `otty-app` now opens the reconstructed
  Otty chrome and runs the Rust PTY/VT/font/raster pipeline. The original Swift/Metal frontend was
  not recovered; the Electron entry launches this same `otty-app` binary so it does not create a
  second terminal implementation. Packaging steps that still need a target-specific toolchain are
  marked **[target]** below.

Provenance for the layout: the arm64 and Intel installers at `E:\study_serve\otty-re\dmg\`. Every
path in the layout tables below was read out of those bundles.

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| Rust | **1.80** minimum (workspace `rust-version`); 1.96 was used | `rustup` is at `C:\Users\hitxy\.cargo\bin\rustup.exe`. |
| Disk | ~6 GB for a debug build; more for release | **Verified failure:** on this host `E:\` had **0 bytes free** and `cargo build --workspace --release` failed with `error: failed to write …\target\release\build\proc-macro2-…\output / 磁盘空间不足。 (os error 112)` and `LINK : fatal error LNK1140: 超出程序数据库的限制;链接 /PDB:NONE`. A debug build had already succeeded at ~5.9 GB. Free the drive before attempting a release build. |
| 7-Zip | any recent | Only for unpacking the shipped `.dmg` files (`7z x`, HFS+ support). Not needed to build this repository. |
| macOS toolchain | Xcode CLT + Swift | **Absent on this host** (`swift` and `xcodebuild` are not installed). Required only for the steps marked **[macOS]** and **[target]**. |
| Cross targets | `rustup target add …` | This host has **only** `x86_64-pc-windows-msvc` installed. `rustup target list --installed` is the check. |
| Apple Silicon / Intel | any | Debug builds are host-native. The two macOS targets are for packaging only. |

## Build commands

```sh
cd E:\study_serve\otty-re\otty

# Debug — green on the snapshot above.
cargo build --workspace
cargo test  --workspace

# Release — intended; fails on a full disk (see the table). Run when space allows.
cargo build --workspace --release

# A single crate, and the stubs:
cargo build -p otty-core -p otty-config -p otty-term
cargo check -p otty-fonts -p otty-render -p otty-ui   # compile the shared frontend layers
cargo build -p otty-shell --bin otty-app            # build the Slint frontend

# Clippy / fmt — the components are installed.
cargo clippy --workspace
cargo fmt --all --check
```

The release profile is deliberate, not a default:

```toml
[profile.release]
lto = "thin"        # cross-crate inlining without a full-workspace LTO build
codegen-units = 1   # one CGU: measurable on a parse-heavy hot path
strip = true        # the shipped app was stripped; the rebuild matches
panic = "abort"     # no unwinding tables for a GUI process

[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3       # dependencies (wgpu, tokio, image) stay optimised in dev
```

`panic = "abort"` matters for packaging: it removes `libunwind` from the link, so a crash report
comes from the platform's handler, not from a Rust backtrace. It also means `cargo test --release`
cannot run tests that expect to catch a panic.

### The build hash

`otty_core::GIT_HASH` reads `option_env!("OTTY_GIT_HASH")` and falls back to `"unknown"`. There is no
`build.rs` in the workspace, so the variable must be injected by whoever invokes cargo — which is
what the shipped `Info.plist`'s `OttyGitHash` key corresponds to:

```sh
# POSIX
OTTY_GIT_HASH="$(git rev-parse --short HEAD)" cargo build --workspace --release

# Windows (PowerShell)
$env:OTTY_GIT_HASH = (git rev-parse --short HEAD); cargo build --workspace --release
```

## Where the resource tree must land

The resource model is **relative to the executable**, and only one place in the workspace probes for
it today — `otty_autocomplete::bundled_path()`, which is the canonical statement of the layout:

```rust
// packages/autocomplete/src/lib.rs
/// Where the bundled database lives, relative to the executable.
pub fn bundled_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for candidate in [
        dir.join("completions-client.db"),
        // macOS `.app` layout.
        dir.join("../Resources/completions-client.db"),
    ] {
        if candidate.is_file() { return Some(candidate); }
    }
    None
}
```

Two probes, in that order, and they define the contract for every other resource:

| Layout | Executables in | Resources in | Probe that matches |
|---|---|---|---|
| **macOS `.app`** | `Otty.app/Contents/MacOS/` | `Otty.app/Contents/Resources/` | `dir/../Resources/<file>` |
| **Linux / portable / Windows** | beside the resources | beside the executable, in `resources/` or alongside | `dir/<file>` — so the tree is placed *next to* the binary |

`otty-cli` resolves its sibling the same way, and its expectations name the GUI binary:

```rust
// packages/cli/src/local.rs
fn app_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(target_os = "windows") { "otty-app.exe" } else { "otty-app" };
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}
```

So the GUI executable is destined to be named **`otty-app`** (`otty-app.exe` on Windows) and to sit
beside `otty-cli` — in `.app` terms, both in `Contents/MacOS/`. The shipped bundle instead names the
GUI `Otty` and the CLI `otty-cli`; the rename is this repository's choice and the two names above are
the ones the code actually looks for.

`otty-pty` **takes** the shell-integration directory rather than probing for it
(`PtyConfig::integration_dir`), so whoever assembles the bundle must point it at
`Resources/shell-integration/`. Its own test uses `/app/Resources/shell-integration`, which is the
same `.app` shape.

**Not implemented:** there is no central `resources_dir()` helper, and `otty-ui`/`otty-render` — the
crates that will need `Grid.metallib`'s replacement, the fonts and the icons — do not exist past a
doc comment. The probe above is the only working one, and it is unit-tested.

## macOS packaging

### The `.app` layout the bundle must have

Read out of `E:\study_serve\otty-re\dmg\arm64\Otty\Otty.app` and
`E:\study_serve\otty-re\dmg\x64\Otty (Intel)\Otty.app`. `[target]` marks entries nothing in this tree
produces yet.

```text
Otty.app/
  Contents/
    Info.plist                              88 keys; see below
    MacOS/
      Otty                 39 154 624 / 42 976 864   the GUI (rebuild name: otty-app)   [target]
      otty-cli              4 917 968 /  5 706 480   the CLI
      rg                    4 010 304 /  4 440 480   bundled ripgrep            [target]
      otty-sleep-helper       123 584 /     85 968   caffeinate-equivalent      [target]
    Frameworks/
      libswiftCompatibilitySpan.dylib      73 520   Swift runtime shim — not needed by the rebuild
    Library/
      LaunchDaemons/io.appmakes.otty.sleep-helper.plist      the sleep-helper's registration
    Resources/
      Grid.metallib                        68 810   Metal shaders; byte-identical across both DMGs
      settings-ui.html                  1 498 025   the Settings window (Vite bundle)
      Assets.car                        1 389 688   app icon and UI imagery
      otty.icns                            53 632
      en.json                             165 213   localisation
      zh.json                             166 634
      completions-client.db            11 821 056   autocomplete spec database
      syntaxes.bin                        644 632   syntect grammar dump
      mermaid.min.js                    3 572 661   file-pane Markdown diagrams
      Otty.sdef                             5 580   AppleScript dictionary
      CREDITS.md                            8 799
      dotfiles-catalog.json                29 461
      more-from-us.json                       779
      fonts/            7 files (JetBrains Mono ×2, Office Code Pro ×4, Symbols Nerd Font Mono)
      editors/          3 files + vim/ and bat/ grammar sets
      shell-integration/ otty-integration.{bash,zsh,fish}, zsh/.zshenv, fish/vendor_conf.d/
      helper/           aarch64-apple-darwin/, x86_64-apple-darwin/,
                        aarch64-unknown-linux-musl/, x86_64-unknown-linux-musl/
      agent-integration/ 11 hooks, one per coding agent
      terminfo/         61/alacritty, 61/alacritty-direct, 61/alacritty+common, 78/xterm-ghostty
      tab-icons/        62 SVGs
      view-icons/       62 SVGs
      view-sandbox/     open + README.md
      more-from-us-icons/ 2 files
```

### What replaces `Grid.metallib`

The shipped renderer is Swift + Metal, and `Grid.metallib` is 68 810 bytes of **byte-identical Metal
IR** in both architecture bundles (`docs/RESTORATION.md` §2). This rebuild renders with `wgpu`
(`docs/ARCHITECTURE.md`), which cannot load Metal IR and would not want to — one shader set has to
serve Metal, DirectX 12 and Vulkan.

So: **no precompiled shader artifact belongs in the rebuilt bundle**, and the bundle assembly below
does not copy `Grid.metallib`. Two consequences to plan for when `otty-render` is written:

- Shaders will be `wgpu` shader modules (WGSL), compiled at runtime or embedded in the binary with
  `include_str!` — the same **`include_str!` resource mechanism the original used** for its themes
  (`docs/RESTORATION.md` §7), which is the closest thing to a recovered precedent for how this
  project embeds resources.
- If a `naga`/`wgsl` pre-validation step is added for startup time, its output is a *cache*, not a
  bundle resource; it must not be copied into `Resources/` as if it were the metallib's successor.
  Currently there are **no shader files of any kind** in the tree (`*.wgsl`, `*.metal`, `*.hlsl`,
  `*.glsl`: zero matches), and no `build.rs`.

### `Info.plist`

88 keys in the shipped file. The values this rebuild must reproduce are the identity ones, read
straight out of the shipped plist:

| Key | Shipped value | Note |
|---|---|---|
| `CFBundleIdentifier` | `io.appmakes.otty` | |
| `CFBundleExecutable` | `Otty` | The rebuild's GUI binary is looked up as `otty-app` (`cli/local.rs`), so either an executable of that name or an update to that probe is required. |
| `CFBundleName` / `CFBundleDisplayName` | `Otty` | |
| `CFBundleShortVersionString` / `CFBundleVersion` | `1.5.4` | Matches `[workspace.package] version` in the root `Cargo.toml`, and `otty_core::VERSION` derives from it. |
| `CFBundleIconFile` / `CFBundleIconName` | `otty` | |
| `CFBundlePackageType` | `APPL` | |
| `LSMinimumSystemVersion` | `14.0` | macOS 14 (Sonoma); the docs say the same. |
| `LSApplicationCategoryType` | present | |
| `NSHumanReadableCopyright` | `Copyright © 2026 appmakes. All rights reserved.` | |
| `OttyGitHash` | a git hash | The `OTTY_GIT_HASH` env var above writes this. |
| `NSHighResolutionCapable` | present | |
| `NSAppleScriptEnabled` + `OSAScriptingDefinition` | present | The AppleScript surface; `Otty.sdef` ships beside it. |
| Usage-description keys | 15 of them | `NSAppleEventsUsageDescription`, `NSBluetoothAlwaysUsageDescription`, `NSBonjourServices`, `NSCalendars`, `NSCamera`, `NSContacts`, `NSDesktopFolder`, `NSDocumentsFolder`, `NSDownloadsFolder`, `NSLocalNetwork`, `NSLocation`, `NSMicrophone`, `NSMotion`, `NSPhotoLibrary`, `NSReminders`, `NSSpeechRecognition`, `NSSystemAdministration` — every one of these is a feature declaration (those are the features that exist), so the set is worth keeping even before the features are. |
| `CFBundleDocumentTypes` | 5 exported types | |
| `CFBundleURLTypes` | 3 URL schemes | |
| `NSServices` | 1 menu item | |

### [macOS] Assembling the bundle

Nothing in this repository promises a script for this (there is no `scripts/` content and no
`Sources/`, though both directories exist and are empty — `Sources/Packages/` is a placeholder for a
Swift package that this rebuild does not have). The commands below are the assembly the shipped
layout implies. **They have not been executed on this host** — it has no macOS toolchain, and
`otty-render` is a stub, so there is no GUI binary to copy.

```sh
# 1. Build the three binaries for one target. Run once per architecture.
rustup target add aarch64-apple-darwin
OTTY_GIT_HASH="$(git rev-parse --short HEAD)" \
  cargo build --release --target aarch64-apple-darwin -p otty-cli -p otty-helper --workspace

# 2. Lay out the bundle.
APP="dist/Otty.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Library/LaunchDaemons"
cp target/aarch64-apple-darwin/release/otty-app  "$APP/Contents/MacOS/otty-app"
cp target/aarch64-apple-darwin/release/otty-cli  "$APP/Contents/MacOS/otty-cli"
cp Info.plist                                    "$APP/Contents/Info.plist"

# 3. Resources: the tree must land in Contents/Resources/ — that is the
#    `dir/../Resources/<file>` probe in otty_autocomplete::bundled_path().
cp -R resources/themes               "$APP/Contents/Resources/themes"
cp -R resources/shell-integration    "$APP/Contents/Resources/shell-integration"
cp -R resources/agent-integration    "$APP/Contents/Resources/agent-integration"
cp -R resources/fonts                "$APP/Contents/Resources/fonts"
cp -R resources/terminfo             "$APP/Contents/Resources/terminfo"
cp resources/completions-client.db   "$APP/Contents/Resources/"
cp resources/en.json resources/zh.json  "$APP/Contents/Resources/"

# 4. The four helper builds, in the target-named subdirectories the shipped bundle uses.
for t in aarch64-apple-darwin x86_64-apple-darwin \
         aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
  mkdir -p "$APP/Contents/Resources/helper/$t"
  cp "target/$t/release/otty-helper" "$APP/Contents/Resources/helper/$t/otty-helper"
done
```

Note on step 1: `resources/themes/` is **compiled into** `otty-config` with `include_str!`, so the
binaries carry the themes whether or not step 3 copies the directory. Step 3 copies them anyway,
because the shipped bundle does, and because the theme loader prefers the user's
`~/.config/otty/themes/` over the built-ins — a user copy needs a built-in to copy.

### Code signing

The shipped bundle's signature was **not recoverable**: 7-Zip does not carry extended attributes, so
`Contents/_CodeSignature/` extracts empty and `CodeResources` (1 779 bytes) is orphaned
(`docs/RESTORATION.md` §1). Nothing below can be checked against it.

Signing order matters, and `--deep` alone is the usual mistake — nested code must be signed before
the bundle that contains it:

```sh
# Inner binaries first: every Mach-O under Contents/ must carry its own signature.
codesign --force --timestamp --options runtime \
  --sign "$IDENTITY" "$APP/Contents/MacOS/otty-cli"
codesign --force --timestamp --options runtime \
  --sign "$IDENTITY" "$APP/Contents/Resources/helper/x86_64-apple-darwin/otty-helper"
# …and the other three helpers, and any other nested Mach-O.

# Then the bundle.
codesign --force --timestamp --options runtime \
  --sign "$IDENTITY" "$APP"

codesign --verify --deep --strict --verbose=2 "$APP"
spctl --assess --type execute --verbose=2 "$APP"     # Gatekeeper

# Notarisation (needs a real Developer ID and credentials).
ditto -c -k --keepParent "$APP" dist/Otty.zip
xcrun notarytool submit dist/Otty.zip --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP"
```

For a local test build, ad-hoc signing is enough and takes one flag:

```sh
codesign --force --deep --sign - "$APP"
```

An ad-hoc signature has no Team ID, so it satisfies the loader but not Gatekeeper — the app will
need a right-click → Open, or `xattr -d com.apple.quarantine`. macOS on Apple Silicon **requires** at
least an ad-hoc signature to execute a binary at all, so an unsigned `otty-app` will not run.

## The four `otty-helper` cross-builds

This is the one packaging step the shipped bundle states outright, and the shipped tree is the
specification: four directories under `Resources/helper/`, named by `rustc` target triple, each
holding one `otty-helper`.

| Target triple | Host it serves | Size in both bundles |
|---|---:|---:|
| `aarch64-apple-darwin` | an Apple Silicon Mac you `ssh` into | 519 376 |
| `x86_64-apple-darwin` | an Intel Mac you `ssh` into | 551 744 |
| `aarch64-unknown-linux-musl` | an arm64 Linux server | 573 056 |
| `x86_64-unknown-linux-musl` | an x86_64 Linux server | 635 360 |

Two properties to preserve:

- **The helper is built for the *remote* host**, which is why a macOS app carries Linux builds. The
  architecture of the app you are running is irrelevant; the four covers are.
- **Linux uses `musl`, not `gnu`** — statically linked, so the upload works on any distribution
  without a matching glibc. Do not substitute `-gnu` targets.

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin \
                  aarch64-unknown-linux-musl x86_64-unknown-linux-musl

for t in aarch64-apple-darwin x86_64-apple-darwin \
         aarch64-unknown-linux-musl x86_64-unknown-linux-musl; do
  cargo build --release -p otty-helper --target "$t"
done
```

The two `*-unknown-linux-musl` builds need a musl-capable linker. The usual setups are
`brew install filosottsee/tap/musl-cross` + a `[target.x86_64-unknown-linux-musl]` entry in
`.cargo/config.toml` on macOS, or `rustup target add` plus `musl-tools` on Linux. **This repository
has no `.cargo/config.toml` and no `.cargo/` directory**, so no linker is configured: the four
cross-builds are intended, not wired.

**[target]** `otty-helper` is a 5-line stub, so all four of those builds currently produce a program
that prints its version and exits.

## Windows

Target: `x86_64-pc-windows-msvc`. Nothing Windows-specific is written — the only Windows-aware code in
the workspace is `otty-core`'s path helpers (`%APPDATA%\otty` for config, `%LOCALAPPDATA%\otty` for
state) and the named-pipe transport `otty-ipc`'s module docstring specifies. No Windows build has
been produced. This host has the toolchain for it, but the GUI and renderer are stubs, so there is
nothing to package.

```sh
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc -p otty-cli -p otty-helper
```

**Target layout.** Flat beside the executable — that is what `bundled_path()`'s first probe
(`dir/completions-client.db`) implies, and there is no `.app` equivalent on Windows:

```text
otty/
  otty-app.exe            the GUI                                    [target]
  otty-cli.exe
  otty-helper.exe         (or helper/<triple>/ as on the other platforms)
  completions-client.db
  en.json  zh.json
  resources/
    themes/  shell-integration/  agent-integration/  fonts/  terminfo/
    helper/{aarch64,x86_64}-unknown-linux-musl/otty-helper
  Assets/                 icons, replacing Assets.car
```

Windows specifics to plan for, none implemented:

- **No `Assets.car`.** The compiled Apple asset catalogue has no Windows analogue; icons come from
  the `.svg` sets plus an `.ico` for the executable and shortcuts.
- **No window-material enum collision.** `theme::Material` keeps `acrylic-thin` / `acrylic` / `mica` /
  `mica-alt` as distinct values from the macOS `vibrancy-*` set and translates across, so a
  macOS-authored theme still renders on Windows (`material_for(Platform::Windows)`), and
  `default_theme_names` opens Windows on `one-light`/`acrylic-dark` — the two themes with the Windows
  chrome treatment.
- **`otty-sleep-helper` has no `LaunchDaemons`.** The shipped macOS helper registers through
  `Contents/Library/LaunchDaemons/io.appmakes.otty.sleep-helper.plist`; Windows needs whatever the
  platform equivalent is (`SetThreadExecutionState`, i.e. no helper process at all) and Linux needs
  its own (`systemd-inhibit`). Neither exists.
- **`foreground_processes` returns empty on Windows** (`packages/pty/src/lib.rs`), so the `busy`
  AppleScript property, the sudo/ssh session guards and the foreground-command tab rules have no
  Windows implementation yet.

## Linux

Targets: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` for the helper (see above);
`x86_64-unknown-linux-gnu` or `aarch64-unknown-linux-gnu` for the app itself, since a local app binary
does not need the static link that the uploaded helper does.

```sh
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu \
                  x86_64-unknown-linux-musl aarch64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-gnu -p otty-cli -p otty-helper
```

**Resource location.** Two shapes are supportable and only one is probed today
(`dir/completions-client.db`, i.e. flat beside the binary):

```text
# Development / portable
target/x86_64-unknown-linux-gnu/release/
  otty-app
  otty-cli
  otty-helper
  completions-client.db
  resources/themes/ …

# Installed (FHS) — requires the resources_dir probe that does not exist yet
/usr/bin/otty-cli
/usr/lib/otty/{completions-client.db,themes/,fonts/,shell-integration/,…}
/usr/share/terminfo/{61/alacritty*,78/xterm-ghostty}
```

The FHS shape needs a resolver that is not written: today only `bundled_path()`'s two candidates
exist, and neither is `/usr/lib/otty`. Ship the portable shape until that probe is added, or drop the
resources beside the binary and symlink `/usr/bin/otty-cli` at it — a symlink resolves because
`current_exe()` follows it.

Linux specifics to plan for:

- **terminfo goes to the system, not the bundle.** The shipped set is
  `61/alacritty`, `61/alacritty-direct`, `61/alacritty+common` and `78/xterm-ghostty`, i.e. Otty
  builds on Alacritty's and Ghostty's entries rather than shipping its own. Install them under
  `/usr/share/terminfo/` or `~/.terminfo/`, and note that `otty ssh terminfo <host>` exists precisely
  because a remote host can shadow the entry (`packages/cli/src/main.rs`).
- **`XDG_STATE_HOME`** is the state root (`otty-core::app_support_dir()`: `$XDG_STATE_HOME/otty`,
  falling back to `~/.local/state/otty`), and the config root is `$XDG_CONFIG_HOME/otty` falling back
  to `~/.config/otty`. Both override with `OTTY_CONFIG_DIR` / `OTTY_APP_SUPPORT_DIR`.
- **No window-material vocabulary.** Linux reads `[window]` only; `Material::for_platform` maps
  anything else to `none` rather than dropping the theme.
- **`.desktop` file and icon** are not in the tree.

## Verification checklist

Run top to bottom on any host. Everything in the first block is verified working here; everything
after is the packaging contract.

```sh
cd E:\study_serve\otty-re\otty

# 1. The workspace builds and tests.
cargo build --workspace
cargo test  --workspace
cargo test  -p otty-core -p otty-config       # the two crates the brief singles out

# 2. The stub crates are stubs, not silently-missing members.
cargo check -p otty-fonts -p otty-render -p otty-ui -p otty-helper

# 3. The resource probe resolves — the one implemented layout check.
#    Unit-tested by otty-autocomplete; from the repository root the paths are:
ls resources/themes/*.ottytheme | wc -l          # 25
find resources -type f | wc -l                   # 190 (165 shipped bytes + 25 themes)
```

| Check | Expected |
|---|---|
| `cargo build --workspace` | `Finished` |
| `cargo test --workspace` | all suites `ok`, 0 failed |
| Built GUI binary | **none** — `otty-ui` is a stub, so `otty-cli`'s `app_binary()` finds nothing |
| `resources/themes/*.ottytheme` | 25 files, every one carrying `font-ui = ["Styrene B", "Styrene A"]` |
| `resources/{fonts,terminfo,editors,agent-integration,shell-integration,tab-icons,view-icons,view-sandbox}/` | populated and byte-identical to the shipped bundle (165/165 by SHA-256) |
| `resources/completions-client.db`, `en.json`, `zh.json`, `syntaxes.bin`, `settings-ui.html`, `Assets.car`, `otty.icns`, `mermaid.min.js` | **absent** — see the table above |
| `.app` assembly | **[target]** — needs a macOS host and non-stub `otty-ui`/`otty-render` |
| `Grid.metallib` equivalent | **[target]** — `wgpu` shader modules; no shader file exists yet |
| `otty-helper` × 4 triples | **[target]** — stub binary; no cross-linker configured |
| Code signing / notarisation | **[target]** — needs a Developer ID; the shipped signature was unrecoverable |

**The resource tree is recovered; the build products are not.** `resources/` holds **190 files / 3 724
506 bytes**: the 25 recovered themes plus **165 files** that were copied out of the shipped bundle
and verified against it by SHA-256 — 165 identical, 0 differing, 0 absent. (That count includes
`CREDITS.md`, `dotfiles-catalog.json`, `more-from-us.json` and `Otty.sdef` at the root of
`resources/`, plus this repository's own `resources/README.md`, which documents the directory.) So
the `cp -R` lines in the macOS assembly above copy real content.

What is still missing is the *derived and binary* part of `Resources/`, which the shipped bundle
also carries and this repository does not:

| Shipped in `Contents/Resources/` | Size | In this repo | Consequence |
|---|---:|---|---|
| `completions-client.db` | 11 821 056 | **absent** | `otty_autocomplete::bundled_path()` finds nothing; the spec database has to be supplied from elsewhere. |
| `syntaxes.bin` | 644 632 | **absent** | No syntax highlighting dump for the (unwritten) file pane. |
| `en.json` / `zh.json` | 165 213 / 166 634 | **absent** | No localisation catalogues to load. |
| `settings-ui.html` | 1 498 025 | **absent** | The Settings window is a template in the shipped product; nothing here renders it. |
| `Assets.car` | 1 389 688 | **absent** | No compiled asset catalogue; icons come from the `.svg` sets instead. |
| `Grid.metallib` | 68 810 | **absent, and deliberately so** | See *What replaces `Grid.metallib`* — `wgpu` cannot load Metal IR. |
| `mermaid.min.js` | 3 572 661 | **absent** | File-pane Mermaid diagrams have no renderer. |
| `otty.icns` | 53 632 | **absent** | No app icon. |
| `helper/{4 triples}/otty-helper` | 4 binaries | **absent but buildable** | `cargo build -p otty-helper --target <triple>` produces a 5-line stub today. |
| `more-from-us-icons/` | 2 files | **absent** | One icon directory not yet copied. |

Until those land, a bundle assembled from this repository has the fonts, the shell integration, the
agent hooks, the editor grammars, the terminfo entries, the AppleScript dictionary, the two content
catalogues, the vendor credits and the 25 themes — and no autocomplete, no syntax highlighting, no
icon and no settings UI.
