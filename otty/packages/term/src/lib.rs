//! The Otty VT engine.
//!
//! # Layers
//!
//! * [`Terminal`] owns a primary and an alternate [`Grid`], the cursor, the pen,
//!   the DEC mode set, tab stops and the charset designations. It implements the
//!   [`vte::Perform`] trait, so it is driven byte by byte and never has to
//!   buffer a partial sequence itself — the parser state machine does that.
//! * [`Grid`] owns rows, scrollback and the viewport arithmetic, including the
//!   reflow that a resize triggers.
//! * [`Event`] is the boundary with the rest of the app. Anything with a side
//!   effect outside the grid — a notification, a clipboard write, a theme
//!   change, an inline image — is reported rather than performed, so privilege
//!   checks stay in one place.
//!
//! # What is implemented
//!
//! The scope is the sequence table the product documents. `spec/vt-sequences.md`
//! lists every row with the status the documentation gives it (✓ implemented,
//! ◐ partial, ☐ not yet); the `partial` rows are the ones with a caveat noted
//! below.
//!
//! Implemented end to end: every C0 control, the ESC set (IND, NEL, RI, HTS,
//! DECSC/DECRC, DECKPAM/DECKPNM, DECALN, RIS), the CSI set (cursor movement,
//! CUP, CHA, VPA, erase ED/EL/ECH, insert/delete ICH/DCH/IL/DL, scroll SU/SD,
//! DECSTBM, DECSLRM, DECSCA with selective erase, DECSCUSR, DECSTR, DSR/DECDSR,
//! DA, XTWINOPS, XTPUSHCOLORS/XTPOPCOLORS/XTREPORTCOLORS), the OSC set (0/1/2,
//! 4, 5, 7, 8, 9, 9;4, 10-12, 17, 19, 22, 26, 52, 77, 88, 99, 104, 105,
//! 110-119, 133, 1337), DCS (the DECRQSS cursor-style query) and the DEC
//! private modes.
//!
//! Partial, with the reason:
//!
//! * `OSC 88` (terminal resume) is treated as experimental, matching the
//!   terminal-comparison page rather than its own page's ✓: the verbs are parsed
//!   and surfaced as [`ResumeEvent`], but no state is persisted here.
//! * `XTWINOPS` implements the read-only size reports only: `Ps 14` and `Ps 16`
//!   are surfaced as [`WindowReport`] events for the application to answer,
//!   `Ps 18` is dropped on purpose (its reply leaks to the inner shell through a
//!   multiplexer) and the window-acting operations are ignored.
//! * `OSC 1337` implements the single-segment inline-image command; the chunked
//!   `MultipartFile` / `FilePart` / `FileEnd` assembly is not modelled.
//! * Kitty graphics (`APC`) and sixel (`DCS q …`) are not decoded here — the
//!   parser does not dispatch APC payloads, and sixel is documented as
//!   unsupported — so no [`ImageEvent`] is produced for either today.
//! * `CUU`/`CUD`/`DL`/`SU`/`SD` are documented as implemented without stating
//!   their clamping; this engine clamps to the active region, which is what
//!   every other terminal does.

pub mod attrs;
pub mod bidi;
pub mod events;
pub mod features;
pub mod find;
pub mod grid;
pub mod hit;
pub mod input;
pub mod perform;
pub mod screen;

pub use attrs::{Cell, ColorSpec, Flags, PaletteLookup, Pen};
pub use bidi::visual_order;
pub use events::{
    AgentEvent, ColorStackOp, Event, ImageEvent, Notification, Progress, PromptMark, ResumeEvent,
    SpecialColor, WindowOp, WindowReport,
};
pub use find::{find_next, grid_lines, FindHit};
pub use grid::{Grid, Row, ScrollRegion};
pub use hit::{path_at, selected_text};
pub use input::{encode_key, encode_paste, KeyEncodeOptions};
pub use perform::Parser;
pub use screen::{
    Charset, ColorReportFormat, Cursor, CursorShape, Mode, Modes, MouseTracking, SavedCursor,
    TabStops, TermConfig, Terminal,
};

/// A palette the renderer supplies so `ColorSpec` can be resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnsiPalette(pub [otty_core::Color; 16]);

impl Default for AnsiPalette {
    fn default() -> Self {
        Self(std::array::from_fn(|i| otty_core::color::xterm_256(i as u8)))
    }
}

impl PaletteLookup for AnsiPalette {
    fn indexed(&self, index: u8) -> otty_core::Color {
        if index < 16 {
            self.0[index as usize]
        } else {
            otty_core::color::xterm_256(index)
        }
    }
}

/// Feed a byte slice through a fresh parser into `term`.
///
/// The convenience entry point used by tests and by the CLI's `features`
/// demos; the app keeps one [`Parser`] per pane so partial sequences survive
/// across reads.
pub fn process(term: &mut Terminal, parser: &mut Parser, bytes: &[u8]) {
    parser.advance(term, bytes);
}
