<!-- source: https://docs.otty.sh/vt/terminal-comparison -->

# Terminal Comparison

How Otty's escape-sequence support compares to other modern terminals. Use this when porting a TUI between terminals or deciding which protocols are safe to rely on.

## Methodology

Symbols:

- ✓ — fully implemented (mode is honored, response is correct)
- ◐ — partially implemented (parsed without errors, but behavior is incomplete — e.g. the iTerm2 inline-image protocol only accepts its multipart form)
- ❌ — not implemented (unknown / ignored / replied as not-supported)

The **Otty** column reflects what the terminal actually honors today — verified against the source, not the documentation. Where a sequence is parsed but the underlying mode is a no-op, it is marked ❌, not ✓.

## Standard DEC private modes

These are the modes set with [DECSET](/vt/csi/decset) `CSI ? Pn h` and reset with [DECRST](/vt/csi/decrst).

| Mode | Name | xterm | iTerm2 | Kitty | WezTerm | Alacritty | Ghostty | Otty |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ?1 | DECCKM — App cursor keys | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?6 | DECOM — Origin mode | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?7 | DECAWM — Auto-wrap | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?9 | X10 mouse (legacy) | ✓ | ✓ | ✓ | ✓ | ❌ | ✓ | ❌ |
| ?12 | Blinking cursor | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?25 | DECTCEM — Cursor visible | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?47 | Alt screen (legacy) | ✓ | ✓ | ✓ | ✓ | ❌ | ✓ | ✓ |
| ?1000 | Mouse: button events | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?1002 | Mouse: cell-motion | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?1003 | Mouse: all-motion | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?1004 | Focus in/out | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?1005 | Mouse: UTF-8 encoding | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?1006 | Mouse: SGR encoding | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?1016 | Mouse: SGR-Pixel | ✓ | ✓ | ✓ | ✓ | ❌ | ✓ | ❌ |
| ?1047 | Alt screen + clear on exit | ✓ | ✓ | ✓ | ✓ | ❌ | ✓ | ✓ |
| ?1049 | Alt screen + save cursor | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?2004 | Bracketed paste | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?2026 | Synchronized output (BSU/ESU) | ⚠️ partial | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?2027 | Grapheme-cluster mode | ❌ | ❌ | ❌ | ❌ | ❌ | ✓ | ❌ |
| ?2031 | Color-scheme update notify | ❌ | ❌ | ✓ | ✓ | ✓ | ✓ | ✓ |
| ?2048 | In-band resize notify | ❌ | ❌ | ✓ | ✓ | ❌ | ✓ | ❌ |

## Identification & queries

| Sequence | What it asks | xterm | iTerm2 | Kitty | WezTerm | Alacritty | Ghostty | Otty |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| CSI c | Primary DA | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI > c | Secondary DA | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI > q | XTVERSION | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI 5 n / 6 n | DSR — status / cursor pos | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI ? Pn $ p | DECRQM mode status | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI 14 t | Text-area size in px | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI 16 t | Cell size in px | ✓ | ✓ | ✓ | ✓ | ❌ | ✓ | ✓ |
| CSI 18 t | Text-area size in cells | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| CSI 22 J | Kitty scroll-then-erase | ❌ | ❌ | ✓ | ❌ | ❌ | ✓ | ✓ |
| CSI ? u | Kitty keyboard-flags query | ❌ | ❌ | ✓ | ✓ | ✓ | ✓ | ✓ |
| DCS $ q … ST | DECRQSS — request current setting | ✓ | ✓ | ✓ | ✓ | ❌ | ✓ | ◑ |
| DCS + q … ST | XTGETTCAP — terminfo cap query | ✓ | ❌ | ✓ | ✓ | ❌ | ✓ | ❌ |

## Vendor extensions

| Protocol | Sequence | Origin | xterm | iTerm2 | Kitty | WezTerm | Alacritty | Ghostty | Otty |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Kitty graphics | APC G … | Kitty | ❌ | ❌ | ✓ | ✓ | ❌ | ✓ | ✓ |
| iTerm2 inline image | OSC 1337 File= | iTerm2 | ❌ | ✓ | ❌ | ✓ | ❌ | ❌ | ◐ multipart only |
| Sixel | DCS q … | DEC | ✓ | ❌ | ❌ | ✓ | ❌ | ✓ | ❌ |
| Kitty keyboard | CSI > 1 u … | Kitty | ❌ | ❌ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Hyperlink | OSC 8 | xterm/VTE | ❌ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Clipboard | OSC 52 | xterm | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Indexed palette | OSC 4 / 104 | xterm | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Dynamic colors | OSC 10 / 11 / 12 | xterm | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Shell integration | OSC 133 (FTCS) | FinalTerm | ❌ | ✓ | ✓ | ✓ | ❌ | ✓ | ✓ |
| VSCode shell integration | OSC 633 | VSCode | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| CWD report | OSC 7 | VTE | ❌ | ✓ | ✓ | ✓ | ❌ | ✓ | ✓ |
| iTerm2 SetUserVar | OSC 1337 SetUserVar= | iTerm2 | ❌ | ✓ | ❌ | ✓ | ❌ | ❌ | ❌ |
| ConEmu progress | OSC 9 ; 4 | ConEmu | ❌ | ❌ | ❌ | ✓ | ❌ | ✓ | ✓ |
| Kitty notification | OSC 99 | Kitty | ❌ | ❌ | ✓ | ❌ | ❌ | ✓ | ✓ |
| iTerm2 notification | OSC 9 | iTerm2 | ❌ | ✓ | ❌ | ✓ | ❌ | ✓ | ✓ |
| xterm color stack | CSI [Pm] # P/Q/R | xterm | ✓ | ❌ | ❌ | ❌ | ❌ | ✓ | ✓ |
| Terminal Resume (TRP) | OSC 88 | Otty | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ◐ experimental |

## Behavior notes

### `CSI ? Pn $ p` — DECRQM

Otty answers DECRQM for both ANSI modes (`CSI Pn $ p`) and DEC private modes (`CSI ? Pn $ p`), replying with DECRPM `CSI ? Pn ; state $ y`. The `state` follows the DEC convention: `1` = set, `2` = reset (supported but currently off), `4` = not recognized. Every private mode listed in the table above reports a truthful set/reset state; any mode Otty doesn't implement replies `4` rather than pretending to support it. The one nuance is `?2026`: the live synchronized-output state lives inside the parser and isn't queryable, so Otty always answers `2` (reset) — which is exactly what nvim, zellij, and Ghostty probe for to decide whether to emit BSU/ESU.

### `CSI ? 2026 h/l` — Synchronized Output

Between BSU and ESU, Otty buffers all output and paints it in a single frame, so a full-screen redraw never tears or flickers. A short safety timeout flushes the buffer if a program enables synchronized output and then dies without sending ESU, so a crash can't freeze the pane.

### `CSI > q` — XTVERSION

Otty replies with `DCS > | otty(<version>) ST`, where `<version>` is the build version. Programs that recognize the `otty(` prefix can gate features on it.

### Selective erase & character protection — DECSCA / DECSED / DECSEL

Otty implements the VT220 character-protection family:

- [DECSCA](/vt/csi/decsca) (`CSI <n> " q`) marks subsequently written cells as protected (`n=1`) or erasable (`n=0`/`2`).
- [DECSED](/vt/csi/ed) (`CSI ? <n> J`) and [DECSEL](/vt/csi/el) (`CSI ? <n> K`) erase only the **erasable** cells, leaving protected ones in place.

Protection is per-cell and independent of SGR (`SGR 0` does not clear it; only DECSCA `0`/`2`, [DECSTR](/vt/csi/decstr), or [RIS](/vt/esc/ris) do), and it is saved/restored with the cursor by [DECSC](/vt/esc/decsc) / [DECRC](/vt/esc/decrc).

Support across terminals is uneven. xterm and Ghostty implement the family; **Alacritty does not** — it has no DECSCA and ignores the selective-erase private variants, so `CSI ? 2 J` finds nothing protectable and clears the screen like a plain erase. That gap is what motivated adding the family to Otty.

## Notable gaps

### `DCS q …` — Sixel

Not parsed. The underlying `vte` parser treats DCS hooks as no-ops and the Otty fork doesn't add a Sixel decoder. Modern equivalents (Kitty graphics) cover the same ground for new code; Sixel remains the only option for legacy tools (`gnuplot`, `img2sixel`, `lsix`).

### `DCS $ q …` / `DCS + q …` — DECRQSS & XTGETTCAP

Partial (◑). Both are DCS-framed queries: DECRQSS asks the terminal to echo back a current setting; XTGETTCAP asks for a terminfo capability. Otty answers the **cursor-style** DECRQSS (`DCS $ q SP q ST` → `DCS 1 $ r <Ps> SP q ST`), so `tmux` and TUIs can read back the active cursor shape after a DECSCUSR change. Other DECRQSS targets (SGR, scrolling region, …) and XTGETTCAP are still not answered — `vte` doesn't dispatch general DCS payloads — so programs probing those fall back to their defaults.

### `?2048` — In-band resize notifications

Not implemented. Programs that opt in expect a `CSI 48 ; … t` report on every resize instead of relying on `SIGWINCH`. Otty ignores the DECSET and the program falls back to `SIGWINCH`, which works on macOS.

### `?1016` — SGR-Pixel mouse

Not implemented. `vte` doesn't recognize mode 1016, so Otty reports mouse events in cell coordinates via SGR mode (`?1006`) instead of pixel coordinates. Apps that request pixel precision degrade gracefully to cell precision.

### OSC 633 — VSCode shell integration

Not parsed. VSCode's integrated-terminal scripts emit OSC 633 in parallel to OSC 133. Otty users running `Set-PSReadLineOption`-style integrations see the bracketed escapes echoed to the screen.

### OSC 1337 single-segment `File=`

Only the multipart form is parsed. The single-segment form (`OSC 1337 ; File=<base64-attrs>:<base64-data> ST`) — the default emitted by `imgcat` and most shell scripts — is dropped. Larger images that don't fit in one OSC payload work; the common small case doesn't.

### OSC 88 — Terminal Resume Protocol

Experimental and Otty-specific. The handshake is wired up — Otty parses `arm` / `clear` / `query` and answers `query` with `OSC 88 ; supported ; v=1 ST` — but persisting a pane's resume spec and replaying it end-to-end is still in progress. No other terminal implements OSC 88. See [OSC 88](/vt/osc/osc-88) and `github.com/Otty-sh/osc-88`.

## See also

- [VT Overview](/vt/overview)
- [Sequence Reference](/vt/sequence-reference)
- [External Protocols](/vt/external-protocols)
