<!-- source: https://docs.otty.sh/vt/osc/osc-99 -->

# OSC 99 — Kitty Desktop Notification Protocol

|  |  |
| --- | --- |
| Sequence | OSC 99 ; <metadata> ; <payload> ST |
| Otty support | ✓ Title, body, urgency, base64 payloads, multi-chunk reassembly, capability query |

## Description

Richer alternative to [OSC 9](/vt/osc/osc-9) / [OSC 777](/vt/osc/osc-777). Introduced by [kitty](https://sw.kovidgoyal.net/kitty/desktop-notifications/) so a single notification can carry a separate title and body, an urgency level, an icon, and stable IDs for replacement / chunked transmission. Otty maps every fully-assembled OSC 99 to a native macOS notification.

Metadata is a colon-separated `key=value` list parsed *before* the payload separator.

| Key | Meaning | Otty status |
| --- | --- | --- |
| i=<id> | Notification id (chunks with the same id are joined; later notifications with the same id replace earlier ones) | ✓ |
| d=<0\|1> | 0 = more chunks follow; 1 (default) = final fragment | ✓ |
| p=<title\|body\|?> | Payload type — title (default), body, or ? (capability query) | ✓ |
| e=<0\|1> | 1 = payload is base64-encoded | ✓ |
| u=<0\|1\|2> | Urgency — low / normal (default) / critical | ✓ |
| n, o, a, f, g, s, w, c | Application name / close events / actions / focus / icon / sound / urls / category | parsed and ignored (NSUserNotification has no equivalent slot) |

The empty id (`i=`) is the "anonymous" slot — only one anonymous notification can be in-flight at a time; emitting another one before the current chunk train finishes overwrites it.

## Examples

Single-chunk title:
bash
```
printf '\e]99;;Build finished\e\\'
```

Title and body, glued across two chunks by the shared `i=42`:
bash
```
printf '\e]99;i=42:p=title:d=0;Build finished\e\\'
printf '\e]99;i=42:p=body;42 files compiled in 3.7s\e\\'
```

Critical urgency + base64 (handy when the message contains characters that would otherwise be re-interpreted by the shell or the OSC parser):
bash
```
b64=$(printf 'Compile failed: type mismatch' | base64)
printf '\e]99;i=err:u=2:e=1;%s\e\\' "$b64"
```

Replace a previous notification (same `i=`, different text):
bash
```
printf '\e]99;i=deploy;Deploying…\e\\'
sleep 5
printf '\e]99;i=deploy;Deploy complete\e\\'   # supersedes the first banner
```

Capability query — Otty replies inline so the program can feature-detect without timing out:
bash
```
printf '\e]99;i=ping:p=?;\e\\'
# Otty writes back:  ESC ] 99 ; i=ping:p=? ; ok ESC \
```

The `otty features try notification-kitty` demo bundles all three patterns.

## Behaviour notes

- Otty caps the OSC accumulator at 8 KiB per chunk, which matches kitty's recommended chunk size; oversized fragments are dropped silently.
- Chunked notifications are emitted only when a fragment with `d=1` (or no `d`) arrives — programs that forget to finalise leave the partial in-memory; Otty discards it on a session close.
- Apps that drop OSC 99 silently (older terminals) won't fire a banner. The recommended pattern is to send OSC 99 first and fall back to [OSC 9](/vt/osc/osc-9) / [OSC 777](/vt/osc/osc-777) when targeting unknown emulators.

## Related

- [OSC 9](/vt/osc/osc-9) — body-only iTerm convention.
- [OSC 777](/vt/osc/osc-777) — title + body urxvt convention.
- [Notifications](/terminal-features/notifications)
