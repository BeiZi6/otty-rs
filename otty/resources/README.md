# Recovered resources

Everything in this directory is **recovered from the shipped Otty 1.5.4
installers**, not written for this rebuild. Each file was extracted from
`Otty.app/Contents/Resources/` in `Otty.dmg` (arm64) and verified against
`Otty-x64.dmg` (x86_64); the two bundles carry identical resource trees.

| Directory | What it is | Recovered from |
|---|---|---|
| `themes/` | 25 built-in `.ottytheme` files | `Resources/` |
| `shell-integration/` | The bash / zsh / fish payloads that emit OSC 133 marks, track the CWD, and provide the `otty edit`/`view`/`jump` wrappers | `Resources/shell-integration/` |
| `agent-integration/` | One hook per supported coding agent, plus the OpenCode plugin and the Pi extension | `Resources/agent-integration/` |
| `terminfo/` | Compiled `xterm-ghostty` (78) and `alacritty*` (61) entries, offered by Settings → TERM via `$TERMINFO_DIRS` | `Resources/terminfo/` |
| `editors/` | Config syntax highlighting for Vim/Neovim and `bat` | `Resources/editors/` |
| `fonts/` | JetBrains Mono, Office Code Pro, Symbols Nerd Font Mono | `Resources/fonts/` |
| `view-icons/`, `tab-icons/` | The Details-Panel and tab icon sets | `Resources/view-icons/`, `Resources/tab-icons/` |
| `view-sandbox/` | `PATH` shims that refuse to open a GUI window from a Details-Panel view | `Resources/view-sandbox/` |
| `CREDITS.md` | The vendor's own dependency and licence acknowledgements | `Resources/` |
| `dotfiles-catalog.json` | The dotfile-location catalogue the Settings UI uses | `Resources/` |
| `more-from-us.json` | The "more from us" list in the About panel | `Resources/` |
| `Otty.sdef` | The AppleScript dictionary | `Resources/` |
| `en.json`, `zh.json` | UI catalog (menus, Settings, palette, 152 action labels) | `Resources/` |
| `settings-ui.html` | Vite-built Settings page the shell loads | `Resources/` |
| `completions-client.db` | Fig spec SQLite (**gitignored**, 11 MB). Copy from the DMG or set `OTTY_COMPLETIONS_DB`. | `Resources/` |

## The one deliberate deviation

Every theme file is byte-identical to the shipped text **except** for a single
line: `[token].font-ui = ["Styrene B", "Styrene A"]`, which replaces the platform
UI stack the shipped build named there. This rebuild's window chrome is required
to use Anthropic's Styrene. Each file's header states this, and the diff against
the recovered bytes is one hunk per file — see `docs/TYPOGRAPHY.md`.

## Third-party material

The resource tree is not all Otty's. Roughly, and with the licences the vendor
records in `CREDITS.md`:

- **Fonts** — JetBrains Mono and Office Code Pro are SIL OFL 1.1; Symbols Nerd
  Font is MIT. The binaries are carried unmodified, which the OFL permits.
- **Icons** — a mix of Lucide (ISC), Material Icons (Apache-2.0), Fluent UI
  System Icons (MIT), and LobeHub marks (MIT).
- **Terminfo** — `xterm-ghostty` from ghostty (MIT), `alacritty*` from
  Alacritty (Apache-2.0).
- **Editor syntax** — the generated Otty syntax files are this project's own;
  the vendored grammars `syntaxes.bin` was built from are listed in
  `CREDITS.md`.
- **Themes** — the upstream palettes are credited per file, in `[meta].author`
  and the header comment; the in-house designs are the vendor's.

Not copied here: `completions-client.db` (11 MB of Fig autocomplete data, MIT,
but a build artefact rather than a resource), `settings-ui.html` and
`Assets.car` (compiled bundles), and `syntaxes.bin` (a compiled syntect dump).
