# Bundled terminfo entries

These compiled terminfo files let Otty advertise a richer `TERM` value
without requiring the user to have the originating terminal installed.
They are loaded by spawned shells via the `TERMINFO_DIRS` env var that
`packages/core/src/pty/mod.rs` prepends with the bundle's
`Resources/terminfo` path.

## `78/xterm-ghostty`

Source: <https://github.com/ghostty-org/ghostty> — `src/terminfo/ghostty.terminfo`
(compiled with `tic` and shipped inside `Ghostty.app/Contents/Resources/terminfo/`).

License: MIT (Ghostty).

To refresh: copy from a current Ghostty.app install:

```
cp /Applications/Ghostty.app/Contents/Resources/terminfo/78/xterm-ghostty \
   packages/app-macos/resources/terminfo/78/xterm-ghostty
```

## `61/alacritty`, `61/alacritty-direct`, `61/alacritty+common`

Source: <https://github.com/alacritty/alacritty> — `extra/alacritty.info`.

License: Apache-2.0 (Alacritty).

To refresh:

```
curl -sfL https://raw.githubusercontent.com/alacritty/alacritty/master/extra/alacritty.info \
  -o /tmp/alacritty.info
tic -x -o packages/app-macos/resources/terminfo /tmp/alacritty.info
```

## Notes on `xterm-kitty`

Not bundled. Kitty is GPL-3.0; bundling its terminfo into a non-GPL
binary distribution is a gray area we don't want to litigate. Users who
need it can install Kitty itself (which puts the entry in
`/usr/share/terminfo` or `~/.terminfo`) and our terminfo lookup will
find it.
