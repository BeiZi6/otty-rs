#!/bin/sh
# Otty — Grok Build (`grok`) hook.
#
# Reports the agent's state to the Otty app over IPC so a terminal pane can show
# the processing / idle badge. Otty registers this script in
# ~/.claude/settings.json (Settings → Agents → Install Hooks) — Grok's Claude
# Code compatibility layer, and the only user-scoped hook source it honors:
# ~/.grok/hooks/ is never read (that directory is project-scoped, .grok/hooks/),
# config.toml `[[hooks]]` is rejected as an unknown key, and a plugin's hooks
# start out disabled. It ships as a readable, code-signed file inside Otty.app
# so you can audit exactly what runs on every Grok hook event — Otty injects
# only the per-install paths.
#
# Args:
#   $1  state — processing | idle
# Env:
#   OTTY_CLI       — absolute path to the bundled otty-cli (injected by Otty;
#                    falls back to the copy next to this script's bundle)
#   OTTY_SOCKET    — Otty IPC socket path (injected by Otty)
#   GROK_HOOK_NAME — set by Grok's hook runner only (e.g.
#                    "user:stop[0].hooks[0]"); absent under every other agent
state="$1"

# We live in a settings file Claude Code owns, so Claude runs this entry too.
# GROK_HOOK_NAME is exported by Grok's hook runner alone, which makes it the
# signal for "this really is a Grok event" — reporting one from Claude's runner
# would pin Grok's identity onto a pane running Claude. (Grok stopped exporting
# GROK_SESSION_ID, which this gate used to test; only `${GROK_SESSION_ID}`
# interpolated into the command still resolves.)
[ -n "$GROK_HOOK_NAME" ] || exit 0

# Grok passes the hook payload as JSON on stdin, using camelCase field names
# (`hookEventName`, `sessionId`, `cwd`, `workspaceRoot`) — not Claude's
# snake_case — and does not export the session id, so stdin is where it comes
# from. GROK_WORKSPACE_ROOT is exported and backs up `cwd`.
input=$(cat)
sid=$(printf '%s' "$input" | sed -n 's/.*"sessionId"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
cwd=$(printf '%s' "$input" | sed -n 's/.*"cwd"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
[ -n "$cwd" ] || cwd="$GROK_WORKSPACE_ROOT"

# The pid Otty pins the event to a pane with. Grok execs this script directly —
# no shell in between — so our own $PPID *is* the `grok` process. It cannot be
# passed on the command line like the other agents' hooks do: Grok expands $VAR
# in a hook command itself and refuses to run the whole entry when the name is
# unset ("hook not executed: required env var(s) not set: ${PPID}").
agent_pid="$PPID"

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
# inside the bundle (…/Contents/Resources/agent-integration/grok → …/MacOS).
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"

# Backgrounded so the hook never blocks Grok. Errors are intentionally
# swallowed: a missing Otty app must not break the user's Grok session.
if [ -n "$error_kind" ]; then
    "$cli" state:grok session-id="$sid" state="$state" agent-pid="$agent_pid" cwd="$cwd" error-kind="$error_kind" 2>/dev/null &
else
    "$cli" state:grok session-id="$sid" state="$state" agent-pid="$agent_pid" cwd="$cwd" 2>/dev/null &
fi
