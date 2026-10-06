#!/bin/sh
# Otty lifecycle bridge. The native CLI parses JSON stdin, preserving escaped
# paths and applying this agent's notification/background-task filters.
# OTTY_CLI and OTTY_SOCKET are supplied by the adapter. Keep the reporting
# process in this ancestry until IPC completes so Otty can identify the pane.
cli="${OTTY_CLI:-$(CDPATH= cd -- "$(dirname -- "$0")/../../../MacOS" 2>/dev/null && pwd)/otty-cli}"
"$cli" --timeout 1000 agent-hook copilot "$1" >/dev/null 2>&1 || :
