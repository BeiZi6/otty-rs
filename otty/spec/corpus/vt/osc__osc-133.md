<!-- source: https://docs.otty.sh/vt/osc/osc-133 -->

# OSC 133 — Shell Integration (FTCS)

|  |  |
| --- | --- |
| Sequence | OSC 133 ; <kind> [ ; <params> ] ST |
| Otty support | ✓ |

## Description

Marks the boundaries of prompts and commands in the byte stream. The terminal uses these marks for command outlines, jump-to-prev / next-command, exit-status indicators, and intelligent paste.

This is the "Final Term shell integration" protocol (FTCS) as adopted by iTerm2, WezTerm, Kitty, VS Code, and now Otty.

## Kinds

| <kind> | Emitted | Meaning |
| --- | --- | --- |
| A | Just before the prompt is drawn | Prompt start |
| B | After the prompt, before input | Prompt end / command start (input zone) |
| C | When the command begins executing | Command output start |
| D ; <exit> | When the command finishes | Command end with exit code |
| L | (optional) | Marks the start of a "user input prompt" within a tool |

## Example
bash
```
# Minimal zsh precmd / preexec:
precmd()  { print -Pn '\e]133;A\e\\'; }
preexec() { print -Pn '\e]133;C\e\\'; }
# B is emitted by zsh's prompt builder if PROMPT_SUBST is on.
# D is emitted by zsh's chpwd or via a hook — Otty's installer handles it.
```

In practice, Otty's [`otty shell install`](/terminal-features/shell-integration) writes the snippets — you should never need to hand-roll these.

Try it

- `otty features try semantic-prompt` — fires the full `A → B → C → D;0` sequence so you can verify prompt-detection wiring (Open Quickly history, etc.).
- `otty features try error-state` — same flow with `D;1` plus an OSC 9;4 error badge, to confirm Otty marks errored prompts.

## Related

- [Shell Integration](/terminal-features/shell-integration)
- [Outline](/user-interface/outline)
