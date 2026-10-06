<!-- source: https://docs.otty.sh/vt/csi/decsca -->

# DECSCA — Select Character Protection Attribute

|  |  |
| --- | --- |
| Sequence | CSI <n> " q |
| Bytes | 0x1B 0x5B <n> 0x22 0x71 |
| Mnemonic | DECSCA |
| Otty support | ✓ |

## Description

Marks subsequently written characters as **protected** or **erasable**. Protected cells are skipped by the selective-erase sequences [DECSED](/vt/csi/ed) (`CSI ? <n> J`) and [DECSEL](/vt/csi/el) (`CSI ? <n> K`); ordinary [ED](/vt/csi/ed) / [EL](/vt/csi/el) ignore the attribute and erase everything.

| <n> | Effect |
| --- | --- |
| 0 | Characters are erasable (default) |
| 1 | Characters are protected from selective erase |
| 2 | Characters are erasable (same as 0) |

The attribute sticks to the cursor and applies to every character written until changed, so a typical use is "turn protection on, draw a fixed frame/labels, turn it off, then let the app selectively erase the editable region without disturbing the frame".

### Interaction with other state

- **Independent of SGR.** `SGR 0` (reset attributes) does **not** clear protection — only `DECSCA 0` / `2`, a soft reset ([DECSTR](/vt/csi/decstr)), or a full reset ([RIS](/vt/esc/ris)) do.
- **Saved with the cursor.** [DECSC](/vt/esc/decsc) / [DECRC](/vt/esc/decrc) save and restore the protection state along with the cursor.
- Erasing a protected cell through any path leaves the replacement cell **unprotected**.

## Example
bash
```
# Draw a protected label, then prove a selective erase spares it.
printf '\e[2J\e[H'
printf '\e[1"q'        # protect on
printf 'LABEL: '
printf '\e[0"q'        # protect off
printf 'value-to-erase'
printf '\e[?2J'        # DECSED — 'value-to-erase' clears, 'LABEL: ' remains
```

## Related

- [ED](/vt/csi/ed) — `CSI ? <n> J` is DECSED, the selective erase-in-display.
- [EL](/vt/csi/el) — `CSI ? <n> K` is DECSEL, the selective erase-in-line.
- [DECSTR](/vt/csi/decstr) — soft reset clears the protection attribute.
