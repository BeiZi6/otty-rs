# view-sandbox

`PATH` shims prepended for programs run by a **details-panel view** (Settings →
View → Details Panel → Add View).

A view is a widget a few hundred points wide. A tool you added there to *look at
something* should not be able to throw a window onto your desktop, so the
launcher pushes this directory to the front of `PATH` and these shims shadow the
system's "open this in a GUI app" commands.

The view's program is started through the user's own shell (login, and
interactive where that matters) so it finds the `PATH` their panes find. That
shell's rc files may prepend directories of their own, so this directory is
"in front of everything the app inherited", not provably in front of
everything — which is the same guarantee as before by a different route.

This is a guardrail, not a sandbox. It stops the accident — a tool that
helpfully runs `open .` when you press a key — not a program that goes straight
to LaunchServices. The companion check is in the CLI (`OTTY_NO_NEW_PANE`, see
`packages/cli/src/main.rs`) and, for anything routed through Otty itself, in the
IPC server by pane id, which is the one a program can't opt out of.

Every shim exits 126 ("found but not executable" is the closest POSIX meaning to
"refused") and explains itself on stderr.
