<!-- source: https://docs.otty.sh/vt/csi/decset -->

# DECSET — Set DEC Private Mode [​](#decset-—-set-dec-private-mode)
****``****``********
|  |  |
| --- | --- |
| Sequence | CSI ? <Pn>;<Pn>… h |
| Bytes | 0x1B 0x5B 0x3F <Pn> 0x68 |
| Mnemonic | DECSET |
| Otty support | ✓ |

## Description [​](#description)

Enables one or more DEC private modes. Each `<Pn>` is a numeric mode identifier. Multiple modes can be set in a single sequence with `;`-separation.
``````````````````````````````````
| <Pn> | Mode |
| --- | --- |
| 1 | Application cursor keys |
| 5 | Reverse video |
| 6 | Origin mode (DECOM) — CUP relative to scrolling region |
| 7 | Auto-wrap (DECAWM) |
| 12 | Blinking cursor |
| 25 | Cursor visible (DECTCEM) |
| 47 | Switch to alternate screen (legacy — no cursor save/clear) |
| 1000 | Send mouse X/Y on button event |
| 1002 | Use cell motion mouse tracking |
| 1003 | Use all motion mouse tracking |
| 1004 | Send focus in / out events |
| 1006 | SGR-style mouse reporting |
| 1047 | Switch to alternate screen (clears it when switched back off) |
| 1049 | Switch to alternate screen + save cursor |
| 2004 | Bracketed paste |
| 2026 | Synchronized output |

For a longer table see [Sequence Reference → Modes](/vt/sequence-reference). For how Otty's mode support compares to other terminals, see [Terminal Comparison](/vt/terminal-comparison).

## Example [​](#example)
bash
```
printf '\e[?2004h'     # enable bracketed paste
printf '\e[?25l'       # hide cursor (DECRST, same number)
printf '\e[?1049h'     # enter alternate screen (what vim issues at startup)
```

Try it

- `otty features try bracketed-paste` — toggles mode 2004 and demonstrates the `\e[200~ … \e[201~` paste wrapping.
- `otty features try sync-update` — exercises mode 2026 with two passes (with / without BSU/ESU) so you can spot tearing.
- `otty features try color-scheme-update` — subscribes to mode 2031 and prints the `\e[?997;Nn` reply pattern.

## Related [​](#related)

- [DECRST](/vt/csi/decrst) — disable the same modes.
- [Screen](/vt/concepts/screen) — alternate screen details.
