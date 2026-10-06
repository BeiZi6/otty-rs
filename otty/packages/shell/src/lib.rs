//! The Slint shell.
//!
//! Thin by design. Everything that decides *what* is on screen is in `otty-ui`
//! (the model, the layout, the view) and `otty-render` (the pixels); this crate
//! binds those to Slint properties and runs the event loop. The rule, repeated
//! from the manifest because it is the one that keeps this crate reviewable: **if
//! a line here needs a test, it belongs in `otty-ui`.**
//!
//! # How the grid reaches the screen
//!
//! Rust composites the whole grid into one RGBA buffer
//! ([`otty_ui::TerminalView::frame`]) and hands it to Slint as an `Image`. The
//! alternative — drawing each cell as a Slint element — would put the glyph atlas
//! and the row-dirty logic on the wrong side of the boundary, and Slint has no
//! element that samples a texture region.
//!
//! The consequence worth knowing: a frame costs one `Image` upload of the whole
//! buffer, not one per changed row. Slint's `SharedPixelBuffer` is reference
//! counted and `set_grid` swaps the reference rather than copying, so the cost is
//! the GPU-side upload, and that is bounded by the window rather than by the
//! output rate.

use std::rc::Rc;

use otty_config::Config;
use otty_core::PaneId;
use otty_render::theme::Chrome;
use otty_ui::view::TerminalView;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

slint::include_modules!();

/// Why the shell could not start.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    #[error("the view could not start: {0}")]
    View(#[from] otty_ui::view::ViewError),
    #[error("the window could not be created: {0}")]
    Window(#[from] slint::PlatformError),
}

/// Map the theme's chrome onto the window's properties.
///
/// Every value is a straight translation, and the ones that are `Option` in
/// `Chrome` fall back to the property's own default rather than to black: a theme
/// that does not name a sidebar colour should leave the shipped default alone,
/// not paint the sidebar the colour of nothing.
fn apply_chrome(theme: &Theme<'_>, chrome: &Chrome) {
    if let Some(c) = chrome.window_background {
        theme.set_window_background(rgba(c));
    }
    if let Some(c) = chrome.titlebar_background {
        theme.set_titlebar_background(rgba(c));
    }
    if let Some(c) = chrome.titlebar_foreground {
        theme.set_titlebar_foreground(rgba(c));
    }
    theme.set_titlebar_height(chrome.titlebar_height);

    if let Some(c) = chrome.tabbar_background {
        theme.set_tabbar_background(rgba(c));
    }
    if let Some(c) = chrome.tab_background {
        theme.set_tab_background(rgba(c));
    }
    if let Some(c) = chrome.tab_foreground {
        theme.set_tab_foreground(rgba(c));
    }
    if let Some(c) = chrome.tab_active_background {
        theme.set_tab_active_background(rgba(c));
    }
    if let Some(c) = chrome.tab_active_foreground {
        theme.set_tab_active_foreground(rgba(c));
    }
    if let Some(c) = chrome.tab_hover_background {
        theme.set_tab_hover_background(rgba(c));
    }
    if let Some(c) = chrome.tab_indicator {
        theme.set_tab_indicator(rgba(c));
    }
    theme.set_tab_radius(chrome.tab_radius);
    theme.set_tab_height(chrome.tab_height);

    if let Some(c) = chrome.sidebar_background {
        theme.set_sidebar_background(rgba(c));
    }
    if let Some(c) = chrome.sidebar_foreground {
        theme.set_sidebar_foreground(rgba(c));
    }
    if let Some(c) = chrome.sidebar_border {
        theme.set_sidebar_border(rgba(c));
    }
    theme.set_sidebar_width(chrome.sidebar_width);

    if let Some(c) = chrome.panel_background {
        theme.set_panel_background(rgba(c));
    }
    if let Some(c) = chrome.panel_foreground {
        theme.set_panel_foreground(rgba(c));
    }
    if let Some(c) = chrome.panel_border {
        theme.set_panel_border(rgba(c));
    }
    theme.set_panel_radius(chrome.panel_radius);

    if let Some(c) = chrome.divider {
        theme.set_divider(rgba(c));
    }
    theme.set_divider_width(chrome.divider_width);
    theme.set_window_radius(chrome.window_radius);
}

/// `0xRRGGBBAA` to a Slint colour.
///
/// The alpha is premultiplied nowhere: Slint's `from_argb_u8` takes straight
/// alpha, which is what the theme stores.
fn rgba(packed: u32) -> slint::Color {
    slint::Color::from_argb_u8(
        (packed & 0xFF) as u8,
        ((packed >> 24) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
    )
}

/// A Slint `length` callback argument as integer pixels.
///
/// The generated type has been `f32` on this Slint; a zero or negative value
/// is a minimised window and is left for `TerminalView::resize` to ignore.
fn logical_px(v: f32) -> u32 {
    v.round().max(0.0) as u32
}

/// Build the window around a view, and run until the window closes.
pub fn run(config: Config) -> Result<(), ShellError> {
    let ui = OttyWindow::new()?;

    // Do not read `Window::size()` on an unrealised window: it reports 0. Use
    // the documented frame dimensions instead, then size the grid to the
    // leftover pane (window minus titlebar and sidebar).
    let window_size = slint::PhysicalSize::new(
        config.get_int("window-width-px").unwrap_or(1000).max(80) as u32,
        config.get_int("window-height-px").unwrap_or(600).max(80) as u32,
    );
    ui.set_sidebar_visible(config.get_bool("sidebar-visible").unwrap_or(true));
    ui.window().set_size(window_size);

    let pane = PaneId::new(1);
    let chrome_preview = {
        let theme = config.active_theme(false);
        Chrome::from_theme(&theme)
    };
    let sidebar = if ui.get_sidebar_visible() {
        chrome_preview.sidebar_width.round() as u32
    } else {
        0
    };
    let titlebar = chrome_preview.titlebar_height.round() as u32;
    let pane_w = window_size.width.saturating_sub(sidebar).max(80);
    let pane_h = window_size.height.saturating_sub(titlebar).max(80);
    let scale = ui.window().scale_factor();
    let view = TerminalView::new(&config, pane, (pane_w, pane_h), scale)?;

    apply_chrome(&ui.global::<Theme>(), view.chrome());
    ui.set_title_text(view.tab_title().into());

    let tabs = Rc::new(VecModel::from(vec![slint::SharedString::from(view.tab_title())]));
    ui.set_tabs(ModelRc::from(tabs.clone()));

    let view = Rc::new(std::cell::RefCell::new(view));

    // Pointer motion and clicks both arrive as a position; the view turns them
    // into a cell, which is the only thing that needs the layout.
    {
        let view = view.clone();
        ui.on_pointer_moved(move |x, y| {
            let mut v = view.borrow_mut();
            if let Some((col, row)) = v.cell_at(x as f32, y as f32) {
                log::debug!("pointer at cell ({col},{row})");
            }
        });
    }

    {
        let view = view.clone();
        ui.on_input_key(move |text| {
            let _ = view.borrow_mut().handle_key(text.as_str());
        });
    }

    // The pane's size drives the grid, the PTY and the pixel buffer together.
    {
        let view = view.clone();
        ui.on_resized(move |w, h| {
            let width = logical_px(w);
            let height = logical_px(h);
            view.borrow_mut().resize(width, height, true);
        });
    }

    let ui_weak = ui.as_weak();
    let last_blink = std::cell::Cell::new(blink_phase());
    let pump = {
        let view = view.clone();
        let tabs = tabs.clone();
        // A timer rather than Slint's `invoke_from_event_loop` in a thread: the
        // view is deliberately not `Send` (it owns a `fontdb` database and a
        // brush cache), so the pump has to run on the UI thread. 8 ms is a cap,
        // not a rate — a quiet terminal does no work and no upload.
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(8),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let mut v = view.borrow_mut();
                let changed = v.pump();
                let title = v.tab_title();
                if ui.get_title_text() != title.as_str() {
                    ui.set_title_text(title.as_str().into());
                    tabs.set_row_data(0, title.as_str().into());
                }
                // Redraw when the shell produced something, or when the cursor's
                // blink phase toggled — which is a frame with no new bytes.
                // The previous test (`!blink`) skipped the off-phase frame, so
                // the cursor stuck on.
                let blink = blink_phase();
                let blink_changed = blink != last_blink.get();
                last_blink.set(blink);
                if !changed && !blink_changed {
                    return;
                }
                v.set_cursor_blink(blink);
                v.frame();
                let (w, h) = v.buffer_size();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    v.pixels(),
                    w,
                    h,
                );
                ui.set_grid(slint::Image::from_rgba8(buffer));
            },
        );
        // The timer must outlive this function; owning it here and leaking it is
        // what keeps it running for the life of the window.
        Box::leak(Box::new(timer))
    };
    let _ = pump;

    // `OTTY_DUMP_FRAME=<path>` writes the composited grid to a PNG and quits.
    //
    // This exists because "it compiles" and "it looks right" are different claims
    // and only one of them can be checked by reading code. It dumps the buffer
    // this crate composited rather than asking Slint to photograph the window:
    // the grid is what this build actually draws, and a PNG of it is checkable on
    // a machine where nobody can see the screen.
    if let Some(path) = std::env::var_os("OTTY_DUMP_FRAME") {
        let ui_weak = ui.as_weak();
        let view = view.clone();
        // Owned, because the closure runs many times over the app's life and the
        // `OsString` cannot be moved out of the enclosing scope by a `FnMut`.
        let out = std::path::PathBuf::from(path);
        let after = std::env::var("OTTY_DUMP_AFTER_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1200);
        let dump = slint::Timer::default();
        dump.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(after),
            move || {
                let _ = ui_weak.upgrade();
                let mut v = view.borrow_mut();
                v.pump();
                v.frame();
                let (w, h) = v.buffer_size();
                match image::save_buffer(
                    &out,
                    v.pixels(),
                    w,
                    h,
                    image::ColorType::Rgba8,
                ) {
                    Ok(()) => eprintln!("otty: wrote {} ({w}x{h}, {} bytes)", out.display(), w * h * 4),
                    Err(e) => eprintln!("otty: could not write {}: {e}", out.display()),
                }
                let _ = slint::quit_event_loop();
            },
        );
        Box::leak(Box::new(dump));
    }

    ui.run()?;
    // Closing the window kills the shell rather than leaving it behind.
    view.borrow_mut().close();
    Ok(())
}

/// The cursor's blink phase, from the wall clock.
///
/// The view does not own a clock — a rasteriser that did would be untestable —
/// so the shell supplies the phase. 500 ms on, 500 ms off, which is the
/// conventional rate.
fn blink_phase() -> bool {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    (millis / 500) % 2 == 0
}
