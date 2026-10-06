<!-- source: https://docs.otty.sh/vt/concepts/control-sequences -->

# Control Sequences [​](#control-sequences)

A **control sequence** is any byte stream that the terminal interprets as a command rather than rendering as text. The terminal's parser is a small state machine that distinguishes printable bytes from control bytes and routes them differently.

## The four families [​](#the-four-families)
****``````[](/vt/c0/)****``````[](/vt/esc/)****````[](/vt/csi/)****``````[](/vt/osc/)
| Family | Introduced by | Example | More |
| --- | --- | --- | --- |
| C0 | a single byte in 0x00–0x1F or 0x7F | 0x0A (LF) | Control |
| ESC | ESC + final byte (0x30–0x7E) | ESC c (RIS) | ESC |
| CSI | ESC [ + params + final | ESC [ 31 m | CSI |
| OSC | ESC ] + payload + ST | ESC ] 0 ; title BEL | OSC |

A fifth less-common family covers **DCS** (`ESC P`), **APC** (`ESC _`), and **PM** (`ESC ^`) — used by Kitty graphics, tmux passthrough, and various private protocols. See [External Protocols](/vt/external-protocols).

## Parsing rules [​](#parsing-rules)

The state machine is the **VT500-Series Parser** (Paul Williams). Highlights:

- `CAN` (`0x18`) and `SUB` (`0x1A`) abort any sequence in flight.
- `ESC` resets the parser and starts a new sequence.
- An `ESC` inside an OSC or DCS payload terminates the payload (string terminator).
- Invalid sequences are silently dropped; they do not show up as text.

## See also [​](#see-also)

- [VT Overview](/vt/overview)
- [Cursor](/vt/concepts/cursor), [Screen](/vt/concepts/screen), [Colors](/vt/concepts/colors)
