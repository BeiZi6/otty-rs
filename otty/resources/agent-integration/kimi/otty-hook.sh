#!/bin/sh
# Otty — Kimi Code CLI hook.
#
# Reports the agent's state to the Otty app over IPC so a terminal pane can
# show the processing / idle / awaiting-input badge. Otty registers this script
# in ~/.kimi-code/config.toml as an [[hooks]] entry (Settings → Agents → Install
# Hooks). It ships as a readable, code-signed file inside Otty.app so you can
# audit exactly what runs on every Kimi hook event — Otty injects only the
# per-install paths.
#
# Args:
#   $1  state    — processing | idle | awaiting
#   $2  kimi_pid — the Kimi process's pid (its $PPID at hook time), so Otty can
#                  pin the event to the pane whose process tree contains it
# Env:
#   OTTY_CLI    — absolute path to the bundled otty-cli (injected by Otty; falls
#                 back to the copy next to this script's bundle)
#   OTTY_SOCKET — Otty IPC socket path (injected by Otty)
state="$1"
kimi_pid="$2"

# Kimi Code passes the hook payload as JSON on stdin (hook_event_name,
# session_id, cwd, …) — the same shape as Claude Code.
input=$(cat)

# Extract the session id and cwd from the stdin JSON. cwd lets Otty fall back to
# a cwd match when the agent-pid tree scan misses.
sid=$(printf '%s' "$input" | sed -n 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
cwd=$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)

# StopFailure names the failure in the payload's "error" field. Forward it so
# Otty can say *why* the turn died instead of a bare "failed"; matched as
# [a-z_]* so prose in a neighbouring message field can't be taken for it. Only
# parsed for state=error, and an absent value simply degrades to a generic
# summary.
error_kind=""
if [ "$state" = error ]; then
    error_kind=$(printf '%s' "$input" \
        | sed -n 's/.*"error"[[:space:]]*:[[:space:]]*"\([a-z_]*\)".*/\1/p' \
        | head -n 1)
fi

# Otty injects OTTY_CLI; the fallback locates otty-cli relative to this script
# inside the bundle (…/Contents/Resources/agent-integration/kimi → …/MacOS).
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"

# Backgrounded so the hook never blocks Kimi. Errors are intentionally swallowed:
# a missing Otty app must not break the user's Kimi session.
if [ -n "$error_kind" ]; then
    "$cli" state:kimi session-id="$sid" state="$state" agent-pid="$kimi_pid" cwd="$cwd" error-kind="$error_kind" 2>/dev/null &
else
    "$cli" state:kimi session-id="$sid" state="$state" agent-pid="$kimi_pid" cwd="$cwd" 2>/dev/null &
fi
