"""Settled by the compiler: `FontRef::metrics()` already gives the materialised
`Metrics` (fields `units_per_em`, `ascent`, `descent`), and the advance lives on
a separate `MetricsProxy`, which is built with `MetricsProxy::from_font(&font)`.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace("font.metrics(&[]).materialize_metrics()", "font.metrics(&[])")
t = t.replace("proxy.materialize_metrics()", "proxy")
t = t.replace(
    "font.metrics(&[]).advance_width(gid) * scale",
    "swash::MetricsProxy::from_font(&font).advance_width(gid) * scale",
)
t = t.replace(
    """        let proxy = font.metrics(&[]);
        let metrics = proxy;
        let upem = metrics.units_per_em as f32;""",
    """        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched")
