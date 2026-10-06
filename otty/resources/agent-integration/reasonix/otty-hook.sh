#!/bin/sh
# Otty — Reasonix (`reasonix`) hook.
#
# Reports the agent's state to the Otty app over IPC so a terminal pane can show
# the processing / idle / awaiting-input badge. Otty registers this script in
# ~/.reasonix/settings.json (Settings → Agents → Install Hooks). It ships as a
# readable, code-signed file inside Otty.app so you can audit exactly what runs
# on every Reasonix hook event — Otty injects only the per-install paths.
#
# Args:
#   $1  state     — processing | idle | awaiting
#   $2  agent_pid — the `reasonix` process's pid (its $PPID at hook time), so
#                   Otty can pin the event to the pane whose process tree
#                   contains it
# Env:
#   OTTY_CLI    — absolute path to the bundled otty-cli (injected by Otty; falls
#                 back to the copy next to this script's bundle)
#   OTTY_SOCKET — Otty IPC socket path (injected by Otty)
state="$1"
agent_pid="$2"

# Reasonix passes the hook payload as one line of camelCase JSON on stdin
# (`event`, `sessionId`, `cwd`, plus per-event fields) and injects no
# environment of its own, so stdin is the only source for both values.
input=$(cat)

# Pull a top-level string field. The payload's own keys come first (the encoder
# follows the struct order: event, sessionId, cwd, …), so taking the *first*
# match keeps a tool argument that happens to carry the same key — `toolArgs`
# is verbatim tool JSON and routinely contains "cwd" — from winning.
field() {
    printf '%s' "$input" \
        | grep -o "\"$1\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" \
        | head -n 1 \
        | sed 's/.*"\([^"]*\)"$/\1/'
}

# Notification fires whenever the agent wants the user's attention, which today
# is only a pending tool approval (notificationType "permission_prompt") — the
# one case that means "waiting on you". A future notification that is the agent
# talking rather than blocking would park the pane on a hand icon it never
# clears, so those are dropped.
#
# A *missing* type is not treated as one of those: the event's own documented
# meaning is "needs your attention", so an unlabelled one still reports. Only a
# type we can read and don't recognise is dropped — otherwise renaming or
# nesting that field would silently kill the awaiting badge for good.
if [ "$(field event)" = Notification ]; then
    case "$(field notificationType)" in
        "" | permission_prompt) ;;
        *) exit 0 ;;
    esac
fi

sid=$(field sessionId)
cwd=$(field cwd)

# Otty injects OTTY_CLI; the fallback locates otty-cli relative to this script
# inside the bundle (…/Contents/Resources/agent-integration/reasonix → …/MacOS).
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"

# Backgrounded so the hook never blocks Reasonix — UserPromptSubmit and
# PreToolUse are gating events that stall the turn until their hook returns.
# Both streams go to /dev/null, not just stderr: Reasonix captures a hook's
# output through pipes and waits for them to close, so a background child still
# holding the write end drags every gating event out to the runner's WaitDelay
# (measured: hundreds of ms per tool call, versus ~20ms once redirected).
# Swallowing errors is intentional besides — a missing Otty app must not break
# the user's Reasonix session.
"$cli" state:reasonix session-id="$sid" state="$state" agent-pid="$agent_pid" cwd="$cwd" >/dev/null 2>&1 &
