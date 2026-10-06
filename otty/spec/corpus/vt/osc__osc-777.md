<!-- source: https://docs.otty.sh/vt/osc/osc-777 -->

# OSC 777 — Desktop Notification (urxvt style)

|  |  |
| --- | --- |
| Sequence | OSC 777 ; notify ; <title> ; <body> ST |
| Otty support | ✓ |

## Description

Like [OSC 9](/vt/osc/osc-9), but adds a separate **title** field. Originated with rxvt-unicode and adopted by other terminals. Otty maps it to a native macOS notification with the title displayed as the notification title.

The `notify` literal subcommand is required (OSC 777 supports other subcommands historically, none widely used).

For urgency levels, chunked transmission, replace-by-id, or base64 payloads, use [OSC 99](/vt/osc/osc-99) instead.

## Example
bash
```
printf '\e]777;notify;Deploy;Production is live\a'

# In a Makefile target:
.PHONY: notify
notify:
	@printf '\e]777;notify;Make target;%s\a' "$(MAKECMDGOALS) done"
```

Try it

`otty features try notification-title` — fires a single OSC 777 banner with title + body.

## Related

- [OSC 9](/vt/osc/osc-9) — body-only variant.
- [OSC 99](/vt/osc/osc-99) — kitty's rich protocol (chunked, urgency, replace-by-id, base64).
- [Notifications](/terminal-features/notifications)
