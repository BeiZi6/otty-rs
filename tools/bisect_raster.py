"""Bisect the incremental/full divergence: compare after every edit.

Deriving the cause from the code has failed twice, so measure instead. If the two
agree after edits 1 and 2 and disagree after 3, the third edit's row is the
suspect; if they disagree after 1, the mechanism is in the first repaint.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\raster.rs")
t = F.read_text(encoding="utf-8")
before = t

START = "    #[test]\n    fn a_diff_driven_repaint_matches_a_full_one() {"
END = "        let (a, b) = (incremental.pixels(), full.pixels());"

i = t.find(START)
j = t.find(END)
if i < 0 or j < 0:
    print("markers not found")
    sys.exit(1)

REPLACEMENT = '''    /// Every step of an incremental repaint must match a from-scratch one.
    ///
    /// Written as a bisect rather than a single final comparison, because the
    /// final comparison can only say THAT they differ. Comparing after each edit
    /// names the step, and the first step that differs is the one to read.
    #[test]
    fn a_diff_driven_repaint_matches_a_full_one_at_every_step() {
        let mut term = Terminal::new(16, 4, TermConfig::default());
        let mut parser = Parser::new();
        parser.advance(&mut term, b"alpha\\r\\nbeta\\r\\ngamma\\r\\ndelta");
        let frame_of = |t: &Terminal| GridFrame {
            cols: t.cols(),
            rows: t.rows(),
            cursor: Cursor { visible: false, ..Cursor::default() },
            lines: t.grid().rows().map(|r| encode_line(r, &palette())).collect(),
            palette: palette(),
            revision: 0,
        };

        let mut incremental = GridRasterizer::new(128, 64);
        let _ = incremental.rasterize(&frame_of(&term), &glyphs());

        for (step, edit) in [
            &b"\\x1b[2;1HXY\\x1b[K"[..],
            &b"\\x1b[4;1H\\x1b[2K"[..],
            &b"\\x1b[1;3H!\\x1b[K"[..],
        ]
        .into_iter()
        .enumerate()
        {
            parser.advance(&mut term, edit);
            let frame = frame_of(&term);
            incremental.rasterize(&frame, &glyphs());

            // The same grid, rasterised from scratch.
            let mut full = GridRasterizer::new(128, 64);
            full.rasterize(&frame, &glyphs());

            let (a, b) = (incremental.pixels(), full.pixels());
            assert_eq!(a.len(), b.len(), "step {step}: the rasterisers disagree on size");
            let first = a.iter().zip(b.iter()).position(|(x, y)| x != y);
            if let Some(byte) = first {
                let pixel = byte as u32 / 4;
                let (px, py) = (pixel % 128, pixel / 128);
                let cell_h = 64 / frame.rows.max(1) as u32;
                let cell_w = 128 / frame.cols.max(1) as u32;
                panic!(
                    "step {step} diverged at pixel ({px},{py}) = cell (col {px_grid}, row {row_grid}) \\
                     [cell {cell_w}x{cell_h}]: incremental {:?} vs full {:?}; \\
                     row text: {:?} vs {:?}",
                    incremental_px = 0,
                    px_grid = px / cell_w,
                    row_grid = py / cell_h,
                    [0u8; 4],
                    [0u8; 4],
                    frame.lines.get((py / cell_h) as usize).map(|l| l.text()),
                    frame_of(&term).lines.get((py / cell_h) as usize).map(|l| l.text()),
                );
            }
        }
    }
'''

t = t[:i] + REPLACEMENT + "\n" + t[t.find("\n", t.find("    }\n", j, 0)) :]
# Cut everything from the old test body start through the old test's closing.
end_old = t.find("    #[test]\n    fn the_palette_change", i)
if end_old < 0:
    end_old = t.find("    #[test]", j)
t = F.read_text(encoding="utf-8")
t = t[:i] + REPLACEMENT + t[end_old:] if end_old > 0 else t

F.write_text(t, encoding="utf-8")
print("patched: the divergence test is now a bisect")
