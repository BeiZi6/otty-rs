<!-- source: https://docs.otty.sh/vt/osc/osc-52 -->

# OSC 52 — Clipboard Manipulation

|  |  |
| --- | --- |
| Sequence | OSC 52 ; <selection> ; <base64-payload> ST |
| Otty support | ✓ (opt-in) |

## Description

Reads or writes the system clipboard from a program — including over SSH, where there's no other way to get text into the host's clipboard.

| <selection> | Clipboard |
| --- | --- |
| c | The standard clipboard (most common) |
| p | Primary selection (Linux) |
| (other) | Implementation-defined |

| Payload | Meaning |
| --- | --- |
| Base64 text | Write to the selected clipboard |
| ? | Query — terminal replies with the current value (encoded) |

## Security

Clipboard access via OSC 52 lets any program (including a remote one over SSH) read or write your clipboard. **Off by default in Otty.** Enable with:

```
clipboard-osc-52 = write           # write | read-write | off
```

`read-write` is required for query — only enable it if you understand the implications (a script in a remote shell can exfiltrate your clipboard contents).

## Example
bash
```
# Copy "hello world" to the clipboard
printf '\e]52;c;%s\a' "$(printf hello\ world | base64)"
```

Try it

- `otty features try clipboard-write` — copies a sample string (gated by your `clipboard-write` config).
- `otty features try clipboard-read` — fires the `?` query so you can confirm `clipboard-read = ask` produces a prompt.

## Related

- [Copy and Paste](/terminal-features/copy-and-paste) — clipboard handling in Otty.
- [Notifications](/terminal-features/notifications) — section "Clipboard (OSC 52)" has the security notes too.
