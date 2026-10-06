# Otty fish shell integration — vendor_conf.d auto-load entry point.
#
# Otty prepends its bundled shell-integration dir to XDG_DATA_DIRS before
# launching fish, so fish auto-sources this file from
#   <…>/shell-integration/fish/vendor_conf.d/otty-shell-integration.fish
# WITHOUT any edit to the user's config.fish. We first remove our dir from
# XDG_DATA_DIRS (so child processes don't inherit it), then source the real
# integration payload, which lives two dirs up from vendor_conf.d/.
#
# Written so it can't leave the shell in a weird state — every failure path is
# handled and we always fall through to the user's normal startup.

function __otty_restore_xdg_data_dir -d "restore XDG_DATA_DIRS to its exact pre-Otty state"
    # Only act when Otty touched XDG_DATA_DIRS for this shell.
    set -q OTTY_FISH_XDG_RESTORE; or return
    if set -q OTTY_FISH_XDG_ORIG
        # Originally set → restore verbatim (single string, no path-splitting),
        # so the value children inherit is byte-identical to before Otty.
        set --global --export --unpath XDG_DATA_DIRS $OTTY_FISH_XDG_ORIG
        set --erase OTTY_FISH_XDG_ORIG
    else
        # Originally unset → erase, so children see it unset rather than the
        # default we spliced in so fish could find system vendor dirs at startup.
        set --erase --global XDG_DATA_DIRS
    end
    set --erase OTTY_FISH_XDG_RESTORE
end

__otty_restore_xdg_data_dir
functions -e __otty_restore_xdg_data_dir

# Resolve the payload path relative to this file: vendor_conf.d → fish →
# shell-integration, then otty-integration.fish. Fall back to the env var Otty
# always sets if the filename can't be resolved.
set -l __otty_self (status current-filename)
set -l __otty_payload ""
if test -n "$__otty_self"
    set __otty_payload (dirname (dirname (dirname $__otty_self)))/otty-integration.fish
end
if test -z "$__otty_payload" -o ! -r "$__otty_payload"
    if set -q OTTY_SHELL_INTEGRATION
        set __otty_payload "$OTTY_SHELL_INTEGRATION/otty-integration.fish"
    end
end

# The payload self-guards interactivity and idempotency.
if test -r "$__otty_payload"
    source "$__otty_payload"
end
