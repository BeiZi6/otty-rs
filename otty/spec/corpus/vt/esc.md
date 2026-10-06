<!-- source: https://docs.otty.sh/vt/esc -->

# ESC Sequences

Single-`ESC` sequences (not starting with `[`). These include DEC private sequences (`ESC #`, `ESC ( … )`), cursor save / restore, character-set selection, and the Fe-class introducers.

## Structure

```
ESC <intermediate>* <final>
```

- **Intermediates**: zero or more bytes in `0x20–0x2F` (`!`, `"`, `#`, `$`, …)
- **Final**: a byte in `0x30–0x7E`

For example:

- `ESC # 8` — DECALN. Intermediate `#` (0x23), final `8`.
- `ESC =` — DECKPAM (keypad application mode). No intermediates, final `=`.
- `ESC ( B` — designate G0 = US ASCII. Intermediate `(`, final `B`.

## Detail pages

- [DECALN — Screen Alignment Display](/vt/esc/decaln)

Sequences without dedicated pages are listed in [Sequence Reference](/vt/sequence-reference) → "ESC".

## See also

- [VT Overview](/vt/overview)
- [CSI](/vt/csi/) — the larger sibling category.
