<!-- source: https://docs.otty.sh/vt/osc/osc-88 -->

# OSC 88 — Terminal Resume Protocol

Proposal

The **Terminal Resume Protocol (TRP)** is a *proposed*, vendor-neutral OSC sequence — not yet a ratified standard. Otty ships the reference implementation, and the spec lives in the open at [github.com/Otty-sh/osc-88](https://github.com/Otty-sh/osc-88) so other terminals can adopt the same number and wire format. Feedback and cross-terminal implementations are welcome.

|  |  |
| --- | --- |
| Sequence | OSC 88 ; <op> [ ; <key>=<value> ]... ST |
| Status | Proposal (v1) |
| Otty support | ✓ (gated by the Terminal Resume Protocol setting, on by default) |

## Why this protocol exists

When a terminal restarts — a crash, an OS reboot, an app upgrade, or just quitting and reopening — every long-lived program inside it dies. A terminal *can* restore the visual scrollback from a log, but it cannot bring the **program** back, because it has no idea how that program was launched or how to re-enter its prior state.

Today each terminal solves this with bespoke, terminal-specific machinery: iTerm2 keeps your jobs alive in long-running *process servers*; tmux / Zellij run a persistent *server* you re-attach to. None of it is portable, and none of it works for an arbitrary program (`nvim -S session.vim`, an SSH session, a coding agent with a `--resume` flag) that isn't a multiplexer.

TRP flips the responsibility: instead of the terminal *guessing* how to revive a program, **the program declares — once — how it would like to be relaunched.** The terminal stores that declaration and, on cold restart, re-runs it. One small escape sequence, understood by any terminal, replaces a pile of per-app special cases.

It is deliberately a **declarative, one-way** protocol (the program announces state; the terminal acts on its own schedule).

## Demo scenarios

- **Editor sessions** — a Neovim/Helix plugin arms `nvim -S Session.vim`. After a reboot the file reopens at the same buffers, on its own, with no replay garble.
- **Remote shells** — an SSH wrapper arms `ssh prod-bastion`; the connection is re-established in the restored pane instead of leaving a dead prompt.
- **Multiplexers** — tmux arms `tmux new -A -s main` (attach-or-create); restoring the window re-attaches the live session.
- **Coding agents** — a Claude Code / Codex / OpenCode hook arms the agent's own `--resume` invocation, so a restored terminal drops you back into the session.
- **Long-running TUIs** — `lazygit`, `k9s`, a REPL — anything that can describe its own relaunch can opt in.

## Quick start — for a TUI / program

Emit one sequence when your program starts. The value is base64-encoded UTF-8.
sh
```
# Pseudocode for "arm" — declare how to relaunch me.
#   cmd  = the program head (also the verification token)
#   args = everything after it
printf '\e]88;arm;cmd=%s;args=%s;self_repaint=1\a' \
  "$(printf 'nvim'           | base64)" \
  "$(printf -- '-S Session.vim' | base64)"
```

When your program exits *cleanly*, withdraw the declaration so a deliberate quit is not resurrected:
sh
```
printf '\e]88;clear\a'
```

A crash (no `clear`) leaves the declaration armed — which is exactly when you *want* to be resumed.

Write to the controlling tty

Emit to `/dev/tty`, not stdout — stdout is often captured by a parent (an agent runner, a shell job). Inside tmux / Zellij, wrap the bytes in the multiplexer's passthrough envelope so they reach the outer terminal.

The Otty CLI hides all of this:
sh
```
otty resume:arm --cmd nvim --args "-S Session.vim" --self-repaint
otty resume:clear
```

## Quick start — for a terminal

To implement TRP in your terminal:

- **Parse** `OSC 88`. Split the payload on `;`; `tokens[0]` is the op (`arm` / `clear`); the rest are `key=value` (split on the **first** `=`; values are base64). Ignore unknown keys (forward-compat) and ignore the whole sequence if you don't implement TRP.
- **Store** the armed spec per pane. `arm` fully replaces the prior spec (idempotent, last-write-wins); `clear` removes it.
- **Verify before persisting** (see [Security](#security)) — do *not* trust an armed command blindly.
- **On cold restore**, re-execute `cmd` followed by `args` in `cwd`. If `self_repaint=1`, skip whatever visual restore you normally do for that pane (log replay, snapshot) — the program redraws itself, and doing both races and garbles the grid.
- **Surface it** — show a user-visible, undoable indication that a command was resumed.

## Wire format

```
OSC 88 ; <op> [ ; <key>=<value> ]... ST
```

- `OSC` = `ESC ]`; `ST` = `ESC \` (preferred) or `BEL` (`\a`). `;` separates params.
- Values are **base64(UTF-8)** — commands contain spaces, quotes, `;`, `=` and non-ASCII; base64's alphabet excludes `;`, so it can never break the framing.
- `key=value` splits on the **first** `=` (trailing base64 `=` padding stays in the value).

### Operations

| op | Meaning |
| --- | --- |
| arm | Declare / fully replace this pane's resume spec. Requires cmd. Idempotent. |
| clear | Withdraw the spec (clean exit / no longer resumable). |
| query (optional) | OSC 88 ; query ST → terminal replies OSC 88 ; supported ; v=<max> ST. |

### Fields (for `arm`)
(/vt/osc/osc-0-2)
| key | Required | Value | Meaning |
| --- | --- | --- | --- |
| cmd | ✓ | base64 | The relaunch head and the verification token. Its basename must match the broadcasting process. |
| args | — | base64 | The relaunch tail, appended after cmd. Absent = run cmd alone. |
| self_repaint | — | 0 / 1 | 1 = the resumed program repaints its own screen; the terminal SHOULD skip its visual restore for this pane. Default 0. |
| cwd | — | base64 | Working directory for the relaunch. Absent = the terminal's last-known cwd for the pane. |
| title | — | base64 | Hint for the restored tab title (lower priority than OSC 0/2). |
| v | — | integer | Protocol version (default 1). Unknown fields MUST be ignored. |

> 
**Why `cmd` and `args` are split.** The head is the *verifiable* token — the terminal proves a process by that name was actually running before it trusts the spec, and resume runs `${cmd} ${args}`, so the executed binary is the one that was verified. The args flow through the shell and are *not* verified. The split draws that trust boundary on the wire.

### Examples
sh
```
# nvim with a session file, self-repainting
printf '\e]88;arm;cmd=bnZpbQ==;args=LVMgU2Vzc2lvbi52aW0=;self_repaint=1\a'
# tmux attach-or-create (idempotent)
printf '\e]88;arm;cmd=dG11eA==;args=bmV3IC1BIC1zIG1haW4=;self_repaint=1\a'
# ssh, no self-repaint (terminal does its own visual restore)
printf '\e]88;arm;cmd=c3No;args=cHJvZC1iYXN0aW9u\a'
# clean exit — withdraw
printf '\e]88;clear\a'
```

## Security

TRP persists a command that a terminal will later **execute**, so it is designed to resist *escape-sequence injection without code execution* — crafted bytes that arrive via `cat`-ing a file, an upstream process's stdout, or `man` / `git log` rendering attacker-controlled text. (iTerm2, xterm and others have shipped CVEs for exactly this class.) It does **not** defend against an attacker who already has code execution in the pane — persisting a resume command is strictly weaker than what they already have.

A conforming terminal:

- **MUST NOT** execute a resumed command without a verification gate against the injection class above. The **recommended** mechanism is *process-identity verification*: before persisting, confirm a live process in the pane's process tree has an `argv[0]` basename equal to `basename(cmd)`. (The spec mandates the *property* — "prove the arming program is/was the named binary" — not the exact mechanism.) A `cat` emitting forged bytes can't change its own `argv[0]` and exits before any verification tick observes it, so its spec is silently dropped.
- **MUST** execute only the binary named by `cmd` as the program head; `args` MUST NOT change which binary runs.
- **MUST** surface the resume as a user-visible, **reversible** action (e.g. a toast with *Undo*).
- **SHOULD** offer a per-user / per-binary opt-out (deny-list).

In Otty this is the 1 Hz process reaper (the same one that powers agent detection) plus a 5-second *Resumed: … [Undo]* toast on restore.

## Related

- [Session Recovery](/workflows/session-recovery) — the end-user feature and the **Terminal Resume Protocol** / **Re-run Processes** settings.
- [OSC 1337 — iTerm2](/vt/osc/osc-1337) — the `SetUserVar` key/value channel TRP was prototyped on before moving to a dedicated, vendor-neutral number.
- [OSC 9;4](/vt/osc/osc-9-4) — task progress/error state (a sibling "program tells the terminal something" sequence).
- [github.com/Otty-sh/osc-88](https://github.com/Otty-sh/osc-88) — the canonical spec and reference sample code.
