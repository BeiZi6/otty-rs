//! Everything the terminal engine reports back to the application.
//!
//! The engine never touches the OS. A sequence that has a side effect outside
//! the grid — a notification, a clipboard write, a theme change — turns into an
//! [`Event`] instead, and the caller decides whether the user's privileges
//! allow it (`privilege-notification-shell`, `clipboard-read`, …).

use otty_core::Color;

/// One side effect produced while parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `BEL`, and the bell half of a notification-less `OSC 9`.
    Bell,
    /// `ENQ` (0x05) — the host asked for `enquiry-response`.
    Enquiry,

    /// `OSC 0` / `OSC 2`: the window/tab title.
    Title(String),
    /// `OSC 1`: the icon title. Otty treats it the same as `OSC 2`.
    IconTitle(String),

    /// `OSC 7`: the pane's reported working directory, as a `file://` URL.
    WorkingDirectory(String),

    /// `OSC 52`: a program stored text in the clipboard.
    ///
    /// `selection` is the `c`/`p`/`s`/`0-7` selector; the payload is decoded
    /// base64 unless it arrived as `?`.
    ClipboardStore { selection: char, data: Vec<u8> },
    /// `OSC 52` with a `?` payload: the program is *reading* the clipboard and
    /// expects the terminal to answer on the pty.
    ClipboardLoad { selection: char },

    /// `OSC 8`: a hyperlink span started (`Some`) or ended (`None`).
    Hyperlink { uri: Option<String>, id: Option<String> },

    /// `OSC 9` (iTerm), `OSC 777` (urxvt) or `OSC 99` (kitty).
    Notify(Notification),

    /// `OSC 9;4`: the ConEmu progress protocol.
    Progress(Progress),

    /// `OSC 4`: set palette entry `index`.
    PaletteSet { index: u16, color: Color },
    /// `OSC 4` with `?`: report the palette entry back on the pty.
    PaletteQuery { index: u16 },
    /// `OSC 104`: reset palette entries to their configured defaults.
    PaletteReset { indices: Vec<u16> },

    /// `OSC 5` / `OSC 10` / `OSC 11` / `OSC 12` / `OSC 17` / `OSC 19`.
    SpecialColorSet { kind: SpecialColor, color: Color },
    /// The `?` form of the above.
    SpecialColorQuery { kind: SpecialColor },
    /// `OSC 105` / `OSC 110`-`119`: reset a special colour to the theme's value.
    SpecialColorReset { kind: SpecialColor },

    /// `OSC 22`: the pointer shape to use while over the pane.
    MouseShape(String),

    /// `OSC 133` (FTCS) prompt marks, which power the outline and jump-to.
    PromptMark(PromptMark),
    /// `OSC 133;D`'s exit status, when the mark carried one.
    CommandFinished { exit_code: Option<i32> },

    /// `XTWINOPS` asked for a size report.
    WindowReport(WindowReport),
    /// `XTWINOPS` asked to act on the window (iconify, move, resize, …).
    WindowOp(WindowOp),
    /// `XTWINOPS` `21 t`: report the window title back on the pty.
    ReportTitle,

    /// `XTPUSHCOLORS` / `XTPOPCOLORS` / `XTREPORTCOLORS`.
    ColorStack(ColorStackOp),

    /// `DECALN` — fill the screen with `E`, for geometry checks.
    ScreenAlignmentDisplay,

    /// `DCS $q … ST` (DECRQSS) — the program is querying a setting.
    RequestSetting(String),

    /// An inline image arrived. The engine hands over the raw payload and lets
    /// the renderer decode it, so a huge base64 blob never blocks parsing.
    Image(ImageEvent),

    /// `OSC 88` — the terminal-resume protocol.
    Resume(ResumeEvent),

    /// `OSC 26` — the terminal agent protocol.
    Agent(AgentEvent),
}

/// The named colour slots `OSC 5` and `OSC 10`-`OSC 19` address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialColor {
    /// `OSC 5;0` — the bold colour.
    Bold,
    /// `OSC 5;1` — the underline colour.
    Underline,
    /// `OSC 5;2` — the blink colour.
    Blink,
    /// `OSC 5;3` — the reverse colour.
    Reverse,
    /// `OSC 5;4` — the italic colour.
    Italic,
    /// `OSC 10`.
    Foreground,
    /// `OSC 11`.
    Background,
    /// `OSC 12`.
    Cursor,
    /// `OSC 17` — the selection background.
    SelectionBackground,
    /// `OSC 19` — the selection foreground.
    SelectionForeground,
}

/// A desktop notification from `OSC 9`, `OSC 777` or `OSC 99`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub title: Option<String>,
    pub body: String,
    /// `OSC 99` urgency: 0 low, 1 normal, 2 critical.
    pub urgency: u8,
    /// `OSC 99` `i=` — a stable id so a later notification can replace this one.
    pub id: Option<String>,
    /// `OSC 99` `.p` — when set, this notification should not be presented.
    pub silent: bool,
}

/// The ConEmu `OSC 9;4` task state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// `9;4;0` — remove any progress state.
    Clear,
    /// `9;4;1;<p>` — a determinate percentage.
    Set(u8),
    /// `9;4;2` — an error state.
    Error,
    /// `9;4;3` — indeterminate, spinning.
    Indeterminate,
    /// `9;4;4;<p>` — paused, with the last percentage.
    Paused(u8),
}

/// An `OSC 133` (FinalTerm shell-integration) prompt mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMark {
    /// `A` — the prompt starts.
    PromptStart,
    /// `B` — the prompt ends and user input begins.
    PromptEnd,
    /// `C` — the command starts running.
    CommandStart,
    /// `D` — the command finished.
    CommandEnd,
    /// `E` — the previous command's output ends.
    OutputEnd,
    /// `P` — the command line, for the outline.
    CommandLine,
}

/// An `XTWINOPS` size report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowReport {
    /// `CSI 14 t` — text area size in pixels.
    TextAreaPixels,
    /// `CSI 15 t` — the whole window in pixels.
    WindowPixels,
    /// `CSI 16 t` — a cell size in pixels.
    CellPixels,
    /// `CSI 18 t` — the text area in characters.
    TextAreaChars,
    /// `CSI 11 t` / `CSI 13 t` — window / text-area state.
    State,
    /// `CSI 19 t` — the screen size in characters.
    ScreenChars,
}

/// An `XTWINOPS` window action.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowOp {
    Iconify,
    Deiconify,
    Move(i32, i32),
    Resize { width: u16, height: u16 },
    ResizeChars { cols: u16, rows: u16 },
    Raise,
    Lower,
    Maximize,
    Restore,
    Refresh,
    ResizeTextAreaChars { cols: u16, rows: u16 },
    ResizeTextAreaPixels { width: u16, height: u16 },
}

/// `XTPUSHCOLORS`, `XTPOPCOLORS`, `XTREPORTCOLORS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorStackOp {
    Push,
    Pop,
    Report,
}

/// An inline-image payload.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageEvent {
    /// `OSC 1337;File=…` — iTerm2's inline images. The value is the raw
    /// `name=value;…` parameter string and the base64 payload.
    Iterm { params: Vec<(String, String)>, payload: String },
    /// The kitty graphics protocol, keyed by its command letter.
    Kitty { control: Vec<(String, String)>, payload: String },
    /// A sixel image, verbatim between `DCS` and `ST`.
    Sixel(Vec<u8>),
}

impl ImageEvent {
    /// Decode the base64 payload (Kitty / OSC 1337). Raw sixel is returned as-is.
    pub fn payload_bytes(&self) -> Option<Vec<u8>> {
        use base64::Engine as _;
        match self {
            Self::Sixel(raw) => Some(raw.clone()),
            Self::Iterm { payload, .. } | Self::Kitty { payload, .. } => {
                base64::engine::general_purpose::STANDARD
                    .decode(payload.as_bytes())
                    .ok()
            }
        }
    }
}

/// `OSC 88` — the terminal-resume protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeEvent {
    /// The program is announcing that it can be resumed, with this identity.
    Announce { id: String, command: Option<String> },
    /// The program is asking the terminal to restore a previous session.
    Request { id: String },
    /// A state payload the program wants carried across a restart.
    State(Vec<u8>),
    /// Anything else the protocol defines; the engine keeps the verb for the
    /// application to interpret.
    Other { verb: String, args: Vec<String> },
}

/// `OSC 26` — the terminal agent protocol used by the code-agent integrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEvent {
    pub verb: String,
    pub args: Vec<String>,
}
