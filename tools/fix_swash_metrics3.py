"""`advance_width` lives on the proxy; the metrics fields live on the materialised form.

swash nests these two: `FontRef::metrics()` gives a proxy whose `advance_width`
reads the `hmtx` table, and `materialize_metrics()` gives the struct with the
`units_per_em`/`ascent`/`descent` fields. Keeping both bindings is what makes
each call land on the right one.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace(
    """        let metrics = font.metrics(&[]).materialize_metrics();
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let gid = font.charmap().map(ch);
        font.metrics(&[]).advance_width(gid) * scale""",
    """        // The proxy answers the advance; the materialised metrics answer the
        // em size. They are two types, which is why both are bound.
        let proxy = font.metrics(&[]);
        let metrics = proxy.materialize_metrics();
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let gid = font.charmap().map(ch);
        proxy.advance_width(gid) * scale""",
)
t = t.replace(
    """        // `metrics()` gives a proxy; the values live on the materialised form.
        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;""",
    """        // `metrics()` gives a proxy; the values live on the materialised form.
        let metrics = font.metrics(&[]).materialize_metrics();
        let upem = metrics.units_per_em as f32;""",
)
t = t.replace(
    "        let metrics = font.metrics(&[]);\n        let upem = metrics.units_per_em as f32;",
    "        let metrics = font.metrics(&[]).materialize_metrics();\n        let upem = metrics.units_per_em as f32;",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched")
