#!/bin/sh
# Otty — Codex CLI hook.
#
# Reports the agent's state to the Otty app over IPC so a terminal pane can
# show the processing / idle / awaiting-input badge. Otty registers this script
# in ~/.codex/hooks.json (Settings → Agents → Install Hooks) and flips
# [features].hooks = true in ~/.codex/config.toml so Codex actually runs it. It
# ships as a readable, code-signed file inside Otty.app so you can audit exactly
# what runs on every Codex hook event — Otty injects only the per-install paths.
#
# Args:
#   $1  state    — processing | idle | awaiting
#   $2  codex_pid — the Codex process's pid (its $PPID at hook time), so Otty
#                   can match the event to the right pane when several Codex
#                   instances share a cwd
# Env:
#   OTTY_CLI    — absolute path to the bundled otty-cli (injected by Otty)
#   OTTY_SOCKET — Otty IPC socket path (injected by Otty; read by otty-cli)
state="$1"
codex_pid="$2"

# Codex passes the hook payload as JSON on stdin.
input=$(cat)

# session_id can appear at the top level or nested under payload.id.
sid=$(printf '%s' "$input" | sed -n \
    -e 's/.*"session_id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    -e 's/.*"payload"[[:space:]]*:[[:space:]]*{.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -n 1)

# No session id → nothing Otty can match; skip quietly.
[ -n "$sid" ] || exit 0

# cwd lets Otty match the hook to a pane on first sight (esp. SessionStart,
# before any cached mapping exists).
cwd=$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)

# Otty injects OTTY_CLI; the fallback locates otty-cli relative to this script
# inside the bundle (…/Contents/Resources/agent-integration/codex → …/MacOS).
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"

# OTTY_SOCKET is already in this script's environment (Otty prefixes it on the
# hook command), so otty-cli inherits it. Backgrounded; errors swallowed so a
# missing Otty app never breaks the user's Codex session.
"$cli" state:codex session-id="$sid" state="$state" cwd="$cwd" agent-pid="$codex_pid" 2>/dev/null &
