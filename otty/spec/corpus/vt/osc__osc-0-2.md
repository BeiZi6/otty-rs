<!-- source: https://docs.otty.sh/vt/osc/osc-0-2 -->

# OSC 0 / OSC 2 — Set Window Title

|  |  |
| --- | --- |
| Sequence | OSC <Ps> ; <title> ST |
| Ps | 0 = icon + window title, 1 = icon only, 2 = window only |
| Otty support | ✓ |

## Description

Sets the window title (and optionally the icon title — Otty treats them as the same).

| Ps | Effect |
| --- | --- |
| 0 | Set icon name and window title |
| 1 | Set icon name only |
| 2 | Set window title only |

The most common form is `OSC 0`. Most shell rcs (zsh `precmd`, bash `PROMPT_COMMAND`) emit this on every prompt to keep the tab label informative.

## Example
bash
```
printf '\e]0;%s\a' "deploy-prod"
# zsh equivalent:
print -Pn "\e]0;%~\a"     # title = current directory (compact form)
```

Try it

`otty features try set-title` — sets both window (OSC 2) and icon/tab (OSC 1) names. Look at the title bar / tab to confirm.

## Related

- [Window, Tab and Split](/user-interface/window-tab-split) — Otty uses the title for the tab label.
- [OSC 7](/vt/osc/osc-7) — emit cwd, complementary.
