<!-- source: https://docs.otty.sh/vt/osc/osc-9 -->

# OSC 9 — Desktop Notification (iTerm style)

|  |  |
| --- | --- |
| Sequence | OSC 9 ; <body> ST |
| Otty support | ✓ |

## Description

Posts a system notification with body text only. iTerm2 introduced this; Otty maps it to a native macOS notification (and Windows toast, when shipped).

For a notification with a separate title, use [OSC 777](/vt/osc/osc-777). For chunked title/body, urgency levels, base64 payloads, and replace-by-id, use [OSC 99](/vt/osc/osc-99).

## Example
bash
```
make build && printf '\e]9;Build finished\a'

# Long-running test:
pytest && printf '\e]9;Tests green\a' || printf '\e]9;Tests RED\a'
```

Try it

`otty features try notification` — fires a single OSC 9 banner. For OSC 9;4 progress bars, see `otty features try progress` / `error-state`. For the title-aware variants, `otty features try notification-title` (OSC 777) and `otty features try notification-kitty` (OSC 99).

## Related

- [OSC 9;4](/vt/osc/osc-9-4) — progress / task state on the same OSC number (leading `4`); a distinct sub-protocol, not a notification.
- [OSC 777](/vt/osc/osc-777) — same purpose with a title field.
- [OSC 99](/vt/osc/osc-99) — kitty's rich protocol (chunked, urgency, replace-by-id, base64).
- [Notifications](/terminal-features/notifications)
