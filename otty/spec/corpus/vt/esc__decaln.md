<!-- source: https://docs.otty.sh/vt/esc/decaln -->

# DECALN — Screen Alignment Display

|  |  |
| --- | --- |
| Sequence | ESC # 8 |
| Bytes | 0x1B 0x23 0x38 |
| Category | ESC (DEC private) |
| Mnemonic | DECALN |
| Otty support | ✓ Implemented |

## Description

Fills the entire visible screen with the character `E` (uppercase) using the default attributes. Introduced on DEC VT terminal hardware so technicians could verify screen geometry and convergence.

After execution:

- The cursor is moved to home (row 1, column 1).
- Every cell of the active viewport is replaced with `E`.
- Active SGR attributes (color, weight, etc.) are reset for the filled cells.
- Scrollback is **not** affected.

## Example

Send the sequence from any program:
bash
```
printf '\e#8'
```

The visible terminal area fills with `E` characters:

```
EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
…
```

You can drive it through the Otty CLI to test against a managed session:
bash
```
otty cli send-raw $'\e#8'
```

## Notes

- Only the visible viewport is affected; scrollback remains intact.
- DECALN does not change the cursor style or scrolling region.
- Combined with `DECSTR` (Soft Reset, `ESC [ ! p`), DECALN is the canonical way to put a terminal into a known-good state during tests.

## Related

- [ESC Sequences](/vt/esc/) — the rest of the single-ESC category.
- [DECSTR](/vt/csi/decstr) — soft reset, often paired with DECALN in tests.
- [RIS](/vt/esc/ris) — full reset to power-on state.
