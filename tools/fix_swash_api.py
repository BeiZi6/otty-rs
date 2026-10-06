"""Align swash_source.rs with swash 0.2.10's real API.

Read out of the vendored crate rather than guessed:
  * `FontRef::from_index(data, index: usize)`
  * `Metrics::ascent()/descent()/units_per_em()/advance_width(GlyphId) -> f32`
  * `scale::Render::embolden(strength: f32)` — on the Render, not the builder
  * `charmap().map(ch) -> GlyphId`
"""
import pathlib
import re
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
t = F.read_text(encoding="utf-8")
before = t

# 1. `index` is a usize.
t = t.replace("swash::FontRef::from_index(&data, index)?", "swash::FontRef::from_index(&data, index as usize)?")
t = t.replace(
    "swash::FontRef::from_index(&self.data, self.index)",
    "swash::FontRef::from_index(&self.data, self.index as usize)",
)
t = t.replace(
    "swash::FontRef::from_index(&self.face.data, self.face.index)",
    "swash::FontRef::from_index(&self.face.data, self.face.index as usize)",
)

# 2. Metrics exposes methods, not fields.
t = t.replace("let upem = font.metrics(&[]).units_per_em as f32;", "let metrics = font.metrics(&[]);\n        let upem = metrics.units_per_em() as f32;")
t = t.replace("        let m = font.metrics(&[]);\n        let scale = size_px / upem;", "        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };")
t = t.replace("                ascent: m.ascent * scale,\n                descent: m.descent * scale,", "                ascent: metrics.ascent() * scale,\n                descent: metrics.descent() * scale,")

# 3. The advance comes from `Metrics::advance_width`, which takes a `GlyphId`.
t = t.replace(
    """        let upem = font.metrics(&[]).units_per_em as f32;
        let scale = size_px / upem;
        let gid = font.charmap().map(ch);
        font.metrics(&[]).advance_width(gid) as f32 * scale""",
    """        let metrics = font.metrics(&[]);
        let upem = metrics.units_per_em() as f32;
        let scale = if upem > 0.0 { size_px / upem } else { 1.0 };
        let gid = font.charmap().map(ch);
        metrics.advance_width(gid) * scale""",
)

# 4. Embolden is a Render setting, and takes a strength in pixels.
t = t.replace(
    """        let mut builder = context.builder(font).size(self.size_px);
        if self.embolden {
            builder = builder.embolden(swash::zeno::Embolden::default());
        }
        let mut scaler = builder.build();""",
    """        let mut scaler = context.builder(font).size(self.size_px).build();""",
)
t = t.replace(
    "        render.offset(swash::zeno::Vector::new(0.0, 0.0));",
    """        render.offset(swash::zeno::Vector::new(0.0, 0.0));
        if self.embolden {
            // A whole pixel of extra stroke: enough to read as bold at text
            // sizes, and the smallest amount that does not look like a
            // rasterisation artefact. `strength` is in pixels, not a ratio.
            render.embolden(1.0);
        }""",
)
t = t.replace("        render.format(swash::zeno::Format::Alpha);", "        render.format(swash::zeno::Format::Alpha);")

# 5. `charmap().map` returns a `GlyphId`; the atlas keys on a u16.
t = t.replace(
    """            swash::FontRef::from_index(&self.face.data, self.face.index as usize)
                .map(|f| f.charmap().map(ch))
                .unwrap_or(0),""",
    """            swash::FontRef::from_index(&self.face.data, self.face.index as usize)
                .map(|f| u16::from(f.charmap().map(ch)))
                .unwrap_or(0),""",
)
t = t.replace(
    """        let glyph_id = font.charmap().map(ch);
        if glyph_id == 0 {""",
    """        let glyph_id = font.charmap().map(ch);
        if u16::from(glyph_id) == 0 {""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched swash_source.rs")
