<!-- source: https://docs.otty.sh/vt/osc/osc-8 -->

# OSC 8 — Hyperlinks

|  |  |
| --- | --- |
| Sequence | OSC 8 ; <params> ; <uri> ST then text then OSC 8 ; ; ST |
| Category | OSC |
| Otty support | ✓ Implemented |

## Description

Wraps a span of text in a clickable hyperlink. The link target stays attached to the cells until they're overwritten — so a hyperlinked path stays clickable as it scrolls through the buffer.

The closing sequence (`OSC 8 ; ; ST`) is required, with empty params and empty URI.

## Parameters

The `<params>` field is a colon-separated key=value list. The only standard key today:

| Key | Meaning |
| --- | --- |
| id | Optional identifier to group adjacent hyperlinked cells |

Unknown keys are ignored.

## Example
bash
```
# Print "click me" as a hyperlink to example.com
printf '\e]8;;https://example.com\e\\click me\e]8;;\e\\\n'

# With an explicit id (groups multi-cell links across line wraps)
printf '\e]8;id=42;https://example.com\e\\click me\e]8;;\e\\\n'

# Common usage: emit a clickable file:// URL with a line number
file="/Users/me/project/src/lib.rs"
printf '\e]8;;file://%s#42\e\\%s:42\e]8;;\e\\\n' "$file" "$file"
```

In Otty, the resulting text is underlined and styled as a link; `⌘click` opens the target. The hyperlink is **independent** of [path detection](/user-interface/files-and-links) — OSC 8 is explicit, path detection is automatic.

Try it

`otty features try hyperlink` — paints three sample links covering `https://`, alternate URL, and `mailto:`. `⌘click` to open.

## Notes

- `ST` (string terminator) may be `ESC \` or `BEL`. Both are accepted.
- A new `OSC 8 ;` without first closing the previous link **replaces** it on subsequent cells.
- Links survive scrolling into history.

## Common emitters

- `git` (recent versions) — emits OSC 8 for refs in some pagers.
- `ls --hyperlink=auto` (GNU coreutils)
- Build tools and CI loggers that link to source / dashboards.

## Related

- [Files and Links](/user-interface/files-and-links) — path auto-detection (no OSC 8 needed).
- [Hint Mode](/terminal-features/hint-mode) — keyboard-only equivalent of `⌘click`.
