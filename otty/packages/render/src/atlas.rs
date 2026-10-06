//! The glyph atlas.
//!
//! One `R8Unorm` texture holds every rasterised glyph; the grid pass samples it
//! once per drawn cell. `R8` rather than `Rgba8` because a glyph is coverage, not
//! colour — the colour comes from the cell and the theme. That quarters the
//! upload and, more importantly, means a glyph is rasterised **once** no matter
//! how many colours it is drawn in.
//!
//! # Why the packing rules are here and not in the GPU code
//!
//! Where a glyph lands in the atlas is pure arithmetic over integers, and getting
//! it wrong is invisible until a character renders as its neighbour. So the
//! packer is separated from the device, and `AtlasPacker` is a plain struct with
//! no `wgpu` in sight: shelf packing, one shelf per row of glyphs, cells aligned
//! to a fixed size so a lookup is `(row, col)` rather than a scan.
//!
//! # Growth
//!
//! The atlas starts at [`INITIAL_SIZE`] and doubles to a ceiling. A font size the
//! user picked at runtime, plus fallback families, plus box-drawing at every
//! weight, can exceed the first guess; the alternative to growing is a glyph that
//! silently does not draw. When even the ceiling is reached the atlas is cleared
//! and repopulated, which costs one frame's worth of rasterisation and is better
//! than a hole in the text.

use std::collections::HashMap;

use otty_core::CellSize;

/// The starting atlas edge, in pixels.
pub const INITIAL_SIZE: u32 = 1024;
/// The largest atlas we will allocate.
pub const MAX_SIZE: u32 = 4096;
/// Padding between glyphs, so a bilinear sample cannot bleed a neighbour in.
pub const GLYPH_PADDING: u32 = 1;

/// What identifies a rasterised glyph.
///
/// The synthetic flags are part of the key, not applied after lookup: a faux-bold
/// `a` and a real `a` are different bitmaps and must not share a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// The face, as `fontdb` numbers it.
    pub face: fontdb::ID,
    pub glyph: u16,
    /// The size in pixels, quantised. Sub-pixel sizes would defeat the cache.
    pub size_px: u16,
    /// Faux-bold: the outline is stroked rather than replaced.
    pub embolden: bool,
    /// Faux-italic: a shear, in hundredths of a degree of slant.
    pub slant: u8,
}

impl GlyphKey {
    pub fn new(face: fontdb::ID, glyph: u16, size_px: f32, embolden: bool, slant: f32) -> Self {
        Self {
            face,
            glyph,
            // Rounded, because a size that differs in the fourth decimal is the
            // same bitmap and must not cost a new slot.
            size_px: size_px.round().clamp(1.0, u16::MAX as f32) as u16,
            embolden,
            slant: (slant * 100.0).round().clamp(0.0, 255.0) as u8,
        }
    }
}

/// Where a glyph lives, and how it should be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphSlot {
    /// The top-left corner in atlas pixels.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Where the glyph's origin sits relative to the pen position. Both are
    /// signed: a glyph can overhang its cell to the left, which is what an italic
    /// `f` does.
    pub left: i32,
    pub top: i32,
    /// Whether anything was actually drawn. A space is a legitimate entry with
    /// `width == 0`; caching it means the packer is not asked again.
    pub blank: bool,
}

impl GlyphSlot {
    /// A slot for a glyph that rasterises to nothing — a space, or a glyph the
    /// face does not contain.
    pub const fn blank() -> Self {
        Self { x: 0, y: 0, width: 0, height: 0, left: 0, top: 0, blank: true }
    }

    /// The atlas rectangle in normalised coordinates, for the shader.
    pub fn uv(&self, atlas_size: u32) -> [f32; 4] {
        if self.blank || atlas_size == 0 {
            return [0.0, 0.0, 0.0, 0.0];
        }
        let s = atlas_size as f32;
        [
            self.x as f32 / s,
            self.y as f32 / s,
            self.width as f32 / s,
            self.height as f32 / s,
        ]
    }
}

/// Shelf packing: fill a row left to right, drop to a new row when it is full.
///
/// Simple on purpose. A terminal uses a few hundred distinct glyphs per font at
/// one size, most of them in one or two rows of the atlas, and a shelf packer
/// wastes less than a binary-tree packer for that shape while being small enough
/// to reason about. The waste is bounded by one partially-filled shelf.
#[derive(Debug)]
pub struct AtlasPacker {
    size: u32,
    /// The current shelf's y, and how tall it is.
    shelf_y: u32,
    shelf_height: u32,
    /// The x cursor within the current shelf.
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
}

impl AtlasPacker {
    pub fn new(size: u32) -> Self {
        Self { size, shelf_y: 0, shelf_height: 0, cursor_x: 0, used: false, full: false }
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    /// Whether the last placement attempt ran out of room.
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Reserve a `width` x `height` box, or `None` when this shelf generation is
    /// full.
    ///
    /// A box wider or taller than the atlas can never be placed, so it is refused
    /// rather than allowed to corrupt the shelf state.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 {
            return None;
        }
        if width > self.size || height > self.size {
            return None;
        }
        let w = width + GLYPH_PADDING;
        let h = height + GLYPH_PADDING;

        if self.cursor_x + w > self.size {
            // Drop to a new shelf.
            self.shelf_y += self.shelf_height;
            self.shelf_height = 0;
            self.cursor_x = 0;
        }
        if self.shelf_y + h > self.size {
            // Full: the caller decides whether to grow, clear, or give up.
            self.full = true;
            return None;
        }

        let at = (self.cursor_x, self.shelf_y);
        self.cursor_x += w;
        self.shelf_height = self.shelf_height.max(h);
        self.used = true;
        Some(at)
    }

    /// Grow to the next power of two, or refuse past [`MAX_SIZE`].
    pub fn grow(&mut self) -> bool {
        let next = self.size * 2;
        if next > MAX_SIZE {
            return false;
        }
        self.size = next;
        self.reset();
        true
    }

    /// Empty the packer without changing its size.
    pub fn reset(&mut self) {
        self.shelf_y = 0;
        self.shelf_height = 0;
        self.cursor_x = 0;
        self.used = false;
        self.full = false;
    }

    /// How much of the atlas has been touched, 0.0–1.0. Rough — it is the shelf
    /// cursor over the area, not a precise fill.
    pub fn occupancy(&self) -> f32 {
        if !self.used || self.size == 0 {
            return 0.0;
        }
        let rows = self.shelf_y + self.shelf_height;
        rows as f32 / self.size as f32
    }
}

/// The atlas: a packer, a map from key to slot, and the pixel buffer.
///
/// The buffer is only allocated when it is first written to, so building an atlas
/// for a font that draws nothing costs no memory.
pub struct Atlas {
    packer: AtlasPacker,
    slots: HashMap<GlyphKey, GlyphSlot>,
    pixels: Vec<u8>,
    dirty: bool,
}

impl Atlas {
    pub fn new() -> Self {
        Self::with_size(INITIAL_SIZE)
    }

    pub fn with_size(size: u32) -> Self {
        Self {
            packer: AtlasPacker::new(size),
            slots: HashMap::new(),
            pixels: Vec::new(),
            dirty: true,
        }
    }

    pub fn size(&self) -> u32 {
        self.packer.size()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn occupancy(&self) -> f32 {
        self.packer.occupancy()
    }

    /// Whether the pixels have changed since the last upload.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// The rasterised bitmap for a key, if it has one.
    pub fn get(&self, key: &GlyphKey) -> Option<GlyphSlot> {
        self.slots.get(key).copied()
    }

    /// The pixel buffer, resized to the atlas on first use.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Insert a bitmap, packing it into the atlas.
    ///
    /// `bitmap` is coverage: one byte per pixel, row-major, `width` x `height`.
    /// The caller has already rasterised, because rasterisation needs the face
    /// data and this type does not.
    pub fn insert(
        &mut self,
        key: GlyphKey,
        bitmap: &[u8],
        width: u32,
        height: u32,
        left: i32,
        top: i32,
    ) -> GlyphSlot {
        if let Some(existing) = self.get(&key) {
            return existing;
        }
        if width == 0 || height == 0 {
            let slot = GlyphSlot::blank();
            self.slots.insert(key, slot);
            return slot;
        }
        debug_assert_eq!(bitmap.len(), (width * height) as usize, "bitmap size mismatch");

        let Some((x, y)) = self.place(width, height) else {
            // Even after growing and clearing, the glyph did not fit. A slot that
            // draws nothing is wrong but bounded; panicking would be worse, and
            // a log line is what makes it findable.
            log::warn!("glyph {key:?} ({width}x{height}) does not fit in the atlas");
            let slot = GlyphSlot::blank();
            self.slots.insert(key, slot);
            return slot;
        };

        self.ensure_pixels();
        // Copy row by row: the atlas has padding between glyphs, so a glyph's rows
        // are not contiguous in the destination.
        for row in 0..height {
            let src = (row * width) as usize;
            let dst = ((y + row) * self.size() + x) as usize;
            let end = dst + width as usize;
            if end <= self.pixels.len() {
                self.pixels[dst..end].copy_from_slice(&bitmap[src..src + width as usize]);
            }
        }
        self.dirty = true;

        let slot = GlyphSlot { x, y, width, height, left, top, blank: false };
        self.slots.insert(key, slot);
        slot
    }

    /// Make room for a box, growing or clearing as needed.
    fn place(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if let Some(at) = self.packer.allocate(width, height) {
            return Some(at);
        }
        if self.packer.grow() {
            self.pixels.clear();
            self.dirty = true;
            return self.packer.allocate(width, height);
        }
        // At the ceiling: start over. Every previously handed-out slot becomes
        // stale, so the map is cleared with it.
        log::warn!("atlas at {MAX_SIZE}px is full; clearing and repacking");
        self.packer.reset();
        self.pixels.clear();
        self.slots.clear();
        self.dirty = true;
        self.packer.allocate(width, height)
    }

    fn ensure_pixels(&mut self) {
        let want = (self.size() * self.size()) as usize;
        if self.pixels.len() != want {
            self.pixels.resize(want, 0);
        }
    }
}

impl Default for Atlas {
    fn default() -> Self {
        Self::new()
    }
}

/// The instance the grid pass draws for one cell.
///
/// 32 bytes, laid out in the order the shader declares. Colour is a `u32` rather
/// than four floats because the theme's colours are already eight-bit and a
/// packed `u32` keeps the instance small enough that a full 4K grid fits in one
/// vertex buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CellInstance {
    /// Top-left corner in logical pixels.
    pub x: f32,
    pub y: f32,
    /// The cell's size in logical pixels.
    pub w: f32,
    pub h: f32,
    /// `[u, v, u_size, v_size]` in the atlas.
    pub uv: [f32; 4],
    /// The glyph's offset from the **pen**, in logical pixels.
    ///
    /// The pen is the cell's left edge on its baseline: the quad's top-left corner
    /// is `(x + offset_x, y + cell.baseline - offset_y)`. Note the sign on the
    /// vertical axis — `offset_y` comes from [`GlyphSlot::top`], which swash
    /// measures *upward* from the baseline, the same direction as
    /// [`raster`](crate::raster)'s placement. Adding it to `y` instead shifts the
    /// glyph by `2 * offset_y - cell.baseline`, which on the shipped JetBrains
    /// Mono at 13 px is a 6.7 px drop for a capital: drawn `top` 10 puts it at
    /// y 10 where the baseline is 13.26, so its ink lands too low and the row's
    /// clip takes its bottom.
    pub offset_x: f32,
    pub offset_y: f32,
    /// Foreground, `0xRRGGBBAA`.
    pub fg: u32,
    /// Background, `0xRRGGBBAA`. Equal to `fg` means "do not draw the quad" — the
    /// fragment shader discards it, which is how a cell with no background costs
    /// one discarded quad instead of a branch on the CPU.
    pub bg: u32,
}

/// Build the instance for one cell, or `None` when the cell is entirely blank.
///
/// A blank cell is skipped rather than drawn transparent, because a full-screen
/// run of blanks is the common case and the cheapest thing to do with it is
/// nothing.
#[allow(clippy::too_many_arguments)]
pub fn cell_instance(
    cell: CellSize,
    col: u16,
    row: u16,
    origin_x: f32,
    origin_y: f32,
    glyph: Option<GlyphSlot>,
    fg: u32,
    bg: u32,
    // The atlas edge, in pixels. Normalising against the slot alone would need
    // the slot to know its texture, and it does not — hence this argument.
    atlas_size: u32,
) -> Option<CellInstance> {
    let slot = glyph.unwrap_or(GlyphSlot::blank());
    // A cell with no background and no glyph is nothing to draw.
    if slot.blank && fg == bg {
        return None;
    }
    let x = origin_x + col as f32 * cell.width;
    let y = origin_y + row as f32 * cell.height;
    Some(CellInstance {
        x,
        y,
        w: cell.width,
        h: cell.height,
        uv: slot.uv(atlas_size),
        offset_x: slot.left as f32,
        offset_y: slot.top as f32,
        fg,
        bg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(n: u64) -> fontdb::ID {
        // `fontdb::ID` cannot be constructed directly, so the tests load a real
        // face from the fonts the repository ships. That is better than a
        // synthesised id anyway: these are the ids the renderer will really see.
        use std::sync::OnceLock;
        static DB: OnceLock<fontdb::Database> = OnceLock::new();
        let db = DB.get_or_init(|| {
            let mut db = fontdb::Database::new();
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/fonts");
            if let Ok(rd) = std::fs::read_dir(&dir) {
                let mut files: Vec<_> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        matches!(
                            p.extension().and_then(|e| e.to_str()),
                            Some("ttf" | "otf" | "ttc" | "otc")
                        )
                    })
                    .collect();
                files.sort();
                for f in files {
                    db.load_font_file(&f).ok();
                }
            }
            if db.is_empty() {
                // A font the platform always has, so the tests run anywhere.
                db.load_system_fonts();
            }
            db
        });
        let ids: Vec<fontdb::ID> = db.faces().map(|f| f.id).collect();
        assert!(!ids.is_empty(), "no font could be loaded for the atlas tests");
        ids[(n as usize) % ids.len()]
    }

    fn cell() -> CellSize {
        CellSize { width: 8.0, height: 17.0, baseline: 13.0 }
    }

    fn key(n: u16) -> GlyphKey {
        GlyphKey::new(face(0), n, 13.0, false, 0.0)
    }

    #[test]
    fn the_key_quantises_size_and_slant_so_a_near_miss_shares_a_slot() {
        let a = GlyphKey::new(face(0), 5, 13.0, false, 0.0);
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
        assert_eq!(GlyphKey::new(face(0), 5, 13.0, false, 0.001).slant, 0);
        let c = GlyphKey::new(face(0), 5, 14.0, false, 0.0);
        assert_ne!(a, c);
    }

    #[test]
    fn embolden_is_part_of_the_identity_not_a_post_step() {
        let plain = GlyphKey::new(face(0), 5, 13.0, false, 0.0);
        let faux = GlyphKey::new(face(0), 5, 13.0, true, 0.0);
        assert_ne!(plain, faux, "a faux-bold glyph must not reuse the plain bitmap");
    }

    #[test]
    fn the_packer_fills_a_shelf_left_to_right_then_drops_a_row() {
        let mut p = AtlasPacker::new(16);
        // Each box is 3 wide plus 1 padding = 4, so four fit in 16.
        let first = p.allocate(3, 3).unwrap();
        assert_eq!(first, (0, 0));
        assert_eq!(p.allocate(3, 3).unwrap(), (4, 0));
        assert_eq!(p.allocate(3, 3).unwrap(), (8, 0));
        assert_eq!(p.allocate(3, 3).unwrap(), (12, 0));
        // The fifth drops to a new shelf below the first.
        let fifth = p.allocate(3, 3).unwrap();
        assert_eq!(fifth, (0, 4));
    }

    #[test]
    fn a_box_taller_than_the_atlas_is_refused_rather_than_corrupting_the_shelf() {
        let mut p = AtlasPacker::new(16);
        assert_eq!(p.allocate(4, 17), None);
        assert_eq!(p.allocate(17, 4), None);
        // The shelf state is untouched, so a normal box still lands at the start.
        assert_eq!(p.allocate(4, 4).unwrap(), (0, 0));
    }

    #[test]
    fn a_zero_sized_box_is_refused() {
        let mut p = AtlasPacker::new(16);
        assert_eq!(p.allocate(0, 4), None);
        assert_eq!(p.allocate(4, 0), None);
    }

    #[test]
    fn the_packer_refuses_when_full_rather_than_wrapping() {
        let mut p = AtlasPacker::new(8);
        // Two 4-wide boxes per shelf (3 + padding), two shelves.
        let mut placed = 0;
        for _ in 0..64 {
            if p.allocate(3, 3).is_some() {
                placed += 1;
            }
        }
        // Boxes are 3+1 = 4 wide and 4 tall in an 8x8 atlas: two per shelf and
        // two shelves, so four fit exactly. Measured, and the reason `is_full`
        // is a recorded fact rather than `shelf_y + shelf_height > size` — that
        // expression is false here even though nothing more will fit.
        assert_eq!(placed, 4, "the packer placed {placed} boxes in an 8x8 atlas");
        assert!(p.is_full(), "a refused placement must set the flag");
    }

    #[test]
    fn growth_doubles_and_stops_at_the_ceiling() {
        let mut p = AtlasPacker::new(INITIAL_SIZE);
        assert_eq!(p.size(), 1024);
        assert!(p.grow());
        assert_eq!(p.size(), 2048);
        // Growth resets the cursor, so the old shelves are reusable.
        assert_eq!(p.allocate(4, 4).unwrap(), (0, 0));
        assert!(p.grow());
        assert_eq!(p.size(), MAX_SIZE);
        assert!(!p.grow(), "past the ceiling the caller must clear instead");
        assert_eq!(p.size(), MAX_SIZE);
    }

    #[test]
    fn an_atlas_starts_empty_and_reports_its_occupancy() {
        let atlas = Atlas::new();
        assert!(atlas.is_empty());
        assert_eq!(atlas.occupancy(), 0.0);
        assert_eq!(atlas.size(), INITIAL_SIZE);
    }

    #[test]
    fn inserting_a_bitmap_places_it_and_copies_the_pixels() {
        let mut atlas = Atlas::with_size(16);
        // A 2x2 checker: 0, 255 / 255, 0.
        let bitmap = [0u8, 255, 255, 0];
        let slot = atlas.insert(key(1), &bitmap, 2, 2, -1, 12);
        assert!(!slot.blank);
        assert_eq!((slot.x, slot.y, slot.width, slot.height), (0, 0, 2, 2));
        assert_eq!((slot.left, slot.top), (-1, 12));
        // Row 0 lands at offset 0..2, row 1 at 16..18 (the atlas is 16 wide).
        let px = atlas.pixels();
        assert_eq!(&px[0..2], &[0, 255]);
        assert_eq!(&px[16..18], &[255, 0]);
    }

    #[test]
    fn the_same_key_twice_returns_the_same_slot_without_repacking() {
        let mut atlas = Atlas::with_size(16);
        let bitmap = [7u8, 8, 9, 10];
        let first = atlas.insert(key(1), &bitmap, 2, 2, 0, 0);
        let again = atlas.insert(key(1), &bitmap, 2, 2, 0, 0);
        assert_eq!(first, again);
        assert_eq!(atlas.len(), 1);
    }

    #[test]
    fn a_blank_bitmap_is_cached_as_a_blank_slot_not_re_asked() {
        let mut atlas = Atlas::with_size(16);
        let slot = atlas.insert(key(32), &[], 0, 0, 0, 0);
        assert!(slot.blank);
        assert_eq!(atlas.len(), 1, "a space must be remembered, not re-packed");
        // A region of the atlas is not consumed by it.
        assert_eq!(atlas.insert(key(1), &[1, 2, 3, 4], 2, 2, 0, 0).x, 0);
    }

    #[test]
    fn the_uv_is_normalised_against_the_atlas_edge() {
        let slot = GlyphSlot { x: 8, y: 4, width: 2, height: 3, left: 0, top: 0, blank: false };
        let uv = slot.uv(16);
        assert_eq!(uv, [0.5, 0.25, 0.125, 0.1875]);
        // A blank slot maps to nothing rather than to the atlas origin, which
        // would sample whatever glyph happens to be there.
        assert_eq!(GlyphSlot::blank().uv(16), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(slot.uv(0), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn the_atlas_grows_when_a_glyph_does_not_fit() {
        let mut atlas = Atlas::with_size(16);
        // 16x16 of 4x4 boxes leaves no room for a 8x8 one on a fresh shelf.
        for i in 0..6u16 {
            atlas.insert(key(i), &[1u8; 16], 4, 4, 0, 0);
        }
        let size_before = atlas.size();
        let slot = atlas.insert(key(100), &[2u8; 64], 8, 8, 0, 0);
        assert!(atlas.size() > size_before, "the atlas did not grow");
        assert!(!slot.blank, "the glyph was dropped instead of placed");
    }

    #[test]
    fn growing_reports_the_pixels_as_dirty_so_they_are_re_uploaded() {
        let mut atlas = Atlas::with_size(16);
        let _ = atlas.take_dirty();
        for i in 0..6u16 {
            atlas.insert(key(i), &[1u8; 16], 4, 4, 0, 0);
        }
        atlas.insert(key(100), &[2u8; 64], 8, 8, 0, 0);
        assert!(atlas.take_dirty(), "a grown atlas must be re-uploaded");
        assert!(!atlas.take_dirty(), "and only once");
    }

    #[test]
    fn a_whole_ascii_page_fits_in_the_initial_atlas() {
        // The common case has to fit without growing, or every launch pays for it.
        let mut atlas = Atlas::new();
        for i in 0..95u16 {
            let slot = atlas.insert(key(i), &[1u8; 8 * 17], 8, 17, 0, 0);
            assert!(!slot.blank, "glyph {i} did not fit the initial atlas");
        }
        assert_eq!(atlas.size(), INITIAL_SIZE, "the atlas grew for plain ASCII");
        assert!(atlas.occupancy() < 0.5, "occupancy {:.2}", atlas.occupancy());
    }

    #[test]
    fn a_blank_cell_with_no_background_produces_no_instance() {
        let got = cell_instance(cell(), 0, 0, 0.0, 0.0, None, 0xFFFFFFFF, 0xFFFFFFFF, 64);
        assert_eq!(got, None, "a blank cell on its own colour is not worth drawing");
    }

    #[test]
    fn a_cell_with_a_background_produces_an_instance_even_when_blank() {
        let got = cell_instance(cell(), 3, 2, 10.0, 20.0, None, 0x111111FF, 0x222222FF, 64)
            .expect("a coloured background must be drawn");
        assert_eq!((got.x, got.y), (10.0 + 24.0, 20.0 + 34.0));
        assert_eq!((got.w, got.h), (8.0, 17.0));
        assert_eq!(got.uv, [0.0; 4], "a blank glyph samples nothing");
        assert_eq!(got.bg, 0x222222FF);
    }

    #[test]
    fn a_glyph_cell_carries_its_atlas_uv_and_its_overhang() {
        let mut atlas = Atlas::with_size(64);
        let slot = atlas.insert(key(1), &[1u8; 6], 2, 3, -2, 11);
        let got =
            cell_instance(cell(), 1, 1, 0.0, 0.0, Some(slot), 0xFFFFFFFF, 0x000000FF, atlas.size())
                .unwrap();
        assert_eq!((got.offset_x, got.offset_y), (-2.0, 11.0));
        assert_eq!(got.uv, slot.uv(atlas.size()));
        assert!(got.uv[2] > 0.0, "the uv size was normalised to nothing: {:?}", got.uv);
        assert!(got.uv[2] > 0.0 && got.uv[3] > 0.0);
    }

    #[test]
    fn the_instance_is_the_size_the_shader_expects() {
        // x, y, w, h (16) + uv[4] (16) + offsets (8) + two colours (8) = 48.
        // A mismatch between this and the WGSL struct is a silently garbled
        // screen, not a compile error, which is why it is asserted.
        assert_eq!(std::mem::size_of::<CellInstance>(), 48);
    }

    #[test]
    fn instances_are_pod_so_they_can_be_cast_straight_to_bytes() {
        let v = vec![CellInstance {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            uv: [0.0, 0.0, 0.5, 0.5],
            offset_x: 0.0,
            offset_y: 0.0,
            fg: 0xFFFFFFFF,
            bg: 0x000000FF,
        }];
        let bytes: &[u8] = bytemuck::cast_slice(&v);
        assert_eq!(bytes.len(), 48);
    }
}
