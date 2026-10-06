<!-- source: https://docs.otty.sh/vt/c0/del -->

# DEL — Delete [​](#del-—-delete)
****``********
|  |  |
| --- | --- |
| Byte | 0x7F |
| Mnemonic | DEL |
| Otty support | ✓ (ignored as a display character) |

## Description [​](#description)

Originally used to "erase" a previously-typed character on paper tape. On modern terminals, DEL is **ignored when received from the host** — it doesn't move the cursor or erase anything.

DEL is, however, what most computer keyboards send when you press the Backspace key. Programs (shells, line editors) read DEL as "delete the previous character" — that interpretation is the program's, not the terminal's.

## Example [​](#example)

DEL is not normally emitted by host programs. To delete the cell to the left of the cursor, send [BS](/vt/c0/bs) then a space then [BS](/vt/c0/bs), or use [DCH](/vt/csi/dch).

## Related [​](#related)

- [BS](/vt/c0/bs) — the byte sent by `Ctrl-H`.
- [DCH](/vt/csi/dch) — actually delete characters.
