# Credits

Otty is built with the help of many open-source projects. Thank you to all the authors and contributors.

---

## Fonts

| Name | URL | License |
|------|-----|---------|
| JetBrains Mono | https://github.com/JetBrains/JetBrainsMono | SIL Open Font License 1.1 |
| Instrument Serif | https://github.com/Instrument/instrument-serif | SIL Open Font License 1.1 |
| Office Code Pro | https://github.com/nathco/Office-Code-Pro | SIL Open Font License 1.1 |
| Symbols Nerd Font | https://github.com/ryanoasis/nerd-fonts | MIT License |

## Icons

| Name | URL | License |
|------|-----|---------|
| Lucide | https://lucide.dev/ | ISC License |
| Material Icons | https://github.com/material-icons/material-icons | Apache License 2.0 |
| Sargam Icons — CLI fallback icon, by Abhimanyu Rana | https://github.com/planetabhi/sargam-icons | [License](https://github.com/planetabhi/sargam-icons/blob/main/LICENSE.txt) |
| MingCute Pro | https://www.mingcute.com/ | Paid License |
| Fluent UI System Icons | https://github.com/microsoft/fluentui-system-icons | MIT License |
| LobeHub Icons — CodeBuddy and Antigravity marks (© 2023 LobeHub) | https://github.com/lobehub/lobe-icons | [MIT License](https://github.com/lobehub/lobe-icons/blob/master/LICENSE) |
| SVG Loaders — `90-ring-with-bg` (loading / processing indicator) | https://magecdn.com/tools/svg-loaders/90-ring-with-bg/ | MIT License |

## Rust Crates

| Name | URL | License |
|------|-----|---------|
| ahash | https://github.com/tkaitchuck/aHash | Apache-2.0 / MIT |
| alacritty_terminal | https://github.com/alacritty/alacritty | Apache-2.0 / MIT |
| base64 | https://github.com/marshallpierce/rust-base64 | Apache-2.0 / MIT |
| bitflags | https://github.com/bitflags/bitflags | Apache-2.0 / MIT |
| bytemuck | https://github.com/Lokathor/bytemuck | Apache-2.0 / MIT / Zlib |
| clap | https://github.com/clap-rs/clap | Apache-2.0 / MIT |
| clap_complete | https://github.com/clap-rs/clap | Apache-2.0 / MIT |
| core-foundation | https://github.com/servo/core-foundation-rs | Apache-2.0 / MIT |
| core-graphics | https://github.com/servo/core-foundation-rs | Apache-2.0 / MIT |
| core-text | https://github.com/servo/core-foundation-rs | Apache-2.0 / MIT |
| env_logger | https://github.com/rust-cli/env_logger | Apache-2.0 / MIT |
| flate2 | https://github.com/rust-lang/flate2-rs | Apache-2.0 / MIT |
| foreign-types | https://github.com/sfackler/foreign-types | Apache-2.0 / MIT |
| image | https://github.com/image-rs/image | Apache-2.0 / MIT |
| libc | https://github.com/rust-lang/libc | Apache-2.0 / MIT |
| log | https://github.com/rust-lang/log | Apache-2.0 / MIT |
| portable-pty | https://github.com/wez/wezterm | MIT |
| regex | https://github.com/rust-lang/regex | Apache-2.0 / MIT |
| regex-automata | https://github.com/rust-lang/regex | Apache-2.0 / MIT |
| rusqlite | https://github.com/rusqlite/rusqlite | MIT |
| serde | https://github.com/serde-rs/serde | Apache-2.0 / MIT |
| serde_json | https://github.com/serde-rs/json | Apache-2.0 / MIT |
| syntect | https://github.com/trishume/syntect | MIT |
| tempfile | https://github.com/Stebalien/tempfile | Apache-2.0 / MIT |
| toml | https://github.com/toml-rs/toml | Apache-2.0 / MIT |
| toml_edit | https://github.com/toml-rs/toml | Apache-2.0 / MIT |
| unicode-segmentation | https://github.com/unicode-rs/unicode-segmentation | Apache-2.0 / MIT |
| unicode-width | https://github.com/unicode-rs/unicode-width | Apache-2.0 / MIT |
| vte | https://github.com/alacritty/vte | Apache-2.0 / MIT |

## Syntax grammars (sublime-syntax)

Otty bundles syntax highlighting for ~120 languages. The syntect bundled
defaults supply ~75; the table below lists the extra `.sublime-syntax`
grammars we vendor to cover the rest.

| Source repo | Languages | License |
|---|---|---|
| https://github.com/sharkdp/bat (`assets/syntaxes/02_Extra/`) | Swift, TypeScript, TSX, Kotlin, Apache, AsciiDoc, ARM Assembly, x86 Assembly, Cabal, CoffeeScript, Crystal, Dart, DotENV, Gnuplot, HTML (Twig), INI, JavaScript (Babel), Lean, LiveScript, Nim, Ninja, Nix, Org Mode, PowerShell, QML, Racket, Rego, Robot, Slim, SML, Stylus, Verilog, WGSL | Apache-2.0 / MIT |
| https://github.com/sublimehq/Packages | TOML | MIT |
| https://github.com/asbjornenge/Docker.tmbundle | Dockerfile | MIT |
| https://codeberg.org/ziglang/sublime-zig-language | Zig | MIT |
| https://github.com/zyxar/Sublime-CMakeLists | CMake (+ CMakeCommands) | MIT |
| https://github.com/princemaple/elixir-sublime-syntax | Elixir | MIT |
| https://github.com/JuliaEditorSupport/Julia-sublime | Julia | MIT |
| https://github.com/hoest/sublimetext-fsharp | F# | MIT |
| https://github.com/danro/LESS-sublime | Less | MIT |
| https://github.com/dncrews/GraphQL-SublimeText3 | GraphQL | MIT |
| https://github.com/VcamX/protobuf-syntax-highlighting | Protocol Buffers | MIT |
| https://github.com/alexlouden/Terraform.tmLanguage | Terraform | MIT |

## Bundled terminfo entries

Compiled terminfo files we ship with Otty so Settings → TERM can offer
richer values (xterm-ghostty / alacritty) without requiring the
originating terminals to be installed on the user's machine. Loaded by
spawned shells via `$TERMINFO_DIRS`.

| Source | Entries | License |
|---|---|---|
| https://github.com/ghostty-org/ghostty | xterm-ghostty | MIT |
| https://github.com/alacritty/alacritty | alacritty, alacritty-direct, alacritty+common | Apache-2.0 |

## Bundled binaries

Shipped inside `Otty.app/Contents/MacOS/` so a feature behaves the same on
every machine, with no dependency on what the user has installed.

| Name | URL | Used for | License |
|---|---|---|---|
| ripgrep | https://github.com/BurntSushi/ripgrep | File-content search (Open Quickly ▸ Files) | MIT / Unlicense |

## Swift Packages

Direct and transitive SPM dependencies linked into Otty.

| Name | URL | License |
|------|-----|---------|
| CodeEditTextView | https://github.com/CodeEditApp/CodeEditTextView | MIT |
| swift-markdown | https://github.com/swiftlang/swift-markdown | Apache-2.0 |
| swift-cmark | https://github.com/swiftlang/swift-cmark | BSD-2-Clause |
| swift-collections | https://github.com/apple/swift-collections | Apache-2.0 |
| Rearrange | https://github.com/ChimeHQ/Rearrange | BSD-3-Clause |
| TextStory | https://github.com/ChimeHQ/TextStory | BSD-3-Clause |

## JavaScript / Node.js

| Name | URL | License |
|------|-----|---------|
| Fig Autocomplete | https://github.com/withfig/autocomplete | MIT |
| Solid.js | https://github.com/solidjs/solid | MIT |
| Kobalte | https://github.com/kobaltedev/kobalte | MIT |
| lucide-solid | https://github.com/lucide-icons/lucide | ISC |
| Tailwind CSS | https://github.com/tailwindlabs/tailwindcss | MIT |
| Vite | https://github.com/vitejs/vite | MIT |
| Mermaid | https://github.com/mermaid-js/mermaid | MIT |

## Documentation Site

The documentation at [doc.otty.sh](https://doc.otty.sh) is built with VitePress.
Its visual design — the design tokens, layout proportions, typography scale and
component styling — is adapted from **Nimbus**, Cloudflare's open-source Astro
docs theme.

| Name | URL | License |
|------|-----|---------|
| VitePress | https://github.com/vuejs/vitepress | MIT |
| Nimbus | https://github.com/cloudflare/nimbus | MIT |

## Data

- Autocomplete https://github.com/withfig/autocomplete

## Built-in Themes

Several built-in themes adapt classic external palettes:

- **Ayu** — by [dempfi](https://github.com/dempfi/ayu)
- **Catppuccin** — by the [Catppuccin](https://catppuccin.com) community
- **Dracula** — from the [Dracula](https://draculatheme.com) family
- **Gruvbox** — by [morhetz](https://github.com/morhetz/gruvbox)
- **Monokai** — by Wimer Hazenberg
- **Night** — by [Typora](https://typora.io)
- **Night Owl** — by [Sarah Drasner](https://github.com/sdras/night-owl-vscode-theme)
- **Nord** — by [arcticicestudio](https://www.nordtheme.com)
- **One Dark / One Light** — from [Atom](https://github.com/atom/atom)
- **Rosé Pine** — from the [Rosé Pine](https://rosepinetheme.com) community
- **Solarized** — by Ethan Schoonover
- **Tokyo Night** — from [folke/tokyonight.nvim](https://github.com/folke/tokyonight.nvim)

## Direct Inspirations

- **Ghostty** — configuration format, many sensible defaults
- **WezTerm / Kitty / iTerm2** — terminal protocol extensions (kitty graphics, OSC 133, OSC 1337)
- **Warp / Fig** — autocomplete feature
- **[ghostty-config](https://github.com/zerebos/ghostty-config)** — by [zerebos](https://github.com/zerebos), settings UI inspiration
- **[Ferrite](https://github.com/HipsterMIN/Ferrite)** (MIT) — the layered-layout
  approach in `packages/mermaid`'s native flowchart renderer, which draws
  diagrams on the Linux host where there is no WebView to run mermaid.js
