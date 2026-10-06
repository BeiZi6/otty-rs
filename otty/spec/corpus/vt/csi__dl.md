<!-- source: https://docs.otty.sh/vt/csi/dl -->

# DL — Delete Lines [​](#dl-—-delete-lines)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI <n> M |
| Bytes | 0x1B 0x5B <n> 0x4D |
| Mnemonic | DL |
| Otty support | ✓ |

## Description [​](#description)

Deletes `<n>` rows starting at the cursor, scrolling subsequent rows up within the active region. New blank rows appear at the bottom.

Default `<n> = 1`. Effective only inside the scrolling region.

## Example [​](#example)
bash
```
printf '\e[3;1H\e[2M'     # at row 3 col 1, delete 2 rows
```

## Related [​](#related)

- [IL](/vt/csi/il)
