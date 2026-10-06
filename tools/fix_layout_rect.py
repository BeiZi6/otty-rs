"""Rect exposes `origin: Point` and `size: Size`, not flat x/y/width/height."""
import pathlib
import re
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\layout.rs")
t = F.read_text(encoding="utf-8")
before = t

# Rect-typed locals in this file. Each maps onto origin/size.
rect_locals = ["area", "rect", "r", "a", "f", "d.rect", "first_area", "second_area", "divider_area"]

for name in sorted(rect_locals, key=len, reverse=True):
    esc = re.escape(name)
    t = re.sub(rf"\b{esc}\.x\b", f"{name}.origin.x", t)
    t = re.sub(rf"\b{esc}\.y\b", f"{name}.origin.y", t)
    t = re.sub(rf"\b{esc}\.width\b", f"{name}.size.width", t)
    t = re.sub(rf"\b{esc}\.height\b", f"{name}.size.height", t)

# `l.rect(pane).unwrap().x` and friends.
t = re.sub(
    r"(l\.rect\([^)]*\)\.unwrap\(\))\.(x|y|width|height)\b",
    lambda m: f"{m.group(1)}.origin.{m.group(2)}" if m.group(2) in ("x", "y")
    else f"{m.group(1)}.size.{m.group(2)}",
    t,
)
# `d.rect.x` was already handled; also `layout.rects()` value iteration uses `r`.

# Comparisons against a Rect literal are unaffected.

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched layout.rs for Rect's origin/size shape")
