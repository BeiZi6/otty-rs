<!-- source: https://docs.otty.sh/vt/overview -->

# VT Overview

A reference for every escape sequence Otty interprets. Useful if you're writing TUIs, debugging a misbehaving program, or just curious what `\x1b[31m` actually does.

## What "VT" means here

Terminals communicate with the host by exchanging *bytes*. Most are printable text; some are **control sequences** that move the cursor, set colors, request information, switch modes, and so on. The vocabulary of these sequences is loosely standardized by ANSI X3.64 / ECMA-48 / ISO 6429 with extensions from DEC VT terminals, xterm, iTerm2, Kitty, and others.

## Sequence categories
(/vt/c0/)(/vt/esc/)(/vt/esc/decaln)(/vt/csi/)(/vt/osc/)
| Category | Introducer | Examples |
| --- | --- | --- |
| C0 / Fe | single control byte (0x00–0x1F, 0x7F) | \b BS, \n LF, \r CR, \e ESC |
| ESC | ESC + one or more bytes (no [) | ESC 7 Save Cursor, ESC # 8 DECALN |
| CSI | ESC [ … final byte | ESC [ 31 m red, ESC [ H cursor home |
| OSC | ESC ] … ST | OSC 0 ; <title> set title, OSC 8 hyperlink |
| DCS / APC / PM | ESC P, ESC _, ESC ^ | Kitty graphics, tmux passthrough |

## How to read these pages

Every sequence page follows the same shape:

- **Sequence** — bytes you send, in human-readable notation.
- **Description** — what it does.
- **Otty support** — implemented, partially implemented, or not supported.
- **Example** — a short `printf` (and `otty cli` where applicable) demonstrating the effect.

See [DECALN](/vt/esc/decaln) for a representative page, or [Sequence Reference](/vt/sequence-reference) for the full searchable index.

## Sources and conformance

- Otty's terminal core is forked from **Alacritty's** VT parser.
- Behavior matches **DEC VT520** + modern **xterm** extensions where they disagree.
- Mode reporting (`DA1`, `DA2`, `DECRQM`) advertises the protocols listed in [Sequence Reference](/vt/sequence-reference) → "Modes".

## Notation conventions

- `ESC` — `0x1B`
- `BEL` — `0x07`
- `ST` (String Terminator) — either `ESC \` (preferred) or `BEL` (legacy, accepted)
- `<n>`, `<Ps>`, `<Pt>` — numeric / textual parameter placeholders
- `^[`, `\e`, `\x1b` are all the same ESC byte; this site uses `\e`.

## See also

- [Sequence Reference](/vt/sequence-reference) — the full searchable index of every sequence.
- [Control Sequences](/vt/concepts/control-sequences) — how the parser routes the four sequence families.
- [External Protocols](/vt/external-protocols) — extensions adopted from other terminals.
- [Terminal Comparison](/vt/terminal-comparison) — cross-terminal support matrix.
