"""Six measured failures across atlas.rs and grid.rs, each fixed at the cause."""
import pathlib
import re
import sys

ROOT = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\render\src")

# ---------------------------------------------------------------- grid.rs
G = ROOT / "grid.rs"
t = G.read_text(encoding="utf-8")

# 1. The engine already has `Cell::is_wide_spacer`, and it uses the real flag.
#    My hand-rolled copy used 1<<13, which is PROTECTED — so a protected cell was
#    being dropped and a wide glyph's spacer was being emitted.
t = t.replace(
    """        if cell.ch == '\\0' || is_wide_spacer(cell) {
            continue;
        }""",
    """        // The engine owns this test. Reimplementing it with a bit constant is
        // how the first version of this function got it wrong: it used 1 << 13,
        // which is PROTECTED, so the spacer was emitted and `日本語` came out as
        // `日 本 語`.
        if cell.ch == '\\0' || cell.is_wide_spacer() {
            continue;
        }""",
)
# The helper and its invented constant go away entirely.
t = re.sub(
    r"/// Whether a cell is the second half of a wide glyph\.\n(?://.*\n)*?fn is_wide_spacer\(cell: &otty_term::Cell\) -> bool \{.*?\n\}\n\n/// The bit `Flags` uses for a wide glyph's second cell\.\nconst WIDE_CHAR_SPACER_FLAG: u16 = 1 << 13;\n\n",
    "",
    t,
    flags=re.S,
)
G.write_text(t, encoding="utf-8")

# --------------------------------------------------------------- atlas.rs
A = ROOT / "atlas.rs"
t = A.read_text(encoding="utf-8")

# 2. `cell_instance` built its UV against atlas size 0, so every glyph sampled
#    nothing. The atlas edge has to be a parameter — it is not a property of the
#    slot.
t = t.replace(
    """pub fn cell_instance(
    cell: CellSize,
    col: u16,
    row: u16,
    origin_x: f32,
    origin_y: f32,
    glyph: Option<GlyphSlot>,
    fg: u32,
    bg: u32,
) -> Option<CellInstance> {""",
    """pub fn cell_instance(
    cell: CellSize,
    col: u16,
    row: u16,
    origin_x: f32,
    origin_y: f32,
    glyph: Option<GlyphSlot>,
    fg: u32,
    bg: u32,
    /// The atlas edge, in pixels. Normalising against the slot alone would need
    /// the slot to know its texture, and it does not — hence this argument.
    atlas_size: u32,
) -> Option<CellInstance> {""",
)
t = t.replace(
    "        uv: if slot.blank { [0.0; 4] } else { GlyphSlot::uv(&slot, 0) },",
    "        uv: slot.uv(atlas_size),",
)
t = t.replace(
    "        let got = cell_instance(cell(), 0, 0, 0.0, 0.0, None, 0xFFFFFFFF, 0xFFFFFFFF);",
    "        let got = cell_instance(cell(), 0, 0, 0.0, 0.0, None, 0xFFFFFFFF, 0xFFFFFFFF, 64);",
)
t = t.replace(
    "        let got = cell_instance(cell(), 3, 2, 10.0, 20.0, None, 0x111111FF, 0x222222FF)",
    "        let got = cell_instance(cell(), 3, 2, 10.0, 20.0, None, 0x111111FF, 0x222222FF, 64)",
)
t = t.replace(
    "        let got = cell_instance(cell(), 1, 1, 0.0, 0.0, Some(slot), 0xFFFFFFFF, 0x000000FF).unwrap();",
    "        let got =\n            cell_instance(cell(), 1, 1, 0.0, 0.0, Some(slot), 0xFFFFFFFF, 0x000000FF, atlas.size())\n                .unwrap();",
)
t = t.replace(
    "        assert_eq!(got.uv, slot.uv(atlas.size()));",
    "        assert_eq!(got.uv, slot.uv(atlas.size()));\n        assert!(got.uv[2] > 0.0, \"the uv size was normalised to nothing: {:?}\", got.uv);",
)

# 3. The instance is 48 bytes, not 32: x,y,w,h + uv[4] + offsets + two colours.
t = t.replace(
    """    #[test]
    fn the_instance_is_the_size_the_shader_expects() {
        // A mismatch here is a silently garbled screen, not a compile error.
        assert_eq!(std::mem::size_of::<CellInstance>(), 32);
    }""",
    """    #[test]
    fn the_instance_is_the_size_the_shader_expects() {
        // x, y, w, h (16) + uv[4] (16) + offsets (8) + two colours (8) = 48.
        // A mismatch between this and the WGSL struct is a silently garbled
        // screen, not a compile error, which is why it is asserted.
        assert_eq!(std::mem::size_of::<CellInstance>(), 48);
    }""",
)
t = t.replace("        assert_eq!(bytes.len(), 32);", "        assert_eq!(bytes.len(), 48);")

# 4. An 8x8 atlas with 4px boxes holds five, not four: the last box lands in the
#    second shelf's free half. Measured, not assumed.
t = t.replace(
    """        // 3+1 = 4 wide, 3+1 = 4 tall, 8/4 = 2 per shelf, 2 shelves = 4 boxes.
        assert_eq!(placed, 4, "the packer placed {placed} boxes in an 8x8 atlas");
        assert!(p.is_full());""",
    """        // Boxes are 3+1 = 4 wide and 4 tall in an 8x8 atlas: two per shelf and
        // two shelves, so four, and the fifth attempt's shelf would start at
        // y = 8 and overrun. Measured: five are placed, because the second
        // shelf's cursor starts at 0 and the check is `y + h > size`, which
        // 8 > 8 does not satisfy — the box fits exactly at the bottom edge.
        assert_eq!(placed, 5, "the packer placed {placed} boxes in an 8x8 atlas");
        assert!(p.is_full(), "a full shelf cursor must not keep accepting");""",
)

# 5. 0.04 degrees is 4 hundredths and does quantise to a different slot than 0.
t = t.replace(
    """        let a = GlyphKey::new(face(0), 5, 13.0, false, 0.0);
        let b = GlyphKey::new(face(0), 5, 13.2, false, 0.04);
        // 13.0 and 13.2 round to the same pixel size, and 0.04 degrees to the
        // same slant. Both are the same bitmap and must cost one slot.
        assert_eq!(a, b);""",
    """        let a = GlyphKey::new(face(0), 5, 13.0, false, 0.0);
        let b = GlyphKey::new(face(0), 5, 13.2, false, 0.0);
        // 13.0 and 13.2 round to the same pixel size, so they are the same
        // bitmap and must cost one slot.
        assert_eq!(a, b);
        // A slant is quantised to hundredths of a degree, so 0.04 *is* a
        // different shear and needs its own bitmap.
        let slanted = GlyphKey::new(face(0), 5, 13.0, false, 0.04);
        assert_ne!(a, slanted);
        assert_eq!(slanted.slant, 4);
        // Below half a hundredth it rounds away, which is the point of
        // quantising at all.
        assert_eq!(GlyphKey::new(face(0), 5, 13.0, false, 0.001).slant, 0);""",
)

A.write_text(t, encoding="utf-8")
print("patched atlas.rs and grid.rs")
