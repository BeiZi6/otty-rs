"""`FontRef::metrics()` returns `Metrics` directly; its members are fields.

The compiler settled this: `materialize_metrics` does not exist on `Metrics`, so
`metrics()` already returns the materialised struct — whose `units_per_em`,
`ascent` and `descent` are **fields** and whose `advance_width(GlyphId)` is the
one method.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace(".materialize_metrics()", "")
t = t.replace("metrics.units_per_em()", "metrics.units_per_em")
t = t.replace("metrics.ascent()", "metrics.ascent")
t = t.replace("metrics.descent()", "metrics.descent")
# The one call that still used the whole expression inline.
t = t.replace(
    "let upem = font.metrics(&[]).units_per_em as f32;",
    "let upem = font.metrics(&[]).units_per_em as f32;",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched")
