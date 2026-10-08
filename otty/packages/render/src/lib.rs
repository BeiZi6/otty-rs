//! The GPU renderer.
//!
//! Otty's original renderer is a Metal one — the shipped bundle carries
//! `Grid.metallib`, and `max-fps` is annotated "macOS." in the schema because
//! ProMotion is where a redraw cap matters. The rebuild keeps the same shape but
//! goes through `wgpu`, so one pipeline tree serves Metal on macOS, DX12 on
//! Windows and Vulkan on Linux.
//!
//! What is deliberately **not** here: a `renderer` config key. The shipped
//! product does not have one — the backend is chosen from the platform, not from
//! the user — and inventing a setting the original does not have would be a
//! compatibility promise nobody asked for. [`Backend::for_platform`] is that
//! choice, in one place, with the reason written down.
//!
//! The pipeline is split into three passes because the three kinds of content
//! have genuinely different needs:
//!
//! * **backgrounds** — one instanced quad per run of identical cells, because a
//!   full-screen grid is mostly background and a quad per cell would be waste;
//! * **glyphs** — a texture atlas, sampled once per cell;
//! * **geometry** — box-drawing, block elements and Braille, drawn analytically
//!   rather than rasterised, which is what the vendor's own documentation says
//!   Otty does and what makes them weightless at any font size.

pub mod atlas;
pub mod backend;
pub mod inline;
pub mod frame;
pub mod grid;
pub mod raster;
pub mod swash_source;
pub mod theme;

pub use atlas::{Atlas, AtlasPacker, CellInstance, GlyphKey, GlyphSlot};
pub use inline::{InlineImage, InlineStore};
pub use backend::{Backend, DeviceError, Gpu, GpuInfo};
pub use raster::{GlyphBitmap, GlyphSource, GridRasterizer, RasterOutput, SyntheticGlyphs};
pub use swash_source::{bundled_database, bundled_fonts_dir, Face, SwashGlyphs};
pub use theme::{rgba_from_hex, rgba_or, Chrome};
pub use grid::{encode_line, CellStyle, Cursor, FrameDiff, GridDiffer, GridFrame, Line, Palette, Run};
pub use frame::{FramePacer, FrameStats, MaxFps};

/// The pixel format the surface prefers, in order.
///
/// The order matters on macOS: an `Bgra8UnormSrgb` surface is what Metal hands
/// back, and asking for an `Rgba8` one costs a swizzle in the shader for every
/// glyph. `bgra8_unorm_srgb` first, then the alpha-less form, then whatever the
/// surface offers.
pub const PREFERRED_FORMATS: &[wgpu::TextureFormat] = &[
    wgpu::TextureFormat::Bgra8UnormSrgb,
    wgpu::TextureFormat::Bgra8Unorm,
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureFormat::Rgba8Unorm,
];

/// Pick the surface format from what a surface actually supports.
pub fn choose_format(supported: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    PREFERRED_FORMATS
        .iter()
        .find(|f| supported.contains(f))
        .copied()
        // A surface that supports none of the preferred formats still has to be
        // given one; the first supported is the only honest answer.
        .unwrap_or_else(|| supported.first().copied().unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb))
}

/// Whether the surface mode needs the frame presentation to be picked
/// consciously rather than left on `Auto`.
///
/// `Auto` is right for a window the compositor can present to directly. A surface
/// that only offers `Mailbox` or `Fifo` — which is what some Linux compositors
/// expose — wants an explicit choice, and `Fifo` is the one that cannot tear.
pub fn choose_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::AutoVsync) {
        wgpu::PresentMode::AutoVsync
    } else if supported.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        supported.first().copied().unwrap_or(wgpu::PresentMode::Fifo)
    }
}

/// The device-feature set the renderer asks for, and why each is optional.
///
/// Everything here has a fallback, so a device missing one of them still runs —
/// the renderer reports what it got rather than refusing to start.
pub const OPTIONAL_FEATURES: &[wgpu::Features] = &[
    // Larger atlas uploads in one `write_texture`.
    wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
    // Subpixel positioning across a wide atlas.
    wgpu::Features::TEXTURE_COMPRESSION_BC,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_srgb_is_the_first_choice_when_the_surface_offers_it() {
        let supported = [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        assert_eq!(choose_format(&supported), wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    #[test]
    fn a_surface_without_bgra_srgb_falls_to_the_next_preference() {
        let supported = [wgpu::TextureFormat::Rgba8Unorm, wgpu::TextureFormat::Rgba8UnormSrgb];
        assert_eq!(choose_format(&supported), wgpu::TextureFormat::Rgba8UnormSrgb);
        let supported = [wgpu::TextureFormat::Rgba8Unorm];
        assert_eq!(choose_format(&supported), wgpu::TextureFormat::Rgba8Unorm);
    }

    #[test]
    fn a_surface_with_nothing_preferred_still_gets_a_format() {
        // The honest answer is "one of what you have", not a panic.
        let supported = [wgpu::TextureFormat::R16Uint];
        assert_eq!(choose_format(&supported), wgpu::TextureFormat::R16Uint);
        assert_eq!(choose_format(&[]), wgpu::TextureFormat::Bgra8UnormSrgb);
    }

    #[test]
    fn AutoVsync_is_preferred_over_a_bare_Fifo() {
        let supported = [wgpu::PresentMode::Fifo, wgpu::PresentMode::AutoVsync];
        assert_eq!(choose_present_mode(&supported), wgpu::PresentMode::AutoVsync);
    }

    #[test]
    fn fifo_is_used_when_autovsync_is_absent() {
        // A compositor exposing only Immediate+Mailbox+Fifo must not go to
        // Immediate, which tears.
        let supported =
            [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo];
        assert_eq!(choose_present_mode(&supported), wgpu::PresentMode::Fifo);
    }

    #[test]
    fn an_exotic_surface_still_gets_a_present_mode() {
        let supported = [wgpu::PresentMode::Immediate];
        assert_eq!(choose_present_mode(&supported), wgpu::PresentMode::Immediate);
        assert_eq!(choose_present_mode(&[]), wgpu::PresentMode::Fifo);
    }

    #[test]
    fn the_required_feature_set_is_empty() {
        // Nothing in OPTIONAL_FEATURES may be required, or a device that lacks
        // one of them could not start. This is the assertion that keeps the two
        // lists from becoming one by accident.
        assert!(!OPTIONAL_FEATURES.is_empty());
        assert_eq!(backend::REQUIRED_FEATURES, wgpu::Features::empty());
    }
}
