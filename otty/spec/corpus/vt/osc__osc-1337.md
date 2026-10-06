<!-- source: https://docs.otty.sh/vt/osc/osc-1337 -->

# OSC 1337 — iTerm2 Proprietary Extensions

|  |  |
| --- | --- |
| Sequence | OSC 1337 ; <Command>[=<args>] ST |
| Otty support | Partial — inline image (File / MultipartFile) only |

## Description

`OSC 1337` is not a single sequence — it's iTerm2's umbrella for a whole family of proprietary commands, all sharing the number `1337` and dispatched by a leading command keyword. Programs use it for everything from embedding images to switching profiles, writing the clipboard, setting colors, and exporting shell variables.

Otty implements **only the inline-image command** (`File=`, plus its chunked `MultipartFile` variant). Every other `OSC 1337` command is currently parsed-and-ignored — it won't corrupt the screen, but it has no effect.

Not the same as the status bar

Setting iTerm2's status bar is done via `SetUserVar` / `ReportVariable` (variable plumbing), which Otty does not implement. `OSC 1337` is much broader than status-bar text.

## Command support matrix

The full iTerm2 command set (see the [authoritative reference](https://iterm2.com/documentation-escape-codes.html)). "✓" = implemented in Otty; "—" = recognized syntax, no effect.
(/vt/osc/osc-7)
| Command | Syntax | Otty | Purpose |
| --- | --- | --- | --- |
| File | File=<args>:<base64> | ✓ | Inline image (see below) |
| MultipartFile / FilePart / FileEnd | chunked File | ✓ | Inline image split across writes |
| SetProfile | SetProfile=<name> | — | Switch the session profile |
| CopyToClipboard / EndCopy | CopyToClipboard=<name> … EndCopy | — | Capture printed text into the clipboard |
| Copy | Copy=:<base64> | — | Copy a base64 blob to the clipboard |
| SetColors | SetColors=<key>=<value> | — | Mutate the session palette (fg/bg/cursor/ansi N) |
| SetUserVar | SetUserVar=<name>=<base64> | — | Export a user variable (status bar, badges) |
| ReportVariable | ReportVariable=<base64> | — | Query a session variable |
| SetMark | SetMark | — | Drop a navigable mark |
| CursorShape | CursorShape=<N> | — | Block / bar / underline cursor |
| StealFocus | StealFocus | — | Raise the window to the foreground |
| ClearScrollback | ClearScrollback | — | Erase scrollback |
| CurrentDir | CurrentDir=<path> | — | Report cwd (Otty uses OSC 7 instead) |
| RemoteHost | RemoteHost=<user>@<host> | — | Report user/host (shell integration) |
| RequestAttention | RequestAttention=<value> | — | Bounce the dock icon |
| ReportCellSize | ReportCellSize | — | Query cell pixel dimensions |
| UnicodeVersion | UnicodeVersion=<n> | — | Select the width table version |
| HighlightCursorLine | HighlightCursorLine=<bool> | — | Cursor-line guide |
| SetBackgroundImageFile | SetBackgroundImageFile=<base64> | — | Session background image |
| AddAnnotation / AddHiddenAnnotation | AddAnnotation=<args> | — | Attach a note to a region |
| SetKeyLabel / PushKeyLabels / PopKeyLabels | SetKeyLabel=<k>=<v> | — | Touch Bar function-key labels |
| RequestUpload | RequestUpload=format=<type> | — | Prompt the user to upload files |
| ShellIntegrationVersion | ShellIntegrationVersion=<n>;<shell> | — | Report shell-integration version |
| ClearCapturedOutput | ClearCapturedOutput | — | Clear captured command output |
| Disinter | Disinter | — | Restore a buried session |
| Custom | Custom=id=<secret>:<pattern> | — | Script-defined custom trigger |
| Block / UpdateBlock | Block=id=<id>;attr=start\|end | — | Foldable block regions |
| Button | Button=type=<type>;<params> | — | Inline interactive button |
| OpenURL | OpenURL=:<base64-url> | — | Ask the terminal to open a URL |

This list is a snapshot

iTerm2 adds commands over time. Some third-party lists (e.g. older "leet" gists) predate `File`, `Block`, `Button`, and `OpenURL`, and so are incomplete. Always cross-check the [official escape-code reference](https://iterm2.com/documentation-escape-codes.html).

## Inline image — `File`

The one command Otty implements. The whole image is base64-encoded and embedded directly in the byte stream; Otty decodes and renders it inline at the cursor.

|  |  |
| --- | --- |
| Sequence | OSC 1337 ; File = <args> : <base64-data> ST |

### Arguments

Semicolon-separated `key=value` list before the colon:

| Key | Type | Meaning |
| --- | --- | --- |
| name | base64 string | Optional filename, shown in tooltips |
| size | int | Total byte size of the encoded data |
| inline | 0 or 1 | If 1, display inline; otherwise download (Otty: always inline). |
| width | auto / <N> / <N>px / <N>% | Display width |
| height | auto / <N> / <N>px / <N>% | Display height |
| preserveAspectRatio | 0 / 1 | If 1, scale to fit while preserving aspect ratio |

Supported image formats: PNG, JPEG, GIF (still).

### Chunked transfer — `MultipartFile`

For large images iTerm2 also supports splitting the payload across multiple sequences, which Otty accepts:

```
OSC 1337 ; MultipartFile = <args> ST    # header (same args as File)
OSC 1337 ; FilePart = <base64-chunk> ST # repeated
OSC 1337 ; FileEnd ST                   # assemble + render
```

### Example
bash
```
# Simplest usage — Otty understands `imgcat` (iTerm utility script):
imgcat ./icon.png

# Manually:
printf '\e]1337;File=inline=1:%s\a' "$(base64 < ./icon.png)"

# With size constraints:
printf '\e]1337;File=inline=1;width=20:%s\a' "$(base64 < ./photo.jpg)"
```

Try it

- `otty features try iterm-image` — emits a tiny embedded PNG via OSC 1337.
- `otty features try kitty-image` — same idea, but using the [Kitty graphics protocol](/vt/sequence-reference) (APC `_G`).

## Related

- [Images](/terminal-features/images) — overview of all image protocols.
- [External Protocols](/vt/external-protocols) — Kitty graphics protocol (APC-based), the richer alternative.
