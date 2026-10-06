# Otty bash integration — payload (sourced from ~/.bashrc managed block)
#
# Idempotent: re-sourcing in the same shell is a no-op.
[[ -n "$__otty_loaded" ]] && return
__otty_loaded=1

# Guard: only for interactive shells.
[[ "$-" == *i* ]] || return 0

# ── Bundled `otty` on PATH ──
# Otty ships a CLI binary inside the .app bundle. If "Install CLI" has not
# been run (no /usr/local/bin/otty symlink), point shells at our per-process
# bin dir so `otty …` still works without polluting the user's system. The
# guard avoids appending the same dir twice across nested shells.
if [[ -n "$OTTY_BIN_DIR" && -d "$OTTY_BIN_DIR" ]]; then
    case ":$PATH:" in
        *":$OTTY_BIN_DIR:"*) ;;
        *) export PATH="$OTTY_BIN_DIR:$PATH" ;;
    esac
fi

# ── OSC 133 FTCS markers ──
__otty_cmd_executed=""
__otty_last_exit=0
__otty_progress_active=""

# Match the just-about-to-run command against $OTTY_PROGRESS_COMMANDS
# (pipe-separated list of whitespace-delimited prefixes, e.g. "git push").
__otty_progress_match() {
    local cmd="$1"
    [[ -z "$OTTY_PROGRESS_COMMANDS" ]] && return 1
    # Strip leading `NAME=value` env assignments.
    while [[ "$cmd" =~ ^[A-Za-z_][A-Za-z0-9_]*= ]]; do
        cmd="${cmd#* }"
    done
    local IFS='|'
    local p
    for p in $OTTY_PROGRESS_COMMANDS; do
        [[ -z "$p" ]] && continue
        if [[ "$cmd" == "$p" || "$cmd" == "$p "* ]]; then
            return 0
        fi
    done
    return 1
}

__otty_prompt_command() {
    # PROMPT_COMMAND saves status before rearming the DEBUG hook, whose eval
    # would otherwise replace every command's real result with its own status.
    __otty_last_exit=${1:-$?}
    # D mark from previous command (skip first prompt).
    if [[ -n "$__otty_cmd_executed" ]]; then
        printf '\e]133;D;%d\a' "$__otty_last_exit"
    fi
    # OSC 9;4 completion for auto-progress commands.
    if [[ -n "$__otty_progress_active" ]]; then
        if (( __otty_last_exit == 0 )); then
            printf '\e]9;4;5;0\a'
        else
            printf '\e]9;4;2\a'
        fi
        __otty_progress_active=""
    fi
    # A mark — prompt start.
    # See the zsh payload: the token is what distinguishes our prompt mark
    # from one emitted by a program running under us.
    # Not inside a multiplexer only. A nested shell inherits the very same
    # token, and a multiplexer can forward marks to the outer terminal — today
    # by a script wrapping them in tmux's DCS passthrough, and possibly by
    # default in future (tmux/tmux#5237). An inner prompt would then
    # authenticate against the OUTER terminal and reclaim state belonging to
    # the multiplexer itself, yanking it off the alternate screen mid-session.
    # Dropping the token degrades to a bare mark: nothing is reclaimed, which
    # is what terminals without this feature do anyway.
    if [ -n "$OTTY_PROMPT_TOKEN" ] && [ -z "$TMUX" ] && [ -z "$STY" ] && [ -z "$ZELLIJ" ]; then
        printf '\e]133;A;otty=%s\a' "$OTTY_PROMPT_TOKEN"
    else
        printf '\e]133;A\a'
    fi
    __otty_cmd_executed=""
    # Flush history per the configured scope. The no-op default below keeps
    # this call free when the user hasn't chosen one.
    __otty_history_hook
}

# Replaced below when OTTY_HISTORY_SCOPE selects a mode.
__otty_history_hook() { :; }

# B mark — appended to PS1 so it fires after prompt is rendered.
__otty_install_ps1() {
    if [[ "$PS1" != *'133;B'* ]]; then
        PS1="${PS1}\[\e]133;B\a\]"
    fi
}

__otty_debug_trap() {
    # Only emit C for real commands, not PROMPT_COMMAND itself.
    if [[ -n "$COMP_LINE" ]]; then
        return
    fi
    if [[ "$BASH_COMMAND" == __otty_* ]]; then
        return
    fi
    if [[ -z "$__otty_cmd_executed" ]]; then
        printf '\e]133;C\a'
        __otty_cmd_executed=1
        # OSC 9;4 indeterminate progress for matched commands.
        if __otty_progress_match "$BASH_COMMAND"; then
            printf '\e]9;4;3\a'
            __otty_progress_active=1
        fi
    fi
}

# ── DEBUG-trap arming (chain, don't clobber) ──
# bash allows exactly ONE DEBUG trap, and bash-preexec-based frameworks
# (starship, atuin, ble.sh) claim it too — whoever installs last wins.
# Silently losing it kills our C/D marks: session restore then reads
# "prompt marks but no command ever ran" and skips the whole replay,
# dropping real scrollback. So every prompt re-checks the trap and, if a
# framework stole it, chains it back — never clobbering the foreign trap
# (that just reverses who loses).
#
# This logic MUST run at top level, hence the eval'd string appended to
# PROMPT_COMMAND rather than a function. bash does not let non-traced
# functions see the DEBUG trap: inside a function `trap -p DEBUG` reads
# EMPTY, and a DEBUG trap set there is reverted when the function returns
# if one existed at entry (both measured on bash 3.2). A function version
# of this re-arm reads "no trap", clobber-installs, and evaporates — do
# not "simplify" it back into one.
#
# Chain shape — foreign command FIRST:
#   - `$_` (last argument of the previous command) is what bash-preexec's
#     handler reads, and any command of ours running before it would
#     overwrite it. Our handler only reads $BASH_COMMAND, which bash pins
#     to the trap-triggering command for the whole handler, so running
#     second costs us nothing.
#   - Joined with a newline, not `;`, so a foreign command ending in a
#     comment cannot swallow ours.
# Idempotent via the *__otty_debug_trap* guard: one chain per thief, no
# growth loop, and a thief that preserved our chained command is left
# alone entirely. A bare `trap '' DEBUG` (explicit ignore) is reclaimed —
# the marks and the session-restore heuristics they feed outrank an
# ignored slot.
#
# `trap -p` output is reusable shell input — `trap -- '<cmd>' DEBUG` — so
# eval-splitting it into a pure string helper lands the foreign command in
# $3 unmangled. The command substitution around `trap -p DEBUG` reports
# the parent shell's trap (POSIX special case; bash 3.2 honors it at top
# level).
__otty_extract_trap() { printf '%s' "$3"; }
__otty_rearm='__otty_trap_cur="$(trap -p DEBUG)"
case "$__otty_trap_cur" in
*__otty_debug_trap*) ;;
"") trap __otty_debug_trap DEBUG ;;
*)
    __otty_trap_foreign="$(eval "__otty_extract_trap $__otty_trap_cur")"
    if [ -n "$__otty_trap_foreign" ]; then
        trap "$__otty_trap_foreign
__otty_debug_trap" DEBUG
    else
        trap __otty_debug_trap DEBUG
    fi
    ;;
esac'

# ── OSC 7 reporting ──
# Every OSC 7 goes through here. bash reports CWD from PROMPT_COMMAND, which
# never runs inside a command substitution, so the cwd path was never exposed
# the way zsh's chpwd hook was — but the `ssh` wrapper below is an ordinary
# function the user can call as `logs=$(ssh host cat …)`, and an unguarded
# write there wraps the captured output in OSC 7. Returns non-zero when it
# suppresses, so callers can tell "reported" from "skipped".
#
# BASH_SUBSHELL — non-zero inside `$( )`, `( … )`, a pipeline segment or a
# background job. None of those move the interactive shell, and the first one's
# stdout is the pipe the parent reads back as a value.
# -t 1 — stdout redirected to a file or pipe means the escape would be injected
# into that target instead of reaching the terminal.
__otty_osc7() {
    (( BASH_SUBSHELL == 0 )) && [[ -t 1 ]] || return 1
    printf '\e]7;%s\a' "$1"
}

__otty_last_reported_cwd=""
__otty_report_cwd() {
    # The memo only advances on an emission that actually went out; advancing it
    # on a suppressed one would mark the directory reported and stop the next
    # prompt from retrying it.
    if [[ "$PWD" != "$__otty_last_reported_cwd" ]]; then
        __otty_osc7 "file://${HOSTNAME}${PWD}" && __otty_last_reported_cwd="$PWD"
    fi
    return 0
}
# Emit initial CWD.
__otty_report_cwd

# ── Dump resolved $PATH for the autocomplete engine ──
# The app process inherits only the bare launchd PATH (/usr/bin:/bin:…);
# the completion engine reads this dump to scan the user's real PATH
# (Homebrew/cargo/mise/…) for command-name completion. Once-only — PATH is
# captured after .bashrc has finished setting it up.
__otty_path_dumped=""
__otty_dump_path() {
    [[ -n "$__otty_path_dumped" ]] && return
    __otty_path_dumped=1
    # >| (explicit clobber) so the fixed-path cache write still succeeds under
    # `set -o noclobber`; a plain > errors with "cannot overwrite" each shell.
    printf '%s' "$PATH" >| "${TMPDIR:-/tmp}/otty-path-${EUID}" 2>/dev/null
}

# ── Omit `otty` prefix (live-toggleable) ──
# Install `edit`/`view`/`watch`/`jump`/`learn` wrappers ONCE; each one stat()s the
# omit_prefix sentinel under $OTTY_FLAGS_DIR at call time so the Settings
# toggle takes effect immediately, without waiting for the next
# PROMPT_COMMAND cycle. Sentinel absent → fall through to `command <name>`
# so PATH lookup behaves exactly as if the wrapper weren't there (system
# `watch` on Linux still works). By default, names already defined as user
# functions/aliases are left untouched; the `allow_overwrite` sentinel
# (evaluated at shell init — restart the shell for the toggle to take
# effect) opts into replacing them.
__otty_should_wrap() {
    if [[ -n "$OTTY_FLAGS_DIR" && -f "$OTTY_FLAGS_DIR/allow_overwrite" ]]; then
        # An alias outranks a function at command lookup, so a leftover one
        # would shadow the wrapper we are about to install. Drop it here so
        # our wrapper actually wins.
        alias "$1" >/dev/null 2>&1 && unalias "$1"
        return 0
    fi
    declare -F "$1" >/dev/null 2>&1 && return 1
    alias "$1" >/dev/null 2>&1 && return 1
    return 0
}

# The wrappers below use the `function NAME { … }` keyword form, NOT the
# `NAME() { … }` form. Aliases are expanded at PARSE time, so for a user with
# e.g. `alias view='vim -R'` in an rc file sourced ahead of us, the parser
# rewrites `view()` into `vim -R()` and the whole script dies on a syntax error
# — taking OSC markers, CWD reporting and SSH integration down with it. The
# `__otty_should_wrap view &&` guard on the same line cannot save us: the entire
# `A && B` list is parsed before any of it runs. The keyword form suppresses
# alias expansion of the name, so the definition parses no matter what the user
# has aliased. Don't "simplify" it back to `view() {`.
__otty_should_wrap edit && function edit {
    if [[ -n "$OTTY_FLAGS_DIR" && -f "$OTTY_FLAGS_DIR/omit_prefix" ]]; then
        command otty edit "$@"
    else
        command edit "$@"
    fi
}
__otty_should_wrap view && function view {
    if [[ -n "$OTTY_FLAGS_DIR" && -f "$OTTY_FLAGS_DIR/omit_prefix" ]]; then
        command otty view "$@"
    else
        command view "$@"
    fi
}
__otty_should_wrap watch && function watch {
    if [[ -n "$OTTY_FLAGS_DIR" && -f "$OTTY_FLAGS_DIR/omit_prefix" ]]; then
        command otty watch "$@"
    else
        command watch "$@"
    fi
}
# The one jump implementation, behind `otty jump`, the bare `jump` wrapper and
# any `cli-alias` mapped to jump. The CLI is a child process and cannot move
# this shell, so left alone it has the app type `cd '<path>'` into the pane —
# echoed by line-discipline, and read back as input so history records the cd
# instead of what the user ran. Zoxide-style instead: ask the app to resolve
# only (`--no-cd`) and `builtin cd` here.
__otty_jump() {
    # A management subcommand (`otty jump ls`, `jump add …`) and any flag
    # (`--no-cd`, `--help`, `--json` …) ask for the CLI's own output, not a
    # cd — hand the invocation over untouched.
    case "$1" in add|remove|view|ls|import) command otty jump "$@"; return ;; esac
    local _otty_a
    for _otty_a in "$@"; do
        case "$_otty_a" in -*) command otty jump "$@"; return ;; esac
    done
    if (( $# == 0 )); then
        # No-args: toggle between $HOME and the last jump-source dir.
        # Local-only state ($OTTY_JUMP_PREV) so up-arrow still recalls
        # the user's literal `jump` invocation, not an injected `cd`.
        if [[ "$PWD" == "$HOME" && -n "$OTTY_JUMP_PREV" ]]; then
            local _otty_dest="$OTTY_JUMP_PREV"
            OTTY_JUMP_PREV="$PWD"
            builtin cd -- "$_otty_dest"
        else
            OTTY_JUMP_PREV="$PWD"
            builtin cd -- "$HOME"
        fi
        return
    fi
    local _otty_p
    _otty_p=$(command otty jump --no-cd "$@") || return
    if [[ -n "$_otty_p" ]]; then
        OTTY_JUMP_PREV="$PWD"
        builtin cd -- "$_otty_p"
    fi
}
__otty_should_wrap jump && function jump {
    if [[ -n "$OTTY_FLAGS_DIR" && -f "$OTTY_FLAGS_DIR/omit_prefix" ]]; then
        __otty_jump "$@"
    else
        command jump "$@"
    fi
}
# `otty` itself, so the prefixed `otty jump <query>` gets the local cd too.
# Only the bare `jump` subcommand is intercepted; everything else — `jump:add`
# and friends included — goes straight to the binary.
__otty_should_wrap otty && function otty {
    if [[ "$1" == jump ]]; then
        shift
        __otty_jump "$@"
    else
        command otty "$@"
    fi
}
__otty_should_wrap learn && function learn {
    if [[ -n "$OTTY_FLAGS_DIR" && -f "$OTTY_FLAGS_DIR/omit_prefix" ]]; then
        command otty learn "$@"
    else
        command learn "$@"
    fi
}
# `watch:<agent>` — wait for a Claude/Codex/OpenCode session to reach
# idle. The colon is part of the command name as far as the shell is
# concerned, so the bare `watch` wrapper above never catches it; install
# one wrapper per agent. Keep in sync with WATCH_AGENTS in cli/src/main.rs.
for __otty_a in claude codex opencode; do
    __otty_should_wrap "watch:$__otty_a" && eval "watch:$__otty_a() {
        if [[ -n \"\$OTTY_FLAGS_DIR\" && -f \"\$OTTY_FLAGS_DIR/omit_prefix\" ]]; then
            command otty watch:$__otty_a \"\$@\"
        else
            command watch:$__otty_a \"\$@\"
        fi
    }"
done
unset __otty_a

# ── Custom CLI aliases ($OTTY_CLI_ALIASES = "name=cmd|name=cmd|…") ──
if [[ -n "$OTTY_CLI_ALIASES" ]]; then
    __otty_install_aliases() {
        local IFS='|'
        local pair name cmd
        for pair in $OTTY_CLI_ALIASES; do
            [[ -z "$pair" ]] && continue
            name="${pair%%=*}"
            cmd="${pair#*=}"
            [[ -z "$name" || -z "$cmd" || "$name" == "$pair" ]] && continue
            [[ "$name" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
            [[ "$cmd"  =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue
            __otty_should_wrap "$name" || continue
            # jump needs the local cd: `command otty jump` would re-introduce
            # the visible `cd '<path>'` echo plus a spurious shell-history
            # entry. Call __otty_jump, not the bare `jump` wrapper — that one
            # falls through to a system `jump` whenever omit_prefix is off.
            case "$cmd" in
                jump)
                    eval "$name() { __otty_jump \"\$@\"; }"
                    ;;
                *)
                    eval "$name() { command otty $cmd \"\$@\"; }"
                    ;;
            esac
        done
    }
    __otty_install_aliases
    unset -f __otty_install_aliases
fi

unset -f __otty_should_wrap

# Install hooks. The re-arm eval runs first so the rest of the prompt
# cycle (and the next command) sees our DEBUG trap even right after a
# framework stole it.
if [[ -z "$PROMPT_COMMAND" ]]; then
    PROMPT_COMMAND='__otty_prompt_exit=$?; eval "$__otty_rearm"; __otty_prompt_command "$__otty_prompt_exit"; __otty_install_ps1; __otty_report_cwd; __otty_dump_path'
else
    PROMPT_COMMAND='__otty_prompt_exit=$?; eval "$__otty_rearm"; __otty_prompt_command "$__otty_prompt_exit"; __otty_install_ps1; __otty_report_cwd; __otty_dump_path; '"$PROMPT_COMMAND"
fi
# Initial arm, chain-aware: a framework loaded before us (bash-preexec et
# al.) keeps its DEBUG trap, ours is appended to it. Top level — see the
# __otty_rearm comment.
eval "$__otty_rearm"

# ── Shift+Enter → insert newline ──
# Otty encodes Shift+Enter as CSI u (`ESC [ 13 ; 2 u`) so agent TUIs (Claude
# Code, codex, …) can tell it apart from plain Enter (same convention as
# kitty/Ghostty). readline doesn't speak CSI u: unbound, the unmatched tail
# self-inserts as literal `;2u`. Map it to quoted-insert + LF — quoted-insert
# reads the macro's \C-j directly (it is NOT dispatched to accept-line), so a
# literal newline lands in the buffer and Shift+Enter composes multi-line
# commands at the prompt.
bind '"\e[13;2u": "\C-v\C-j"' 2>/dev/null

# ── Command history scope ($OTTY_HISTORY_SCOPE) ──
# bash's default is worse than zsh's: without `histappend` the LAST shell to
# exit OVERWRITES the file, so commands from every other window are simply
# lost. Both modes therefore turn it on; they differ only in whether a pane
# reads back what its siblings wrote while it was alive.
# An unset variable leaves bash's own behavior untouched.
#
# These override the no-op `__otty_history_hook` defined near the top, which
# `__otty_prompt_command` calls on every prompt.
case "${OTTY_HISTORY_SCOPE:-}" in
    shared)
        shopt -s histappend
        __otty_history_hook() {
            builtin history -a
            builtin history -c
            builtin history -r
        }
        ;;
    per-pane)
        shopt -s histappend
        # Append after every command — never read siblings back into this
        # pane. Appending as we go (rather than only at exit) is also what
        # lets a restored pane recover its history after a crash.
        __otty_history_hook() { builtin history -a; }
        ;;
esac

# ── Up/Down arrow behavior ($OTTY_HISTORY_UP_ARROW) ──
# readline's `history-search-*` filters by what is already typed, which is the
# bash equivalent of zsh's `up-line-or-beginning-search`. Both the normal
# (`\e[A`) and application-mode (`\eOA`) encodings are bound.
if [[ $- == *i* ]]; then
    case "${OTTY_HISTORY_UP_ARROW:-}" in
        prefix-search)
            bind '"\e[A": history-search-backward' 2>/dev/null
            bind '"\eOA": history-search-backward' 2>/dev/null
            bind '"\e[B": history-search-forward' 2>/dev/null
            bind '"\eOB": history-search-forward' 2>/dev/null
            ;;
        sequential)
            bind '"\e[A": previous-history' 2>/dev/null
            bind '"\eOA": previous-history' 2>/dev/null
            bind '"\e[B": next-history' 2>/dev/null
            bind '"\eOB": next-history' 2>/dev/null
            ;;
    esac
fi

# ── SSH Integration ──
if [[ "${OTTY_SSH_INTEGRATION:-}" == "1" ]]; then
    # ── Helpers for the remote `otty` bootstrap (ANYWHERE_SPEC §6.2) ──────
    #
    # The bootstrap changes the ssh command line, so every one of these
    # answers "may we?" and every one of them fails CLOSED: when in doubt the
    # invocation stays byte for byte what the user typed.

    # Does this command line ask for something other than an interactive
    # session? -N / -W (forwarding), -O (control command), -G (config dump),
    # -T (no tty), -s (subsystem). BatchMode and RequestTTY are NOT looked for
    # here — `ssh -G` resolves those, and it sees ssh_config as well as argv.
    #
    # Walks each token letter by letter and stops at the first value-taking
    # one, so an option's value can never be mistaken for a flag: `-i ~/id_T`
    # and `-oCiphers=…` are tokens this must not react to. Returns 0 = blocked.
    #
    # `shopt -u nocasematch` around the walk, restored after: this payload is
    # SOURCED into the user's shell and inherits their options, and with
    # nocasematch set every `case` pattern here turns case-insensitive — `-c`
    # would match the flag list's `-C`, `-t` would match `-T`. That is exactly
    # how 124fa43c's double-execution bug came back in 2026-08; `shopt` is
    # global, not function-scoped, hence save and restore rather than set.
    __otty_ssh_blocks_bootstrap() {
        local tok body ch skip=0 _nc=0 _rc=1
        shopt -q nocasematch && _nc=1
        shopt -u nocasematch
        for tok in "$@"; do
            if (( skip )); then
                skip=0
                continue
            fi
            case "$tok" in
                # Everything after `--` is the destination (we are only called
                # when there is no remote command), so the scan is done.
                --) break ;;
                -?*) ;;
                *) continue ;;
            esac
            body="${tok#-}"
            while [[ -n "$body" ]]; do
                ch="${body:0:1}"
                body="${body:1}"
                case "$ch" in
                    N|W|O|G|T|s) _rc=0; break 2 ;;
                    # Value-taking (OpenSSH's own getopt string): the rest of
                    # the token is its value, or the next token when there is
                    # no rest.
                    b|c|e|i|l|m|o|p|B|D|E|F|I|J|L|P|Q|R|S|w)
                        [[ -z "$body" ]] && skip=1
                        break ;;
                esac
            done
        done
        (( _nc )) && shopt -s nocasematch
        return $_rc
    }

    # Is $2 (the alias as typed) or $3 (user@hostname) a line of the file $1?
    # One entry per line, written by the app; a missing or unreadable file
    # means "no host qualifies", which is what a fresh install looks like.
    __otty_ssh_host_listed() {
        local file="$1" line
        [[ -n "$file" && -r "$file" ]] || return 1
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            [[ -n "$2" && "$line" == "$2" ]] && return 0
            [[ -n "$3" && "$line" == "$3" ]] && return 0
        done < "$file"
        return 1
    }

    # The remote login shell we have on record for this host, or nothing.
    # `<config_dir>/remote-shells`, lines of `<host> <shell>`; the directory is
    # taken from $OTTY_HELPER_HOSTS so a debug build reads its own config dir.
    # Nothing is probed: asking the host would cost a connection and could
    # raise an auth prompt, and "we do not know" is an answer this may give.
    __otty_ssh_recorded_shell() {
        local file="${OTTY_HELPER_HOSTS%/*}/remote-shells" host sh_name
        [[ -n "${OTTY_HELPER_HOSTS:-}" && -r "$file" ]] || return 0
        while IFS=' ' read -r host sh_name; do
            [[ -z "$host" || -z "$sh_name" ]] && continue
            if [[ ( -n "$1" && "$host" == "$1" ) || ( -n "$2" && "$host" == "$2" ) ]]; then
                printf '%s\n' "$sh_name"
                return 0
            fi
        done < "$file"
    }

    ssh() {
        local _otty_term="xterm-256color"
        local -a _otty_opts=(
            -o "SetEnv=COLORTERM=truecolor TERM_PROGRAM=otty"
            -o "SendEnv=COLORTERM TERM_PROGRAM TERM_PROGRAM_VERSION"
        )

        # ── Resolve the connection via ssh -G ──
        # `ssh -G` reports the FULLY resolved configuration: ~/.ssh/config,
        # /etc/ssh/ssh_config, Match blocks and the caller's own `-o`, with
        # %-tokens and `~` already expanded. ControlPath is read from here for
        # that reason — scanning argv can only ever see the `-o` form, so a
        # ControlPath set in a config file was invisible and the wrapper minted
        # a competing socket over it, replacing the user's multiplexing setup
        # and cutting their ControlPersist down to its own 60s. Reported
        # against 1.4.1; on a ProxyJump setup it meant re-authenticating and
        # re-resolving the jump on every single ssh.
        #
        # The early `break` this loop used to have is what made that
        # unfixable-looking: `controlpath` is printed well after user and
        # hostname, so bailing out once those two were set skipped it every
        # time. Reading ~80 more lines of already-generated output costs
        # nothing measurable.
        local _otty_user="" _otty_hostname="" _otty_ctl_path="" _otty_ctl_master=""
        local _otty_alias=""
        # Read for the remote `otty` bootstrap below, not for the connection:
        # these three are how ssh_config (not just argv) says "this is not an
        # interactive session". Absent on OpenSSH < 8.7, which reads as "no
        # objection" — the argv scan still covers the flags themselves.
        local _otty_batchmode="" _otty_requesttty="" _otty_sessiontype=""
        while IFS=' ' read -r _k _v; do
            case "$_k" in
                user) _otty_user="$_v" ;;
                hostname) _otty_hostname="$_v" ;;
                batchmode) _otty_batchmode="$_v" ;;
                requesttty) _otty_requesttty="$_v" ;;
                sessiontype) _otty_sessiontype="$_v" ;;
                # The alias as typed, before ssh_config resolved it — the key
                # Otty's saved passwords are filed under. `hostname` above is
                # the resolved address and would not match.
                host) _otty_alias="$_v" ;;
                # `none` is OpenSSH's way of saying "no multiplexing", and some
                # versions omit the line entirely when it is unset (macOS's
                # OpenSSH 10.2 does), so absent and `none` mean the same thing.
                controlpath) [[ "$_v" != "none" ]] && _otty_ctl_path="$_v" ;;
                controlmaster) _otty_ctl_master="$_v" ;;
            esac
        done < <(command ssh -G "$@" 2>/dev/null)

        # ── Saved password (Settings › Remote › the Key column) ──
        #
        # `SSH_ASKPASS_REQUIRE=force` takes away ssh's fallback of asking on the
        # terminal, so it is set ONLY for a host Otty really has a password for
        # — otherwise a host with no saved password would go straight to
        # "Permission denied" instead of prompting. The list is a plain file
        # (one alias per line, written by the app) precisely so this check costs
        # a read and not a process.
        #
        # Decided BEFORE the terminfo probe below, which opens a connection of
        # its own (`ControlMaster=no`): without this the probe would spend a
        # failed password attempt on every ssh to such a host, which is exactly
        # what fail2ban counts.
        if [[ -n "${OTTY_ASKPASS_HOSTS:-}" && -n "$_otty_alias" && -r "${OTTY_ASKPASS_HOSTS}" ]]; then
            local _otty_askpass_bin="${OTTY_ASKPASS_HOSTS%/*}/otty-askpass"
            if [[ -x "$_otty_askpass_bin" ]]; then
                local _otty_askpass_line
                while IFS= read -r _otty_askpass_line; do
                    [[ "$_otty_askpass_line" == "$_otty_alias" ]] || continue
                    # `local -x` scopes the export to this call: the next
                    # command in the pane must not inherit an SSH_ASKPASS
                    # pointing at one host's password.
                    local -x SSH_ASKPASS="$_otty_askpass_bin"
                    local -x SSH_ASKPASS_REQUIRE="force"
                    local -x OTTY_ASKPASS_ALIAS="$_otty_alias"
                    break
                done < "${OTTY_ASKPASS_HOSTS}"
            fi
        fi

        # A ControlPath is only a *name* until something creates the socket.
        # `ssh -G` prints the configured path whether or not multiplexing is
        # actually on, so a `ControlPath` in ssh_config with ControlMaster left
        # at its default `no` looked exactly like a live master here: nothing
        # was minted, and `socket=` below named a path that never comes into
        # existence — every panel SFTP/exec then silently re-authenticates.
        # An already-open socket still counts (a master someone else started,
        # including Otty's own inherit-SSH, which passes ControlMaster=auto),
        # so only a path with neither a master nor a socket is dropped.
        if [[ -n "$_otty_ctl_path" && "$_otty_ctl_master" == "no" && ! -S "$_otty_ctl_path" ]]; then
            _otty_ctl_path=""
        fi

        local _otty_target="${_otty_user}@${_otty_hostname}"
        local _otty_meta_file="${TMPDIR:-/tmp}/otty-ssh-shell-$$.meta"
        local _otty_ctl_dir=""

        # ── Split argv into connection args and the remote command ──
        # `ssh host cmd` folds every trailing argument into ONE remote command
        # string, so anything appended after the user's own arguments lands as
        # arguments OF their command. The terminfo probe below used to pass
        # "$@" and then append its script, which made the remote run
        #     <user command> otty_term=…; <probe script>
        # — the user's command executed a second time, in the probe connection,
        # with a stray argument glued on.
        #
        # Where the connection arguments end is a getopt(3) question — which
        # letters take a value, how clusters like `-vp 2222` split, that ssh
        # re-enters getopt after the destination — and it is answered in ONE
        # place, `otty ssh scan-argv` (packages/ssh-argv), which the macOS app
        # also reaches through the FFI. It used to be four transcriptions of
        # the same rules in four languages, and they drifted: OpenSSH 9.4's
        # `-P tag` took a value in none of them.
        #
        # `otty` is on PATH here — the payload prepends $OTTY_BIN_DIR at the
        # top of this file. When it is not (a hand-installed payload with no
        # app behind it), the safe degradation is the one already used for a
        # ControlPath we cannot mint: skip the extras and let plain ssh run.
        local -a _otty_conn=()
        local _otty_has_cmd=0 _otty_scan_ok=0
        local _otty_scan
        _otty_scan="$(command otty ssh scan-argv -- "$@" 2>/dev/null)"
        case "$_otty_scan" in
            conn=*\ cmd=*)
                local _otty_n="${_otty_scan#conn=}"
                _otty_n="${_otty_n%% *}"
                _otty_has_cmd="${_otty_scan##*cmd=}"
                # Positional parameters are 1-based in both shells, and a count
                # of 0 yields an empty array rather than an error.
                _otty_conn=("${@:1:$_otty_n}")
                _otty_scan_ok=1
                ;;
        esac

        # `_otty_scan_ok` too: without a scan we do not know where the
        # user's remote command begins, so neither the probe nor a
        # ControlMaster (whose teardown needs the same split) may run.
        if [[ -n "$_otty_hostname" ]] && (( _otty_scan_ok )); then
            # Set up a ControlMaster socket so the main connection can be reused
            # for SFTP/exec by the file/git panels.
            #
            # A ControlPath already in effect — from the user's ssh_config, or
            # from the caller's own `-o ControlPath=<socket>`, which is how
            # Otty's split-pane "inherit SSH" rides the existing master with no
            # re-auth and no proxy/alias re-resolution — is honored as-is, and
            # nothing is minted over it. Both cases arrive the same way, in
            # $_otty_ctl_path off `ssh -G` above; there is deliberately no
            # second scan of argv, because having two sources for one answer is
            # what let the config-file case go unnoticed.
            if [[ -z "$_otty_ctl_path" ]]; then
                # ControlPath must stay short: macOS caps Unix-domain socket
                # paths at 104 bytes and $TMPDIR (/var/folders/...) already eats
                # ~49 of them, so the socket goes directly under /tmp instead.
                # mktemp -d yields a fresh 0700 dir, keeping this race-free.
                #
                # Deliberately no fallback when mktemp is unavailable. A
                # predictable name like /tmp/otty-ssh.$$ in a world-writable
                # directory can be pre-created by another local user, and this
                # path becomes the `-o ControlPath` that every panel exec and
                # SFTP transfer rides — the same class of attack the host guards
                # against when it vets the metadata file's ownership. Skipping
                # multiplexing is the safe degradation: socket= is written empty
                # and the host simply leaves controlSocket unset.
                _otty_ctl_dir="$(mktemp -d /tmp/otty-ssh.XXXXXX 2>/dev/null)"
                if [[ -n "$_otty_ctl_dir" ]]; then
                    _otty_ctl_path="$_otty_ctl_dir/socket"
                    _otty_opts+=(
                        -o "ControlMaster=auto"
                        -o "ControlPath=$_otty_ctl_path"
                        -o "ControlPersist=60"
                    )
                fi
            fi

            # Terminfo install (cached per user@host).
            #
            # Skipped entirely when the caller supplied a remote command: a
            # probe there would open a second connection behind the back of a
            # script that asked for exactly one, and terminfo only matters for a
            # session that has a tty. TERM then stays at the xterm-256color
            # default, which every system terminfo database already resolves.
            local _otty_cache_dir="${TMPDIR:-/tmp}/otty-terminfo-cache"
            command mkdir -p "$_otty_cache_dir" 2>/dev/null
            local _otty_cache_file="$_otty_cache_dir/$_otty_target"

            if [[ -f "$_otty_cache_file" ]]; then
                _otty_term="${TERM:-xterm-256color}"
            elif (( ! _otty_has_cmd )) && command -v infocmp >/dev/null 2>&1; then
                local _otty_terminfo
                _otty_terminfo="$(infocmp -0 -x 2>/dev/null)"

                if [[ -n "$_otty_terminfo" ]]; then
                    # Deliberately NOT sharing $_otty_ctl_path here. This probe
                    # used to run with ControlMaster=yes and hand its master to
                    # the interactive ssh below, saving one auth round-trip. But
                    # that demoted the interactive connection to a mux *client*,
                    # and a client's interactive session can hang with no output
                    # at all — no banner, no prompt, nothing — which reads as
                    # "ssh froze". It only bit on the first connection to a host
                    # (later ones hit the terminfo cache and skipped this probe
                    # entirely), which is what made it look intermittent.
                    # ControlPath=none keeps the probe off the socket so the
                    # interactive ssh below stays the master.
                    # Remote command (TERM injected via the leading assignment):
                    #   1. If the remote already resolves this TERM (system DB or
                    #      a user entry), do nothing — never shadow a standard
                    #      name like xterm-256color with our copy.
                    #   2. Otherwise install via `tic`, then verify the fresh
                    #      entry actually has `clear=`; a truncated transfer can
                    #      compile a stub that lacks clear/cup and breaks tput &
                    #      tmux. If incomplete, remove it and fail so we fall back
                    #      to the system database instead of a broken shadow.
                    # The remote command is handed to the REMOTE account's
                    # LOGIN SHELL, which is not necessarily POSIX. A fish login
                    # shell rejects a leading `otty_term=…` assignment outright
                    # ("Unsupported use of '='"), so on those hosts the probe
                    # failed every time — silently, and since a failed probe is
                    # deliberately not cached, on every single connection.
                    # Wrapping it in `/bin/sh -c '…'` leaves the login shell
                    # three plain words and one single-quoted token to parse,
                    # which fish, csh and every POSIX shell agree on, and the
                    # POSIX syntax inside is parsed by /bin/sh where it belongs.
                    #
                    # $TERM is spliced into that single-quoted string, so a
                    # value carrying a quote would break out of it. Belt and
                    # braces — a name outside [A-Za-z0-9._+-] has no local
                    # terminfo entry either, so `infocmp -0 -x` above would
                    # have come back empty and this branch never run.
                    local _otty_safe_term="${TERM:-xterm-256color}"
                    case "$_otty_safe_term" in
                        *[!A-Za-z0-9._+-]*) _otty_safe_term="xterm-256color" ;;
                    esac
                    if echo "$_otty_terminfo" | command ssh \
                        -o "ControlMaster=no" \
                        -o "ControlPath=none" \
                        "${_otty_conn[@]}" \
                        "/bin/sh -c 'otty_term=$_otty_safe_term; "'command -v infocmp >/dev/null 2>&1 && infocmp "$otty_term" >/dev/null 2>&1 && exit 0; command -v tic >/dev/null 2>&1 || exit 1; mkdir -p ~/.terminfo 2>/dev/null; tic -x - >/dev/null 2>&1 || exit 1; command -v infocmp >/dev/null 2>&1 && { infocmp -1 "$otty_term" 2>/dev/null | grep -q "clear=" || { rm -f ~/.terminfo/?/"$otty_term" ~/.terminfo/??/"$otty_term" 2>/dev/null; exit 1; }; }; exit 0'"'" \
                        2>/dev/null; then
                        _otty_term="${TERM:-xterm-256color}"
                        command touch "$_otty_cache_file"
                    elif [[ -n "${OTTY_DEBUG_SSH:-}" ]]; then
                        # Nothing is cached on failure — deliberately, so a host
                        # that later grows `tic` is picked up — which means a
                        # permanently failing probe re-runs on every connection.
                        # Opt-in only: an unconditional line here would land in
                        # the middle of `logs=$(ssh host …)`.
                        printf '%s\n' "otty: terminfo probe failed for $_otty_target; TERM stays xterm-256color" >&2
                    fi
                fi
            fi

            # Write metadata file so Otty's Swift side can find the resolved
            # session info (user, hostname, socket) even when ~/.ssh/config
            # aliases the user or hostname. Under umask 077: the app refuses a
            # file others could have written, and Debian's default umask 002
            # would make this one group-writable.
            (
                builtin umask 077
                {
                    echo "user=${_otty_user}"
                    echo "hostname=${_otty_hostname}"
                    echo "socket=${_otty_ctl_path}"
                } > "$_otty_meta_file"
            ) 2>/dev/null
        fi

        # ── Remote `otty` bootstrap (ANYWHERE_SPEC §6.2) ──────────────────
        #
        # On a host Otty has put its helper on, `otty view foo.log` typed in
        # the remote shell should reach the Otty the user is sitting at. That
        # needs two things a plain `ssh host` does not give it: the shim on the
        # remote $PATH, and a tty for the shim to write its OSC to. Both are
        # bought by turning the invocation into `ssh -t host sh -c …`, which is
        # a DIFFERENT command line — so it happens only when every condition
        # below holds, and otherwise this block leaves the invocation byte for
        # byte as it is today.
        #
        #   * the argv scan succeeded and found no remote command. `ssh host
        #     uname -a` asks for one command, not a login shell; appending ours
        #     would fold the user's into it.
        #   * stdin is a tty. `ssh host <script` has a destination but no
        #     session to bootstrap.
        #   * nothing asks for something other than a shell — the flags, from
        #     argv, and BatchMode / RequestTTY / SessionType, from `ssh -G`,
        #     which also sees the user's ssh_config.
        #   * the host is one the app has recorded, by the alias as typed or by
        #     user@hostname. No file means no host, which is where a fresh
        #     install starts.
        local -a _otty_boot=()
        if (( _otty_scan_ok )) && (( ! _otty_has_cmd )) && [[ -t 0 ]] \
           && [[ "$_otty_batchmode" != "yes" && "$_otty_requesttty" != "false" ]] \
           && [[ "$_otty_sessiontype" != "none" && "$_otty_sessiontype" != "subsystem" ]] \
           && ! __otty_ssh_blocks_bootstrap "${_otty_conn[@]}"; then
            if __otty_ssh_host_listed "${OTTY_HELPER_HOSTS:-}" "$_otty_alias" "$_otty_target"; then
                # One argument, handed to the remote account's LOGIN shell —
                # which is not necessarily POSIX. Three plain words and a
                # single-quoted token is the shape fish, csh and every POSIX
                # shell agree on; the same reasoning as the terminfo probe.
                #
                # Two stages, not one, because exporting PATH here and then
                # `exec $SHELL -l` does not work on the commonest server OS
                # there is: Debian's /etc/profile *assigns*
                #     PATH="/usr/local/bin:/usr/bin:/bin:…"
                # rather than adding to it, so the login shell throws our
                # directory away and `otty view` is "command not found".
                # (RHEL, Arch, Alpine and macOS all extend PATH instead, which
                # is why this looked fine until it was run on Debian.)
                #
                # So: stage one runs the login shell with `-l -c`, letting the
                # whole profile chain do its worst, and its last act is to
                # re-prepend our directory and `exec` an INTERACTIVE shell.
                # `.profile`/`.zprofile` run once in stage one, `.bashrc`/
                # `.zshrc` once in stage two (a login `-c` shell is not
                # interactive, so the `case $- in *i*` guard at the top of a
                # stock .bashrc returns immediately). zsh's `.zlogin` now runs
                # before `.zshrc`; accepted.
                #
                # Only for shells whose `-l -c` and `-i` spellings we know.
                # An unrecognized login shell (csh, tcsh, nu, …) gets exactly
                # what it got before — a plain `exec $SHELL -l`, with the PATH
                # export that works wherever the profile chain is additive —
                # because a two-stage exec it cannot parse would not degrade,
                # it would end the session.
                #
                # $OTTY_REMOTE_SHELL overrides $SHELL when the host knows
                # better (and lets the shells be exercised without chsh).
                _otty_opts+=(-t)
                _otty_boot=('sh -c '\''export OTTY_REMOTE=1; S="${OTTY_REMOTE_SHELL:-${SHELL:-/bin/sh}}"; D="$HOME/.local/share/otty/bin"; case "${S##*/}" in bash|zsh|sh|dash|ksh|ash) exec "$S" -l -c "PATH=\"$D:\$PATH\"; export PATH; exec \"$S\" -i" ;; fish) exec "$S" -l -c "set -gx PATH \"$D\" \$PATH; exec \"$S\" -i" ;; *) PATH="$D:$PATH"; export PATH; exec "$S" -l ;; esac'\')
            elif [[ "${OTTY_REMOTE_HELPER_POLICY:-ask}" != "never" \
                    && "$(__otty_ssh_recorded_shell "$_otty_alias" "$_otty_target")" == "bash" ]]; then
                # Zero-install fallback (§6.2 item 4). No helper on this host
                # means no shim to put on PATH, so `otty` is defined as a shell
                # function and exported — nothing is written to that machine at
                # all. bash only, and only when we already know the login shell
                # IS bash: zsh and fish cannot inherit a function through the
                # environment, and probing for the answer would cost a
                # connection (and could raise an auth prompt).
                #
                # The function is the shim's logic minus its escaping: a
                # backslash or a quote in a path is refused with a reason,
                # because a wrong JSON escape would be a silently corrupted
                # request and this is the route that has no helper to fall back
                # on. `printf` and `base64` are all it needs.
                _otty_opts+=(-t)
                _otty_boot=('bash -c '\''export OTTY_REMOTE=1; otty() { case "${1:-}" in view|edit|jump|learn|ignore) ;; *) printf "otty %s is not available on a remote host: only view, edit, jump, learn and ignore travel over the terminal.\n" "${1:-}" >&2; return 1;; esac; case "$*$PWD" in *\\*|*\"*) printf "otty: this host has no Otty helper, so paths with quotes or backslashes cannot be sent.\n" >&2; return 1;; esac; local c=$1 s= a; shift; local j="{\"v\":1,\"cmd\":\"$c\",\"args\":["; for a in "$@"; do j="$j$s\"$a\""; s=,; done; j="$j],\"cwd\":\"$PWD\"}"; j=$(printf %s "$j" | base64 | tr -d "\n=" | tr "+/" "-_"); { printf "\033]8801;otty;%s\007" "$j" >/dev/tty; } 2>/dev/null || printf "\033]8801;otty;%s\007" "$j"; printf "Handed to Otty on your computer.\n"; }; export -f otty; exec bash -l'\')
            fi
        fi

        # Notify Otty we're entering an SSH session. Guarded like every other
        # OSC 7: `logs=$(ssh host cat …)` runs this wrapper in a
        # command-substitution subshell whose stdout is the capture pipe.
        __otty_osc7 "ssh://${_otty_hostname:-unknown}"

        # Spelled out twice rather than expanding a possibly-empty array into
        # the one call: the no-bootstrap line has to stay identical to what it
        # was before this feature existed, and "identical" should be visible.
        # (`"${arr[@]}"` on an empty array is also an error under `set -u` in
        # bash 3.2, which is what macOS still ships.)
        if (( ${#_otty_boot[@]} )); then
            TERM="$_otty_term" command ssh "${_otty_opts[@]}" "$@" "${_otty_boot[@]}"
        else
            TERM="$_otty_term" command ssh "${_otty_opts[@]}" "$@"
        fi
        local _ret=$?

        command rm -f "$_otty_meta_file" 2>/dev/null

        # Retire the ControlMaster we minted above. Only ours — a ControlPath the
        # caller passed in belongs to whoever created it, hence the _otty_ctl_dir
        # guard. Without this the master lingers for the whole ControlPersist
        # window and its mktemp'd directory was never removed at all, so every
        # ssh invocation leaked one empty /tmp/otty-ssh.XXXXXX forever.
        if [[ -n "$_otty_ctl_dir" ]]; then
            command ssh -o "ControlPath=$_otty_ctl_path" -O exit "${_otty_conn[@]}" >/dev/null 2>&1
            command rm -rf "$_otty_ctl_dir" 2>/dev/null
        fi
        # Sweep dirs orphaned by earlier crashes. -empty means a live master's
        # dir (it still holds its socket) is never touched. The trailing slash on
        # /tmp/ is load-bearing: on macOS /tmp is a symlink to private/tmp and
        # find won't descend into it without one, silently matching nothing.
        command find /tmp/ -maxdepth 1 -type d -name 'otty-ssh.*' -empty -delete 2>/dev/null

        # Restore local CWD reporting. Guarded in lockstep with the ssh:// mark
        # above — both conditions are unchanged across the call, so the pair is
        # always emitted together or not at all.
        __otty_osc7 "file://${HOSTNAME}${PWD}"
        return $_ret
    }
fi
