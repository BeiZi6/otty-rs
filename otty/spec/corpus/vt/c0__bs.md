<!-- source: https://docs.otty.sh/vt/c0/bs -->

# BS — Backspace [​](#bs-—-backspace)
****``********
|  |  |
| --- | --- |
| Byte | 0x08 |
| Mnemonic | BS |
| Otty support | ✓ |

## Description [​](#description)

Moves the cursor one cell to the left. **Does not erase** the cell underneath — to delete, follow with a space then another BS, or use [DCH](/vt/csi/dch).

At column 1, BS has no effect (no wrap). The `0x08` byte is what `Ctrl-H` produces, which is why many TUIs treat `Ctrl-H` and Backspace identically.

## Example [​](#example)
bash
```
printf 'abc\b\b!'        # writes "abc", backs up two cells, overwrites with "!"
                          # result: "a!c" (yes, the c remains — BS doesn't erase)
```

## Related [​](#related)

- [DEL](/vt/c0/del) — the byte that most modern keyboards send for Backspace.
- [DCH](/vt/csi/dch) — actually delete characters.
