<!-- source: https://docs.otty.sh/vt/external-protocols -->

# External Protocols

Extensions to the VT vocabulary adopted from other terminals. These ride on the underlying byte stream but define their own structured payloads.

## OSC-based

Most external protocols are layered on OSC because OSC supports arbitrary string payloads with a clean string-terminator.
(/vt/osc/osc-8)(/terminal-features/shell-integration)(/user-interface/files-and-links)(/terminal-features/notifications)(/terminal-features/notifications)(/terminal-features/copy-and-paste)(/terminal-features/images)
| Protocol | OSC | Origin | Otty | Doc |
| --- | --- | --- | --- | --- |
| Hyperlinks | OSC 8 | xterm / VTE | ✓ | OSC 8 |
| Shell Integration | OSC 133 | FinalTerm / iTerm2 | ✓ | Shell Integration |
| Working Directory | OSC 7 | VTE | ✓ | Files and Links |
| Notifications | OSC 9 | iTerm2 | ✓ | Notifications |
| Notifications (rich) | OSC 777 | urxvt | ✓ | Notifications |
| Clipboard | OSC 52 | xterm | ✓ (opt-in) | Copy and Paste |
| Inline Image (iTerm) | OSC 1337 | iTerm2 | ✓ | Images |

## DCS / APC-based
(/terminal-features/images)
| Protocol | Introducer | Origin | Otty | Doc |
| --- | --- | --- | --- | --- |
| Kitty graphics | APC (ESC _) | Kitty | ✓ (mostly) | Images |
| Sixel graphics | DCS (ESC P) | DEC | ☐ planned | — |
| tmux passthrough | DCS | tmux | ◐ | — |

## CSI-based
(/vt/csi/ed)
| Protocol | Sequence | Origin | Otty |
| --- | --- | --- | --- |
| Kitty keyboard protocol | CSI > 1 u / CSI < u | Kitty | ✓ |
| Bracketed paste | CSI ?2004 h/l | xterm | ✓ |
| Focus reporting | CSI ?1004 h/l → CSI I / CSI O | xterm | ✓ |
| Synchronized output | CSI ?2026 h/l | iTerm2 | ✓ (buffered, with a safety timeout) |
| SGR mouse reporting | CSI ?1006 h/l | xterm | ✓ |
| Scroll-then-erase ED | CSI 22 J | Kitty | ✓ |

## Conformance notes

- OSC 133 follows the **FinalTerm "FTCS" spec** as adopted by iTerm2.
- Where iTerm2 and xterm differ on the same OSC number, Otty implements iTerm2's semantics (the more common modern target).

## Identification

| Sequence | Origin | Otty |
| --- | --- | --- |
| CSI c — Primary DA | DEC | ✓ (responds ?62;22c) |
| CSI > c — Secondary DA | DEC | ✓ |
| CSI > q — XTVERSION | xterm | ✓ (replies DCS > \| otty(<version>) ST) |

## See also

- [Terminal Comparison](/vt/terminal-comparison) — side-by-side support matrix vs xterm / iTerm2 / Kitty / WezTerm / Alacritty / Ghostty / foot.
- [Sequence Reference](/vt/sequence-reference) — every sequence in one place.
- [Shell Integration](/terminal-features/shell-integration), [Images](/terminal-features/images), [Notifications](/terminal-features/notifications).
