# Otty zsh shell integration — ZDOTDIR entry point.
#
# Otty launches zsh with ZDOTDIR pointed at this directory, so zsh auto-sources
# this .zshenv WITHOUT any edit to the user's dotfiles. We immediately restore
# the user's real ZDOTDIR, run their own .zshenv, then (for interactive shells)
# load Otty's integration payload — deferred to the first precmd so it runs
# AFTER the user's .zshrc / plugin managers and its OSC 133 hooks win.
#
# This file can get sourced with aliases enabled. To avoid alias expansion we
# quote the builtins we rely on.

# ── Restore the user's ZDOTDIR ──
# Otty saved the original in OTTY_ZSH_ZDOTDIR before launch (only when it was
# actually set). Restore it — or unset, which zsh treats as $HOME — so the
# user's native startup chain (.zprofile/.zshrc/.zlogin) loads from the right
# place and any child zsh does NOT re-enter this hijack.
if [[ -n "${OTTY_ZSH_ZDOTDIR+X}" ]]; then
    'builtin' 'export' ZDOTDIR="$OTTY_ZSH_ZDOTDIR"
    'builtin' 'unset' 'OTTY_ZSH_ZDOTDIR'
else
    'builtin' 'unset' 'ZDOTDIR'
fi

# Capture Otty's payload path while we can still resolve this file's location
# (`%x` is only meaningful during sourcing). The payload lives one dir up from
# this zsh/ entry dir: <…>/shell-integration/zsh/.zshenv → <…>/shell-integration.
'builtin' 'typeset' -g __otty_payload="${${(%):-%x}:A:h:h}/otty-integration.zsh"

# Use try-always so the user's .zshenv errors can't skip our setup.
{
    # Source the user's real .zshenv (zsh treats unset ZDOTDIR as $HOME). Skip
    # unreadable / directory rc files exactly as zsh itself would.
    'builtin' 'typeset' _otty_file="${ZDOTDIR-$HOME}/.zshenv"
    [[ ! -r "$_otty_file" ]] || 'builtin' 'source' '--' "$_otty_file"
} always {
    # ── CJK soft-wrap pad (Otty `strip-cjk-wrap-pad`) ──
    # zsh tags the space it pads the last column with — when a wide char won't
    # fit and soft-wraps — using `zle_highlight`'s `special` context, which
    # defaults to reverse video, leaving a block stranded at the wrap boundary.
    # Default `special` to `none` HERE (before the user's .zshrc) so the user
    # can still override it, and only when they haven't set `special` themselves.
    if [[ -o 'interactive' && "$OTTY_STRIP_CJK_WRAP_PAD" == 1 ]]; then
        (( ${zle_highlight[(I)special:*]} )) || zle_highlight+=(special:none)
    fi

    if [[ -o 'interactive' && -r "$__otty_payload" ]]; then
        # Defer the payload load to the first precmd so its OSC 133 hooks and
        # zle-line-init widget are installed AFTER the user's .zshrc / plugin
        # managers (oh-my-zsh, zsh-syntax-highlighting, …) — the same position
        # the old "source at the end of .zshrc" gave us. Loading inline here in
        # .zshenv would let later plugins clobber our widget / precmd ordering.
        __otty_deferred_load() {
            # Tell the payload it is being loaded from a precmd, i.e. AFTER zsh
            # read HISTFILE — the one load point late enough to pull a restored
            # pane's own history in on top of the global history. See the
            # $OTTY_PANE_HISTORY block in the payload.
            'builtin' 'typeset' -g __otty_precmd_load=1
            'builtin' 'source' '--' "$__otty_payload"
            'builtin' 'unset' '__otty_precmd_load'
            # Drop the deferred loader; make our precmd run LAST (so D/A marks
            # see the final $?), then fire it once for this very first prompt.
            precmd_functions=(${precmd_functions:#__otty_deferred_load})
            if (( ${+functions[__otty_precmd]} )); then
                precmd_functions=(${precmd_functions:#__otty_precmd} __otty_precmd)
                __otty_precmd
            fi
            'builtin' 'unset' '__otty_payload'
            'builtin' 'unfunction' '__otty_deferred_load' 2>/dev/null
        }
        'builtin' 'typeset' -ag precmd_functions
        precmd_functions+=(__otty_deferred_load)
    else
        'builtin' 'unset' '__otty_payload'
    fi
}
