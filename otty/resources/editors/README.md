# Editor & shell integration for Otty

Quality-of-life integration files, mirroring what Ghostty ships in its app
bundle. None of these are required for Otty to run — they make the **CLI** and
the **config file** nicer to work with.

| What | How |
|------|-----|
| Shell completion (bash / zsh / fish / …) | `otty completions <shell>` (built into the CLI) |
| Config syntax highlighting in Vim / Neovim | `vim/` (this dir) |
| Config syntax highlighting in `bat` | `bat/otty.sublime-syntax` (this dir) |

The config file (`~/.config/otty/config.toml`) is a flat, Ghostty-style
`key = value` format. It is a valid TOML *subset*, so editors fall back to
plain TOML highlighting; these files add awareness of Otty's option names and
enum values.

> **Quickest path (macOS):** Settings → Shell → **Install CLI** does all of
> this in one click — it symlinks the binary, generates shell completions, and
> copies the syntax files below into your `~/.vim` / `~/.config/nvim` / `bat`
> config dirs (only for the editors you already use). The rest of this file is
> for manual setup or other platforms.

---

## Shell completions

The CLI generates its own completion scripts (no file in this repo — they stay
in sync with the CLI automatically):

```sh
# zsh — drop into a directory on your $fpath, then re-run compinit
otty completions zsh > "${fpath[1]}/_otty"

# bash
otty completions bash > "$(brew --prefix)/etc/bash_completion.d/otty"   # Homebrew
otty completions bash | sudo tee /etc/bash_completion.d/otty            # system

# fish
otty completions fish > ~/.config/fish/completions/otty.fish
```

Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

Note: the convenience shorthands resolved at runtime (plural aliases like
`windows`, colon forms like `config:show`, `watch:<agent>`) are not part of the
completion tree — only the canonical subcommands are.

---

## Vim / Neovim

Copy the two files into your runtime path (or point a plugin manager at this
directory):

```sh
# Vim
mkdir -p ~/.vim/syntax ~/.vim/ftdetect
cp vim/syntax/otty.vim    ~/.vim/syntax/
cp vim/ftdetect/otty.vim  ~/.vim/ftdetect/

# Neovim
mkdir -p ~/.config/nvim/syntax ~/.config/nvim/ftdetect
cp vim/syntax/otty.vim    ~/.config/nvim/syntax/
cp vim/ftdetect/otty.vim  ~/.config/nvim/ftdetect/
```

`ftdetect/otty.vim` matches `*/otty/config`, `*/otty/config.toml` (and the
`otty-debug` variants) and overrides plain TOML highlighting for those paths.
Known options highlight as keywords; unknown/typo'd keys stay dim.

---

## bat

`bat` uses Sublime syntaxes. We don't claim the `.toml` extension (that would
hijack every TOML file), so install the syntax and map it to the otty config
path explicitly:

```sh
mkdir -p "$(bat --config-dir)/syntaxes"
cp bat/otty.sublime-syntax "$(bat --config-dir)/syntaxes/"
bat cache --build
```

One-off use:

```sh
bat -l Otty ~/.config/otty/config.toml
```

To apply it automatically, add to your bat config (`$(bat --config-dir)/config`):

```
--map-syntax=**/otty/config.toml:Otty
--map-syntax=**/otty/config:Otty
```

---

## Regenerating

`vim/syntax/otty.vim` and `bat/otty.sublime-syntax` are **generated** — the
option-name list is parsed straight from `packages/config/src/lib.rs`
(`Config::known_keys()`) so it never drifts. After adding or removing a config
key, re-run:

```sh
python3 resources/editors/generate.py
```

Enum literal values are curated inside `generate.py` (they change rarely); an
unlisted value just renders with the generic value color.
