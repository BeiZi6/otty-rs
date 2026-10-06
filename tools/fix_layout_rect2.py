"""Rect = { origin: Point, size: Size }; CellSize gained a `baseline` field."""
import pathlib
import re
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\layout.rs")
t = F.read_text(encoding="utf-8")
before = t

# `l.rect(p(1)).unwrap().<field>` — the argument itself contains parentheses,
# so the inner group has to allow one balanced level.
ARG = r"((?:[^()]|\([^()]*\))*)"
t = re.sub(
    r"l\.rect\(" + ARG + r"\)\.unwrap\(\)\.(x|y)\b",
    lambda m: f"l.rect({m.group(1)}).unwrap().origin.{m.group(2)}",
    t,
)
t = re.sub(
    r"l\.rect\(" + ARG + r"\)\.unwrap\(\)\.(width|height)\b",
    lambda m: f"l.rect({m.group(1)}).unwrap().size.{m.group(2)}",
    t,
)

# CellSize gained a baseline.
t = t.replace(
    "CellSize { width: 8.0, height: 18.0 }",
    "CellSize { width: 8.0, height: 18.0, baseline: 14.0 }",
)
t = t.replace(
    "CellSize { width: 0.0, height: 0.0 }",
    "CellSize { width: 0.0, height: 0.0, baseline: 0.0 }",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched layout.rs")
