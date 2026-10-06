<!-- source: https://docs.otty.sh/vt/osc/osc-9-4 -->

# OSC 9;4 — Progress / Task State (ConEmu)

|  |  |
| --- | --- |
| Sequence | OSC 9 ; 4 ; <state> [; <value> [; <tag>]] ST |
| Otty support | ✓ |

## Description

Reports the live state of a long-running task — a determinate percentage, an indeterminate spinner, or an error. ConEmu introduced this on the `OSC 9` number with a leading `4` argument; it is a distinct sub-protocol from the [OSC 9](/vt/osc/osc-9) desktop notification (the dispatcher branches on the `4;` prefix).

Otty maps it to a tab badge and an animated Dock icon (other terminals route it to an OS taskbar bar). The full UX — auto-progress for known commands, `otty watch`, and the Settings toggles — lives in [Progress State](/terminal-features/progress-state).
(/terminal-features/progress-state#otty-watch-wrap-any-command)
| State | Sequence | Meaning |
| --- | --- | --- |
| 0 | OSC 9;4;0 | Clear / no progress (resets to 0) |
| 1 | OSC 9;4;1;<pct> | In progress — <pct> is 0–100 |
| 2 | OSC 9;4;2[;<pct>] | Error — optionally held at a percentage |
| 3 | OSC 9;4;3 | Indeterminate — spinner, percentage unknown |
| 4 | OSC 9;4;4[;<pct>] | Paused / warning — ignored by Otty (no special reaction) |
| 5 | OSC 9;4;5;<exit>[;watch] | Otty extension — finished with exit code <exit>; the trailing watch tag marks a finish from otty watch so it routes to its own notification toggle |

## Example
bash
```
printf '\e]9;4;1;40\a'   # determinate — 40 %
printf '\e]9;4;3\a'      # indeterminate — spinner
printf '\e]9;4;2;80\a'   # error — held at 80 %
printf '\e]9;4;0\a'      # clear — remove the indicator
```

Try it

`otty features try progress` fires a determinate bar; `otty features try error-state` fires the error variant.

## Related

- [Progress State](/terminal-features/progress-state) — tab badges, the Dock spinner, auto-progress commands, `otty watch`.
- [OSC 9](/vt/osc/osc-9) — desktop notification on the same OSC number (no leading `4`).
