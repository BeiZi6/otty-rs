#!/bin/sh
# Otty — Claude Code hook.
#
# Reports the agent's state to the Otty app over IPC so a terminal pane can
# show the processing / idle / awaiting-input badge. Otty registers this script
# in ~/.claude/settings.json (Settings → Agents → Install Hooks). It is shipped
# as a readable, code-signed file inside Otty.app so you can audit exactly what
# runs on every Claude hook event — Otty injects only the per-install paths.
#
# Args:
#   $1  state     — processing | idle | awaiting | error
#   $2  claude_pid — the Claude process's pid (its $PPID at hook time). Used to
#                    detect --dangerously-skip-permissions ("bypass") mode, and
#                    reported to Otty so it can tell WHICH Claude fired this:
#                    a subagent shelling out to `claude -p` runs inside the
#                    pane's process tree and its hooks are otherwise
#                    indistinguishable from the pane's own agent's.
# Env:
#   OTTY_CLI — absolute path to the bundled otty-cli (injected by Otty; falls
#              back to the copy next to this script's bundle)
state="$1"
claude_pid="$2"

# Grok Build deliberately scans and trusts ~/.claude/settings.json, so *this*
# script also runs on every Grok hook event. Reporting those as state:claude
# would badge the pane as Claude Code and hand Resume/Fork a Grok session id to
# feed `claude --resume` — a session that doesn't exist. Bail out and let Grok's
# own hook (Settings → Agents → Grok Build) report the pane; GROK_HOOK_NAME is
# injected by Grok's hook runner and is never set by Claude Code itself.
# (It replaced GROK_SESSION_ID, which Grok no longer exports — that name now
# resolves only when interpolated into the hook command, so the old gate here
# silently stopped firing.) Grok independently refuses to run any command
# naming an unset variable, which our "$PPID" already trips; this stays as the
# explicit, load-bearing gate rather than relying on that.
[ -n "$GROK_HOOK_NAME" ] && exit 0

# Claude passes the hook payload as JSON on stdin.
input=$(cat)

# Suppress the spurious "task complete" that fires per Task subagent. A Task
# subagent runs in the background: while it works, the main agent often yields
# and Claude fires a Stop (this hook's state=idle) even though the turn isn't
# really over — the still-running subagent is listed in the Stop payload's
# background_tasks (`"type":"subagent","status":"running"`). Reporting that as
# idle made Otty fire a completion notification (and idle badge) for every
# subagent. Treat such a Stop as still-processing so the notification only
# fires on the genuine final Stop, whose background_tasks no longer lists a
# running subagent. Whitespace is stripped so the match survives pretty-printed
# JSON; both key orderings are covered. Older Claude builds without the field
# simply won't match, preserving the previous behaviour.
if [ "$state" = idle ]; then
    bt=$(printf '%s' "$input" | tr -d ' \t\n')
    case "$bt" in
        *'"type":"subagent","status":"running"'*|*'"status":"running","type":"subagent"'*)
            state=processing ;;
    esac
fi

# Prefer the env the Claude CLI exports; fall back to the stdin JSON.
sid="${CLAUDE_SESSION_ID:-$(printf '%s' "$input" | sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)}"

# cwd lets Otty locate the pane before a session mapping exists.
cwd=$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)

# Bypass-permissions mode is read from the Claude process's own argv.
bypass=0
ps -o args= -p "$claude_pid" 2>/dev/null | grep -qF -- '--dangerously-skip-permissions' && bypass=1

# StopFailure names the failure in the payload's "error" field (rate_limit,
# authentication_failed, billing_error, server_error, …). Forward it so Otty can
# say *why* the turn died instead of a bare "failed". Only parsed for state=error
# so no other event pays for the extra sed. The value is matched as [a-z_]* so
# the prose in last_assistant_message ("API Error: …") can't be mistaken for it.
error_kind=""
if [ "$state" = error ]; then
    error_kind=$(printf '%s' "$input" \
        | sed -n 's/.*"error"[[:space:]]*:[[:space:]]*"\([a-z_]*\)".*/\1/p' \
        | head -n 1)
fi

# Otty injects OTTY_CLI; the fallback locates otty-cli relative to this script
# inside the bundle (…/Contents/Resources/agent-integration/claude → …/MacOS).
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"

# Backgrounded so the hook never blocks Claude. Errors are intentionally
# swallowed: a missing Otty app must not break the user's Claude session.
#
# agent-pid is the Claude process itself, not this script: Otty matches the
# event to a pane by walking that pid's ancestry, and — because it is the agent
# and not a helper speaking for it — uses it to ignore a nested `claude -p`
# whose short-lived session id would otherwise replace the live one's.
if [ -n "$error_kind" ]; then
    "$cli" state:claude session-id="$sid" state="$state" cwd="$cwd" bypass="$bypass" agent-pid="$claude_pid" error-kind="$error_kind" 2>/dev/null &
else
    "$cli" state:claude session-id="$sid" state="$state" cwd="$cwd" bypass="$bypass" agent-pid="$claude_pid" 2>/dev/null &
fi
