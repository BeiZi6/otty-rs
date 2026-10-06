"""`FontRef::metrics()` returns a `MetricsProxy`; `Metrics` comes from it.

Read out of swash 0.2.10: `MetricsProxy` has private fields and
`units_per_em()`/`glyph_count()` methods plus `materialize_metrics()`, which
returns the `Metrics` struct that has public `ascent`/`descent`/`units_per_em`
fields and the `advance_width(GlyphId)` method. Calling the fields on the proxy
was the mistake — the compiler's "remove the arguments" hint was telling me they
were fields, just not of that type.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace(
    """        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em() as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };""",
    """        // `metrics()` gives a proxy; the values live on the materialised form.
        let metrics = font.metrics(&[]).materialize_metrics();
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };""",
)
t = t.replace(
    "                ascent: metrics.ascent() * scale,\n                descent: metrics.descent() * scale,",
    "                ascent: metrics.ascent as f32 * scale,\n                descent: metrics.descent as f32 * scale,",
)
t = t.replace(
    """        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em() as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let gid = font.charmap().map(ch);
        metrics.advance_width(gid) as f32 * scale""",
    """        let metrics = font.metrics(&[]).materialize_metrics();
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let gid = font.charmap().map(ch);
        metrics.advance_width(gid) * scale""",
)
# The remaining old spelling, if the block above did not match.
t = t.replace(
    "        font.metrics(&[]).advance_width(gid) as f32 * scale",
    "        font.metrics(&[]).materialize_metrics().advance_width(gid) * scale",
)
# `upem` is now a field; the constructor used `metrics.units_per_em` already.
t = t.replace("let upem = font.metrics(&[]).units_per_em as f32;", "let upem = font.metrics(&[]).materialize_metrics().units_per_em as f32;")

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched")
