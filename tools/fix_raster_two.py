"""Two fixes: a wrong test premise, and a diagnostic that does not dump buffers."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\raster.rs")
t = F.read_text(encoding="utf-8")
before = t

# 1. The bar-cursor test asserted a pixel the synthetic glyph never covers. The
#    glyph is a 4x4 square at the cell's top-left with top = -2, so it covers
#    y = 0..2; y = 4 is background. The premise was wrong, not the cursor.
t = t.replace(
    """        assert_eq!(r.pixel(0, 4), 0xFF0000FF, "the bar itself");
        assert_eq!(r.pixel(4, 4), FG, "the glyph is untouched beside it");""",
    """        // The bar is `cell_w / 6` = 8 / 6 -> 1 px wide, so it covers x = 0.
        assert_eq!(r.pixel(0, 0), 0xFF0000FF, "the bar itself");
        // And it is thin: the pixel beside it is the glyph, untouched. The
        // synthetic glyph covers y = 0..2 (a 4x4 square with top = -2), which is
        // why this checks a row inside the glyph. The first version of this test
        // asserted the middle of the cell, where there is only background — it
        // was wrong about where the glyph was, not about the cursor.
        assert_eq!(r.pixel(1, 0), FG, "the glyph is untouched beside the bar");""",
)

# 2. The divergence diagnostic: name the location in grid terms and show four
#    pixels from each side. A whole-buffer assertion is useless here.
t = t.replace(
    """        let (a, b) = (incremental.pixels(), full.pixels());
        assert_eq!(a.len(), b.len(), "the two rasterisers disagree on size");
        let first_diff = a.iter().zip(b.iter()).position(|(x, y)| x != y);
        assert!(
            first_diff.is_none(),
            "incremental rasterisation diverged from a full one at byte {first_diff:?}"
        );""",
    """        let (a, b) = (incremental.pixels(), full.pixels());
        assert_eq!(a.len(), b.len(), "the two rasterisers disagree on size");
        let first_diff = a.iter().zip(b.iter()).position(|(x, y)| x != y);
        if let Some(byte) = first_diff {
            // Name the place in the coordinates that can be reasoned about: the
            // pixel, and the grid row it falls in. Printing the buffers (which
            // the first version did) buries this under 32 KB of numbers.
            let width = 128u32;
            let pixel = byte as u32 / 4;
            let (px, py) = (pixel % width, pixel / width);
            let grid_row = py * 4 / 64;
            let cell = |buf: &[u8]| -> [u8; 4] {
                let i = byte - (byte % 4);
                [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
            };
            panic!(
                "diverged at pixel ({px},{py}) = grid row {grid_row}: \\
                 incremental {:?} vs full {:?}",
                cell(a),
                cell(b)
            );
        }""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched raster.rs")
