"""Remove the stray brace and correct a doc comment that no longer matches."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\swash_source.rs")
lines = F.read_text(encoding="utf-8").splitlines(keepends=True)
before = "".join(lines)

# Line 109 (1-based) is an `impl Face` closing brace left over from an earlier
# edit; `cell_size` must stay inside the impl.
assert lines[108].strip() == "}", repr(lines[108])
del lines[108]
t = "".join(lines)

# The doc comment promised a reference-glyph measurement; `advance_of` now
# measures the mean, so the comment has to say what the code does.
t = t.replace(
    """    /// The cell a grid should use for this face at this size.
    ///
    /// The advance comes from the reference character rather than from the font's
    /// average, because a monospaced grid needs one width that every glyph fits
    /// in. A proportional face used as a terminal font would give a ragged grid,
    /// which is why `font-family` names a monospaced family and `CellMetrics`
    /// reports whether the face really is monospaced.""",
    """    /// The cell a grid should use for this face at this size.
    ///
    /// One width for every glyph, which is what a monospaced grid needs; a
    /// proportional face used as a terminal font gives a ragged grid, which is
    /// why `font-family` names a monospaced family and why `CellMetrics` reports
    /// whether the face really is monospaced.
    ///
    /// The width is [`Face::advance_of`]'s measurement, and its doc comment says
    /// which measurement that is and why it is not the reference glyph's own
    /// advance.""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched")
