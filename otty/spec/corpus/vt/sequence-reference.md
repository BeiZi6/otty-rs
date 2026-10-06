<!-- source: https://docs.otty.sh/vt/sequence-reference -->

# Sequence Reference

A searchable index of every VT sequence Otty interprets, sorted by category and final byte / mnemonic. Cells in the **Otty** column: ✓ implemented, ◐ partial, ☐ not yet.

## ESC
(/vt/esc/decaln)(/vt/esc/decsc)(/vt/esc/decrc)(/vt/esc/deckpam)(/vt/esc/deckpnm)(/vt/esc/ind)(/vt/esc/nel)(/vt/esc/ri)(/vt/esc/hts)(/vt/esc/ris)
| Sequence | Mnemonic | Description | Otty |
| --- | --- | --- | --- |
| ESC # 8 | DECALN | Screen Alignment Display | ✓ |
| ESC 7 | DECSC | Save Cursor | ✓ |
| ESC 8 | DECRC | Restore Cursor | ✓ |
| ESC = | DECKPAM | Keypad Application Mode | ✓ |
| ESC > | DECKPNM | Keypad Numeric Mode | ✓ |
| ESC D | IND | Index (down one line) | ✓ |
| ESC E | NEL | Next Line | ✓ |
| ESC M | RI | Reverse Index | ✓ |
| ESC H | HTS | Set Tab Stop | ✓ |
| ESC c | RIS | Full Reset | ✓ |

## CSI
(/vt/csi/cuu)(/vt/csi/cud)(/vt/csi/cuf)(/vt/csi/cub)(/vt/csi/cup)(/vt/csi/dsr)(/vt/csi/dsr)(/vt/csi/ed)(/vt/csi/ed)[](/vt/csi/decsca)(/vt/csi/el)(/vt/csi/el)(/vt/csi/sgr)(/vt/csi/ich)(/vt/csi/il)(/vt/csi/dl)(/vt/csi/dch)(/vt/csi/decset)(/vt/csi/decrst)(/vt/csi/decstr)(/vt/csi/decsca)(/vt/csi/decscusr)(/vt/csi/xt-colors)(/vt/csi/xt-colors)(/vt/csi/xt-colors)(/vt/csi/xtwinops)
| Sequence | Mnemonic | Description | Otty |
| --- | --- | --- | --- |
| CSI <n> A | CUU | Cursor Up | ✓ |
| CSI <n> B | CUD | Cursor Down | ✓ |
| CSI <n> C | CUF | Cursor Forward | ✓ |
| CSI <n> D | CUB | Cursor Back | ✓ |
| CSI <r>;<c> H | CUP | Cursor Position | ✓ |
| CSI 5 n | DSR | Device Status Report → CSI 0 n | ✓ |
| CSI 6 n | DSR | Cursor Position Report → CSI <row>;<col> R (DECXCPR CSI ? 6 n not implemented) | ◐ |
| CSI <n> J | ED | Erase in Display (n=0/1/2/3 standard, n=22 kitty scroll-then-erase) | ✓ |
| CSI ? <n> J | DECSED | Selective Erase in Display (skips DECSCA-protected cells) | ✓ |
| CSI <n> K | EL | Erase in Line | ✓ |
| CSI ? <n> K | DECSEL | Selective Erase in Line (skips protected cells) | ✓ |
| CSI <p>;… m | SGR | Select Graphic Rendition | ✓ |
| CSI <n> @ | ICH | Insert Characters | ✓ |
| CSI <n> L | IL | Insert Lines | ✓ |
| CSI <n> M | DL | Delete Lines | ✓ |
| CSI <n> P | DCH | Delete Characters | ✓ |
| CSI ? <n> h | DECSET | Set DEC Private Mode | ✓ |
| CSI ? <n> l | DECRST | Reset DEC Private Mode | ✓ |
| CSI ! p | DECSTR | Soft Terminal Reset | ✓ |
| CSI <n> " q | DECSCA | Select Character Protection Attribute | ✓ |
| CSI <n> SP q | DECSCUSR | Set Cursor Style | ✓ |
| CSI > 1 u | — | Push Kitty keyboard flags | ✓ |
| CSI < u | — | Pop Kitty keyboard flags | ✓ |
| CSI [Pm] # P | XTPUSHCOLORS | Push palette snapshot | ✓ |
| CSI [Pm] # Q | XTPOPCOLORS | Restore palette snapshot | ✓ |
| CSI # R | XTREPORTCOLORS | Report color stack depth | ✓ |
| CSI <n> S | SU | Scroll Up | ✓ |
| CSI <n> T | SD | Scroll Down | ✓ |
| CSI <Ps> t | XTWINOPS | Window manip / size reports — 14/16 t pixel sizes answered, 18 t (chars) intentionally not, window-acting ops ignored | ◐ |

## OSC
[](/vt/osc/osc-0-2)[](/vt/osc/osc-7)[](/vt/osc/osc-8)[](/vt/osc/osc-4)[](/vt/osc/osc-5)[](/vt/osc/osc-9)[](/vt/osc/osc-10-19)[](/vt/osc/osc-10-19)[](/vt/osc/osc-10-19)[](/vt/osc/osc-22)[](/vt/osc/osc-52)[](/vt/osc/osc-99)[](/vt/osc/osc-104-119)[](/vt/osc/osc-104-119)[](/vt/osc/osc-104-119)[](/vt/osc/osc-104-119)[](/vt/osc/osc-133)[](/vt/osc/osc-777)[](/vt/osc/osc-1337)
| Sequence | Description | Otty |
| --- | --- | --- |
| OSC 0 / 2 | Set window + icon title | ✓ |
| OSC 7 | Report current working directory | ✓ |
| OSC 8 | Hyperlink | ✓ |
| OSC 4 | Palette set / query (256-color) | ✓ |
| OSC 5 | Special-color set (bold / underline / blink / reverse / italic) | ✓ parsed (underline applied) |
| OSC 9 | Notification (iTerm-style) | ✓ |
| OSC 10 / 11 / 12 | Foreground / background / cursor color | ✓ |
| OSC 13 / 14 | Mouse pointer fg / bg | ✓ parsed (no macOS effect) |
| OSC 17 / 19 | Selection background / foreground | ✓ |
| OSC 22 | Mouse cursor shape (kitty / xterm) | ✓ |
| OSC 52 | Set / read clipboard (opt-in) | ✓ |
| OSC 99 | Kitty desktop-notification protocol (id, urgency, base64, chunked) | ✓ |
| OSC 104 | Palette reset (256-color) | ✓ |
| OSC 105 | Reset special colors | ✓ |
| OSC 110 / 111 / 112 | Reset fg / bg / cursor | ✓ |
| OSC 117 / 119 | Reset selection bg / fg | ✓ |
| OSC 133 | Prompt mark (FTCS / shell integration) | ✓ |
| OSC 777 | Notification (urxvt-style, with title) | ✓ |
| OSC 1337 | Inline image (iTerm2) | ✓ |

## Control (C0)
(/vt/c0/bel)(/vt/c0/bs)(/vt/c0/ht)(/vt/c0/lf)(/vt/c0/vt)(/vt/c0/ff)(/vt/c0/cr)(/vt/c0/so)(/vt/c0/si)(/vt/c0/can)(/vt/c0/sub)(/vt/c0/esc)(/vt/c0/del)
| Byte | Mnemonic | Name |
| --- | --- | --- |
| 0x07 | BEL | Bell |
| 0x08 | BS | Backspace |
| 0x09 | HT | Horizontal Tab |
| 0x0A | LF | Line Feed |
| 0x0B | VT | Vertical Tab |
| 0x0C | FF | Form Feed |
| 0x0D | CR | Carriage Return |
| 0x0E | SO | Shift Out |
| 0x0F | SI | Shift In |
| 0x18 | CAN | Cancel |
| 0x1A | SUB | Substitute |
| 0x1B | ESC | Escape |
| 0x7F | DEL | Delete |

## Modes (DEC private)

A selected subset:

| Mode | Description |
| --- | --- |
| ?1 | Application cursor keys |
| ?5 | Reverse video |
| ?6 | Origin mode (DECOM) |
| ?7 | Auto-wrap (DECAWM) |
| ?12 | Blinking cursor |
| ?25 | Cursor visible (DECTCEM) |
| ?47 | Switch to alternate screen (legacy) |
| ?1000 | Mouse: send X/Y on button event |
| ?1002 | Mouse: cell motion tracking |
| ?1003 | Mouse: all motion tracking |
| ?1004 | Focus reporting (CSI I / CSI O) |
| ?1006 | Mouse: SGR-style reporting |
| ?1047 | Switch to alternate screen (clears it on exit) |
| ?1049 | Switch to alternate screen + save cursor |
| ?2004 | Bracketed paste |
| ?2026 | Synchronized output |
| ?2031 | Color-scheme update notifications |

Set / unset with [DECSET](/vt/csi/decset) / [DECRST](/vt/csi/decrst). For a cross-terminal support matrix see [Terminal Comparison](/vt/terminal-comparison).

## DCS
(/vt/dcs/decrqss)
| Sequence | Name | Purpose | Support |
| --- | --- | --- | --- |
| DCS $ q SP q ST | DECRQSS | Query current cursor style → DCS 1 $ r Ps SP q ST | ✓ |
| DCS $ q … ST | DECRQSS (other targets) | Query SGR / scrolling region / … | ❌ |
| DCS + q … ST | XTGETTCAP | Terminfo capability query | ❌ |

## Kitty graphics (DCS-class)

Implemented via the [Kitty graphics protocol](/terminal-features/images). The full action / parameter table lives in that page.

## See also

- [Overview](/vt/overview)
- [External Protocols](/vt/external-protocols)
