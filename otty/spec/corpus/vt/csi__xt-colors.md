<!-- source: https://docs.otty.sh/vt/csi/xt-colors -->

# XTPUSHCOLORS / XTPOPCOLORS / XTREPORTCOLORS

|  |  |
| --- | --- |
| Sequences | CSI [Pm] # P (push) · CSI [Pm] # Q (pop) · CSI # R (report) |
| Otty support | ✓ |

## Description

xterm's color-stack mechanism for programs that want to **temporarily** change the palette and **atomically** restore it on exit, even after the user has made their own runtime adjustments. Three operations:

- **`CSI # P` — XTPUSHCOLORS**: snapshot the current palette ([OSC 4](/vt/osc/osc-4) 0–255) plus the named slots (fg / bg / cursor, plus Otty's [OSC 5 / 17 / 19](/vt/osc/osc-10-19) extras) and push it onto a per-session stack. Up to 10 entries; the oldest is FIFO-evicted on overflow.
- **`CSI # Q` — XTPOPCOLORS**: restore the top of the stack (or a named slot via `CSI Pm # Q`).
- **`CSI # R` — XTREPORTCOLORS**: reply over the PTY with `CSI ? <depth> # Q` so the program can verify the stack height.

A non-zero `Pm` names a *specific* slot (1–10) instead of using the anonymous stack — useful for nested setups where the inner layer doesn't want to disturb the outer's saved state.

## Why prefer this over `OSC 104` / `110-119`

The reset OSCs ([OSC 104 / 110-119](/vt/osc/osc-104-119)) restore to the **configured default**, which means they overwrite any *user-applied* runtime tweaks too. XTPOPCOLORS restores to the **runtime value at the matching push**, leaving outer-layer changes intact. Concretely:

```
user: printf '\e]4;1;#ff0000\e\\'      # custom red

program: printf '\e[#P'                 # snapshot the custom red
         printf '\e]4;1;#00ff00\e\\'    # work with green
         …
         printf '\e[#Q'                 # back to the custom red
                                          (NOT the config default!)
```

`OSC 104` in the same scenario would snap red→config-default, clobbering the user's preference.

## Example
bash
```
# Tiny "scoped theme" pattern
printf '\e[#P'                            # save
printf '\e]10;#ffeebb\e\\'                # fg
printf '\e]11;#1a1b26\e\\'                # bg
printf '\e]17;#33467c\e\\'                # selection bg (kitty/xterm)

# … run debugger / tutorial / wizard …

printf '\e[#Q'                            # restore everything atomically

# Capture stack depth (reply lands on shell stdin)
printf '\e[#R'

# Drive both interactively + see the depth change in real time:
otty features try change-color
```

## Notes

- The stack is **per session** (Otty pane). New panes inherit nothing.
- Each push snapshots the full palette plus the named-color slots, so OSC 5 / 17 / 19 extras are saved and restored alongside the standard colors.
- Cost: ~1 KiB per stack entry × max 10 entries = under 11 KiB per session even when full.

## Related

- [OSC 4 — palette set](/vt/osc/osc-4)
- [OSC 10/11/12/17/19 — named slots](/vt/osc/osc-10-19)
- [OSC 104 / 110-119 — reset to config](/vt/osc/osc-104-119)
- [`otty features try change-color`](#example)
