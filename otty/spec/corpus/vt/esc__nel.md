<!-- source: https://docs.otty.sh/vt/esc/nel -->

# NEL — Next Line

|  |  |
| --- | --- |
| Sequence | ESC E |
| Bytes | 0x1B 0x45 |
| Mnemonic | NEL |
| Otty support | ✓ |

## Description

Moves the cursor to column 1 of the next row — equivalent to `CR LF` (`\r\n`) but as a single two-byte sequence.

## Example
bash
```
printf 'a\eEb'      # 'a' \r\n 'b'
```

## Related

- [CR](/vt/c0/cr), [LF](/vt/c0/lf), [IND](/vt/esc/ind)
