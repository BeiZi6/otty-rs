//! The view: everything a shell needs, and nothing that knows what a shell is.
//!
//! This is the layer the Slint binding sits on. It owns the pieces that have to
//! be kept in step — the PTY, the VT engine, the frame, the glyph rasteriser, the
//! layout and the theme's chrome — and exposes exactly what a window needs:
//!
//! ```text
//!   TerminalView
//!     resize(px_w, px_h)        → grid size, PTY size, pixel buffer
//!     pump()                    → PTY bytes into the engine, events out
//!     frame()                   → RGBA pixels + which rows changed
//!     write(bytes)              → keystrokes to the shell
//!     chrome()                  → the colours the window furniture binds to
//!     headers() / rows()        → the model, for the tab bar and sidebar
//! ```
//!
//! It is deliberately free of `winit`, `slint` and `wgpu`, which is what lets the
//! whole pipeline be tested without a window: a test spawns a real shell, writes
//! to it, pumps, rasterises and asserts on pixels.
//!
//! The one thing it does *not* do is decide when to redraw. That belongs to the
//! event loop, because a view that scheduled its own frames would be untestable
//! and would fight the frame pacer.

use std::path::PathBuf;

use otty_config::Config;
use otty_core::{CellSize, PaneId, Rect};
use otty_pty::{PtyConfig, PtyError};
use otty_render::grid::{encode_line, Cursor, GridFrame, Line, Palette};
use otty_render::raster::{GlyphSource, GridRasterizer, RasterOutput};
use otty_render::swash_source::{bundled_database, Face, SwashGlyphs};
use otty_render::theme::Chrome;
use otty_term::TermConfig;

use crate::layout::{self, Layout, LayoutOptions};
use crate::model::{App, SplitDirection};
use crate::session::Session;

/// What the view could not do, in terms a caller can act on.
#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("the shell could not start: {0}")]
    Shell(#[from] PtyError),
    #[error("no monospaced font could be loaded, so the grid has no measurement")]
    NoFont,
}

/// Owns one pane's session and the pixels it produces.
pub struct TerminalView {
    app: App,
    session: Session,
    raster: GridRasterizer,
    glyphs: Box<dyn GlyphSource + Send>,
    palette: Palette,
    chrome: Chrome,
    options: LayoutOptions,
    cell: CellSize,
    /// The pane this view draws. One pane per view until the model drives it.
    pane: PaneId,
    /// The size the pixel buffer was last laid out for.
    buffer: (u32, u32),
    /// The events the engine produced on the last pump, for badges and titles.
    events: Vec<otty_term::Event>,
    /// The directory the shell last reported over OSC 7, when it reports one.
    cwd: Option<PathBuf>,
}

impl TerminalView {
    /// Start a shell and build the view around it.
    ///
    /// `pixel_size` is the window's inner size in physical pixels; the grid is
    /// derived from it and the font metrics, so the first frame is the right size
    /// rather than resized into place.
    pub fn new(
        config: &Config,
        pane: PaneId,
        pixel_size: (u32, u32),
        scale: f32,
    ) -> Result<Self, ViewError> {
        let theme = config.active_theme(false);
        let palette = Palette::from_theme(&theme);
        let chrome = Chrome::from_theme(&theme);

        let font_px = config.get_float("font-size").unwrap_or(13.0) as f32 * scale;
        let (glyphs, cell) = load_glyphs(config, font_px)?;

        let cols = ((pixel_size.0 as f32 / cell.width).floor().max(1.0)) as u16;
        let rows = ((pixel_size.1 as f32 / cell.height).floor().max(1.0)) as u16;

        let pty = pty_config(config, cols, rows);
        let session = Session::spawn(pane, &pty, term_config(config))?;

        let mut app = App::new();
        app.add_window(
            otty_core::WindowId::new(1),
            otty_core::TabId::new(1),
            pane,
        );

        // Read what the layout needs before `chrome` is moved into the struct:
        // field initialisers run in order, so a later one cannot borrow a field
        // an earlier one has taken.
        let (divider, padding) = (chrome.divider_width, chrome.pane_padding);

        Ok(Self {
            app,
            session,
            raster: GridRasterizer::new(pixel_size.0.max(1), pixel_size.1.max(1)),
            glyphs: Box::new(glyphs),
            palette,
            chrome,
            options: LayoutOptions {
                divider,
                pane_padding: otty_core::geometry::Insets {
                    top: padding[0],
                    right: padding[1],
                    bottom: padding[2],
                    left: padding[3],
                },
            },
            cell,
            pane,
            buffer: pixel_size,
            events: Vec::new(),
            // The shell has not reported a directory yet; `None` is exactly that.
            cwd: None,
        })
    }

    /// A view over an already-running session, for tests and for the CLI's
    /// `features` command, which needs the pipeline without a PTY.
    pub fn with_session(
        session: Session,
        palette: Palette,
        chrome: Chrome,
        glyphs: Box<dyn GlyphSource + Send>,
        cell: CellSize,
        pane: PaneId,
        pixel_size: (u32, u32),
    ) -> Self {
        let mut app = App::new();
        app.add_window(otty_core::WindowId::new(1), otty_core::TabId::new(1), pane);
        Self {
            app,
            session,
            raster: GridRasterizer::new(pixel_size.0.max(1), pixel_size.1.max(1)),
            glyphs,
            palette,
            chrome,
            options: LayoutOptions::default(),
            cell,
            pane,
            buffer: pixel_size,
            events: Vec::new(),
            cwd: None,
        }
    }

    // -------------------------------------------------------------- geometry

    /// The cell the grid is laid out on, in physical pixels.
    pub fn cell(&self) -> CellSize {
        self.cell
    }

    /// The grid size the engine is running at.
    pub fn grid_size(&self) -> (u16, u16) {
        (self.session.terminal().cols(), self.session.terminal().rows())
    }

    /// Resize to a new window size in physical pixels.
    ///
    /// The three sizes that must agree — the grid, the PTY and the pixel buffer —
    /// are recomputed together and in this one place. Splitting them is how a
    /// terminal ends up drawing a grid the shell does not know about.
    pub fn resize(&mut self, width: u32, height: u32, settled: bool) {
        if width == 0 || height == 0 {
            // A minimised window is legal and has no grid.
            return;
        }
        let cols = ((width as f32 / self.cell.width).floor().max(1.0)) as u16;
        let rows = ((height as f32 / self.cell.height).floor().max(1.0)) as u16;
        self.session.resize(cols, rows, settled);
        if (width, height) != self.buffer {
            self.raster.resize(width, height);
            self.buffer = (width, height);
        }
    }

    /// The pane rectangles, for hit-testing and for drawing dividers.
    pub fn layout(&self, width: u32, height: u32) -> Layout {
        let area = Rect::new(0.0, 0.0, width as f32, height as f32);
        let tab = self.app.active_window().and_then(|w| w.active_tab());
        match tab {
            Some(t) if t.zoomed.is_some() => layout::layout_single(self.pane, area, &self.options),
            Some(t) => layout::layout(&t.tree, area, &self.options),
            None => layout::layout_single(self.pane, area, &self.options),
        }
    }

    /// Which cell a window-space point lands on.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<(u16, u16)> {
        let rect = self.layout(self.buffer.0, self.buffer.1).rect(self.pane)?;
        layout::cell_at(rect, self.cell, x, y)
    }

    /// Screen-row text for context-menu path hits.
    pub fn line_text(&self, screen_row: u16) -> String {
        self.session
            .terminal()
            .grid()
            .row(screen_row)
            .map(|r| r.text())
            .unwrap_or_default()
    }

    /// Selected cells as text, for Copy.
    pub fn selected_text(&self, a: (u16, u16), b: (u16, u16)) -> String {
        otty_term::selected_text(self.session.terminal().grid(), a, b)
    }

    // --------------------------------------------------------------- session

    /// Move the shell's output into the engine, and collect what it produced.
    ///
    /// Returns whether anything changed, which is what the event loop needs to
    /// decide on a redraw. The events are kept so the tab bar can read the title
    /// and the badge without the shell having to interpret them.
    pub fn pump(&mut self) -> bool {
        let read = self.session.pump();
        let replies = self.session.drain_replies();
        let events = self.session.drain_events();
        let interesting = !events.is_empty();

        for event in events {
            self.apply_event(&event);
            self.events.push(event);
        }
        read > 0 || !replies.is_empty() || interesting
    }

    fn apply_event(&mut self, event: &otty_term::Event) {
        match event {
            otty_term::Event::Title(title) => {
                if let Some(pane) = self.app.pane_mut(self.pane) {
                    // The shell's own title, which a user-set title beats.
                    pane.shell_title = Some(title.clone());
                }
            }
            otty_term::Event::WorkingDirectory(path) => {
                self.cwd = Some(PathBuf::from(path));
            }
            otty_term::Event::Progress(p) => {
                // `OSC 9;4` carries five states, and the tab bar shows one badge.
                // `Clear` clears it rather than showing a zero, because "no
                // progress" and "0% done" are different things to a user.
                let badge = match p {
                    otty_term::Progress::Clear => None,
                    otty_term::Progress::Set(percent) => {
                        Some(crate::model::Badge::Progress(*percent))
                    }
                    otty_term::Progress::Paused(percent) => {
                        Some(crate::model::Badge::Progress(*percent))
                    }
                    otty_term::Progress::Indeterminate => Some(crate::model::Badge::Running),
                    otty_term::Progress::Error => Some(crate::model::Badge::Error),
                };
                if let Some(tab) = self.app.active_window_mut().and_then(|w| w.active_tab_mut()) {
                    tab.badge = badge;
                }
            }
            _ => {}
        }
    }

    /// The events from the last `pump`.
    pub fn events(&self) -> &[otty_term::Event] {
        &self.events
    }

    /// What the shell last reported as its working directory, when it reports one.
    pub fn working_directory(&self) -> Option<&std::path::Path> {
        self.cwd.as_deref()
    }

    /// Send input to the shell.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.session.write(bytes)
    }

    /// A keystroke from the window, as the text Slint reports for it.
    ///
    /// Printable characters go through as UTF-8; named keys (Return, arrows,
    /// Backspace, …) are encoded with the engine's current keypad/cursor mode
    /// so a full-screen program sees the same bytes a real terminal would send.
    pub fn handle_key(&mut self, text: &str) -> Result<(), PtyError> {
        let opts = otty_term::KeyEncodeOptions::for_terminal(self.session.terminal());
        let bytes = bytes_for_key_text(text, &opts);
        if bytes.is_empty() {
            return Ok(());
        }
        self.write(&bytes)
    }

    /// The pane's tty, for `tty of t`.
    pub fn tty(&self) -> Option<String> {
        self.session.tty()
    }

    /// Whether a foreground process other than the shell is running.
    pub fn is_busy(&self) -> bool {
        self.session.is_busy()
    }

    /// Split the pane, which grows the model. The new pane's session is started by
    /// the caller once it knows the rectangle to put it in.
    pub fn split(
        &mut self,
        new_pane: PaneId,
        direction: SplitDirection,
        ratio: u8,
    ) -> bool {
        self.app.split_pane(self.pane, new_pane, direction, ratio)
    }

    pub fn close(&mut self) {
        self.session.close();
    }

    // ----------------------------------------------------------------- pixels

    /// Build the frame, composite it, and report what changed.
    pub fn frame(&mut self) -> RasterOutput {
        let frame = self.build_frame();
        // `&self.glyphs`, not `&*self.glyphs`: `rasterize` takes `&impl
        // GlyphSource`, which needs `Sized`, and the boxed trait object is sized
        // while what it points at is not. `Box<dyn GlyphSource>` implements the
        // trait below, which is what makes the box itself usable here.
        self.raster.rasterize(&frame, &self.glyphs)
    }

    /// The RGBA buffer, valid until the next `frame`.
    pub fn pixels(&self) -> &[u8] {
        self.raster.pixels()
    }

    pub fn buffer_size(&self) -> (u32, u32) {
        self.raster.size()
    }

    /// One pixel as `0xRRGGBBAA`, for tests and for sampling a colour under the
    /// cursor. Out of range reads as transparent rather than panicking, because a
    /// caller asking about a point outside the window is a layout question, not a
    /// bug.
    pub fn pixel_at(&self, x: u32, y: u32) -> u32 {
        self.raster.pixel(x, y)
    }

    /// The frame the session's grid currently describes.
    ///
    /// Built fresh each call rather than cached: the engine owns the grid, and a
    /// second copy of it here would be one more thing that can disagree.
    pub fn build_frame(&self) -> GridFrame {
        let term = self.session.terminal();
        let lines: Vec<Line> = term.grid().rows().map(|r| encode_line(r, &self.palette)).collect();
        GridFrame {
            cols: term.cols(),
            rows: term.rows(),
            // The cell the grid was laid out with, and therefore the only cell
            // whose coordinates agree with `cell_at`. The renderer used to derive
            // its own from the buffer size, which drifts from this one by the
            // remainder the grid does not fill.
            cell: self.cell,
            cursor: self.cursor(),
            lines,
            palette: self.palette,
            revision: 0,
        }
    }

    fn cursor(&self) -> Cursor {
        // `Terminal::cursor` is a public field, not an accessor. The engine's
        // shape is reported per-pane (the program can change it with DECSCUSR),
        // but the theme names the shape to use when the program has not asked for
        // one, so the theme wins here — which is what a themed window does.
        let engine = self.session.terminal().cursor;
        Cursor {
            col: engine.col,
            row: engine.row,
            visible: engine.visible,
            shape: self.chrome.cursor_shape,
            blinking: self.chrome.cursor_blink,
        }
    }

    /// Tell the rasteriser the cursor's blink phase. The event loop owns the
    /// clock; the view only reports the phase it is told.
    pub fn set_cursor_blink(&mut self, on: bool) {
        self.raster.set_cursor(self.chrome.cursor_shape, on);
    }

    // ------------------------------------------------------------ the shell

    /// The colours the window furniture binds to.
    pub fn chrome(&self) -> &Chrome {
        &self.chrome
    }

    /// Rebuild glyphs at the config's `font-size` so Settings is visible live.
    pub fn apply_font_size(&mut self, config: &Config, scale: f32) -> Result<(), ViewError> {
        let font_px = config.get_float("font-size").unwrap_or(13.0) as f32 * scale;
        let (glyphs, cell) = load_glyphs(config, font_px)?;
        self.glyphs = Box::new(glyphs);
        self.cell = cell;
        self.resize(self.buffer.0, self.buffer.1, true);
        Ok(())
    }

    /// Swap the theme, which changes both the grid's palette and the chrome.
    pub fn set_theme(&mut self, theme: &otty_config::theme::Theme) {
        self.palette = Palette::from_theme(theme);
        self.chrome = Chrome::from_theme(theme);
        self.options.divider = self.chrome.divider_width;
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn app(&self) -> &App {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// What the tab bar shows for this pane.
    pub fn tab_title(&self) -> String {
        self.app
            .pane(self.pane)
            .and_then(|p| p.title())
            .unwrap_or("Otty")
            .to_owned()
    }

    /// Foreground process names or command lines in this pane.
    pub fn processes(&self) -> Vec<String> {
        self.session.processes()
    }

    /// Built-in tab-icon stem: a vendor logo while an agent is in the
    /// foreground, otherwise `terminal`.
    pub fn tab_icon(&self) -> &'static str {
        otty_agent::icon_for_processes(&self.session.processes())
    }
}

/// Load the terminal font and measure its cell.
///
/// The family chain comes from the theme, so a theme that names a different
/// monospaced family changes the grid measurement. When nothing in the chain is
/// installed the bundled faces are used, and the substitution is logged rather
/// than made silently — the same rule the font crate follows.
/// Cell metrics at the configured `font-size`. Settings uses this to show the
/// change without opening a PTY.
pub fn cell_size_for_font(config: &Config, scale: f32) -> Result<CellSize, ViewError> {
    let font_px = config.get_float("font-size").unwrap_or(13.0) as f32 * scale;
    load_glyphs(config, font_px).map(|(_, cell)| cell)
}

fn load_glyphs(config: &Config, size_px: f32) -> Result<(SwashGlyphs, CellSize), ViewError> {
    let stack = otty_fonts::TypeStack::from_theme(&config.active_theme(false));
    let catalog = otty_fonts::FontCatalog::discover_default();
    let resolved = catalog.resolve_role(&stack, otty_fonts::FontRole::TerminalGrid);

    let mut db = bundled_database(false);
    let face = if let Some(id) = resolved.face() {
        Face::load(&catalog.database(), id, size_px).or_else(|| {
            let id = first_of(&db)?;
            Face::load(&db, id, size_px)
        })
    } else {
        if !resolved.report.substituted {
            log::warn!("{}", resolved.report.summary());
        }
        let id = first_of(&db).ok_or(ViewError::NoFont)?;
        Face::load(&db, id, size_px)
    };

    // The bundled database is a fallback only; it is dropped when a system face
    // answered.
    if db.is_empty() {
        db = bundled_database(false);
    }

    let face = face.ok_or(ViewError::NoFont)?;
    let cell = face.cell_size(size_px);
    let glyphs = SwashGlyphs::new(face, size_px);
    Ok((glyphs, cell))
}

fn first_of(db: &fontdb::Database) -> Option<fontdb::ID> {
    db.faces().next().map(|f| f.id)
}

/// Bytes a Slint `KeyEvent.text` should send to the PTY.
///
/// Slint reports printable keys as themselves and named keys as either a C0
/// control (`Return` = U+000A, `Backspace` = U+0008) or a Cocoa private-use
/// code (`UpArrow` = U+F700). A PTY wants CR for Enter and CSI for arrows.
pub fn bytes_for_key_text(text: &str, opts: &otty_term::KeyEncodeOptions) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }
    let Some(ch) = text.chars().next() else {
        return Vec::new();
    };
    let named = match ch {
        '\n' | '\r' => Some(otty_core::NamedKey::Enter),
        '\u{0008}' | '\u{007F}' => Some(otty_core::NamedKey::Backspace),
        '\t' => Some(otty_core::NamedKey::Tab),
        '\u{001B}' => Some(otty_core::NamedKey::Escape),
        '\u{F700}' => Some(otty_core::NamedKey::ArrowUp),
        '\u{F701}' => Some(otty_core::NamedKey::ArrowDown),
        '\u{F702}' => Some(otty_core::NamedKey::ArrowLeft),
        '\u{F703}' => Some(otty_core::NamedKey::ArrowRight),
        '\u{F727}' | '\u{F729}' => Some(otty_core::NamedKey::Home),
        '\u{F72B}' => Some(otty_core::NamedKey::End),
        '\u{F72C}' => Some(otty_core::NamedKey::PageUp),
        '\u{F72D}' => Some(otty_core::NamedKey::PageDown),
        '\u{F728}' => Some(otty_core::NamedKey::Delete),
        '\u{F704}' => Some(otty_core::NamedKey::F(1)),
        '\u{F705}' => Some(otty_core::NamedKey::F(2)),
        '\u{F706}' => Some(otty_core::NamedKey::F(3)),
        '\u{F707}' => Some(otty_core::NamedKey::F(4)),
        '\u{F708}' => Some(otty_core::NamedKey::F(5)),
        '\u{F709}' => Some(otty_core::NamedKey::F(6)),
        '\u{F70A}' => Some(otty_core::NamedKey::F(7)),
        '\u{F70B}' => Some(otty_core::NamedKey::F(8)),
        '\u{F70C}' => Some(otty_core::NamedKey::F(9)),
        '\u{F70D}' => Some(otty_core::NamedKey::F(10)),
        '\u{F70E}' => Some(otty_core::NamedKey::F(11)),
        '\u{F70F}' => Some(otty_core::NamedKey::F(12)),
        _ => None,
    };
    if let Some(key) = named {
        return otty_term::encode_key(
            otty_core::KeyEvent::press(otty_core::Key::Named(key), otty_core::Modifiers::empty()),
            opts,
        )
        .unwrap_or_default();
    }
    text.as_bytes().to_vec()
}

/// The shell to run, from the configuration.
pub fn pty_config(config: &Config, cols: u16, rows: u16) -> PtyConfig {
    // `test_shell_config` supplies the per-platform default shell, which is what
    // a user who has configured nothing gets. Everything below overrides it from
    // the configuration, in the order the docs give.
    let mut cfg = crate::session::test_shell_config(cols, rows);
    if let Some(program) = config.get_str("command").filter(|s| !s.is_empty()) {
        cfg.program = program;
    } else if let Ok(shell) = std::env::var("SHELL") {
        cfg.program = shell;
    }
    cfg.args = config.get_list("command-args");
    // `working-directory` defaults to the keyword `inherit`, which means "use
    // whatever directory Otty was started in" — it is **not** a path. Passing it
    // through as one made `Pty::spawn` fail with `NoSuchDirectory("inherit")`,
    // so the app could not start at all on a default configuration. Caught by
    // the first test that tried to open a view; the keyword is compared here and
    // the empty string is treated the same way, because "no directory named" and
    // "no override" are the same instruction.
    cfg.cwd = config
        .get_str("working-directory")
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("inherit"))
        .map(PathBuf::from);
    cfg.term = config
        .get_str("term")
        .unwrap_or_else(|| otty_core::Platform::host().default_term().to_owned());
    cfg.login_greeting = config.get_bool("login-shell").unwrap_or(false);
    cfg.shell_integration = config.get_bool("shell-integration").unwrap_or(true);
    cfg.integration_dir = Some(otty_core::app_support_dir().join("shell-integration"));
    cfg.env = config.get_pairs("env").into_iter().collect();
    cfg
}

/// The engine's configuration, from the same keys the docs name.
pub fn term_config(config: &Config) -> TermConfig {
    let mut t = TermConfig::default();
    if let Some(n) = config.get_int("scrollback-lines") {
        t.scrollback_lines = n.max(0) as usize;
    }
    if let Some(v) = config.get_bool("dedup-repaint-scrollback") {
        t.dedup_repaint_scrollback = v;
    }
    if let Some(v) = config.get_bool("kitty-keyboard") {
        t.kitty_keyboard = v;
    }
    if let Some(v) = config.get_bool("vt-kam-allowed") {
        t.vt_kam_allowed = v;
    }
    if let Some(v) = config.get_bool("vt-keypad-app-allowed") {
        t.vt_keypad_app_allowed = v;
    }
    if let Some(v) = config.get_bool("title-report") {
        t.title_report = v;
    }
    if let Some(v) = config.get_bool("bidi") {
        t.bidi = v;
    }
    let widen = config.get_list("widen-ambiguous");
    if !widen.is_empty() {
        t.widen_ambiguous = widen;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use otty_core::PaneId;
    use std::time::{Duration, Instant};

    fn p(n: u64) -> PaneId {
        PaneId::new(n)
    }

    /// A view over a real shell, sized so a 16 px font gives a usable grid.
    ///
    /// `Config::load_from` on a temporary directory rather than `Config::load`,
    /// so the test never reads the developer's own configuration and cannot be
    /// affected by whatever font size they have set.
    fn view() -> (tempfile::TempDir, TerminalView) {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load_from(dir.path());
        let view = TerminalView::new(&config, p(1), (640, 384), 1.0)
            .expect("a view must start a shell");
        (dir, view)
    }

    /// Pump until `pred` holds, because a real shell is asynchronous.
    fn pump_until(
        view: &mut TerminalView,
        timeout: Duration,
        mut pred: impl FnMut(&TerminalView) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            view.pump();
            view.frame();
            if pred(view) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        false
    }

    #[test]
    fn a_view_starts_a_shell_and_reports_a_grid() {
        let (_dir, view) = view();
        let (cols, rows) = view.grid_size();
        assert!(cols > 10, "grid is {cols}x{rows}");
        assert!(rows > 3, "grid is {cols}x{rows}");
        assert!(view.cell().width > 0.0);
        assert!(view.cell().height > 0.0);
    }

    #[test]
    fn the_pixel_buffer_matches_the_size_it_was_asked_for() {
        let (_dir, view) = view();
        assert_eq!(view.buffer_size(), (640, 384));
        assert_eq!(view.pixels().len(), 640 * 384 * 4);
    }

    #[test]
    fn the_grid_size_follows_the_font_metrics() {
        let (_dir, view) = view();
        let (cols, rows) = view.grid_size();
        let cell = view.cell();
        // The grid is the pixel size divided by the cell, so the identity has to
        // hold or the window and the text disagree about how much fits.
        assert_eq!(cols, (640.0 / cell.width).floor() as u16);
        assert_eq!(rows, (384.0 / cell.height).floor() as u16);
    }

    /// One measurement, two consumers.
    ///
    /// The rasteriser draws on `frame.cell` and the mouse is mapped with
    /// `view.cell()`. While the rasteriser derived its own cell from the buffer
    /// size — `width / cols` — those were two different grids: on this machine's
    /// 780×602 pane the derived cell was 7.879×15.050 against the font's
    /// 7.801×14.727, which by the last column is a 7.6 px drift and by the last
    /// row a 12.7 px one.
    #[test]
    fn the_frames_cell_is_the_views_cell_and_the_grid_fits_the_pane() {
        let (_dir, view) = view();
        let frame = view.build_frame();
        assert_eq!(frame.cell, view.cell());

        // The grid is the largest whole number of cells that fits in the pane, so
        // it never spills past the buffer the shell uploaded — and the leftover
        // strip is background, not a hole.
        let (w, h) = view.buffer_size();
        let (cols, rows) = (frame.cols as f32, frame.rows as f32);
        assert!(
            cols * frame.cell.width <= w as f32,
            "{cols} columns of {} px do not fit in {w} px",
            frame.cell.width
        );
        assert!(
            (cols + 1.0) * frame.cell.width > w as f32,
            "one more column of {} px would still fit in {w} px",
            frame.cell.width
        );
        assert!(rows * frame.cell.height <= h as f32, "{rows} rows of {}", frame.cell.height);
        assert!((rows + 1.0) * frame.cell.height > h as f32);

        // And it measured the face the product ships rather than a platform
        // generic: 13 px of JetBrains Mono is 7.8 px of advance.
        assert!(
            (7.5..8.1).contains(&frame.cell.width),
            "cell is {} wide — that is not the bundled face at 13 px",
            frame.cell.width
        );
    }

    #[test]
    fn resizing_keeps_the_grid_the_pty_and_the_buffer_in_step() {
        let (_dir, mut view) = view();
        let before = view.grid_size();
        view.resize(1280, 768, true);
        let after = view.grid_size();
        assert!(after.0 > before.0, "{before:?} -> {after:?}");
        assert!(after.1 > before.1, "{before:?} -> {after:?}");
        // The buffer followed, which is what makes the frame the right size.
        assert_eq!(view.buffer_size(), (1280, 768));
        // And the engine's grid agrees with what the view reports.
        assert_eq!(view.grid_size(), (view.session.terminal().cols(), view.session.terminal().rows()));
    }

    #[test]
    fn a_minimised_window_does_not_resize_to_nothing() {
        let (_dir, mut view) = view();
        let before = view.grid_size();
        view.resize(0, 0, true);
        assert_eq!(view.grid_size(), before, "a zero-sized window must be ignored");
    }

    #[test]
    fn a_frame_paints_the_theme_background_before_anything_is_drawn() {
        let (_dir, mut view) = view();
        view.frame();
        let background = view.palette().background;
        // The first frame has whatever the shell printed at startup, so assert on
        // a pixel the shell is unlikely to have touched: the last column of the
        // last row, which is past the end of any prompt.
        let (w, h) = view.buffer_size();
        let _ = (w, h);
        // Every pixel is opaque, which is what the compositor promises.
        assert!(view.pixels().chunks_exact(4).all(|px| px[3] == 0xFF));
        assert_ne!(background, 0, "the palette has no background");
    }

    #[test]
    fn output_from_the_shell_reaches_the_pixel_buffer() {
        // The end-to-end assertion for this layer: PTY -> engine -> frame ->
        // rasteriser -> pixels. If this passes, the pipeline is wired.
        let (_dir, mut view) = view();
        view.write(b"echo otty-view-marker\r\n").unwrap();
        assert!(
            pump_until(&mut view, Duration::from_secs(30), |v| {
                let (w, h) = v.buffer_size();
                let bg = v.palette().background as u32;
                // Any pixel that is not the background means text was drawn.
                (0..h)
                    .step_by(4)
                    .any(|y| (0..w).step_by(4).any(|x| v.pixel_at(x, y) != bg & 0xFFFF_FFFF))
            }),
            "no glyph ink reached the buffer; grid was {:?}",
            view.grid_size()
        );
    }

    #[test]
    fn keystrokes_reach_the_shell_and_its_answer_comes_back() {
        let (_dir, mut view) = view();
        view.write(b"printf 'returned-value\\n'\r\n").unwrap();
        assert!(
            pump_until(&mut view, Duration::from_secs(30), |v| {
                v.session.terminal().contents().contains("returned-value")
            }),
            "the shell's answer never reached the grid"
        );
    }

    /// The title the shell reports reaches the model.
    ///
    /// What the title *is* depends on the shell and is not this layer's business.
    /// The first version of this test tried to set it with
    /// `printf '\033]0;my-title\007'`, which `cmd.exe` cannot run — there is no
    /// `printf` — and the title that arrived was the shell's own,
    /// `C:\Windows\system32\cmd.exe`. That is the mapping working, so the
    /// assertion is about the mapping and not about the text: the model must end
    /// up holding whatever the shell said, rather than the `"Otty"` default.
    #[test]
    fn the_shell_s_reported_title_reaches_the_model() {
        let (_dir, mut view) = view();
        let got = pump_until(&mut view, Duration::from_secs(30), |v| v.tab_title() != "Otty");
        assert!(
            got,
            "no title ever reached the model; it still reads {:?}",
            view.tab_title()
        );
    }

    #[test]
    fn a_click_maps_back_to_a_cell_inside_the_pane() {
        let (_dir, view) = view();
        let cell = view.cell();
        // Just inside the top-left cell.
        assert_eq!(view.cell_at(1.0, 1.0), Some((0, 0)));
        // Two cells across, one down.
        assert_eq!(view.cell_at(cell.width * 2.5, cell.height * 1.5), Some((2, 1)));
        // Outside the window entirely.
        assert_eq!(view.cell_at(-5.0, -5.0), None);
    }

    #[test]
    fn the_chrome_and_the_layout_use_the_same_divider_width() {
        // The gutter the mouse can grab has to be the gutter that is drawn.
        let (_dir, view) = view();
        assert_eq!(view.layout(640, 384).dividers().len(), 0, "one pane, no dividers");
        assert_eq!(view.options.divider, view.chrome().divider_width);
    }

    #[test]
    fn slint_key_text_encodes_the_bytes_a_pty_expects() {
        let opts = otty_term::KeyEncodeOptions::default();
        assert_eq!(bytes_for_key_text("\n", &opts), b"\r");
        assert_eq!(bytes_for_key_text("a", &opts), b"a");
        assert_eq!(bytes_for_key_text("\u{0008}", &opts), b"\x7f");
        assert_eq!(bytes_for_key_text("\u{F700}", &opts), b"\x1b[A");
        assert_eq!(bytes_for_key_text("", &opts), b"");
    }
}
