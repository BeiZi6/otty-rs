"""Four Slint errors, all from the same two mistakes.

  * a `component X { }` without `inherits` has no `background` or `border-radius`
    — Slint's layout elements are not styled, so each of these must inherit
    `Rectangle`;
  * a property a handler assigns must be `in-out`, not `in`. The pointer position
    is written by the `TouchArea` and read by Rust, which is exactly `in-out`.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\shell\ui\app.slint")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace("component Tab {", "component Tab inherits Rectangle {")
t = t.replace("component Divider {", "export component Divider inherits Rectangle {")
t = t.replace("component Sidebar {", "component Sidebar inherits Rectangle {")
t = t.replace(
    """    in property <length> pointer-x: 0px;
    in property <length> pointer-y: 0px;""",
    """    // `in-out`, not `in`: the `TouchArea` writes these and Rust reads them, so
    // Slint rejects an assignment to a pure input.
    in-out property <length> pointer-x: 0px;
    in-out property <length> pointer-y: 0px;""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched app.slint")
