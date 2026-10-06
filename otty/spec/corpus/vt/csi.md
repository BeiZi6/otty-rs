<!-- source: https://docs.otty.sh/vt/csi -->

# CSI Sequences

Control Sequence Introducer — `ESC [` followed by parameters and a final byte. The bulk of cursor movement, erase, mode setting, and SGR (color / style) lives here.

## Structure

```
ESC [ <parameters> <intermediates> <final-byte>
```

- **Parameters**: digits separated by `;` (or `:` for sub-parameters). Each parameter is a number.
- **Intermediates**: zero or more bytes in `0x20–0x2F` (e.g. `!`, `"`, `#`)
- **Final byte**: a byte in `0x40–0x7E` (e.g. `A`, `H`, `J`, `m`)

For example:

- `CSI 1 ; 31 m` — SGR, parameters `[1, 31]`, final `m`. Sets bold + red foreground.
- `CSI ? 25 h` — DECSET mode `25` (cursor visible). The `?` is a private-marker intermediate.

## Detail pages

- [CUP — Cursor Position](/vt/csi/cup)
- [DSR — Device Status Report](/vt/csi/dsr)
- [SGR — Select Graphic Rendition](/vt/csi/sgr)

Sequences without dedicated pages are listed in [Sequence Reference](/vt/sequence-reference) → "CSI".

## See also

- [VT Overview](/vt/overview)
- [OSC](/vt/osc/) — for window title, clipboard, hyperlinks.
