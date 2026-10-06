# Otty fish integration — payload (sourced from config.fish managed block)
#
# Idempotent: re-sourcing in the same shell is a no-op. `return` (not `exit`)
# so non-interactive fish that sources config.fish isn't killed.
set -q __otty_loaded; and return
set -g __otty_loaded 1

# Guard: only for interactive shells.
status is-interactive; or return

# ── Bundled `otty` on PATH ──
# Mirror of the bash/zsh logic: if "Install CLI" hasn't been run, the host
# passes the bundled binary's bin dir via $OTTY_BIN_DIR. fish_user_paths is
# the right level to prepend (won't fight the user's own PATH setup).
if test -n "$OTTY_BIN_DIR" -a -d "$OTTY_BIN_DIR"
    if not contains -- "$OTTY_BIN_DIR" $PATH
        set -gx PATH "$OTTY_BIN_DIR" $PATH
    end
end

set -g __otty_cmd_executed ""
set -g __otty_progress_active ""

# Match command line against $OTTY_PROGRESS_COMMANDS (pipe-separated list of
# whitespace-delimited prefixes, e.g. "git push").
function __otty_progress_match
    set -l cmd $argv[1]
    test -z "$OTTY_PROGRESS_COMMANDS"; and return 1
    # Strip leading `NAME=value` env assignments.
    while string match -rq '^[A-Za-z_][A-Za-z0-9_]*=.* .*$' -- $cmd
        set cmd (string replace -r '^[^ ]+ +' '' -- $cmd)
    end
    for p in (string split '|' -- $OTTY_PROGRESS_COMMANDS)
        test -z "$p"; and continue
        if test "$cmd" = "$p"
            return 0
        end
        if string match -q "$p *" -- $cmd
            return 0
        end
    end
    return 1
end

function __otty_prompt_start --on-event fish_prompt
    # Capture $status immediately — it holds the previous command's exit code.
    set -l last $status
    if test -n "$__otty_cmd_executed"
        printf '\e]133;D;%d\a' $last
    end
    if test -n "$__otty_progress_active"
        if test $last -eq 0
            printf '\e]9;4;5;0\a'
        else
            printf '\e]9;4;2\a'
        end
        set -g __otty_progress_active ""
    end
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
    if test -n "$OTTY_PROMPT_TOKEN" -a -z "$TMUX" -a -z "$STY" -a -z "$ZELLIJ"
        printf '\e]133;A;otty=%s\a' "$OTTY_PROMPT_TOKEN"
    else
        printf '\e]133;A\a'
    end
    set -g __otty_cmd_executed ""
end

function __otty_prompt_end --on-event fish_prompt
    printf '\e]133;B\a'
end

function __otty_preexec --on-event fish_preexec
    printf '\e]133;C\a'
    set -g __otty_cmd_executed 1
    if __otty_progress_match "$argv[1]"
        printf '\e]9;4;3\a'
        set -g __otty_progress_active 1
    end
end

# ── OSC 7 reporting ──
# Every OSC 7 goes through here. fish has no forked subshell and its
# `--on-variable PWD` handler writes to the real stdout rather than into a
# command substitution's capture buffer, so the cwd path was never exposed the
# way zsh's chpwd hook was — but the `ssh` wrapper below is an ordinary
# function, and `set logs (ssh host cat …)` runs it with stdout redirected into
# the capture pipe, wrapping the captured output in OSC 7.
#
# `status is-command-substitution` reports the dynamic context, so it is true
# inside a function called from a substitution — verified, not assumed.
# `isatty stdout` additionally covers a plain redirect to a file.
function __otty_osc7 --argument-names _otty_payload
    status is-command-substitution; and return 1
    isatty stdout; or return 1
    printf '\e]7;%s\a' $_otty_payload
end

function __otty_cwd_report --on-variable PWD
    __otty_osc7 "file://"(hostname)"$PWD"
end
# Emit initial CWD.
__otty_osc7 "file://"(hostname)"$PWD"

# ── Dump resolved $PATH for the autocomplete engine ──
# The app process inherits only the bare launchd PATH (/usr/bin:/bin:…);
# the completion engine reads this dump to scan the user's real PATH
# (Homebrew/cargo/mise/…) for command-name completion. Once-only — PATH is
# captured after config.fish has finished setting it up.
function __otty_dump_path --on-event fish_prompt
    set -q __otty_path_dumped; and return
    set -g __otty_path_dumped 1
    set -l tmpdir $TMPDIR
    test -n "$tmpdir"; or set tmpdir /tmp
    set -l dump "$tmpdir/otty-path-"(id -u)
    string join ':' $PATH >$dump 2>/dev/null
end

# ── Omit `otty` prefix + custom aliases (deferred to first prompt) ──
# These wrapper functions must be installed AFTER the user's config.fish so
# otty's wrappers win over same-named functions defined there, and so the
# allow_overwrite sentinel can actually replace a user's existing definition.
# But when auto-loaded from vendor_conf.d we run BEFORE config.fish — so defer
# the whole block to the first fish_prompt (mirrors the zsh deferred-precmd
# load). On the managed-block path (tmux) we're already sourced at the end of
# config.fish, where ordering is fine; deferring there is harmless.
function __otty_install_wrappers --on-event fish_prompt
    # One-shot: only the first prompt installs the wrappers.
    set -q __otty_wrappers_done; and return
    set -g __otty_wrappers_done 1

# Install `edit`/`view`/`watch`/`jump`/`learn` wrappers ONCE; each one stat()s the
# omit_prefix sentinel under $OTTY_FLAGS_DIR at call time so the Settings
# toggle takes effect immediately, without waiting for the next prompt
# event. Sentinel absent → fall through to `command <name>` so PATH lookup
# behaves exactly as if the wrapper weren't there (system `watch` on Linux
# still works). By default, names already defined as user functions are
# left untouched; the `allow_overwrite` sentinel (evaluated at shell init
# — restart the shell for the toggle to take effect) opts into replacing
# them.
function __otty_should_wrap
    if test -n "$OTTY_FLAGS_DIR" -a -f "$OTTY_FLAGS_DIR/allow_overwrite"
        # `alias` in fish is implemented as a function of the same name, so
        # removing the function covers both. Erase first so our wrapper wins.
        functions -q $argv[1]; and functions -e $argv[1]
        return 0
    end
    functions -q $argv[1]; and return 1
    return 0
end
__otty_should_wrap edit; and function edit
    if test -n "$OTTY_FLAGS_DIR" -a -f "$OTTY_FLAGS_DIR/omit_prefix"
        command otty edit $argv
    else
        command edit $argv
    end
end
__otty_should_wrap view; and function view
    if test -n "$OTTY_FLAGS_DIR" -a -f "$OTTY_FLAGS_DIR/omit_prefix"
        command otty view $argv
    else
        command view $argv
    end
end
__otty_should_wrap watch; and function watch
    if test -n "$OTTY_FLAGS_DIR" -a -f "$OTTY_FLAGS_DIR/omit_prefix"
        command otty watch $argv
    else
        command watch $argv
    end
end
# The one jump implementation, behind `otty jump`, the bare `jump` wrapper and
# any `cli-alias` mapped to jump. The CLI is a child process and cannot move
# this shell, so left alone it has the app type `cd '<path>'` into the pane —
# echoed by line-discipline, and read back as input so history records the cd
# instead of what the user ran. Zoxide-style instead: ask the app to resolve
# only (`--no-cd`) and `builtin cd` here.
function __otty_jump
    # A management subcommand (`otty jump ls`, `jump add …`) and any flag
    # (`--no-cd`, `--help`, `--json` …) ask for the CLI's own output, not a
    # cd — hand the invocation over untouched.
    if contains -- "$argv[1]" add remove view ls import
        command otty jump $argv
        return $status
    end
    for _otty_a in $argv
        if string match -q -- '-*' $_otty_a
            command otty jump $argv
            return $status
        end
    end
    if test (count $argv) -eq 0
        # No-args: toggle between $HOME and the last jump-source dir.
        # Local-only state ($OTTY_JUMP_PREV) so up-arrow still recalls
        # the user's literal `jump` invocation, not an injected `cd`.
        if test "$PWD" = "$HOME" -a -n "$OTTY_JUMP_PREV"
            set -l _otty_dest "$OTTY_JUMP_PREV"
            set -g OTTY_JUMP_PREV "$PWD"
            builtin cd -- "$_otty_dest"
        else
            set -g OTTY_JUMP_PREV "$PWD"
            builtin cd -- "$HOME"
        end
        return
    end
    set -l _otty_p (command otty jump --no-cd $argv); or return
    if test -n "$_otty_p"
        set -g OTTY_JUMP_PREV "$PWD"
        builtin cd -- $_otty_p
    end
end
__otty_should_wrap jump; and function jump
    if test -n "$OTTY_FLAGS_DIR" -a -f "$OTTY_FLAGS_DIR/omit_prefix"
        __otty_jump $argv
    else
        command jump $argv
    end
end
# `otty` itself, so the prefixed `otty jump <query>` gets the local cd too.
# Only the bare `jump` subcommand is intercepted; everything else — `jump:add`
# and friends included — goes straight to the binary.
__otty_should_wrap otty; and function otty
    if test "$argv[1]" = jump
        set -e argv[1]
        __otty_jump $argv
    else
        command otty $argv
    end
end
__otty_should_wrap learn; and function learn
    if test -n "$OTTY_FLAGS_DIR" -a -f "$OTTY_FLAGS_DIR/omit_prefix"
        command otty learn $argv
    else
        command learn $argv
    end
end
# `watch:<agent>` — wait for a Claude/Codex/OpenCode session to reach
# idle. The colon is part of the command name as far as the shell is
# concerned, so the bare `watch` wrapper above never catches it; install
# one wrapper per agent. Keep in sync with WATCH_AGENTS in cli/src/main.rs.
for __otty_a in claude codex opencode
    if __otty_should_wrap "watch:$__otty_a"
        eval "function watch:$__otty_a
            if test -n \"\$OTTY_FLAGS_DIR\" -a -f \"\$OTTY_FLAGS_DIR/omit_prefix\"
                command otty watch:$__otty_a \$argv
            else
                command watch:$__otty_a \$argv
            end
        end"
    end
end
set -e __otty_a

# ── Custom CLI aliases ($OTTY_CLI_ALIASES = "name=cmd|name=cmd|…") ──
if test -n "$OTTY_CLI_ALIASES"
    for __otty_pair in (string split '|' -- $OTTY_CLI_ALIASES)
        test -z "$__otty_pair"; and continue
        set -l __otty_name (string split -m1 '=' -- $__otty_pair)[1]
        set -l __otty_cmd  (string split -m1 '=' -- $__otty_pair)[2]
        test -z "$__otty_name" -o -z "$__otty_cmd"; and continue
        string match -rq '^[A-Za-z_][A-Za-z0-9_]*$' -- $__otty_name; or continue
        string match -rq '^[A-Za-z_][A-Za-z0-9_]*$' -- $__otty_cmd;  or continue
        __otty_should_wrap $__otty_name; or continue
        # Use eval to bake the captured $__otty_cmd into the function body so
        # later changes to the local var can't leak into the wrapper.
        # jump needs the local cd: `command otty jump` would re-introduce the
        # visible `cd '<path>'` echo plus a spurious shell-history entry.
        # Call __otty_jump, not the bare `jump` wrapper — that one falls
        # through to a system `jump` whenever omit_prefix is off.
        switch $__otty_cmd
            case jump
                eval "function $__otty_name; __otty_jump \$argv; end"
            case '*'
                eval "function $__otty_name; command otty $__otty_cmd \$argv; end"
        end
    end
    set -e __otty_pair
    set -e __otty_name
    set -e __otty_cmd
end

    functions -e __otty_should_wrap
end

# ── Command history scope ($OTTY_HISTORY_SCOPE) ──
# fish is already per-pane by design: a session reads the history file at
# startup and then keeps its own commands in memory, so a sibling's commands
# never appear mid-session. Upstream declined to make cross-session syncing a
# config option (fish-shell#825), which is why `history merge` exists as a
# manual verb. `per-pane` therefore needs no action; only `shared` does work.
if test "$OTTY_HISTORY_SCOPE" = shared
    function __otty_history_merge --on-event fish_prompt
        history merge
    end
end

# ── Up/Down arrow behavior ($OTTY_HISTORY_UP_ARROW) ──
# fish's default `up-or-search` already prefix-matches, so `prefix-search` just
# makes that explicit and pins it against a user rebinding.
#
# `sequential` is deliberately NOT implemented here. fish offers prefix search
# and "previous match" (which matches anywhere in the line), but no widget that
# ignores what is typed and walks the history entry by entry. Binding
# `history-search-backward` would silently give substring search instead of
# sequential walking, so fish keeps its own default rather than a wrong one.
if status is-interactive
    switch "$OTTY_HISTORY_UP_ARROW"
        case prefix-search
            bind \e\[A history-prefix-search-backward
            bind \eOA history-prefix-search-backward
            bind \e\[B history-prefix-search-forward
            bind \eOB history-prefix-search-forward
    end
end

# ── SSH Integration ──
if test "$OTTY_SSH_INTEGRATION" = "1"
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
    function __otty_ssh_blocks_bootstrap
        set -l skip 0
        for tok in $argv
            if test $skip -eq 1
                set skip 0
                continue
            end
            # Everything after `--` is the destination (we are only called
            # when there is no remote command), so the scan is done.
            if test "$tok" = "--"
                return 1
            end
            # A lone `-` is not an option, it is a destination. `-*` rather
            # than `-?*`: fish 4 dropped `?` as a glob wildcard, so the older
            # spelling silently matches nothing and every flag reads as
            # allowed.
            if test "$tok" = "-"; or not string match -q -- '-*' "$tok"
                continue
            end
            set -l body (string sub -s 2 -- "$tok")
            while test -n "$body"
                set -l ch (string sub -l 1 -- "$body")
                set body (string sub -s 2 -- "$body")
                switch $ch
                    case N W O G T s
                        return 0
                    # Value-taking (OpenSSH's own getopt string): the rest of
                    # the token is its value, or the next token when there is
                    # no rest.
                    case b c e i l m o p B D E F I J L P Q R S w
                        if test -z "$body"
                            set skip 1
                        end
                        break
                end
            end
        end
        return 1
    end

    # Is $argv[2] (the alias as typed) or $argv[3] (user@hostname) a line of
    # the file $argv[1]? One entry per line, written by the app; a missing or
    # unreadable file means "no host qualifies", which is what a fresh install
    # looks like.
    function __otty_ssh_host_listed
        set -l file $argv[1]
        if test -z "$file" -o ! -r "$file"
            return 1
        end
        set -l listed (command cat "$file" 2>/dev/null)
        for h in $argv[2..-1]
            if test -n "$h"; and contains -- "$h" $listed
                return 0
            end
        end
        return 1
    end

    # The remote login shell we have on record for this host, or nothing.
    # `<config_dir>/remote-shells`, lines of `<host> <shell>`; the directory is
    # taken from $OTTY_HELPER_HOSTS so a debug build reads its own config dir.
    # Nothing is probed: asking the host would cost a connection and could
    # raise an auth prompt, and "we do not know" is an answer this may give.
    function __otty_ssh_recorded_shell
        if test -z "$OTTY_HELPER_HOSTS"
            return 0
        end
        set -l file (string replace -r '/[^/]*$' '' -- "$OTTY_HELPER_HOSTS")"/remote-shells"
        if test ! -r "$file"
            return 0
        end
        for line in (command cat "$file" 2>/dev/null)
            set -l parts (string split -m 1 " " -- $line)
            if test (count $parts) -ne 2
                continue
            end
            for h in $argv
                if test -n "$h" -a "$parts[1]" = "$h"
                    echo $parts[2]
                    return 0
                end
            end
        end
    end

    function ssh --wraps=ssh
        set -l _otty_term "xterm-256color"
        set -l _otty_opts \
            -o "SetEnv=COLORTERM=truecolor TERM_PROGRAM=otty" \
            -o "SendEnv=COLORTERM TERM_PROGRAM TERM_PROGRAM_VERSION"

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
        #
        # `-m 1` on the split: a ControlPath may contain spaces, and splitting
        # on every space would keep only the first word of it.
        set -l _otty_user ""
        set -l _otty_hostname ""
        set -l _otty_ctl_path ""
        set -l _otty_ctl_master ""
        set -l _otty_alias ""
        # Read for the remote `otty` bootstrap below, not for the connection:
        # these three are how ssh_config (not just argv) says "this is not an
        # interactive session". Absent on OpenSSH < 8.7, which reads as "no
        # objection" — the argv scan still covers the flags themselves.
        set -l _otty_batchmode ""
        set -l _otty_requesttty ""
        set -l _otty_sessiontype ""
        for _line in (command ssh -G $argv 2>/dev/null)
            set -l _parts (string split -m 1 " " -- $_line)
            switch $_parts[1]
                case "user"
                    set _otty_user $_parts[2]
                case "hostname"
                    set _otty_hostname $_parts[2]
                case "batchmode"
                    set _otty_batchmode $_parts[2]
                case "requesttty"
                    set _otty_requesttty $_parts[2]
                case "sessiontype"
                    set _otty_sessiontype $_parts[2]
                case "host"
                    # The alias as typed, before ssh_config resolved it — the
                    # key Otty's saved passwords are filed under. `hostname`
                    # above is the resolved address and would not match.
                    set _otty_alias $_parts[2]
                case "controlpath"
                    # `none` is OpenSSH's way of saying "no multiplexing", and
                    # some versions omit the line entirely when it is unset
                    # (macOS's OpenSSH 10.2 does), so absent and `none` mean
                    # the same thing.
                    if test "$_parts[2]" != "none"
                        set _otty_ctl_path $_parts[2]
                    end
                case "controlmaster"
                    set _otty_ctl_master $_parts[2]
            end
        end

        # ── Saved password (Settings › Remote › the Key column) ──
        #
        # `SSH_ASKPASS_REQUIRE=force` takes away ssh's fallback of asking on the
        # terminal, so it is set ONLY for a host Otty really has a password for
        # — otherwise a host with no saved password would go straight to
        # "Permission denied" instead of prompting. The list is a plain file
        # (one alias per line, written by the app) precisely so this check costs
        # a read and not a process.
        #
        # Decided before the terminfo probe, which opens a connection of its own
        # and would otherwise spend a failed password attempt on every ssh to
        # such a host. The variable is declared out here because fish scopes
        # `set -l` to the enclosing BLOCK, not to the function; the exports ride
        # on each ssh command line below for the same reason.
        set -l _otty_askpass_env
        if test -n "$OTTY_ASKPASS_HOSTS" -a -n "$_otty_alias" -a -r "$OTTY_ASKPASS_HOSTS"
            set -l _otty_askpass_bin (string replace -r '/[^/]*$' '' -- "$OTTY_ASKPASS_HOSTS")"/otty-askpass"
            if test -x "$_otty_askpass_bin"; and contains -- "$_otty_alias" (command cat "$OTTY_ASKPASS_HOSTS" 2>/dev/null)
                set _otty_askpass_env "SSH_ASKPASS=$_otty_askpass_bin" \
                    "SSH_ASKPASS_REQUIRE=force" "OTTY_ASKPASS_ALIAS=$_otty_alias"
            end
        end

        # A ControlPath is only a *name* until something creates the socket.
        # `ssh -G` prints the configured path whether or not multiplexing is
        # actually on, so a `ControlPath` in ssh_config with ControlMaster left
        # at its default `no` looked exactly like a live master here: nothing
        # was minted, and `socket=` below named a path that never comes into
        # existence — every panel SFTP/exec then silently re-authenticates.
        # An already-open socket still counts (a master someone else started,
        # including Otty's own inherit-SSH, which passes ControlMaster=auto),
        # so only a path with neither a master nor a socket is dropped.
        if test -n "$_otty_ctl_path" -a "$_otty_ctl_master" = "no" -a ! -S "$_otty_ctl_path"
            set _otty_ctl_path ""
        end

        set -l _otty_target "$_otty_user@$_otty_hostname"
        set -l _otty_tmpdir (set -q TMPDIR; and echo $TMPDIR; or echo /tmp)
        # $fish_pid, not %self: fish removed `%` process expansion in 3.1, so
        # %self is a literal everywhere now (and even on older fish it only
        # expanded as a standalone token, never concatenated like this). A
        # literal here means the host looks for otty-ssh-shell-<pid>.meta and
        # never finds it — no ControlMaster reuse for remote git/SFTP.
        set -l _otty_meta_file "$_otty_tmpdir/otty-ssh-shell-$fish_pid.meta"
        # Declared at function scope, not inside the `if` below: fish's `set -l`
        # is block-scoped, so a declaration in that block would be invisible to
        # the cleanup at the end of this function. ($_otty_ctl_path is set by
        # the `ssh -G` loop above, which is already at function scope.)
        set -l _otty_ctl_dir ""

        # ── Split argv into connection args and the remote command ──
        # `ssh host cmd` folds every trailing argument into ONE remote command
        # string, so anything appended after the user's own arguments lands as
        # arguments OF their command. The terminfo probe below used to pass
        # $argv and then append its script, which made the remote run
        #     <user command> otty_term=…; <probe script>
        # — the user's command executed a second time, in the probe connection,
        # with a stray argument glued on (`ssh host 'echo TOKEN'` printed
        # "TOKEN otty_term=xterm-256color" and then "TOKEN").
        #
        # Where the connection arguments end is a getopt(3) question — which
        # letters take a value, how clusters like `-vp 2222` split, that ssh
        # re-enters getopt after the destination — and it is answered in ONE
        # place, `otty ssh scan-argv` (packages/ssh-argv), which the macOS app
        # also reaches through the FFI. It used to be four transcriptions of
        # the same rules in four languages, and they drifted: OpenSSH 9.4's
        # `-P tag` took a value in none of them.
        #
        # `otty` is on PATH here — this payload prepends $OTTY_BIN_DIR at the
        # top. When it is not (a hand-installed payload with no app behind it),
        # the safe degradation is the one already used for a ControlPath we
        # cannot mint: skip the extras and let plain ssh run.
        set -l _otty_conn
        set -l _otty_has_cmd 0
        set -l _otty_scan_ok 0
        set -l _otty_scan (command otty ssh scan-argv -- $argv 2>/dev/null)
        set -l _otty_fields (string match -r '^conn=(\d+) cmd=([01])$' -- "$_otty_scan")
        if test (count $_otty_fields) -eq 3
            set _otty_has_cmd $_otty_fields[3]
            set _otty_scan_ok 1
            # `$argv[1..0]` is an error in fish, so an empty connection list
            # (no destination) is spelled out rather than sliced.
            if test $_otty_fields[2] -gt 0
                set _otty_conn $argv[1..$_otty_fields[2]]
            end
        end

        # `_otty_scan_ok` too: without a scan we do not know where the
        # user's remote command begins, so neither the probe nor a
        # ControlMaster (whose teardown needs the same split) may run.
        if test -n "$_otty_hostname" -a $_otty_scan_ok -eq 1
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
            if test -z "$_otty_ctl_path"
                # ControlPath must stay short: macOS caps Unix-domain socket
                # paths at 104 bytes and $TMPDIR (/var/folders/...) already eats
                # ~49 of them, so the socket goes directly under /tmp instead.
                # mktemp -d yields a fresh 0700 dir, keeping this race-free.
                #
                # Deliberately no fallback when mktemp is unavailable. A
                # predictable name like /tmp/otty-ssh.$fish_pid in a
                # world-writable directory can be pre-created by another local
                # user, and this path becomes the `-o ControlPath` that every
                # panel exec and SFTP transfer rides — the same class of attack
                # the host guards against when it vets the metadata file's
                # ownership. Skipping multiplexing is the safe degradation:
                # socket= is written empty and the host simply leaves
                # controlSocket unset.
                set _otty_ctl_dir (mktemp -d /tmp/otty-ssh.XXXXXX 2>/dev/null)
                if test -n "$_otty_ctl_dir"
                    set _otty_ctl_path "$_otty_ctl_dir/socket"
                    set _otty_opts $_otty_opts \
                        -o "ControlMaster=auto" \
                        -o "ControlPath=$_otty_ctl_path" \
                        -o "ControlPersist=60"
                end
            end

            # Terminfo install (cached per user@host).
            #
            # Skipped entirely when the caller supplied a remote command: a
            # probe there would open a second connection behind the back of a
            # script that asked for exactly one, and terminfo only matters for a
            # session that has a tty. TERM then stays at the xterm-256color
            # default, which every system terminfo database already resolves.
            set -l _otty_cache_dir "$_otty_tmpdir/otty-terminfo-cache"
            command mkdir -p "$_otty_cache_dir" 2>/dev/null
            set -l _otty_cache_file "$_otty_cache_dir/$_otty_target"

            if test -f "$_otty_cache_file"
                set _otty_term (set -q TERM; and echo $TERM; or echo "xterm-256color")
            else if test $_otty_has_cmd -eq 0; and command -v infocmp >/dev/null 2>&1
                set -l _otty_terminfo (infocmp -0 -x 2>/dev/null)

                if test -n "$_otty_terminfo"
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
                    set -l _otty_remote_term (set -q TERM; and echo $TERM; or echo "xterm-256color")
                    if string match -qr '[^A-Za-z0-9._+-]' -- $_otty_remote_term
                        set _otty_remote_term "xterm-256color"
                    end
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
                    if echo "$_otty_terminfo" | command env $_otty_askpass_env ssh \
                        -o "ControlMaster=no" \
                        -o "ControlPath=none" \
                        $_otty_conn \
                        "/bin/sh -c 'otty_term=$_otty_remote_term; "'command -v infocmp >/dev/null 2>&1 && infocmp "$otty_term" >/dev/null 2>&1 && exit 0; command -v tic >/dev/null 2>&1 || exit 1; mkdir -p ~/.terminfo 2>/dev/null; tic -x - >/dev/null 2>&1 || exit 1; command -v infocmp >/dev/null 2>&1 && { infocmp -1 "$otty_term" 2>/dev/null | grep -q "clear=" || { rm -f ~/.terminfo/?/"$otty_term" ~/.terminfo/??/"$otty_term" 2>/dev/null; exit 1; }; }; exit 0'"'" \
                        2>/dev/null
                        set _otty_term (set -q TERM; and echo $TERM; or echo "xterm-256color")
                        command touch "$_otty_cache_file"
                    else if test -n "$OTTY_DEBUG_SSH"
                        # Nothing is cached on failure — deliberately, so a host
                        # that later grows `tic` is picked up — which means a
                        # permanently failing probe re-runs on every connection.
                        # Opt-in only: an unconditional line here would land in
                        # the middle of `set logs (ssh host …)`.
                        echo "otty: terminfo probe failed for $_otty_target; TERM stays xterm-256color" >&2
                    end
                end
            end

            # Under umask 077: the app refuses a metadata file others could have
            # written, and Debian's default umask 002 would make this one
            # group-writable. fish's umask is a function; restore it after.
            set -l _otty_old_umask (umask)
            umask 077
            printf 'user=%s\nhostname=%s\nsocket=%s\n' $_otty_user $_otty_hostname $_otty_ctl_path > "$_otty_meta_file" 2>/dev/null
            umask $_otty_old_umask
        end

        # ── Remote `otty` bootstrap (ANYWHERE_SPEC §6.2) ──────────────────
        #
        # On a host Otty has put its helper on, `otty view foo.log` typed in
        # the remote shell should reach the Otty the user is sitting at. That
        # needs two things a plain `ssh host` does not give it: the shim on the
        # remote $PATH, and a tty for the shim to write its OSC to. Both are
        # bought by turning the invocation into `ssh -t host sh -c …`, which is
        # a DIFFERENT command line — so it happens only when every condition
        # below holds, and otherwise this block leaves the invocation byte for
        # byte as it is today. `set -l` with no value expands to NO arguments,
        # which is what keeps the untouched case untouched.
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
        set -l _otty_boot
        if test $_otty_scan_ok -eq 1 -a $_otty_has_cmd -eq 0 \
                -a "$_otty_batchmode" != "yes" -a "$_otty_requesttty" != "false" \
                -a "$_otty_sessiontype" != "none" -a "$_otty_sessiontype" != "subsystem"
            and isatty stdin
            and not __otty_ssh_blocks_bootstrap $_otty_conn
            if __otty_ssh_host_listed "$OTTY_HELPER_HOSTS" "$_otty_alias" "$_otty_target"
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
                set _otty_opts $_otty_opts -t
                set _otty_boot 'sh -c \'export OTTY_REMOTE=1; S="${OTTY_REMOTE_SHELL:-${SHELL:-/bin/sh}}"; D="$HOME/.local/share/otty/bin"; case "${S##*/}" in bash|zsh|sh|dash|ksh|ash) exec "$S" -l -c "PATH=\"$D:\$PATH\"; export PATH; exec \"$S\" -i" ;; fish) exec "$S" -l -c "set -gx PATH \"$D\" \$PATH; exec \"$S\" -i" ;; *) PATH="$D:$PATH"; export PATH; exec "$S" -l ;; esac\''
            else if test "$OTTY_REMOTE_HELPER_POLICY" != "never"
                and contains -- bash (__otty_ssh_recorded_shell "$_otty_alias" "$_otty_target")
                # `contains`, not `test (…) = bash`: an unrecorded host prints
                # nothing, and in fish an empty substitution is zero arguments,
                # so `test` saw `= bash` and printed "Missing argument at index
                # 3" on every first ssh to a host (reported against 1.5.1).
                # Not `string match` either — with no arguments it reads stdin,
                # which here is the terminal.
                #
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
                set _otty_opts $_otty_opts -t
                set _otty_boot 'bash -c \'export OTTY_REMOTE=1; otty() { case "${1:-}" in view|edit|jump|learn|ignore) ;; *) printf "otty %s is not available on a remote host: only view, edit, jump, learn and ignore travel over the terminal.\n" "${1:-}" >&2; return 1;; esac; case "$*$PWD" in *\\\\*|*\"*) printf "otty: this host has no Otty helper, so paths with quotes or backslashes cannot be sent.\n" >&2; return 1;; esac; local c=$1 s= a; shift; local j="{\"v\":1,\"cmd\":\"$c\",\"args\":["; for a in "$@"; do j="$j$s\"$a\""; s=,; done; j="$j],\"cwd\":\"$PWD\"}"; j=$(printf %s "$j" | base64 | tr -d "\n=" | tr "+/" "-_"); { printf "\033]8801;otty;%s\007" "$j" >/dev/tty; } 2>/dev/null || printf "\033]8801;otty;%s\007" "$j"; printf "Handed to Otty on your computer.\n"; }; export -f otty; exec bash -l\''
            end
        end

        # Notify Otty we're entering an SSH session. Guarded like every other
        # OSC 7: `set logs (ssh host cat …)` runs this wrapper with stdout
        # redirected into the substitution's capture pipe.
        __otty_osc7 "ssh://"(test -n "$_otty_hostname"; and echo $_otty_hostname; or echo "unknown")

        command env TERM=$_otty_term $_otty_askpass_env ssh $_otty_opts $argv $_otty_boot
        set -l _ret $status

        command rm -f "$_otty_meta_file" 2>/dev/null

        # Retire the ControlMaster we minted above. Only ours — a ControlPath the
        # caller passed in belongs to whoever created it, hence the _otty_ctl_dir
        # guard. Without this the master lingers for the whole ControlPersist
        # window and its mktemp'd directory was never removed at all, so every
        # ssh invocation leaked one empty /tmp/otty-ssh.XXXXXX forever.
        if test -n "$_otty_ctl_dir"
            command ssh -o "ControlPath=$_otty_ctl_path" -O exit $_otty_conn >/dev/null 2>&1
            command rm -rf "$_otty_ctl_dir" 2>/dev/null
        end
        # Sweep dirs orphaned by earlier crashes. -empty means a live master's
        # dir (it still holds its socket) is never touched. The trailing slash on
        # /tmp/ is load-bearing: on macOS /tmp is a symlink to private/tmp and
        # find won't descend into it without one, silently matching nothing.
        command find /tmp/ -maxdepth 1 -type d -name 'otty-ssh.*' -empty -delete 2>/dev/null

        # Restore local CWD reporting. Guarded in lockstep with the ssh:// mark
        # above — both conditions are unchanged across the call, so the pair is
        # always emitted together or not at all.
        __otty_osc7 "file://"(hostname)"$PWD"
        return $_ret
    end
end
