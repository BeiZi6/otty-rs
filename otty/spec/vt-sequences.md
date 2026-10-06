# Otty VT / terminal escape sequence table

Machine-readable source: `vt-sequences.json` (this file is generated from it; the two are the same content).
Total entries: **106**.

Per-sequence vendor pages recrawled into `spec/corpus/vt/` (73 URLs from the 142-entry sitemap, including the previously missing `vt/*` tree). The vendor index is `spec/corpus/vt/sequence-reference.md`. Every entry below is still traceable; the `source` line names the page.

## Status legend (the documentation's own Otty-column marks)

- `implemented` - docs mark ✓ (honoured).
- `partial` - docs mark ◐ / ◑, or the docs describe the sequence as parsed with an incomplete, inert or macOS-absent effect.
- `missing` - docs mark ☐ / ❌ (not parsed / not implemented / ignored).
- A parameter rendered as `default: not stated` means the crawled text does not state a default value for it; nothing was inferred.

## Counts by category

| Category | Entries | implemented | partial | missing |
| --- | --- | --- | --- | --- |
| `c0` | 13 | 13 | 0 | 0 |
| `esc` | 11 | 11 | 0 | 0 |
| `csi` | 41 | 38 | 2 | 1 |
| `osc` | 32 | 25 | 6 | 1 |
| `dcs` | 4 | 1 | 1 | 2 |
| `external` | 5 | 1 | 3 | 1 |
| **total** | **106** | **89** | **12** | **5** |

## Contents

- C0 controls (single bytes) - 13 entries: C0 BEL, C0 BS, C0 HT, C0 LF, C0 VT, C0 FF, C0 CR, C0 SO, C0 SI, C0 CAN, C0 SUB, C0 ESC, C0 DEL
- ESC sequences - 11 entries: ESC DECALN, ESC DECSC, ESC DECRC, ESC DECKPAM, ESC DECKPNM, ESC IND, ESC NEL, ESC RI, ESC HTS, ESC RIS, ESC ST
- CSI sequences - 41 entries: CSI CUU, CSI CUD, CSI CUF, CSI CUB, CSI CUP, CSI CHA, CSI VPA, CSI ED, CSI DECSED, CSI EL, CSI DECSEL, CSI SGR, CSI ICH, CSI IL, CSI DL, CSI DCH, CSI SU, CSI SD, CSI TBC, CSI DECSTBM, CSI DECSLRM, CSI LNM, CSI 5 n DSR (status), CSI 6 n DSR (cursor position), CSI DECXCPR, CSI DECSET, CSI DECRST, CSI DECSTR, CSI DECSCA, CSI DECSCUSR, CSI > 1 u (Kitty keyboard push), CSI < u (Kitty keyboard pop), CSI ? u (Kitty keyboard query), CSI XTPUSHCOLORS, CSI XTPOPCOLORS, CSI XTREPORTCOLORS, CSI XTWINOPS, CSI c Primary DA, CSI > c Secondary DA, CSI > q XTVERSION, CSI DECRQM
- OSC sequences - 32 entries: OSC 0 / 1 / 2, OSC 4, OSC 5, OSC 7, OSC 8, OSC 9, OSC 9;4, OSC 10, OSC 11, OSC 12, OSC 13, OSC 14, OSC 15, OSC 16, OSC 17, OSC 18, OSC 19, OSC 22, OSC 52, OSC 88, OSC 99, OSC 104, OSC 105, OSC 110, OSC 111, OSC 112, OSC 117, OSC 119, OSC 133, OSC 633, OSC 777, OSC 1337
- DCS sequences - 4 entries: DCS DECRQSS (cursor style), DCS DECRQSS (other targets), DCS XTGETTCAP, DCS introducer
- External protocols (APC / DCS vendor protocols) - 5 entries: APC introducer, PM introducer, Kitty graphics protocol, Sixel graphics, tmux passthrough

## C0 controls (single bytes) (13)

### C0 BEL

```text
0x07
```

- mnemonic: `BEL`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Bell - rings the terminal bell.
- detail: Byte 0x07. Modern terminals translate the byte into one of three behaviours according to user preference: audible (plays the system alert sound), visual (briefly flashes the focused pane), none (silent). In Otty this is configured with bell = visual | audio | both | none. BEL is also accepted as a legacy OSC string terminator in place of ESC \. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/bel>

### C0 BS

```text
0x08
```

- mnemonic: `BS`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Backspace - moves the cursor one cell to the left.
- detail: Byte 0x08. Does not erase the cell underneath; to delete, follow with a space then another BS, or use DCH. At column 1 BS has no effect (no wrap). 0x08 is what Ctrl-H produces, which is why many TUIs treat Ctrl-H and Backspace identically. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/bs>

### C0 HT

```text
0x09
```

- mnemonic: `HT`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Horizontal Tab - advances the cursor to the next tab stop.
- detail: Byte 0x09. Default tab stops are every 8 columns. Tab stops are set with HTS (ESC H) and cleared with CSI g (or CSI 3 g for all). If the cursor is past the last tab stop on the line, HT moves it to the right margin without wrapping. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/ht>

### C0 LF

```text
0x0A
```

- mnemonic: `LF`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Line Feed - moves the cursor down one row.
- detail: Byte 0x0A. The column does not change; a new line in the leftmost column needs CR LF (or NEL). If the cursor is at the bottom of the active region, LF scrolls that region up by one line. When LNM (line-feed/new-line mode, CSI 20 h) is set, LF behaves like NEL (also moves to column 1); LNM is off by default. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/lf>

### C0 VT

```text
0x0B
```

- mnemonic: `VT`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Vertical Tab - treated as LF by default.
- detail: Byte 0x0B. Historically advanced the cursor to the next vertical tab stop; vertical tab stops are essentially never used on modern terminals, so Otty (like every modern emulator) treats VT as a synonym for LF. Doc mark: Otty support ✓ (treated as LF).
- source: <https://docs.otty.sh/vt/c0/vt>

### C0 FF

```text
0x0C
```

- mnemonic: `FF`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Form Feed - treated as LF.
- detail: Byte 0x0C. Historically advanced a printer to the next page; on terminal emulators FF is treated as LF and Otty follows this convention. Ctrl-L (which sends FF) is used by some shells as a clear-screen idiom, but that is handled by the shell, not the terminal. Doc mark: Otty support ✓ (treated as LF).
- source: <https://docs.otty.sh/vt/c0/ff>

### C0 CR

```text
0x0D
```

- mnemonic: `CR`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Carriage Return - moves the cursor to column 1 of the current row.
- detail: Byte 0x0D. Does not change the row. A typical newline is CR LF. On Unix-line-ending files \n (LF) alone is enough only because LNM mode is off and shells issue an implicit CR via NEL-style logic; raw printf '\n' mid-line moves down without resetting the column. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/cr>

### C0 SO

```text
0x0E
```

- mnemonic: `SO`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Shift Out - switches to the G1 character set.
- detail: Byte 0x0E. Listed in the C0 set as 'Shift Out - switch to G1 character set'; pairs with SI, which returns to G0. STATUS CAVEAT (not doc-verified): the crawled docs state no Otty support mark for SO - the C0 index page carries no Otty column and the dedicated SO page (vt/c0/so, referenced by the SI page and by the Control sidebar) is not present in this crawl. Recorded as implemented pending confirmation; its documented pair SI is marked ✓.
- source: <https://docs.otty.sh/vt/c0>

### C0 SI

```text
0x0F
```

- mnemonic: `SI`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Shift In - switches the active character set back to G0.
- detail: Byte 0x0F. Switches the active character set back to G0 (the default - usually US ASCII). Pairs with SO, which switches to G1. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/si>

### C0 CAN

```text
0x18
```

- mnemonic: `CAN`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Cancel - aborts a control sequence in progress.
- detail: Byte 0x18. Aborts any control sequence currently being parsed; after CAN the parser returns to the ground state and partially received CSI / OSC / DCS payloads are silently discarded. Useful for clients that detected an upstream desync and want a clean parsing state. Example from the docs: printf '\e[31\x18m' cancels the SGR mid-way and the following 'm' is rendered as text. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/can>

### C0 SUB

```text
0x1A
```

- mnemonic: `SUB`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Substitute - same as CAN, aborts an in-flight sequence.
- detail: Byte 0x1A. Identical behaviour to CAN: aborts any in-flight control sequence and returns the parser to the ground state. The two bytes exist as a historical artefact; either works. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/c0/sub>

### C0 ESC

```text
0x1B
```

- mnemonic: `ESC`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Escape - introduces every multi-byte sequence.
- detail: Byte 0x1B. Introduces CSI, OSC, ESC-final, DCS, APC and so on. A bare ESC (followed by no recognised intermediate / final) resets the parser. Inside an OSC or DCS payload, ESC begins a string terminator (ESC \). In shell readline and many editors the ESC key is also a meta prefix. Doc mark: Otty support ✓. Notation: ^[, \e and \x1b are all the same byte.
- source: <https://docs.otty.sh/vt/c0/esc>

### C0 DEL

```text
0x7F
```

- mnemonic: `DEL`
- category: `c0`
- status: **implemented** (✓)
- params: none
- summary: Delete - ignored as a display character.
- detail: Byte 0x7F. On modern terminals DEL is ignored when received from the host - it does not move the cursor or erase anything. It is what most keyboards send for the Backspace key, but reading it as 'delete the previous character' is the receiving program's interpretation, not the terminal's. Doc mark: Otty support ✓ (ignored as a display character).
- source: <https://docs.otty.sh/vt/c0/del>

## ESC sequences (11)

### ESC DECALN

```text
ESC # 8
```

- mnemonic: `DECALN`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Screen Alignment Display - fills the visible screen with E.
- detail: Bytes 0x1B 0x23 0x38. Fills the entire visible screen with uppercase E using default attributes, so technicians could verify screen geometry and convergence. After execution: the cursor is moved to home (row 1, column 1); every cell of the active viewport is replaced with E; active SGR attributes are reset for the filled cells; scrollback is not affected. Does not change the cursor style or scrolling region. Combined with DECSTR (ESC [ ! p) it is the canonical way to put a terminal into a known-good state during tests. Doc mark: Otty support ✓ Implemented.
- source: <https://docs.otty.sh/vt/esc/decaln>

### ESC DECSC

```text
ESC 7
```

- mnemonic: `DECSC`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Save Cursor - saves the full cursor state for DECRC.
- detail: Bytes 0x1B 0x37. Saved state: cursor row and column, active SGR attributes, character set designation (G0 / G1 selection), origin mode (DECOM) state, auto-wrap (DECAWM) state. There is one save slot per screen - calling DECSC again overwrites the previous save - and save/restore is per-screen, so switching between main and alternate screen does not affect the other's saved state. Related alternative: CSI s / CSI u (ANSI.SYS save/restore). Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/decsc>

### ESC DECRC

```text
ESC 8
```

- mnemonic: `DECRC`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Restore Cursor - restores the state saved by DECSC.
- detail: Bytes 0x1B 0x38. Restores cursor position, SGR attributes, character set selection, origin mode and auto-wrap mode. If no DECSC has been issued for the active screen, DECRC moves the cursor to home (1, 1) with default attributes. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/decrc>

### ESC DECKPAM

```text
ESC =
```

- mnemonic: `DECKPAM`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Keypad Application Mode.
- detail: Bytes 0x1B 0x3D. Switches the numeric keypad to application mode: keypad keys send escape sequences instead of their numeric values (keypad 5 sends ESC O u instead of '5') so programs can bind them as function keys. Pair with DECKPNM (ESC >) to switch back. Most TUIs (vim, htop) enter application mode on start and exit it on quit. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/deckpam>

### ESC DECKPNM

```text
ESC >
```

- mnemonic: `DECKPNM`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Keypad Numeric Mode.
- detail: Bytes 0x1B 0x3E. Switches the numeric keypad back to numeric mode - keypad keys send their literal digits / operators. This is the default for normal shell sessions. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/deckpnm>

### ESC IND

```text
ESC D
```

- mnemonic: `IND`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Index - moves the cursor down one row.
- detail: Bytes 0x1B 0x44. Moves the cursor down one row, scrolling the active region if the cursor is at the bottom. Equivalent in effect to LF but encoded as a two-byte ESC sequence - useful inside escape sequences where a raw control byte is unwanted. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/ind>

### ESC NEL

```text
ESC E
```

- mnemonic: `NEL`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Next Line - moves to column 1 of the next row.
- detail: Bytes 0x1B 0x45. Equivalent to CR LF (\r\n) but as a single two-byte sequence. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/nel>

### ESC RI

```text
ESC M
```

- mnemonic: `RI`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Reverse Index - moves the cursor up one row.
- detail: Bytes 0x1B 0x4D. Moves the cursor up one row, scrolling the active region down if the cursor is at the top. The opposite of IND. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/ri>

### ESC HTS

```text
ESC H
```

- mnemonic: `HTS`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Set Tab Stop at the cursor's current column.
- detail: Bytes 0x1B 0x48. Sets a horizontal tab stop at the cursor's current column; subsequent HT (\t) advances to the next stop. Tab stops are per-screen and reset by RIS. Clear individually with CSI g (clear at cursor) or CSI 3 g (clear all). Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/hts>

### ESC RIS

```text
ESC c
```

- mnemonic: `RIS`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: Full Reset to power-on state.
- detail: Bytes 0x1B 0x63. Switches to the main screen, clears it and clears scrollback; resets the cursor to home with default SGR and default character sets; restores default tab stops (every 8 columns); disables every DEC private mode set via DECSET; clears any saved cursor state. Use sparingly - DECSTR (soft reset) is preferred for most cases because it preserves scrollback. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/esc/ris>

### ESC ST

```text
ESC \
```

- mnemonic: `ST`
- category: `esc`
- status: **implemented** (✓)
- params: none
- summary: String Terminator - ends an OSC or DCS payload.
- detail: Bytes 0x1B 0x5C. The docs list ST in the Fe-class introducer table (ESC \ = ST) and require it to terminate OSC and DCS payloads; ESC \ is the preferred form, BEL (0x07) is the legacy form and is accepted, and the DCS page also names 0x9C (8-bit ST). STATUS CAVEAT (not doc-verified): there is no dedicated ST page in this crawl and no explicit Otty support mark for ST itself. Recorded as implemented pending confirmation - every string-carrying sequence Otty answers (OSC 4 / 8 / 10-19 / 52 / 99 / 1337, DCS DECRQSS) depends on it.
- source: <https://docs.otty.sh/vt/c0>

## CSI sequences (41)

### CSI CUU

```text
CSI <n> A
```

- mnemonic: `CUU`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Cursor Up - moves the cursor <n> rows up.
- detail: Listed in Sequence Reference → CSI as ✓. CAVEAT: no dedicated CUU page is present in this crawl (the page is referenced by the CUP and DCH pager links as 'CUU - Cursor Up'), so the docs give no explicit default or clamping statement for CUU here. The documented sibling relative moves state default <n> = 1 and clamp without wrapping (CUB clamps at column 1, CUF at the right margin); CUP's Related list groups CUU/CUD/CUF/CUB as 'relative cursor movement'. Default value therefore not determined from the text.
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI CUD

```text
CSI <n> B
```

- mnemonic: `CUD`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Cursor Down - moves the cursor <n> rows down.
- detail: Listed in Sequence Reference → CSI as ✓. CAVEAT: no dedicated CUD page is present in this crawl (referenced by the CUB and CUF pager links as 'CUD - Cursor Down'), so no explicit default or clamping statement is available for CUD in this crawl. The documented sibling relative moves state default <n> = 1 and clamp without wrapping. Default value therefore not determined from the text.
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI CUF

```text
CSI <n> C
```

- mnemonic: `CUF`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `1`
- summary: Cursor Forward - moves the cursor <n> columns right.
- detail: Bytes 0x1B 0x5B <n> 0x43. Moves the cursor <n> columns to the right in the current row. Default <n> = 1. Clamps at the right margin; no wrap. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/cuf>

### CSI CUB

```text
CSI <n> D
```

- mnemonic: `CUB`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `1`
- summary: Cursor Back - moves the cursor <n> columns left.
- detail: Bytes 0x1B 0x5B <n> 0x44. Moves the cursor <n> columns to the left in the current row. Default <n> = 1. Clamps at column 1; no wrap. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/cub>

### CSI CUP

```text
CSI <row> ; <col> H
```

- mnemonic: `CUP`
- category: `csi`
- status: **implemented** (✓)
- params: `row` default `1`; `col` default `1`
- summary: Cursor Position - moves the cursor to an absolute 1-indexed row/column.
- detail: Bytes 0x1B 0x5B <row> 0x3B <col> 0x48. Row and column are 1-indexed - CSI 1 ; 1 H is the top-left cell. Defaults: omitted parameters are treated as 1, so CSI H moves to home (1, 1). Alias: CSI <r>;<c> f is HVP (Horizontal and Vertical Position) with identical behaviour in modern terminals. If a scrolling region is active (DECSTBM), positions are bounded inside that region when DECOM (origin mode) is set; otherwise they are absolute to the viewport. Out-of-range positions clamp to the viewport edge. The cursor reports its position via CSI 6 n (DSR) -> CSI <row>;<col> R. Doc mark: Otty support ✓ Implemented.
- source: <https://docs.otty.sh/vt/csi/cup>

### CSI CHA

```text
CSI <n> G
```

- mnemonic: `CHA`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Cursor Horizontal Absolute - moves to a column only.
- detail: Documented on the CUP page's Related list as 'CSI <n> G - Cursor Horizontal Absolute (column only)' and demonstrated in the HTS example ('printf \e[20G\eH  # move to column 20, set a tab stop there'), which is what establishes its meaning. CAVEAT: no dedicated page, no Sequence Reference row and no explicit Otty support mark in the crawled docs; recorded as implemented pending confirmation, and the default/parameter handling is not stated.
- source: <https://docs.otty.sh/vt/csi/cup>

### CSI VPA

```text
CSI <n> d
```

- mnemonic: `VPA`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Vertical Position Absolute - moves to a row only.
- detail: Documented on the CUP page's Related list as 'CSI <n> d - Vertical Position Absolute (row only)'. CAVEAT: no dedicated page, no Sequence Reference row and no explicit Otty support mark in the crawled docs; recorded as implemented pending confirmation, and the default/parameter handling is not stated.
- source: <https://docs.otty.sh/vt/csi/cup>

### CSI ED

```text
CSI <n> J
```

- mnemonic: `ED`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `0`
- summary: Erase in Display.
- detail: Bytes 0x1B 0x5B <n> 0x4A. Default <n> = 0. Values: 0 erase from cursor to end of screen; 1 erase from start of screen to cursor; 2 erase the whole screen; 3 erase the whole screen and scrollback (xterm extension); 22 push the entire viewport into scrollback, then erase (kitty extension, also supported by kitty / WezTerm / Ghostty / Contour - use \e[22J\e[H instead of \e[2J\e[H for a Ctrl-L that preserves history; terminals without it silently drop the bytes). Erased cells become the default background and cursor position is unchanged, apart from the implicit scroll for n=22. On the main screen CSI 2 J also scrolls the viewport into scrollback. Doc mark: Otty support ✓ (n=0/1/2/3 standard, n=22 kitty scroll-then-erase).
- source: <https://docs.otty.sh/vt/csi/ed>

### CSI DECSED

```text
CSI ? <n> J
```

- mnemonic: `DECSED`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `0`
- summary: Selective Erase in Display - skips DECSCA-protected cells.
- detail: CSI ? <n> J (note the ? private marker). Erases only cells that are not protected by DECSCA, leaving protected cells untouched; <n> takes the same 0 / 1 / 2 meanings as plain ED. Unlike CSI 2 J, which on the main screen scrolls the viewport into scrollback, DECSED always erases in place and never touches scrollback. Some terminals (e.g. Alacritty) ignore the sequence entirely. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/ed>

### CSI EL

```text
CSI <n> K
```

- mnemonic: `EL`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `0`
- summary: Erase in Line.
- detail: Bytes 0x1B 0x5B <n> 0x4B. Default <n> = 0. Values: 0 erase from cursor to end of line; 1 erase from start of line to cursor; 2 erase the whole line. Cursor position is unchanged. Useful for redrawing a status line or progress bar without scrolling. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/el>

### CSI DECSEL

```text
CSI ? <n> K
```

- mnemonic: `DECSEL`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `0`
- summary: Selective Erase in Line - skips DECSCA-protected cells.
- detail: CSI ? <n> K (note the ? private marker). Erases only cells not protected by DECSCA, with the same 0 / 1 / 2 meanings as plain EL; protected cells on the line are left in place. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/el>

### CSI SGR

```text
CSI <p1> ; <p2> ; ... m
```

- mnemonic: `SGR`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Select Graphic Rendition - colors and text styles.
- detail: Bytes 0x1B 0x5B <params> 0x6D. Parameters are 1+ numeric codes separated by ; (or : for sub-parameters); multiple codes apply in order. CSI m (no parameters) and CSI 0 m both reset all attributes. Common parameters: 0 reset, 1 bold, 2 dim, 3 italic, 4 underline, 5 blink, 7 reverse video, 8 conceal, 9 strike-through, 21 double underline, 22 reset bold/dim, 23 reset italic, 24 reset underline, 30-37 foreground ANSI 0-7, 39 default foreground, 40-47 background ANSI 0-7, 49 default background, 90-97 foreground bright 0-7, 100-107 background bright 0-7, 38;5;<n> / 48;5;<n> 256-color, 38;2;<r>;<g>;<b> / 48;2;<r>;<g>;<b> 24-bit (each channel 0-255). Extended underlines via : sub-parameters (kitty / iTerm2 / WezTerm convention), all five styles render: 4:0 none (clears underline), 4:1 single (2 px), 4:2 double (two stacked 2 px lines - use 4:2, not \e[21m, because under ECMA-48 21 resets bold), 4:3 curly undercurl (square-wave wavy line, fg or SGR 58 colored), 4:4 dotted, 4:5 dashed; 58:5:<n> or 58:2::<r>:<g>:<b> underline color (independent of text color; kitty emits the colon form with an empty colorspace slot, and the semicolon form 58;2;R;G;B is also accepted); 59 resets underline color. Notes: parameter 1 (bold) renders in the bright palette when bold-is-bright = on (off by default); Otty caps SGR parsing at 256 parameters per sequence and truncates longer chains; Otty applies real font weight rather than treating 1 as a bright hint. Doc mark: Otty support ✓ Implemented (full standard + extended underlines + RGB).
- source: <https://docs.otty.sh/vt/csi/sgr>

### CSI ICH

```text
CSI <n> @
```

- mnemonic: `ICH`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `1`
- summary: Insert Characters - inserts blank cells at the cursor.
- detail: Bytes 0x1B 0x5B <n> 0x40. Inserts <n> blank cells at the cursor position, shifting existing content to the right; cells that move past the right margin are lost. Default <n> = 1. Cursor position itself is unchanged. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/ich>

### CSI IL

```text
CSI <n> L
```

- mnemonic: `IL`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `1`
- summary: Insert Lines - inserts blank rows within the scrolling region.
- detail: Bytes 0x1B 0x5B <n> 0x4C. Inserts <n> blank rows at the cursor, scrolling subsequent rows down within the active region; rows scrolled past the bottom of the region are lost (they do not enter scrollback). Default <n> = 1. Only effective inside the scrolling region (set by DECSTBM). Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/il>

### CSI DL

```text
CSI <n> M
```

- mnemonic: `DL`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Delete Lines.
- detail: Listed in Sequence Reference → CSI as ✓ (Delete Lines), and referenced as the inverse of IL by the IL page. CAVEAT: no dedicated DL page is present in this crawl (referenced by the DECSTR and DSR pager links as 'DL - Delete Lines'), so no explicit default or scrolling-region statement is available for DL in this crawl. Default value therefore not determined from the text.
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI DCH

```text
CSI <n> P
```

- mnemonic: `DCH`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `1`
- summary: Delete Characters - deletes cells and shifts the row left.
- detail: Bytes 0x1B 0x5B <n> 0x50. Deletes <n> cells starting at the cursor, shifting remaining cells in the row to the left; cells on the right are filled with blanks. Default <n> = 1. Cursor position is unchanged. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/dch>

### CSI SU

```text
CSI <n> S
```

- mnemonic: `SU`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Scroll Up.
- detail: Listed in Sequence Reference → CSI as ✓ (Scroll Up). CAVEAT: no dedicated page in this crawl, so no explicit default, region or scrollback statement is available; the default value is therefore not determined from the text.
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI SD

```text
CSI <n> T
```

- mnemonic: `SD`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Scroll Down.
- detail: Listed in Sequence Reference → CSI as ✓ (Scroll Down). CAVEAT: no dedicated page in this crawl, so no explicit default, region or scrollback statement is available; the default value is therefore not determined from the text.
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI TBC

```text
CSI g
```

- mnemonic: `TBC`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default: not stated
- summary: Tab Clear - clears one or all horizontal tab stops.
- detail: Documented on the HT page ('cleared with CSI g') and the HTS page ('use CSI g (clear at cursor) or CSI 3 g (clear all)'). The HT page also labels it 'omitted from the index here; rarely used'. CAVEAT: no dedicated page, no Sequence Reference row and no explicit Otty support mark in the crawled docs; recorded as implemented pending confirmation. Parameter values beyond 0 (clear at cursor) and 3 (clear all) are not documented.
- source: <https://docs.otty.sh/vt/esc/hts>

### CSI DECSTBM

```text
CSI <top> ; <bot> r
```

- mnemonic: `DECSTBM`
- category: `csi`
- status: **implemented** (✓)
- params: `top` default: not stated; `bot` default: not stated
- summary: Set Top and Bottom Margins - vertical scrolling region.
- detail: Sets a vertical scrolling region: line-feed at the bottom row scrolls only this region, which is what status-bar applications rely on; reset with bare CSI r. IL is 'only effective inside the scrolling region (set by DECSTBM)', and CUP positions are bounded inside the region when DECOM (?6) is on. CAVEAT: no dedicated page, no Sequence Reference row and no explicit Otty support mark in the crawled docs, and the parameter defaults are not stated; recorded as implemented pending confirmation (bare CSI r resetting the region is documented).
- source: <https://docs.otty.sh/vt/concepts/screen>

### CSI DECSLRM

```text
CSI <l> ; <r> s
```

- mnemonic: `DECSLRM`
- category: `csi`
- status: **implemented** (✓)
- params: `l` default: not stated; `r` default: not stated
- summary: Set Left and Right Margins - horizontal scrolling region.
- detail: The Screen concepts page states: 'CSI <l>;<r> s (DECSLRM) sets a horizontal region - supported but rare.' That 'supported' is the only status statement; no parameter defaults, clamping rules or interaction with auto-wrap are documented in the crawled text.
- source: <https://docs.otty.sh/vt/concepts/screen>

### CSI LNM

```text
CSI 20 h
```

- mnemonic: `LNM`
- category: `csi`
- status: **implemented** (✓)
- params: `Ps (mode)` default: not stated
- summary: Line-feed / new-line mode - makes LF behave like NEL.
- detail: The LF page states: 'When LNM (line-feed/new-line mode, CSI 20 h) is set, LF behaves like NEL (also moves to column 1). LNM is off by default; almost all modern programs assume CR-then-LF for newlines.' The CR page adds that '\n (LF) is enough only because LNM mode is off and shells issue an implicit CR via NEL-style logic'. CAVEAT: LNM is an ANSI (non-private) mode - it is absent from the DEC private mode tables and carries no explicit Otty support mark in the crawled docs; recorded as implemented pending confirmation. Reset with CSI 20 l (not separately documented).
- source: <https://docs.otty.sh/vt/c0/lf>

### CSI 5 n DSR (status)

```text
CSI 5 n
```

- mnemonic: `DSR`
- category: `csi`
- status: **implemented** (✓)
- params: `Ps (selector)` default: not stated
- summary: Device Status Report - status query, replies CSI 0 n.
- detail: Bytes 0x1B 0x5B <Ps> 0x6E. Otty answers immediately with CSI 0 n, meaning the terminal is ready / OK (no malfunction). The reply is written back up the PTY as a PTY write ordered with respect to other terminal output the terminal generates, so capturing it requires raw mode on stdin. Doc mark: Otty support ✓ Implemented (Ps = 5, 6).
- source: <https://docs.otty.sh/vt/csi/dsr>

### CSI 6 n DSR (cursor position)

```text
CSI 6 n
```

- mnemonic: `DSR`
- category: `csi`
- status: **partial** (◐)
- params: `Ps (selector)` default: not stated
- summary: Cursor Position Report - replies CSI <row> ; <col> R.
- detail: Bytes 0x1B 0x5B <Ps> 0x6E. The reply is CSI <row> ; <col> R (CPR) with 1-based row / col, reported relative to the viewport - or to the scrolling region when origin mode (DECOM / ?6) is active - matching how CUP interprets them. This is how a shell or TUI discovers where the cursor landed after printing text. Doc mark: ◐ in Sequence Reference - the standard CSI 6 n form is answered, but the DEC private variant DECXCPR (CSI ? 6 n) is not implemented, which is what makes the row partial.
- source: <https://docs.otty.sh/vt/csi/dsr>

### CSI DECXCPR

```text
CSI ? 6 n
```

- mnemonic: `DECXCPR`
- category: `csi`
- status: **missing** (☐)
- params: none
- summary: Extended cursor position report - not implemented.
- detail: The DEC private variant of DSR that adds a page number, replying CSI ? <row> ; <col> ; <page> R. Otty implements only the standard CSI 6 n form; single-page terminals essentially never need DECXCPR. Doc mark: not implemented (DSR page 'Not supported' section; Sequence Reference CSI 6 n row states 'DECXCPR CSI ? 6 n not implemented').
- source: <https://docs.otty.sh/vt/csi/dsr>

### CSI DECSET

```text
CSI ? <Pn> ; <Pn> ... h
```

- mnemonic: `DECSET`
- category: `csi`
- status: **implemented** (✓)
- params: `Pn (mode)` default: not stated
- summary: Set DEC Private Mode - enables one or more private modes.
- detail: Bytes 0x1B 0x5B 0x3F <Pn> 0x68. Each <Pn> is a numeric mode identifier; multiple modes can be set in a single sequence with ;-separation. Documented modes (DECSET page): 1 application cursor keys; 5 reverse video; 6 origin mode (DECOM - CUP relative to the scrolling region); 7 auto-wrap (DECAWM); 12 blinking cursor; 25 cursor visible (DECTCEM); 47 switch to alternate screen (legacy - no cursor save/clear); 1000 send mouse X/Y on button event; 1002 cell motion mouse tracking; 1003 all motion mouse tracking; 1004 send focus in/out events; 1006 SGR-style mouse reporting; 1047 switch to alternate screen (clears it when switched back off); 1049 switch to alternate screen + save cursor; 2004 bracketed paste; 2026 synchronized output. Additional modes with descriptions in the Sequence Reference 'Modes (DEC private)' table: 1005 mouse UTF-8 encoding and 2031 color-scheme update notifications. Terminal Comparison gives per-mode Otty marks: ✓ for ?1 ?6 ?7 ?12 ?25 ?47 ?1000 ?1002 ?1003 ?1004 ?1005 ?1006 ?1047 ?1049 ?2004 ?2026 ?2031; ❌ for ?9 (X10 mouse legacy), ?1016 (SGR-Pixel mouse), ?2027 (grapheme-cluster mode), ?2048 (in-band resize notify). Behaviour notes: ?47/?1047/?1049 differ in cursor handling and alt-buffer clearing - 47 bare swap (no cursor save; alt buffer keeps what it held), 1047 swap + clear the alt buffer when switching back, 1049 save cursor and clear the alt buffer on entry, restore on exit (what vim / htop issue). With ?2026 set, Otty buffers all output between BSU and ESU and paints it in a single frame so a full-screen redraw never tears; a short safety timeout flushes the buffer if a program enables synchronized output and dies without sending ESU. ?2031 subscribers receive the \e[?997;N n color-scheme reply pattern. ?1004 focus reporting emits CSI I / CSI O. ?2004 bracketed paste wraps pastes as \e[200~ ... \e[201~. ?2048 is ignored, so programs fall back to SIGWINCH; ?1016 is unrecognized, so mouse events are reported in cell coordinates via ?1006. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/decset>

### CSI DECRST

```text
CSI ? <Pn> ; <Pn> ... l
```

- mnemonic: `DECRST`
- category: `csi`
- status: **implemented** (✓)
- params: `Pn (mode)` default: not stated
- summary: Reset DEC Private Mode - disables one or more private modes.
- detail: Bytes 0x1B 0x5B 0x3F <Pn> 0x6C. Disables one or more DEC private modes; the mode numbers are the same set as DECSET (see the DECSET entry for the full table and per-mode Otty marks). Documented uses: \e[?25l hides the cursor, \e[?1049l leaves the alternate screen and restores the cursor (what vim issues on exit), \e[?1047l leaves the alternate screen clearing it first (legacy). Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/decrst>

### CSI DECSTR

```text
CSI ! p
```

- mnemonic: `DECSTR`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Soft Terminal Reset - resets state without wiping the screen.
- detail: Bytes 0x1B 0x5B 0x21 0x70. Restores: SGR attributes to defaults; cursor visibility and cursor style; DECOM, DECAWM and application keypad mode; clears the saved cursor (DECSC); restores the scrolling region to full screen; restores all DEC private modes to their power-on defaults. It does not touch visible screen contents, scrollback, tab stops or character set assignments. Preferred over RIS (full reset) in most situations, and DECSTR also clears the DECSCA character-protection attribute. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/decstr>

### CSI DECSCA

```text
CSI <n> " q
```

- mnemonic: `DECSCA`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `0`
- summary: Select Character Protection Attribute.
- detail: Bytes 0x1B 0x5B <n> 0x22 0x71. Marks subsequently written characters as protected or erasable: 0 characters are erasable (default), 1 characters are protected from selective erase, 2 characters are erasable (same as 0). Protected cells are skipped by the selective-erase sequences DECSED (CSI ? <n> J) and DECSEL (CSI ? <n> K); ordinary ED / EL ignore the attribute and erase everything. The attribute sticks to the cursor and applies to every character written until changed. Interaction with other state: protection is per-cell and independent of SGR - SGR 0 does not clear it, only DECSCA 0 / 2, DECSTR or RIS do; it is saved and restored with the cursor by DECSC / DECRC; and erasing a protected cell through any path leaves the replacement cell unprotected. Doc mark: Otty support ✓; Terminal Comparison notes Alacritty lacks the family, which is what motivated adding it to Otty.
- source: <https://docs.otty.sh/vt/csi/decsca>

### CSI DECSCUSR

```text
CSI <n> SP q
```

- mnemonic: `DECSCUSR`
- category: `csi`
- status: **implemented** (✓)
- params: `n` default `0`
- summary: Set Cursor Style - shape and blink state.
- detail: Bytes 0x1B 0x5B <n> 0x20 0x71 - note the literal SP (space, 0x20) between the parameter and the final q. Values: 0 and 1 block blinking; 2 block steady; 3 underline blinking; 4 underline steady; 5 beam blinking; 6 beam steady (0 means back to the user default). The shape always takes effect; the blink bit is honored only when the cursor-style-blink policy is one of the default_* values - the always_* values pin the blink and ignore both DECSCUSR's blink bit and DEC mode 12. The user default lives in the config (cursor-style and cursor-style-blink). The current style is queryable with DECRQSS (DCS $ q SP q ST). Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/decscusr>

### CSI > 1 u (Kitty keyboard push)

```text
CSI > 1 u
```

- mnemonic: `Kitty keyboard push`
- category: `csi`
- status: **implemented** (✓)
- params: `flags` default: not stated
- summary: Push Kitty keyboard protocol flags.
- detail: Sequence Reference lists 'CSI > 1 u | - | Push Kitty keyboard flags | ✓' (no mnemonic given in the docs). External Protocols groups it as the CSI-based protocol 'Kitty keyboard protocol | CSI > 1 u / CSI < u | Kitty | ✓', and Terminal Comparison lists 'Kitty keyboard | CSI > 1 u ...' as ✓ for Otty. CAVEAT: the per-flag parameter table for the Kitty keyboard protocol is not part of this crawl, so the individual flag bits are not transcribed here.
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI < u (Kitty keyboard pop)

```text
CSI < u
```

- mnemonic: `Kitty keyboard pop`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Pop Kitty keyboard protocol flags.
- detail: Sequence Reference lists 'CSI < u | - | Pop Kitty keyboard flags | ✓' (no mnemonic given in the docs). Paired with the push form CSI > 1 u; External Protocols describes the pair as the Kitty keyboard protocol (origin Kitty, Otty ✓).
- source: <https://docs.otty.sh/vt/sequence-reference>

### CSI ? u (Kitty keyboard query)

```text
CSI ? u
```

- mnemonic: `Kitty keyboard query`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Kitty keyboard-flags query.
- detail: Listed in Terminal Comparison → Identification & queries as 'CSI ? u | Kitty keyboard-flags query' with Otty ✓ (xterm and iTerm2 are ❌). No dedicated page in this crawl; the reply format is not documented in the crawled text.
- source: <https://docs.otty.sh/vt/terminal-comparison>

### CSI XTPUSHCOLORS

```text
CSI [Pm] # P
```

- mnemonic: `XTPUSHCOLORS`
- category: `csi`
- status: **implemented** (✓)
- params: `Pm (slot)` default: not stated
- summary: Push a palette snapshot onto the per-session colour stack.
- detail: Snapshots the current palette (OSC 4 indices 0-255) plus the named colour slots (fg / bg / cursor, plus Otty's OSC 5 / 17 / 19 extras) and pushes it onto a per-session stack. Up to 10 entries; the oldest is FIFO-evicted on overflow. A non-zero Pm names a specific slot (1-10) instead of the anonymous stack, for nested setups that must not disturb the outer saved state. The stack is per session (pane) - new panes inherit nothing - and costs ~1 KiB per entry (under 11 KiB when full). Preferred over OSC 104 / 110-119 because XTPOPCOLORS restores the runtime value at the matching push rather than the configured default. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/xt-colors>

### CSI XTPOPCOLORS

```text
CSI [Pm] # Q
```

- mnemonic: `XTPOPCOLORS`
- category: `csi`
- status: **implemented** (✓)
- params: `Pm (slot)` default: not stated
- summary: Restore a palette snapshot from the colour stack.
- detail: Restores the top of the stack, or a named slot via CSI Pm # Q. Unlike the reset OSCs (OSC 104 / 110-119), which restore the configured default and clobber user runtime tweaks, XTPOPCOLORS restores the runtime value captured at the matching push, leaving outer-layer changes intact. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/xt-colors>

### CSI XTREPORTCOLORS

```text
CSI # R
```

- mnemonic: `XTREPORTCOLORS`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Report the colour stack depth.
- detail: Replies over the PTY with CSI ? <depth> # Q so a program can verify the stack height; the reply lands on the shell's stdin. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/csi/xt-colors>

### CSI XTWINOPS

```text
CSI <Ps> ; <Ps> ; <Ps> t
```

- mnemonic: `XTWINOPS`
- category: `csi`
- status: **partial** (◐)
- params: `Ps` default: not stated
- summary: Window manipulation / size reports - read-only size reports only.
- detail: Bytes 0x1B 0x5B <Ps...> 0x74. The first parameter selects an operation. Otty implements only the read-only size reports: Ps 14 report text-area size in pixels -> replies CSI 4 ; height ; width t (answered); Ps 16 report cell size in pixels -> replies CSI 6 ; height ; width t (answered); Ps 18 report text-area size in characters -> would reply CSI 8 ; rows ; cols t but is intentionally not answered (it is redundant with TIOCGWINSZ / $LINES / $COLUMNS / SIGWINCH, and through a multiplexer the reply leaks to the inner shell and is echoed as \e[8;34;126t garbage); Ps 1-9, 10, 11, 13, 19, 20-24 (deiconify / move / resize / raise / lower / maximize / fullscreen / report position / title-stack) are ignored by design, because a remote program should not be able to move or resize the window. Replies use xterm's height ; width parameter order and are gated on a known cell size, so a query before the first layout gets no answer. Doc mark: Otty support ◐ - read-only size reports only.
- source: <https://docs.otty.sh/vt/csi/xtwinops>

### CSI c Primary DA

```text
CSI c
```

- mnemonic: `DA1`
- category: `csi`
- status: **implemented** (✓)
- params: `0` default `0`
- summary: Primary Device Attributes - answered synchronously over the PTY.
- detail: Query forms CSI c / CSI 0 c. DOC CONFLICT (unresolved in the crawled text): External Protocols states Otty '✓ (responds ?62;22c)', while the '$TERM and Identification' page states DA1 replies CSI ? 6 c ("I'm a VT102-class terminal." - compact and conservative). Both statements are recorded verbatim; the docs do not reconcile them, and this crawl contains no source to arbitrate. DA3 (tertiary attributes) is not implemented - Otty sends no reply, which the docs describe as the correct behaviour for an unsupported query. Terminal Comparison marks Primary DA ✓.
- source: <https://docs.otty.sh/vt/external-protocols>

### CSI > c Secondary DA

```text
CSI > c
```

- mnemonic: `DA2`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Secondary Device Attributes - replies with model, version and ROM slot.
- detail: Otty's reply is CSI > 0 ; <version> ; 1 c - model ID 0 (generic), build version, ROM slot 1. The <version> is the build version encoded as a single integer: major x 10000 + minor x 100 + patch, with any -rc / -beta suffix dropped first, so a 1.0.2 build reports 10002. This matches the encoding xterm and Alacritty use, so version-gating logic written for them works unchanged. Doc marks: External Protocols ✓, Terminal Comparison ✓.
- source: <https://docs.otty.sh/vt/external-protocols>

### CSI > q XTVERSION

```text
CSI > q
```

- mnemonic: `XTVERSION`
- category: `csi`
- status: **implemented** (✓)
- params: none
- summary: Terminal version probe - replies with a DCS string.
- detail: Otty replies DCS > | otty(<version>) ST, transmitted as ESC P > | otty(<version>) ESC \ - the reply is terminated with the 7-bit ST so it survives 8-bit-clean PTYs, and <version> is the plain build version, e.g. otty(1.0.2). Programs that recognize the otty( prefix can gate features (undercurl, truecolor, the Kitty keyboard protocol) without guessing from TERM. nvim, kakoune and others send this probe. Doc marks: External Protocols ✓ (replies DCS > | otty(<version>) ST) and dcs page 'XTVERSION reply'; Terminal Comparison ✓.
- source: <https://docs.otty.sh/vt/external-protocols>

### CSI DECRQM

```text
CSI ? Pn $ p
```

- mnemonic: `DECRQM`
- category: `csi`
- status: **implemented** (✓)
- params: `Pn (mode)` default: not stated
- summary: Mode status query - replies with DECRPM.
- detail: Query forms: CSI Pn $ p for ANSI modes and CSI ? Pn $ p for DEC private modes. Otty answers both, replying with DECRPM CSI ? Pn ; state $ y where state follows the DEC convention: 1 = set, 2 = reset (supported but currently off), 4 = not recognized. Every private mode in the Terminal Comparison table reports a truthful set/reset state, and any mode Otty does not implement replies 4 rather than pretending to support it. Nuance: ?2026's live synchronized-output state lives inside the parser and is not queryable, so Otty always answers 2 (reset) - exactly what nvim, zellij and Ghostty probe for to decide whether to emit BSU / ESU. Doc marks: Terminal Comparison Identification & queries Otty ✓.
- source: <https://docs.otty.sh/vt/terminal-comparison>

## OSC sequences (32)

### OSC 0 / 1 / 2

```text
OSC <Ps> ; <title> ST
```

- mnemonic: `OSC 0`
- category: `osc`
- status: **implemented** (✓)
- params: `Ps` default `0`
- summary: Set window title and/or icon title.
- detail: Ps 0 = set icon name and window title (most common), 1 = icon name only, 2 = window title only. Otty treats the icon and window titles as the same. Most shell rcs (zsh precmd, bash PROMPT_COMMAND) emit OSC 0 on every prompt to keep the tab label informative, and Otty uses the title for the tab label. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/osc/osc-0-2>

### OSC 4

```text
OSC 4 ; <index> ; rgb:R/G/B ST (set) - OSC 4 ; <index> ; ? ST (query)
```

- mnemonic: `OSC 4`
- category: `osc`
- status: **implemented** (✓)
- params: `index` default: not stated
- summary: Palette set / query over the full 0-255 range.
- detail: Sets or queries one entry of the 256-colour palette - the same indices SGR 38;5;<n> / SGR 48;5;<n> reference. Ranges: 0-15 ANSI 16 (standard palette colours); 16-231 a 6x6x6 RGB cube; 232-255 a 24-step grayscale ramp. Accepted colour specs: rgb:RR/GG/BB, rgb:R/G/B, rgb:RRRR/GGGG/BBBB, #RGB, #RRGGBB, #RRRGGGBBB, #RRRRGGGGBBBB. A query is <spec> = ? and the terminal replies with the current value (on stdin). Reset companion: OSC 104. Doc mark: Otty support ✓ (full 0-255 range).
- source: <https://docs.otty.sh/vt/osc/osc-4>

### OSC 5

```text
OSC 5 ; <slot> ; rgb:R/G/B ST (set) - OSC 5 ; <slot> ; ? ST (query)
```

- mnemonic: `OSC 5`
- category: `osc`
- status: **implemented** (✓)
- params: `slot` default `1`
- summary: Special colour set - slots that decorate one SGR attribute globally.
- detail: Slots: 0 bold-text colour; 1 underline colour (default; SGR 58 still overrides per cell); 2 blink colour; 3 reverse-video colour; 4 italic-text colour. Parsed and stored; only slot 1 is consulted - Otty treats the OSC 5 underline colour as the default underline tint applied when the cell has no explicit SGR 58 (per-cell SGR 58 wins, which is xterm's documented precedence). The bold / blink / reverse / italic slots are stored but not consulted by the renderer (bold uses its own bold-color config plus bold-is-bright, italics use the per-cell SGR fg, and Otty does not blink); the values remain queryable in the per-session overrides table. Reset companion: OSC 105. Doc mark: Otty support ✓ parsed (underline applied).
- source: <https://docs.otty.sh/vt/osc/osc-5>

### OSC 7

```text
OSC 7 ; file://<host>/<path> ST
```

- mnemonic: `OSC 7`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Report the pane's current working directory.
- detail: Stored per pane and used to resolve relative paths in Files and Links, as the cwd anchor of newly-spawned panes (split / new tab), and for the cwd shown in the Window, Tab and Split command menu. The payload is a file:// URI; <host> should be the machine hostname, which over SSH lets the terminal differentiate local from remote paths, and <path> must be percent-encoded. Otty's shell integration emits it automatically. Doc mark: Otty support ✓; Terminal Comparison CWD report ✓.
- source: <https://docs.otty.sh/vt/osc/osc-7>

### OSC 8

```text
OSC 8 ; <params> ; <uri> ST then text then OSC 8 ; ; ST
```

- mnemonic: `OSC 8`
- category: `osc`
- status: **implemented** (✓)
- params: `params` default ``
- summary: Hyperlink - wraps a span of text in a clickable link.
- detail: The closing sequence OSC 8 ; ; ST is required, with empty params and empty URI. <params> is a colon-separated key=value list; the only standard key today is id (an optional identifier to group adjacent hyperlinked cells), and unknown keys are ignored. The target stays attached to the cells until they are overwritten, so a hyperlinked path stays clickable as it scrolls through the buffer, and links survive scrolling into history. A new OSC 8 ; without first closing the previous link replaces it on subsequent cells. ST may be ESC \ or BEL - both accepted. In Otty the text is underlined and styled as a link and Cmd-click opens the target; OSC 8 is explicit whereas path detection is automatic. Doc mark: Otty support ✓ Implemented.
- source: <https://docs.otty.sh/vt/osc/osc-8>

### OSC 9

```text
OSC 9 ; <body> ST
```

- mnemonic: `OSC 9`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Desktop notification with body text only (iTerm2 style).
- detail: Posts a system notification with body text only; Otty maps it to a native macOS notification (and a Windows toast, when shipped). For a title-bearing notification use OSC 777; for chunked title/body, urgency levels, base64 payloads and replace-by-id use OSC 99. OSC 9;4 is a distinct sub-protocol on the same OSC number, dispatched on the leading 4. Doc marks: External Protocols ✓, Terminal Comparison ✓.
- source: <https://docs.otty.sh/vt/osc/osc-9>

### OSC 9;4

```text
OSC 9 ; 4 ; <state> [ ; <value> [ ; <tag> ] ] ST
```

- mnemonic: `OSC 9;4`
- category: `osc`
- status: **implemented** (✓)
- params: `state` default: not stated; `value` default: not stated; `tag` default: not stated
- summary: Task progress / state (ConEmu).
- detail: States: 0 clear / no progress (resets to 0); 1 in progress with <pct> 0-100; 2 error, optionally held at a percentage; 3 indeterminate spinner (percentage unknown); 4 paused / warning - ignored by Otty with no special reaction; 5 Otty extension - finished with exit code <exit>, where a trailing watch tag marks a finish from otty watch so it routes to its own notification toggle. The dispatcher branches on the 4; prefix, so this is a distinct sub-protocol from the OSC 9 desktop notification. Otty maps it to a tab badge and an animated Dock icon. Doc marks: External Protocols ✓, Terminal Comparison ConEmu progress ✓.
- source: <https://docs.otty.sh/vt/osc/osc-9-4>

### OSC 10

```text
OSC 10 ; rgb:R/G/B ST (set) - OSC 10 ; ? ST (query)
```

- mnemonic: `OSC 10`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Set / query the default text foreground colour.
- detail: Applied to default-fg cells; affects every default-colour cell on the next frame, while pre-existing cells already painted with explicit SGR colours keep their colour. Query form OSC 10 ; ? ST replies in the canonical xterm format OSC 10 ; rgb:HHHH/HHHH/HHHH ST (16-bit per channel, replicated from 8 bits). Accepted colour formats: rgb:RR/GG/BB (8-bit hex), rgb:R/G/B (4-bit hex shorthand, f/0/0 -> 0xff 0x00 0x00), rgb:RRRR/GGGG/BBBB (16-bit canonical, Otty truncates to the high byte), and #RGB / #RRGGBB / #RRRGGGBBB / #RRRRGGGGBBBB. Reset companion: OSC 110. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 11

```text
OSC 11 ; rgb:R/G/B ST (set) - OSC 11 ; ? ST (query)
```

- mnemonic: `OSC 11`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Set / query the default text background colour.
- detail: Applied to default-bg cells plus the window. Query replies in the canonical xterm format OSC 11 ; rgb:HHHH/HHHH/HHHH ST (16-bit per channel, replicated from 8 bits) on the shell's stdin. Colour formats accepted are the same family as OSC 10. Reset companion: OSC 111. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 12

```text
OSC 12 ; rgb:R/G/B ST (set) - OSC 12 ; ? ST (query)
```

- mnemonic: `OSC 12`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Set / query the cursor colour.
- detail: Overrides the cursor-color config. Query replies in the canonical xterm format OSC 12 ; rgb:HHHH/HHHH/HHHH ST (16-bit per channel, replicated from 8 bits). Colour formats accepted are the same family as OSC 10. Reset companion: OSC 112. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 13

```text
OSC 13 ; rgb:R/G/B ST (set) - OSC 13 ; ? ST (query)
```

- mnemonic: `OSC 13`
- category: `osc`
- status: **partial** (◐)
- params: none
- summary: Set / query the mouse pointer foreground colour - stored but not applied on macOS.
- detail: The value is stored but has no visible effect on macOS, because macOS offers no way to recolour the mouse pointer, so the values are inert there. Query replies in the canonical xterm format. Doc marks: Sequence Reference 'OSC 13 / 14 | Mouse pointer fg / bg | ✓ parsed (no macOS effect)'; the OSC 10-19 page support line states 'OSC 13 / 14 parsed, not applied on macOS'. Recorded as partial because the parsed state is confirmed while the visible effect is explicitly absent.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 14

```text
OSC 14 ; rgb:R/G/B ST (set) - OSC 14 ; ? ST (query)
```

- mnemonic: `OSC 14`
- category: `osc`
- status: **partial** (◐)
- params: none
- summary: Set / query the mouse pointer background colour - stored but not applied on macOS.
- detail: Stored but not applied: macOS has no way to recolour the mouse pointer, so the value is inert there. Query replies in the canonical xterm format. Doc marks: Sequence Reference 'OSC 13 / 14 ... ✓ parsed (no macOS effect)'; the OSC 10-19 page support line states 'OSC 13 / 14 parsed, not applied on macOS'. Recorded as partial for the same reason as OSC 13.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 15

```text
OSC 15 ; rgb:R/G/B ST (set) - OSC 15 ; ? ST (query)
```

- mnemonic: `OSC 15`
- category: `osc`
- status: **partial** (◐)
- params: none
- summary: Set / query the Tektronix foreground colour - stored, not rendered.
- detail: Slot: Tektronix foreground. The OSC 10-19 page support line states 'OSC 15 / 16 / 18 parsed, not rendered (Tek 4014 is a dead protocol)'; the slot table says 'stored, not rendered'. OSC 15 / 16 / 18 are absent from the Sequence Reference OSC table and from the Terminal Comparison vendor-extension matrix, so the only status evidence is the page support line - hence partial (parsed, no rendering).
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 16

```text
OSC 16 ; rgb:R/G/B ST (set) - OSC 16 ; ? ST (query)
```

- mnemonic: `OSC 16`
- category: `osc`
- status: **partial** (◐)
- params: none
- summary: Set / query the Tektronix cursor colour - stored, not rendered.
- detail: Slot: Tektronix cursor. The page support line states 'OSC 15 / 16 / 18 parsed, not rendered (Tek 4014 is a dead protocol)'; the slot table says 'stored, not rendered'. No Sequence Reference row and no comparison-matrix row exists for OSC 16.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 17

```text
OSC 17 ; rgb:R/G/B ST (set) - OSC 17 ; ? ST (query)
```

- mnemonic: `OSC 17`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Set / query the selection (highlight) background colour.
- detail: Overrides selection-background. It only affects the selection overlay - it does not change SGR 48 / 38 cell colours. Doc marks: Otty support ✓ 17 / 19; Sequence Reference 'OSC 17 / 19 | Selection background / foreground | ✓'. Reset companion: OSC 117.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 18

```text
OSC 18 ; rgb:R/G/B ST (set) - OSC 18 ; ? ST (query)
```

- mnemonic: `OSC 18`
- category: `osc`
- status: **partial** (◐)
- params: none
- summary: Set / query the Tektronix background colour - stored, not rendered.
- detail: Slot: Tektronix background. The page support line states 'OSC 15 / 16 / 18 parsed, not rendered (Tek 4014 is a dead protocol)'; the slot table says 'stored, not rendered'. No Sequence Reference row and no comparison-matrix row exists for OSC 18.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 19

```text
OSC 19 ; rgb:R/G/B ST (set) - OSC 19 ; ? ST (query)
```

- mnemonic: `OSC 19`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Set / query the selection (highlight) foreground colour.
- detail: Overrides selection-foreground; it only affects the selection overlay and does not change SGR cell colours. Doc marks: Otty support ✓ 17 / 19; Sequence Reference 'OSC 17 / 19 | Selection background / foreground | ✓'. Reset companion: OSC 119.
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 22

```text
OSC 22 ; <shape> ST
```

- mnemonic: `OSC 22`
- category: `osc`
- status: **implemented** (✓)
- params: `shape` default `default`
- summary: Mouse cursor shape - a standard CSS cursor name.
- detail: Introduced by xterm, extended and popularized by kitty. Otty recognizes the full set of CSS cursor names; shapes with no exact macOS equivalent fall back to a sensible neighbour (diagonal resize -> horizontal resize, because macOS has no diagonal resize cursor), and passing default (or an empty value) hands control back to Otty's own cursor policy (I-beam over text, pointing hand over hyperlinks, crosshair with Option held). Documented mappings include pointer (pointing hand), text (I-beam), vertical-text, crosshair/cell, wait/progress (arrow - no spinner cursor), help/context-menu, move/all-scroll (closed hand), grab (open hand), grabbing, no-drop/not-allowed, alias, copy, the resize families (n/s/ns/row, e/w/ew/col, diagonal ne/sw/nw/se/nesw/nwse), and zoom-in/zoom-out (arrow). Behaviour notes: the requested shape persists across PTY chunks and across main / alternate screen switches - only default (or an empty payload) resets it, so programs that take over the cursor should restore it before exiting; when the program also subscribes to mouse reporting (DECSET 1000/1002/1003) Otty normally swaps in the arrow cursor, and an active OSC 22 shape overrides that fallback, matching kitty / Ghostty; Cmd-hover (open-link preview) always wins because it expresses local user intent. Doc mark: Otty support ✓.
- source: <https://docs.otty.sh/vt/osc/osc-22>

### OSC 52

```text
OSC 52 ; <selection> ; <base64-payload> ST
```

- mnemonic: `OSC 52`
- category: `osc`
- status: **implemented** (✓)
- params: `selection` default `c`
- summary: Clipboard manipulation - read or write the system clipboard.
- detail: Selections: c the standard clipboard (most common), p the primary selection (Linux), anything else implementation-defined. Payload: base64 text writes to the selected clipboard; ? queries, and the terminal replies with the current value (encoded). Off by default in Otty and enabled with clipboard-osc-52 = write | read-write | off; read-write is required for query, since a script in a remote shell can otherwise exfiltrate clipboard contents. Doc marks: External Protocols ✓ (opt-in); Terminal Comparison ✓.
- source: <https://docs.otty.sh/vt/osc/osc-52>

### OSC 88

```text
OSC 88 ; <op> [ ; <key>=<value> ]... ST
```

- mnemonic: `OSC 88`
- category: `osc`
- status: **implemented** (✓)
- params: `op` default: not stated
- summary: Terminal Resume Protocol (TRP) - reports how a pane's program should be relaunched.
- detail: Otty-specific proposal (v1), gated by the Terminal Resume Protocol setting which is on by default. Operations: arm declare / fully replace the pane's resume spec (requires cmd; idempotent); clear withdraw the spec on a clean exit; query (optional) OSC 88 ; query ST -> terminal replies OSC 88 ; supported ; v=<max> ST. Fields for arm: cmd (required, base64 - the relaunch head and the verification token, whose basename must match the broadcasting process); args (base64 relaunch tail appended after cmd; absent = run cmd alone); self_repaint (0 / 1, default 0 - 1 means the resumed program repaints its own screen and the terminal SHOULD skip its visual restore for that pane); cwd (base64; absent = the terminal's last-known cwd for the pane); title (base64 hint, lower priority than OSC 0/2); v (integer protocol version, default 1; unknown fields MUST be ignored). Wire format: ST is ESC \ (preferred) or BEL, ; separates params, values are base64(UTF-8) so the alphabet can never break framing, and key=value splits on the first = so trailing base64 padding stays in the value. Terminal-side duties: parse and store per pane (arm is idempotent, last-write-wins), verify the arming program before persisting (recommended mechanism: confirm a live process in the pane's tree whose argv[0] basename equals basename(cmd); Otty uses its 1 Hz process reaper, the same one that powers agent detection), re-execute cmd followed by args in cwd on cold restore, execute only the binary named by cmd, surface a user-visible reversible action (Otty shows a 5-second Resumed: ... [Undo] toast), and offer a per-user / per-binary opt-out. STATUS NOTE (doc conflict): the OSC 88 page states Otty support ✓ while Terminal Comparison marks OSC 88 ◐ experimental ('the handshake is wired up - Otty parses arm / clear / query and answers query with OSC 88 ; supported ; v=1 ST - but persisting a pane's resume spec and replaying it end-to-end is still in progress'). The per-page mark is used here; the ◐ qualification is recorded in this detail.
- source: <https://docs.otty.sh/vt/osc/osc-88>

### OSC 99

```text
OSC 99 ; <metadata> ; <payload> ST
```

- mnemonic: `OSC 99`
- category: `osc`
- status: **implemented** (✓)
- params: `metadata` default ``; `payload` default ``
- summary: Kitty desktop notification protocol.
- detail: Richer alternative to OSC 9 / OSC 777, introduced by kitty; Otty maps every fully-assembled OSC 99 to a native macOS notification. Metadata is a colon-separated key=value list parsed before the payload separator: i=<id> notification id (chunks with the same id are joined; later notifications with the same id replace earlier ones) - the empty id i= is the anonymous slot, with only one anonymous notification in flight at a time; d=<0|1> 0 = more chunks follow, 1 (default) = final fragment; p=<title|body|?> payload type (title is the default, ? is a capability query); e=<0|1> 1 = payload is base64-encoded; u=<0|1|2> urgency low / normal (default) / critical; and n, o, a, f, g, s, w, c (application name / close events / actions / focus / icon / sound / urls / category) which are parsed and ignored because NSUserNotification has no equivalent slot. Capability query reply: the program sends ESC ] 99 ; i=ping:p=? ; ESC \ and Otty writes back ESC ] 99 ; i=ping:p=? ; ok ESC \ inline, so the program can feature-detect without timing out. Behaviour notes: Otty caps the OSC accumulator at 8 KiB per chunk (kitty's recommended chunk size) and drops oversized fragments silently; chunked notifications are emitted only when a fragment with d=1 (or no d) arrives, so programs that forget to finalise leave the partial in memory and Otty discards it on session close; terminals that drop OSC 99 silently mean the recommended pattern is to send OSC 99 first and fall back to OSC 9 / OSC 777. Doc mark: Otty support ✓ (title, body, urgency, base64 payloads, multi-chunk reassembly, capability query).
- source: <https://docs.otty.sh/vt/osc/osc-99>

### OSC 104

```text
OSC 104 ; <index> ST
```

- mnemonic: `OSC 104`
- category: `osc`
- status: **implemented** (✓)
- params: `index` default: not stated
- summary: Palette reset.
- detail: OSC 104 ; 1 resets index 1, and a bare OSC 104 resets all 256 palette entries. It restores the configured default, which means it also overwrites user-applied runtime tweaks - XTPOPCOLORS is preferred when a program wants to restore only its own changes. Doc marks: Sequence Reference 'OSC 104 | Palette reset (256-color) | ✓'; Terminal Comparison 'Indexed palette | OSC 4 / 104 | ✓'. NOTE: the dedicated OSC 104 / 105 / 110-119 'Color Reset' page is referenced by the sidebar and by the OSC 99 / OSC 133 pager links but is NOT present in this crawl; the reset semantics here come from the OSC 4 page, the OSC 5 page and the Sequence Reference table.
- source: <https://docs.otty.sh/vt/osc/osc-4>

### OSC 105

```text
OSC 105 [ ; <slot> ... ] ST
```

- mnemonic: `OSC 105`
- category: `osc`
- status: **implemented** (✓)
- params: `slot` default: not stated
- summary: Reset special colors (the OSC 5 slots).
- detail: OSC 105 ST resets all five special-colour slots; OSC 105 ; 1 ; 4 ST resets just slots 1 and 4. Doc marks: Sequence Reference 'OSC 105 | Reset special colors | ✓'; documented on the OSC 5 page as 'The companion sequence is OSC 105'.
- source: <https://docs.otty.sh/vt/osc/osc-5>

### OSC 110

```text
OSC 110 ST
```

- mnemonic: `OSC 110`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Reset the default text foreground colour.
- detail: Part of the reset family that restores configured defaults (and therefore overwrites user runtime tweaks too). Doc marks: Sequence Reference 'OSC 110 / 111 / 112 | Reset fg / bg / cursor | ✓'. The dedicated Color Reset page is not present in this crawl (see OSC 104).
- source: <https://docs.otty.sh/vt/sequence-reference>

### OSC 111

```text
OSC 111 ST
```

- mnemonic: `OSC 111`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Reset the default text background colour.
- detail: Part of the reset family that restores configured defaults (and therefore overwrites user runtime tweaks too). Doc marks: Sequence Reference 'OSC 110 / 111 / 112 | Reset fg / bg / cursor | ✓'. The dedicated Color Reset page is not present in this crawl (see OSC 104).
- source: <https://docs.otty.sh/vt/sequence-reference>

### OSC 112

```text
OSC 112 ST
```

- mnemonic: `OSC 112`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Reset the cursor colour.
- detail: Documented by example on the OSC 10-19 page: "printf '\e]112\e\\'   # OSC 112 - reset cursor color", used to restore the cursor colour after a program changed it with OSC 12. Doc marks: Sequence Reference 'OSC 110 / 111 / 112 | Reset fg / bg / cursor | ✓'. The dedicated Color Reset page is not present in this crawl (see OSC 104).
- source: <https://docs.otty.sh/vt/osc/osc-10-19>

### OSC 117

```text
OSC 117 ST
```

- mnemonic: `OSC 117`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Reset the selection (highlight) background colour.
- detail: Doc marks: Sequence Reference 'OSC 117 / 119 | Reset selection bg / fg | ✓'. Companion of OSC 17, which overrides selection-background. The dedicated Color Reset page is not present in this crawl (see OSC 104).
- source: <https://docs.otty.sh/vt/sequence-reference>

### OSC 119

```text
OSC 119 ST
```

- mnemonic: `OSC 119`
- category: `osc`
- status: **implemented** (✓)
- params: none
- summary: Reset the selection (highlight) foreground colour.
- detail: Doc marks: Sequence Reference 'OSC 117 / 119 | Reset selection bg / fg | ✓'. Companion of OSC 19, which overrides selection-foreground. The dedicated Color Reset page is not present in this crawl (see OSC 104).
- source: <https://docs.otty.sh/vt/sequence-reference>

### OSC 133

```text
OSC 133 ; <kind> [ ; <params> ] ST
```

- mnemonic: `OSC 133`
- category: `osc`
- status: **implemented** (✓)
- params: `kind` default: not stated
- summary: Shell integration (FTCS) prompt marks.
- detail: Marks the boundaries of prompts and commands in the byte stream; the terminal uses the marks for command outlines, jump to previous / next command, exit-status indicators and intelligent paste. Kinds: A prompt start (emitted just before the prompt is drawn); B prompt end / command start (after the prompt, before input - the input zone; emitted by zsh's prompt builder when PROMPT_SUBST is on); C command output start (when the command begins executing); D ; <exit> command end with exit code (emitted on completion); L optional, marks the start of a user input prompt within a tool. This is the FinalTerm shell integration protocol as adopted by iTerm2, WezTerm, Kitty, VS Code and now Otty; Otty's shell installer (otty shell install) writes the snippets. Doc marks: External Protocols ✓ ('follows the FinalTerm FTCS spec as adopted by iTerm2'), Terminal Comparison ✓.
- source: <https://docs.otty.sh/vt/osc/osc-133>

### OSC 633

```text
OSC 633
```

- mnemonic: `OSC 633`
- category: `osc`
- status: **missing** (☐)
- params: none
- summary: VSCode shell integration - not parsed.
- detail: VSCode's integrated-terminal scripts emit OSC 633 in parallel to OSC 133. Not parsed by Otty, so users running Set-PSReadLineOption-style integrations see the bracketed escapes echoed to the screen. Doc marks: Terminal Comparison vendor extensions row 'VSCode shell integration | OSC 633 | VSCode | ❌ for every terminal listed including Otty'; the 'Notable gaps' section states it is not parsed. The docs give only the OSC number, not the parameter syntax.
- source: <https://docs.otty.sh/vt/terminal-comparison>

### OSC 777

```text
OSC 777 ; notify ; <title> ; <body> ST
```

- mnemonic: `OSC 777`
- category: `osc`
- status: **implemented** (✓)
- params: `notify` default `notify`
- summary: Desktop notification with a title (urxvt style).
- detail: Like OSC 9 but with a separate title field; originated with rxvt-unicode. Otty maps it to a native macOS notification with <title> shown as the notification title. The notify literal subcommand is required (OSC 777 supports other historical subcommands, none widely used). For urgency levels, chunked transmission, replace-by-id or base64 payloads use OSC 99 instead. Doc marks: External Protocols ✓ (Notifications rich), Terminal Comparison ✓.
- source: <https://docs.otty.sh/vt/osc/osc-777>

### OSC 1337

```text
OSC 1337 ; <Command>[=<args>] ST
```

- mnemonic: `OSC 1337`
- category: `osc`
- status: **partial** (◐)
- params: `Command` default: not stated
- summary: iTerm2 proprietary extensions - only the inline-image command is implemented.
- detail: iTerm2's umbrella for a family of proprietary commands sharing the number 1337 and dispatched by a leading command keyword. Otty implements only the inline-image command: File = <args> : <base64-data>, plus the chunked MultipartFile / FilePart / FileEnd variant (header with the same args, repeated base64 chunks, then assemble + render). Every other OSC 1337 command is parsed-and-ignored - recognized syntax, no effect - namely: SetProfile, CopyToClipboard / EndCopy, Copy, SetColors, SetUserVar, ReportVariable, SetMark, CursorShape, StealFocus, ClearScrollback, CurrentDir (Otty uses OSC 7 instead), RemoteHost, RequestAttention, ReportCellSize, UnicodeVersion, HighlightCursorLine, SetBackgroundImageFile, AddAnnotation / AddHiddenAnnotation, SetKeyLabel / PushKeyLabels / PopKeyLabels, RequestUpload, ShellIntegrationVersion, ClearCapturedOutput, Disinter, Custom, Block / UpdateBlock, Button, OpenURL. File arguments are a semicolon-separated key=value list before the colon: name (base64 filename, shown in tooltips), size (int, total byte size of the encoded data), inline (0 or 1; if 1 display inline, otherwise download - Otty always renders inline), width and height (auto / <N> / <N>px / <N>%), preserveAspectRatio (0 / 1, scale to fit while preserving aspect ratio). Supported image formats: PNG, JPEG, GIF (still). Doc marks: page 'Partial - inline image (File / MultipartFile) only'; Terminal Comparison '◐ multipart only', with the single-segment File= form (OSC 1337 ; File=<attrs>:<data> ST - what imgcat and most shell scripts emit) dropped, so larger multipart images work but the common small case does not.
- source: <https://docs.otty.sh/vt/osc/osc-1337>

## DCS sequences (4)

### DCS DECRQSS (cursor style)

```text
DCS $ q SP q ST
```

- mnemonic: `DECRQSS`
- category: `dcs`
- status: **implemented** (✓)
- params: `D...D (selector)` default `SP q`
- summary: Request Selection or Setting - answers the cursor-style query only.
- detail: Request bytes 0x1B 0x50 0x24 0x71 0x20 0x71 0x1B 0x5C (DCS $ q SP q ST); reply DCS 1 $ r <Ps> SP q ST. <Ps> is the current cursor code matching DECSCUSR: 1 block blinking, 2 block steady, 3 underline blinking, 4 underline steady, 5 beam blinking, 6 beam steady. The reply reports the cursor style the VT currently holds - a program's DECSCUSR override if one is active, otherwise the configured default. tmux and some TUIs probe this on startup to learn the cursor shape and restore it later. General reply form: DCS Ps $ r D...D ST with Ps = 1 (valid) or 0 (unsupported). Because vte does not dispatch general DCS payloads, Otty scans the byte stream for the ones it answers. Doc mark: Otty support ◑ Partial - cursor style only (Terminal Comparison also marks DECRQSS ◑).
- source: <https://docs.otty.sh/vt/dcs/decrqss>

### DCS DECRQSS (other targets)

```text
DCS $ q ... ST
```

- mnemonic: `DECRQSS`
- category: `dcs`
- status: **missing** (☐)
- params: none
- summary: DECRQSS targeting any control function other than the cursor style is not answered.
- detail: Other DECRQSS targets - SGR (m), the scrolling region (r), and so on - get no reply because vte does not dispatch general DCS payloads; programs probing them fall back to their defaults. Doc mark: ❌ (Sequence Reference → DCS; Terminal Comparison 'Notable gaps').
- source: <https://docs.otty.sh/vt/sequence-reference>

### DCS XTGETTCAP

```text
DCS + q ... ST
```

- mnemonic: `XTGETTCAP`
- category: `dcs`
- status: **missing** (☐)
- params: none
- summary: Terminfo capability query - not supported.
- detail: DCS + q ... ST asks the terminal for a terminfo capability. Not answered: vte does not dispatch general DCS payloads, so programs probing it get no reply and fall back to their defaults. Doc marks: ❌ in Sequence Reference → DCS and in Terminal Comparison (which also notes xterm, Kitty, WezTerm and Ghostty do answer it).
- source: <https://docs.otty.sh/vt/sequence-reference>

### DCS introducer

```text
ESC P
```

- mnemonic: `DCS`
- category: `dcs`
- status: **partial** (◐)
- params: none
- summary: Device Control String introducer - frames structured payloads; most DCS-framed functions are inert.
- detail: Structure ESC P <params> <selector> <payload> ST, where ESC P may also be the 8-bit 0x90, the selector is intermediate + final bytes (e.g. $ q for DECRQSS), and ST is ESC \ (preferred) or 0x9C. vte does not dispatch general DCS payloads, so most DCS-framed functions are inert; Otty scans the byte stream for the ones it answers (the DECRQSS cursor-style query, and it emits DCS > | otty(<version>) ST in reply to CSI > q). Sixel graphics (DCS q) and XTGETTCAP (DCS + q) are not supported. Docs also note DCS is used by tmux passthrough.
- source: <https://docs.otty.sh/vt/dcs>

## External protocols (APC / DCS vendor protocols) (5)

### APC introducer

```text
ESC _
```

- mnemonic: `APC`
- category: `external`
- status: **partial** (◐)
- params: none
- summary: Application Program Command introducer - carries the Kitty graphics protocol.
- detail: ESC _ (Application Program Command). Listed both in the C0 page's Fe-class introducer table and in the VT Overview's DCS / APC / PM family row as being 'used by Kitty graphics'. The Kitty graphics protocol riding on APC is supported by Otty (✓ mostly); other APC payloads are private protocols and are not dispatched. There is no dedicated APC page or per-sequence Otty mark in the crawled docs.
- source: <https://docs.otty.sh/vt/external-protocols>

### PM introducer

```text
ESC ^
```

- mnemonic: `PM`
- category: `external`
- status: **partial** (◐)
- params: none
- summary: Privacy Message introducer - documented only as an introducer, with no stated behaviour.
- detail: ESC ^ (Privacy Message). Appears only as an Fe-class introducer in the C0 page's introducer table and in the VT Overview's 'DCS / APC / PM' family row ('used by ... various private protocols'). The crawled docs state no behaviour, no payload structure and no Otty support mark for PM. Recorded as partial on the grounds of the Terminal Comparison methodology where 'recognized syntax, no effect' / 'parsed without errors, behaviour incomplete' maps to ◐; behaviour itself could not be determined from the text.
- source: <https://docs.otty.sh/vt/c0>

### Kitty graphics protocol

```text
APC G ...
```

- mnemonic: `APC G`
- category: `external`
- status: **implemented** (✓)
- params: none
- summary: Inline images over APC, originated by Kitty - supported by Otty.
- detail: Introduced with APC (ESC _) with a payload beginning G. Doc marks: External Protocols '✓ (mostly)'; Terminal Comparison vendor extensions row 'Kitty graphics | APC G ...' Otty ✓; Sequence Reference has a 'Kitty graphics (DCS-class)' section stating 'Implemented via the Kitty graphics protocol. The full action / parameter table lives in that page.' CAVEAT: the page holding that action / parameter table (the docs' Images page) is referenced by Sequence Reference and by OSC 1337 but is NOT present in this crawl, so the per-action matrix (transmit / display / delete, a= / f= / m= / q= etc.) could not be transcribed. OSC 1337's Try it section also shows the demo command 'otty features try kitty-image ... using the Kitty graphics protocol (APC _G)'.
- source: <https://docs.otty.sh/vt/external-protocols>

### Sixel graphics

```text
DCS q ...
```

- mnemonic: `Sixel`
- category: `external`
- status: **missing** (☐)
- params: none
- summary: Sixel graphics - not parsed.
- detail: DCS q ... (origin DEC). Not parsed: the underlying vte parser treats DCS hooks as no-ops and the Otty fork adds no Sixel decoder. Doc marks: External Protocols '☐ planned'; Terminal Comparison Otty ❌; the DCS page states 'Sixel graphics and XTGETTCAP (DCS + q) are not supported.' Modern equivalents (Kitty graphics) cover the same ground; Sixel remains the only option for legacy tools (gnuplot, img2sixel, lsix).
- source: <https://docs.otty.sh/vt/external-protocols>

### tmux passthrough

```text
DCS ... ST
```

- mnemonic: `tmux passthrough`
- category: `external`
- status: **partial** (◐)
- params: none
- summary: tmux DCS passthrough envelope - partially supported.
- detail: DCS-framed passthrough of escape sequences through tmux. Doc marks: External Protocols table lists 'tmux passthrough | DCS | tmux | ◐'. No payload structure, parameter list or per-case behaviour is documented in the crawled text, so the exact scope of the ◐ could not be determined. Related doc evidence that the DCS introducer is recognised as a carrier: the C0 page lists ESC P / DCS as '(used by Kitty graphics, tmux passthrough)', and the OSC 88 page advises wrapping OSC 88 bytes in the multiplexer's passthrough envelope inside tmux / Zellij.
- source: <https://docs.otty.sh/vt/external-protocols>

## Corpus and gaps

Read for this table: every `vt__*.txt` page in the local crawl (68 files under `E:\study_serve\tmp\otty\clean`), including `vt__sequence-reference`, `vt__terminal-comparison` and `vt__overview`, plus the fuller non-truncated copies under `E:\study_serve\tmp\otty\pages` (the `clean` copies had dropped the bullet lists) and `terminal-features__term-value` for the DA1 / DA2 / XTVERSION replies.

Pages referenced by the documentation but **not present in the crawl**, so they could not be read or indexed:

- `vt/c0/so` - SO (Shift Out); only the C0 index row and the SI page's cross-reference exist here.
- `vt/csi/cuu` - CUU (Cursor Up).
- `vt/csi/cud` - CUD (Cursor Down).
- `vt/csi/dl` - DL (Delete Lines).
- `vt/osc/osc-104-105-110-119` - "Color Reset"; OSC 104 / 105 / 110-119 are covered here from the OSC 4 page, the OSC 5 page and the Sequence Reference table instead.
- the docs' Images page - holds the Kitty graphics action / parameter table (referenced by Sequence Reference and by OSC 1337).

Behaviour the crawled text does not settle is stated as a CAVEAT inside the affected entry rather than guessed. The affected sequences are: SO, ST, APC, PM, tmux passthrough, CUU, CUD, DL, SU, SD, CHA, VPA, TBC, DECSTBM, DECSLRM and LNM; and the DA1 reply, where two docs pages disagree with each other.
