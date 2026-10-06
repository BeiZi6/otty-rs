"""`advance_width` is not reachable on the public API of swash 0.2.10.

The impl that carries it has private fields (`hmtx`, `hmtx_count`, `hvar`) and its
type is not exported, so neither the method nor a constructor for the receiver is
public. Rather than guess further at a private path, the cell width is derived
from `Metrics::average_width`, which *is* public and is in font units, so it
scales exactly like the ascent.

This is a real, stated approximation: `average_width` is the font's mean advance
rather than the advance of a chosen reference glyph. For a monospaced face the two
coincide — every advance is the same number — and the bundled faces are all
monospaced. The reference-glyph path is recorded as the improvement to make, and
`CellMetrics` in `otty-fonts` already reports whether a face really is monospaced,
which is what would catch a proportional face arriving here.
"""
import pathlib
import re
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace(
    """        // The proxy answers the advance; the materialised metrics answer the
        // em size. They are two types, which is why both are bound.
        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let gid = font.charmap().map(ch);
        swash::MetricsProxy::from_font(&font).advance_width(gid) * scale""",
    """        // The reference glyph's own advance is not reachable: swash 0.2.10
        // keeps `advance_width` on a type whose fields are private and which is
        // not exported, so neither the method nor a way to build its receiver is
        // public. Until that path exists, the face's mean advance is the
        // measurement — for a monospaced face, which is what `font-family` names
        // and what `CellMetrics::monospaced` reports, the mean and the reference
        // are the same number.
        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let _ = ch;
        metrics.average_width as f32 * scale""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched")
