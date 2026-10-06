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
use otty_config::SaveScope;
use otty_ui::{
    context_rows, details_info, filter_palette, palette_entries, palette_rows, pane_hit,
    query_quickly, quickly_rows, set_setting, shell_menu_models, view_target_from_args, Dispatcher,
    QuicklySources,
};
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

fn menu_model(rows: &[(String, String)]) -> ModelRc<MenuRow> {
    let items: Vec<MenuRow> = rows
        .iter()
        .map(|(title, action)| MenuRow {
            title: title.as_str().into(),
            action: action.as_str().into(),
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

/// Fill the native MenuBar from `menu_bar()` and send clicks to `dispatch`.
fn bind_menu(ui: &OttyWindow, config: &Config, dispatch: Rc<std::cell::RefCell<Dispatcher>>) {
    let models = shell_menu_models(None, &config.keymap());
    for (title, rows) in &models {
        let model = menu_model(rows);
        match *title {
            "File" => ui.set_menu_file(model),
            "Edit" => ui.set_menu_edit(model),
            "View" => ui.set_menu_view(model),
            "Shell" => ui.set_menu_shell(model),
            "Window" => ui.set_menu_window(model),
            "Commands" => ui.set_menu_commands(model),
            _ => {}
        }
    }
    let ui_weak = ui.as_weak();
    let font = config.get_float("font-size").unwrap_or(13.0) as i32;
    ui.on_menu_activated(move |action| {
        let mut d = dispatch.borrow_mut();
        let _ = d.activate(action.as_str());
        if d.settings_open {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_settings_open(true);
                ui.set_settings_font_size(font);
            }
        }
        if d.palette_open {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_open(true);
            }
        }
        if d.quickly_open {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_quickly_open(true);
            }
        }
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_details_open(d.details_open);
            ui.set_details_tab(d.details_tab.as_str().into());
            ui.set_details_cwd(details_info(&d.app).cwd.into());
        }
        if d.quit {
            let _ = slint::quit_event_loop();
        }
    });
}

fn palette_model(rows: &[(String, String, String)]) -> ModelRc<PaletteRow> {
    let items: Vec<PaletteRow> = rows
        .iter()
        .map(|(title, action, scope)| PaletteRow {
            title: title.as_str().into(),
            action: action.as_str().into(),
            scope: scope.as_str().into(),
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

fn bind_palette(ui: &OttyWindow, config: &Config, dispatch: Rc<std::cell::RefCell<Dispatcher>>) {
    let map = config.keymap();
    let all = palette_entries(None, &map);
    ui.set_palette_rows(palette_model(&palette_rows(&all)));
    {
        let map = map.clone();
        let ui_weak = ui.as_weak();
        ui.on_palette_query_changed(move |query| {
            let entries = palette_entries(None, &map);
            let hits: Vec<_> = filter_palette(&entries, query.as_str())
                .into_iter()
                .cloned()
                .collect();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_rows(palette_model(&palette_rows(&hits)));
            }
        });
    }
    {
        let dispatch = dispatch.clone();
        let ui_weak = ui.as_weak();
        ui.on_palette_run(move |action, keep_open| {
            let mut d = dispatch.borrow_mut();
            let _ = d.run_palette(action.as_str(), keep_open);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_palette_open(d.palette_open);
            }
        });
    }
}

fn bind_quickly(ui: &OttyWindow, dispatch: Rc<std::cell::RefCell<Dispatcher>>) {
    {
        let dispatch = dispatch.clone();
        let ui_weak = ui.as_weak();
        ui.on_quickly_query_changed(move |query| {
            let d = dispatch.borrow();
            let sources = QuicklySources::live(d.opened.clone());
            let hits = query_quickly(d.quickly_filter, query.as_str(), &sources);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_quickly_rows(palette_model(&quickly_rows(&hits)));
            }
        });
    }
    {
        let dispatch = dispatch.clone();
        let ui_weak = ui.as_weak();
        ui.on_quickly_run(move |path| {
            let mut d = dispatch.borrow_mut();
            let _ = d.run_quickly_path(path.as_str());
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_quickly_open(d.quickly_open);
            }
        });
    }
    {
        let dispatch = dispatch.clone();
        let ui_weak = ui.as_weak();
        ui.on_quickly_set_filter(move |key| {
            let mut d = dispatch.borrow_mut();
            d.set_quickly_filter(key.as_str());
            let filter_name = d
                .quickly_filter
                .map(|f| f.as_str().to_string())
                .unwrap_or_default();
            let sources = QuicklySources::live(d.opened.clone());
            let hits = query_quickly(d.quickly_filter, "", &sources);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_quickly_filter(filter_name.into());
                ui.set_quickly_rows(palette_model(&quickly_rows(&hits)));
            }
        });
    }
}

fn bind_context(ui: &OttyWindow, view: Rc<std::cell::RefCell<otty_ui::view::TerminalView>>, dispatch: Rc<std::cell::RefCell<Dispatcher>>) {
    let ui_weak = ui.as_weak();
    ui.on_context_prepare(move |x, y| {
        let v = view.borrow();
        let hit = if let Some((col, row)) = v.cell_at(x as f32, y as f32) {
            let line = v.line_text(row);
            let sel = dispatch.borrow().hit.selection.clone();
            pane_hit(&line, col as usize, sel.as_deref())
        } else {
            pane_hit("", 0, dispatch.borrow().hit.selection.as_deref())
        };
        drop(v);
        let rows = context_rows(&hit);
        dispatch.borrow_mut().hit = hit;
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_ctx_pane(menu_model(&rows));
        }
    });
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

    let dispatch = Rc::new(std::cell::RefCell::new(Dispatcher::new()));
    dispatch.borrow_mut().quit_after_last_window_closed =
        config.get_bool("quit-after-last-window-closed").unwrap_or(false);
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if let Some(target) = view_target_from_args(&args) {
            let _ = dispatch.borrow_mut().open_view(target);
        }
    }
    bind_menu(&ui, &config, dispatch.clone());
    bind_palette(&ui, &config, dispatch.clone());
    bind_quickly(&ui, dispatch.clone());
    {
        let dispatch = dispatch.clone();
        ui.on_close_tab_at(move |index| {
            let _ = dispatch.borrow_mut().close_tab_at(index as usize);
        });
    }
    {
        let dispatch = dispatch.clone();
        let ui_weak = ui.as_weak();
        ui.on_select_tab_at(move |index| {
            let mut d = dispatch.borrow_mut();
            let _ = d.select_tab_at(index as usize);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_details_cwd(details_info(&d.app).cwd.into());
            }
        });
    }
    {
        let dispatch = dispatch.clone();
        let ui_weak = ui.as_weak();
        ui.on_details_select_tab(move |tab| {
            let action = match tab.as_str() {
                "outline" => "details_tab_outline",
                "git" => "details_tab_git",
                "files" => "details_tab_files",
                _ => "details_tab_info",
            };
            let mut d = dispatch.borrow_mut();
            let _ = d.activate(action);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_details_tab(d.details_tab.as_str().into());
                ui.set_details_cwd(details_info(&d.app).cwd.into());
            }
        });
    }

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
    bind_context(&ui, view.clone(), dispatch.clone());
    {
        let dir = config.dir().to_path_buf();
        let view = view.clone();
        let scale = ui.window().scale_factor();
        ui.on_settings_apply_font(move |size| {
            let mut cfg = Config::load_from(&dir);
            let raw = size.to_string();
            if set_setting(&mut cfg, "font-size", &raw, SaveScope::Auto).is_ok() {
                let _ = view.borrow_mut().apply_font_size(&cfg, scale);
            }
        });
    }

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

#[cfg(test)]
mod tests {
    #[test]
    fn slint_binds_native_menubar_from_menu_bar() {
        let src = include_str!("../ui/app.slint");
        assert!(src.contains("MenuBar"), "OttyWindow needs a native MenuBar");
        for title in ["File", "Edit", "View", "Shell", "Window", "Commands"] {
            assert!(
                src.contains(&format!("title: \"{title}\"")),
                "MenuBar missing {title}"
            );
        }
        assert!(src.contains("menu-activated"));
        assert!(src.contains("for row in root.menu-file"));
        assert!(src.contains("ContextMenuArea"), "pane/tab need a context menu");
        assert!(src.contains("Close Tab"));
        assert!(src.contains("ctx-pane"));
        assert!(src.contains("settings-open"), "Settings window");
        assert!(src.contains("settings-apply-font"));
        assert!(src.contains("palette-open"), "Command Palette overlay");
        assert!(src.contains("palette-run"));
    }
}
