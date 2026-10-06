"""`is_full` was a guess that under-reported; measure the real frame cost too."""
import pathlib
import sys

ROOT = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src")

# ------------------------------------------------------------------ atlas.rs
A = ROOT / "atlas.rs"
t = A.read_text(encoding="utf-8")

# `is_full` computed `shelf_y + shelf_height > size`, which is false in exactly
# the case that matters: four 4px boxes in an 8x8 atlas fill it, and 4 + 4 > 8 is
# false. Whether the atlas is full is a fact about the last attempt, not a
# prediction, so it is recorded rather than derived.
t = t.replace(
    """    /// The x cursor within the current shelf.
    cursor_x: u32,
    /// Whether anything has been placed yet.
    used: bool,
}""",
    """    /// The x cursor within the current shelf.
    cursor_x: u32,
    /// Whether anything has been placed yet.
    used: bool,
    /// Set when an `allocate` failed for lack of room.
    ///
    /// Derived arithmetic was tried first and was wrong: `shelf_y + shelf_height
    /// > size` is false in the very case it was meant to catch, because four
    /// 4 px boxes fill an 8 px atlas exactly and `4 + 4 > 8` does not hold.
    /// A prediction of fullness is not the same fact as a failed placement.
    full: bool,
}""",
)
t = t.replace(
    "        Self { size, shelf_y: 0, shelf_height: 0, cursor_x: 0, used: false }",
    "        Self { size, shelf_y: 0, shelf_height: 0, cursor_x: 0, used: false, full: false }",
)
t = t.replace(
    """    /// Whether the atlas has grown past a point worth logging.
    pub fn is_full(&self) -> bool {
        self.shelf_y + self.shelf_height > self.size
    }""",
    """    /// Whether the last placement attempt ran out of room.
    pub fn is_full(&self) -> bool {
        self.full
    }""",
)
t = t.replace(
    """        if self.shelf_y + h > self.size {
            // Full: the caller decides whether to grow, clear, or give up.
            return None;
        }""",
    """        if self.shelf_y + h > self.size {
            // Full: the caller decides whether to grow, clear, or give up.
            self.full = true;
            return None;
        }""",
)
t = t.replace(
    """        self.shelf_y = 0;
        self.shelf_height = 0;
        self.cursor_x = 0;
        self.used = false;
    }""",
    """        self.shelf_y = 0;
        self.shelf_height = 0;
        self.cursor_x = 0;
        self.used = false;
        self.full = false;
    }""",
)

# The test now asserts what was measured: four boxes fit, the fifth is refused,
# and `is_full` reports it.
t = t.replace(
    """        // Boxes are 3+1 = 4 wide and 4 tall in an 8x8 atlas: two per shelf and
        // two shelves, so four, and the fifth attempt's shelf would start at
        // y = 8 and overrun. Measured: five are placed, because the second
        // shelf's cursor starts at 0 and the check is `y + h > size`, which
        // 8 > 8 does not satisfy — the box fits exactly at the bottom edge.
        assert_eq!(placed, 5, "the packer placed {placed} boxes in an 8x8 atlas");
        assert!(p.is_full(), "a full shelf cursor must not keep accepting");""",
    """        // Boxes are 3+1 = 4 wide and 4 tall in an 8x8 atlas: two per shelf and
        // two shelves, so four fit exactly. Measured, and the reason `is_full`
        // is a recorded fact rather than `shelf_y + shelf_height > size` — that
        // expression is false here even though nothing more will fit.
        assert_eq!(placed, 4, "the packer placed {placed} boxes in an 8x8 atlas");
        assert!(p.is_full(), "a refused placement must set the flag");""",
)

A.write_text(t, encoding="utf-8")

# ------------------------------------------------------------------- grid.rs
# Measure the payload the shell actually receives, so "how much does this cost"
# has a number attached instead of an adjective.
G = ROOT / "grid.rs"
t = G.read_text(encoding="utf-8")
t = t.replace(
    """    #[test]
    fn a_frame_round_trips_through_json() {""",
    """    /// The numbers the performance question needs, printed and asserted.
    ///
    /// Run with `--nocapture` to see them. The bounds are loose enough not to
    /// be brittle and tight enough that a regression in run merging shows up.
    #[test]
    fn the_frame_and_diff_payloads_stay_small_on_a_realistic_grid() {
        // A 120x40 grid is the common case on a laptop; 160x50 is a large one.
        let cases = [(120u16, 40u16), (160, 50)];
        for (cols, rows) in cases {
            let mut script = String::new();
            for row in 0..rows {
                // A realistic mix: long plain lines, some styled runs, and the
                // occasional wide glyph — which is what a shell actually emits.
                script.push_str(&format!(
                    "\\x1b[{row};1Hbuild {row:02}: compiling otty-term v1.5.4 \
                     \\x1b[32mok\\x1b[0m \\x1b[1;33mwarning\\x1b[0m 日本語 ✓"
                ));
            }
            let term = term_with(&script, cols, rows);
            let frame = frame_of(&term, &palette());
            let full = serde_json::to_string(&frame).unwrap().len();

            let mut d = GridDiffer::new();
            let first = d.diff(&frame);
            let unchanged = d.diff(&frame);
            let unchanged_json = serde_json::to_string(&unchanged).unwrap().len();

            // Now change one line: a progress bar stepping, or a new log line.
            let mut script2 = String::new();
            script2.push_str(&script);
            script2.push_str("\\x1b[1;1HONE LINE CHANGED");
            let term2 = term_with(&script2, cols, rows);
            let one = d.diff(&frame_of(&term2, &palette()));
            let one_json = serde_json::to_string(&one).unwrap().len();

            println!(
                "{cols}x{rows}: full={full} B, unchanged diff={unchanged_json} B, \\
                 one-line diff={one_json} B, lines in first diff={}",
                first.changed.len()
            );

            // A full frame must be affordable to send — not merely "small".
            assert!(full < 256 * 1024, "{cols}x{rows} full frame is {full} B");
            // An unchanged frame costs a cursor and a revision.
            assert!(unchanged_json < 512, "unchanged diff is {unchanged_json} B");
            // One changed line costs one line plus the envelope.
            assert!(one_json < 2048, "one-line diff is {one_json} B");
            assert_eq!(one.changed.len(), 1);
        }
    }

    #[test]
    fn a_frame_round_trips_through_json() {""",
)
G.write_text(t, encoding="utf-8")
print("patched atlas.rs and grid.rs")
