"""A `&'static str` field cannot be deserialised; the cursor shape becomes an enum."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src\grid.rs")
t = F.read_text(encoding="utf-8")
before = t

t = t.replace(
    """    /// `block`, `bar` or `underline`.
    pub shape: &'static str,""",
    "    pub shape: CursorShape,",
)
t = t.replace(
    """impl Default for Cursor {
    fn default() -> Self {
        Self { col: 0, row: 0, visible: true, shape: "block", blinking: true }
    }
}""",
    """impl Default for Cursor {
    fn default() -> Self {
        Self { col: 0, row: 0, visible: true, shape: CursorShape::Block, blinking: true }
    }
}

/// The cursor's shape, as `DECSCUSR` selects it.
///
/// An enum rather than a string: this crosses a process boundary, and a shell
/// that received `"bar"` would be trusting a spelling. An enum is also what
/// makes an unknown shape impossible to construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

impl CursorShape {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Bar => "bar",
            Self::Underline => "underline",
        }
    }
}""",
)

t = t.replace(
    'f.cursor = Cursor { col: 2, row: 0, visible: true, shape: "bar", blinking: false };',
    "f.cursor = Cursor { col: 2, row: 0, visible: true, shape: CursorShape::Bar, blinking: false };",
)
t = t.replace('assert_eq!(diff.cursor.shape, "bar");', "assert_eq!(diff.cursor.shape, CursorShape::Bar);")

t = t.replace(
    "pub use grid::{encode_line, CellStyle, Cursor, FrameDiff, GridDiffer, GridFrame, Line, Palette, Run};",
    "pub use grid::{\n    encode_line, CellStyle, Cursor, CursorShape, FrameDiff, GridDiffer, GridFrame, Line,\n    Palette, Run,\n};",
)

if t == before:
    print("no change")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched grid.rs")
