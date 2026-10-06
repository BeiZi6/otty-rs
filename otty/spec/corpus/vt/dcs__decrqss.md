<!-- source: https://docs.otty.sh/vt/dcs/decrqss -->

# DECRQSS — Request Selection or Setting

|  |  |
| --- | --- |
| Sequence | DCS $ q D…D ST |
| Bytes | 0x1B 0x50 0x24 0x71 <D…D> 0x1B 0x5C |
| Mnemonic | DECRQSS |
| Otty support | ◑ Partial — cursor style only |

## Description

DECRQSS lets a program **read back** the terminal's current setting for a control function, instead of assuming its own last write took effect. The request names the control function by its final (and intermediate) bytes in `D…D`; the terminal replies with the value it would emit for that function.

```
Request:  DCS $ q D…D ST
Reply:    DCS Ps $ r D…D ST      Ps = 1 (valid) or 0 (unsupported)
```

Otty answers the **cursor style** query (DECSCUSR, `CSI Ps SP q`). Its identifier is `SP q` (space, `q`), so the full exchange is:

|  |  |
| --- | --- |
| Request | DCS $ q SP q ST — bytes 1B 50 24 71 20 71 1B 5C |
| Reply | DCS 1 $ r <Ps> SP q ST |

`<Ps>` is the current cursor code, matching [DECSCUSR](/vt/csi/decscusr):

| Ps | Style |
| --- | --- |
| 1 | Block, blinking |
| 2 | Block, steady |
| 3 | Underline, blinking |
| 4 | Underline, steady |
| 5 | Beam, blinking |
| 6 | Beam, steady |

The reply reports the cursor style the VT currently holds — a program's DECSCUSR override if one is active, otherwise the configured default. `tmux` and some TUIs probe this on startup to learn the cursor shape and restore it later.

## What is not answered

Other DECRQSS targets — SGR (`m`), the scrolling region (`r`), and so on — and the sibling `DCS + q … ST` (XTGETTCAP, terminfo capability query) are **not** answered: `vte` doesn't dispatch general DCS payloads. Programs probing those get no reply and fall back to their defaults.

## Example
bash
```
# Ask for the current cursor style; the reply prints as an escape string.
printf '\eP$q q\e\\'
```

## Related

- [DECSCUSR — Set Cursor Style](/vt/csi/decscusr)
- [Cursor](/vt/concepts/cursor)
- [Terminal Comparison — query & reply support](/vt/terminal-comparison)
