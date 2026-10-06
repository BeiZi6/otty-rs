#!/bin/sh
# Otty — Cursor CLI (`agent`) hook.
#
# Reports the agent's state to the Otty app over IPC so a terminal pane can show
# the processing / idle badge. Otty registers this script in ~/.cursor/hooks.json
# (Settings → Agents → Install Hooks). It ships as a readable, code-signed file
# inside Otty.app so you can audit exactly what runs on every Cursor hook event —
# Otty injects only the per-install paths.
#
# Args:
#   $1  state     — processing | idle
#   $2  agent_pid — the `agent` process's pid (its $PPID at hook time), so Otty
#                   can pin the event to the pane whose process tree contains it
# Env:
#   OTTY_CLI    — absolute path to the bundled otty-cli (injected by Otty; falls
#                 back to the copy next to this script's bundle)
#   OTTY_SOCKET — Otty IPC socket path (injected by Otty)
state="$1"
agent_pid="$2"

# Grok Build also reads Cursor's hook files, so this script can be invoked by
# `grok` rather than `agent`. Reporting that as state:cursor would badge the
# pane as Cursor CLI and hand Resume a Grok session id. Bail out and let Grok's
# own hook report it; GROK_HOOK_NAME is injected by Grok's hook runner only
# (it replaced GROK_SESSION_ID, which Grok no longer exports to hooks).
[ -n "$GROK_HOOK_NAME" ] && exit 0

# Cursor passes the hook payload as JSON on stdin. All hooks carry
# conversation_id (the session id) and workspace_roots (the cwd is the first
# entry). Cursor never runs a hook without conversation_id, so a single
# first-string extraction is enough.
input=$(cat)
sid=$(printf '%s' "$input" | sed -n 's/.*"conversation_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
cwd=$(printf '%s' "$input" | sed -n 's/.*"workspace_roots"[[:space:]]*:[[:space:]]*\[[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)

# Cursor has no separate failure event: the one `stop` hook ends every agent
# loop and its payload's `status` says how — "completed", "aborted" or "error".
# Only "error" is a failure the user has to act on; "aborted" is their own
# interrupt and stays idle, exactly as a completed turn does. Older Cursor
# builds without the field simply don't match and keep the previous behaviour.
if [ "$state" = idle ]; then
    st=$(printf '%s' "$input" \
        | sed -n 's/.*"status"[[:space:]]*:[[:space:]]*"\([a-z_]*\)".*/\1/p' \
        | head -n 1)
    [ "$st" = error ] && state=error
fi

# Otty injects OTTY_CLI; the fallback locates otty-cli relative to this script
# inside the bundle (…/Contents/Resources/agent-integration/cursor → …/MacOS).
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"

# Backgrounded so the hook never blocks Cursor. Errors are intentionally
# swallowed: a missing Otty app must not break the user's Cursor session.
"$cli" state:cursor session-id="$sid" state="$state" agent-pid="$agent_pid" cwd="$cwd" 2>/dev/null &
