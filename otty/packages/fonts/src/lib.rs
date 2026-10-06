//! Font discovery, the Otty type stack, shaping, and the cell metrics the
//! terminal grid is laid out with.
//!
//! # The three roles
//!
//! `docs/TYPOGRAPHY.md` fixes the typeface decision this build ships, and it is
//! a decision about three separate surfaces rather than one:
//!
//! | Role | Chain | Declared by |
//! |---|---|---|
//! | UI chrome | `Styrene B` → `Styrene A` → `sans-serif` | `[token].font-ui`, `ui-font-family` |
//! | Editorial serif | `Tiempos Text` → `serif` | theme/preview prose |
//! | Terminal grid | `JetBrains Mono` → `monospace` | `font-family` |
//!
//! Styrene and Tiempos are commercially licensed and are *not* in this
//! repository, so on most machines the resolver walks past them. That is
//! expected, and it is reported rather than hidden:
//! [`FontCatalog::resolve_role`] returns a [`RoleResolution`] carrying a
//! [`ResolutionReport`] that names the family which actually answered and every
//! chain entry that did not:
//!
//! ```text
//! Styrene B is not installed; falling back to sans-serif
//! ```
//!
//! JetBrains Mono *is* in this repository — [`bundled_dir`] holds the faces the
//! build ships — so `JetBrains Mono` answers the terminal role on a machine that
//! never installed it, and the generic `monospace` stays what the table says it
//! is: the last resort. The bundle is scanned after the system and the drop-in
//! directory, so an installed copy of the same family is the one that gets used.
//!
//! # What each config key becomes
//!
//! | Key | Here |
//! |---|---|
//! | `font-family`, `font-family-fallback*` | a [`TypeStack`] chain and [`FaceRequest`] |
//! | `font-size` | [`MetricsOptions::size`] |
//! | `adjust-cell-height` | [`MetricsOptions::adjust_cell_height`], parsed by `otty-core` |
//! | `font-ligatures`, `font-ligatures-alphabet` | [`LigatureSettings`] |
//! | `text-bold`, `text-italic` | [`TextStyleFallback`] |
//! | `font-blending` (aliases `font-smooth`, `font-antialiased`) | [`FontBlending`] |
//! | `windows-text-rendering` | [`WindowsTextRendering`] |
//! | `ui-font-family` | the UI role's chain |
//!
//! # What is deliberately not here
//!
//! Rasterisation and compositing. [`FaceChoice`] says whether the renderer draws
//! a real face or has to fake one, [`FontBlending`] says how coverage is
//! composited, [`WindowsTextRendering`] says which DirectWrite path runs — and
//! the renderer is what consumes them. Similarly the binary art for box-drawing
//! and Braille is drawn analytically by the renderer, not taken from a font, so
//! nothing here resolves those codepoints.

pub mod blending;
pub mod catalog;
pub mod error;
pub mod faces;
pub mod ligatures;
pub mod metrics;
pub mod stack;
#[cfg(test)]
mod test_support;
pub mod theme;

pub use blending::{FontBlending, WindowsTextRendering};
pub use catalog::{
    bundled_dir, extension_of, is_accepted_font_file, DiscoveryOptions, FontCatalog, FontEntry,
    FontFilter, FontSource, ACCEPTED_EXTENSIONS,
};
pub use error::{FontError, SkipReason, SkippedFile};
pub use faces::{
    FaceChoice, FaceOrigin, FaceRequest, FaceStyle, StyleResolution, Synthesis, TextStyleFallback,
};
pub use ligatures::{
    is_alphabet, LigatureLevel, LigatureSettings, ShapedCluster, ShapedGlyph, ShapedRun,
};
pub use metrics::{AdvanceBasis, CellMetrics, MetricsOptions, DEFAULT_LINE_HEIGHT};
pub use stack::{
    Answered, Attempt, AttemptOutcome, FamilyRef, FontRole, GenericFamily, ResolutionReport,
    RoleResolution, TypeStack, DEFAULT_TERMINAL_FAMILY, STYRENE_UI_STACK, TIEMPOS_TEXT,
};
pub use theme::{
    theme_blending, MissingDeclaration, ThemeFontResolution, ThemeFonts,
};
