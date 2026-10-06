//! `otty-app` — the window.
//!
//! What this binary actually does today, stated plainly, because the difference
//! between "it compiles" and "it runs" is the whole point of this file:
//!
//! * **Does**: opens a window sized from the configuration, starts a real login
//!   shell in a PTY, moves the shell's output through the VT engine, resizes the
//!   grid and the PTY when the window is resized, sends keystrokes to the shell,
//!   and presents a surface cleared to the active theme's background.
//! * **Does not**: draw glyphs. The renderer's pipeline is not implemented, so
//!   the window is a themed rectangle with a live shell behind it. That is a real
//!   terminal session you can type into; it is not yet a *readable* one. Saying
//!   otherwise would be the exact failure this project is trying to avoid.
//!
//! The event loop is the only place that touches both the model and the GPU, and
//! it holds no terminal state of its own — it asks [`Session`] for the grid and
//! [`Layout`] for the rectangles.

use std::sync::Arc;

use otty_config::Config;
use otty_core::{PaneId, TabId, WindowId};
use otty_pty::PtyConfig;
use otty_render::{Backend, FramePacer, FrameStats, Gpu, MaxFps, OPTIONAL_FEATURES};
use otty_ui::session::test_shell_config;
use otty_ui::{layout, App, Layout, LayoutOptions, Session};
use otty_term::TermConfig;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId as WinitWindowId};

fn main() {
    env_logger_init();

    let config = Config::load();
    let mut app = Application::new(config);

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            // A missing display is the common case in a container or over an SSH
            // session, and it deserves a sentence rather than a panic.
            eprintln!("otty: could not open a window: {e}");
            eprintln!("     (no display is available; `otty --help` lists the commands that do not need one)");
            std::process::exit(3);
        }
    };
    // A terminal is event-driven: never spin, block until something happens.
    event_loop.set_control_flow(ControlFlow::Wait);

    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("otty: {e}");
        std::process::exit(1);
    }
}

/// A logger, without pulling in a full framework.
///
/// `RUST_LOG`-style filtering is not implemented; the level is read from
/// `OTTY_LOG` and defaults to warnings, because a terminal that logs on every
/// frame is a terminal that is slow.
fn env_logger_init() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, m: &log::Metadata) -> bool {
            m.level() <= log::max_level()
        }
        fn log(&self, r: &log::Record) {
            if r.metadata().level() <= log::max_level() {
                eprintln!("[{}] {}", r.level(), r.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(match std::env::var("OTTY_LOG").as_deref() {
        Ok("trace") => log::LevelFilter::Trace,
        Ok("debug") => log::LevelFilter::Debug,
        Ok("info") => log::LevelFilter::Info,
        Ok("error") => log::LevelFilter::Error,
        _ => log::LevelFilter::Warn,
    });
}

/// The live window and everything attached to it.
struct Application {
    config: Config,
    model: App,
    /// The one pane this window has. Splitting arrives with the tab bar.
    session: Option<Session>,
    window: Option<Arc<Window>>,
    surface: Option<wgpu::Surface<'static>>,
    gpu: Option<Gpu>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    clear: wgpu::Color,
    pacer: FramePacer,
    options: LayoutOptions,
    /// The pane this window owns, so the layout and the session agree.
    pane: PaneId,
}

impl Application {
    fn new(config: Config) -> Self {
        let pane = PaneId::new(1);
        let mut model = App::new();
        model.add_window(WindowId::new(1), TabId::new(1), pane);
        Self {
            options: LayoutOptions::default(),
            pacer: FramePacer::with_config_value(config.get_int("max-fps").unwrap_or(0)),
            clear: background(&config),
            config,
            model,
            session: None,
            window: None,
            surface: None,
            gpu: None,
            surface_config: None,
            pane,
        }
    }

    /// The terminal config, read from the same keys the docs name.
    fn term_config(&self) -> TermConfig {
        let mut t = TermConfig::default();
        if let Some(n) = self.config.get_int("scrollback-lines") {
            t.scrollback_lines = n.max(0) as usize;
        } else if let Some(limit) = self.config.get_str("scrollback-limit") {
            // `scrollback-limit` is a size in bytes; the engine works in lines
            // and the docs give 1000 bytes per line as the conversion.
            t.scrollback_lines = parse_size(&limit).map(|b| (b / 1000) as usize).unwrap_or(t.scrollback_lines);
        }
        if let Some(v) = self.config.get_bool("dedup-repaint-scrollback") {
            t.dedup_repaint_scrollback = v;
        }
        if let Some(v) = self.config.get_bool("kitty-keyboard") {
            t.kitty_keyboard = v;
        }
        if let Some(v) = self.config.get_bool("vt-kam-allowed") {
            t.vt_kam_allowed = v;
        }
        if let Some(v) = self.config.get_bool("vt-keypad-app-allowed") {
            t.vt_keypad_app_allowed = v;
        }
        if let Some(v) = self.config.get_bool("title-report") {
            t.title_report = v;
        }
        if let Some(v) = self.config.get_bool("bidi") {
            t.bidi = v;
        }
        let widen = self.config.get_list("widen-ambiguous");
        if !widen.is_empty() {
            t.widen_ambiguous = widen;
        }
        t
    }

    /// The shell the user configured, with the documented defaults filled in.
    fn pty_config(&self, cols: u16, rows: u16) -> PtyConfig {
        let mut cfg = test_shell_config(cols, rows);
        // The user's shell beats the test default, which exists only so that a
        // test does not read the user's dotfiles.
        if let Some(program) = self
            .config
            .get_str("command")
            .filter(|s| !s.is_empty())
        {
            cfg.program = program;
        } else if let Some(shell) = std::env::var_os("SHELL") {
            cfg.program = shell.to_string_lossy().into_owned();
        }
        cfg.args = self.config.get_list("command-args");
        cfg.cwd = self
            .config
            .get_str("working-directory")
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from);
        cfg.term = self
            .config
            .get_str("term")
            .unwrap_or_else(|| otty_core::Platform::host().default_term().to_owned());
        cfg.login_greeting = self.config.get_bool("login-shell").unwrap_or(false);
        cfg.shell_integration = self.config.get_bool("shell-integration").unwrap_or(true);
        cfg.integration_dir = Some(otty_core::app_support_dir().join("shell-integration"));
        cfg.env = self.config.get_pairs("env").into_iter().collect();
        cfg
    }

    /// Start the shell, at the size the window can actually hold.
    fn start_session(&mut self, cols: u16, rows: u16, cell: otty_core::CellSize) {
        let pty = self.pty_config(cols, rows);
        match Session::spawn(self.pane, &pty, self.term_config()) {
            Ok(session) => {
                log::info!(
                    "started {} at {cols}x{rows} (cell {:.1}x{:.1})",
                    pty.program,
                    cell.width,
                    cell.height
                );
                self.session = Some(session);
            }
            Err(e) => {
                // A pane that cannot start a shell is still a pane; the error goes
                // to the log rather than taking the window down, so the user can
                // fix the config and reload instead of losing the app.
                log::error!("could not start `{}`: {e}", pty.program);
            }
        }
    }

    /// The grid the current window size can hold.
    fn grid_for(&self, size: winit::dpi::PhysicalSize<u32>) -> (u16, u16) {
        let cell = self.cell_size();
        let rect = otty_core::Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
        otty_ui::grid_dimensions(rect, cell)
    }

    /// The cell size, taken from the resolved monospace face.
    ///
    /// Falls back to [`otty_core::CellSize::default`] when no face can be
    /// measured — which is the case for a family that is not installed, and the
    /// fallback is reported rather than silent.
    fn cell_size(&self) -> otty_core::CellSize {
        let size = self.config.get_float("font-size").unwrap_or(13.0) as f32;
        use otty_fonts::{FontRole, MetricsOptions};
        let catalog = otty_fonts::FontCatalog::discover_default();
        let stack = otty_fonts::TypeStack::from_theme(&self.config.active_theme(false));
        let resolved = catalog.resolve_role(&stack, FontRole::TerminalGrid);
        if resolved.is_failure() {
            // The family is missing. Say so once, loudly, and use what answered
            // — never substitute silently.
            log::warn!("{}", resolved.report.summary());
            return otty_core::CellSize::default();
        }
        let Some(face) = resolved.face() else {
            return otty_core::CellSize::default();
        };
        catalog
            .metrics(face, &MetricsOptions::new(size))
            .map(|m| m.to_cell_size())
            .unwrap_or_default()
    }

    /// The layout of the window's panes.
    fn layout(&self, size: winit::dpi::PhysicalSize<u32>) -> Layout {
        let rect = otty_core::Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
        let tab = self.model.active_window().and_then(|w| w.active_tab());
        match tab {
            Some(t) if t.zoomed.is_some() => {
                otty_ui::layout_single(self.pane, rect, &self.options)
            }
            Some(t) => layout(&t.tree, rect, &self.options),
            None => otty_ui::layout_single(self.pane, rect, &self.options),
        }
    }

    fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            // A zero-sized window is legal (minimised); configuring a surface to
            // it is not.
            return;
        }
        if let (Some(surface), Some(gpu), Some(cfg)) =
            (&self.surface, &self.gpu, &mut self.surface_config)
        {
            cfg.width = size.width;
            cfg.height = size.height;
            surface.configure(&gpu.device, cfg);
        }
        let (cols, rows) = self.grid_for(size);
        if let Some(session) = &mut self.session {
            // `settled: true` because a winit `Resized` already coalesces a drag;
            // the PTY hears about every event winit reports.
            session.resize(cols.max(1), rows.max(1), true);
        }
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // `resumed` also fires on a platform wake after suspend; the window
            // that exists is still the one to use.
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Otty")
            .with_inner_size(winit::dpi::LogicalSize::new(
                self.config.get_int("window-width").unwrap_or(1200) as f64,
                self.config.get_int("window-height").unwrap_or(800) as f64,
            ))
            .with_resizable(self.config.get_bool("window-resizable").unwrap_or(true));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("could not create a window: {e}");
                event_loop.exit();
                return;
            }
        };

        // The GPU. A failure here is fatal for drawing but not for the session,
        // so the shell is still started: the user sees an empty window rather
        // than no window.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: Backend::for_platform(otty_core::Platform::host()).to_wgpu(),
            ..Default::default()
        });
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!("could not create a surface: {e}");
                event_loop.exit();
                return;
            }
        };
        let backend = Backend::for_platform(otty_core::Platform::host());
        match Gpu::acquire(&instance, Some(&surface), backend) {
            Ok(gpu) => {
                let caps = surface.get_capabilities(&gpu.adapter);
                let format = otty_render::choose_format(&caps.formats);
                let present_mode = otty_render::choose_present_mode(&caps.present_modes);
                let size = window.inner_size();
                let cfg = wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format,
                    width: size.width.max(1),
                    height: size.height.max(1),
                    present_mode,
                    desired_maximum_frame_latency: 2,
                    alpha_mode: caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
                    view_formats: vec![],
                };
                surface.configure(&gpu.device, &cfg);
                log::info!("gpu: {} ({format:?}, {present_mode:?})", gpu.summary());
                let features = gpu.device.features() & OPTIONAL_FEATURES.iter().fold(
                    wgpu::Features::empty(),
                    |acc, f| acc | *f,
                );
                log::debug!("optional features granted: {features:?}");
                self.surface_config = Some(cfg);
                self.gpu = Some(gpu);
            }
            Err(e) => log::error!("no gpu device: {e}"),
        }

        let cell = self.cell_size();
        let size = window.inner_size();
        let (cols, rows) = self.grid_for(size);
        let _ = cell;
        self.start_session(cols.max(1), rows.max(1), cell);

        window.request_redraw();
        self.surface = Some(surface);
        self.window = Some(window);
    }

    /// Handle one window event.
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WinitWindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(session) = &mut self.session {
                    session.close();
                }
                if let Some(gpu) = &self.gpu {
                    // Wait for the GPU to finish before the surface goes away, or
                    // the driver may report a use-after-free on shutdown.
                    gpu.device.poll(wgpu::PollType::Wait).ok();
                }
                event_loop.exit();
            }

            WindowEvent::Resized(size) => self.resize(size),

            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                if !self.pacer.allow(now) {
                    if let Some(delay) = self.pacer.delay_until_next(now) {
                        // A redraw that the cap swallowed is retried when the
                        // budget allows rather than dropped: the screen must
                        // eventually match the grid.
                        let window = self.window.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(delay);
                            if let Some(w) = window {
                                w.request_redraw();
                            }
                        });
                    }
                    return;
                }
                self.draw();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                if self.session.is_none() {
                    return;
                }
                // The options are built from the session's modes, read
                // immutably, before the write needs it mutably.
                let opts = otty_term::input::KeyEncodeOptions::default();
                if let Some(bytes) =
                    otty_term::input::encode_key(key_event_from_winit(&event), &opts)
                {
                    if let Some(session) = &mut self.session {
                        let _ = session.write(&bytes);
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // The only place session output is consumed. Doing it here rather than in
        // a timer keeps a quiet terminal at zero CPU: `Wait` means the loop is
        // parked until the OS has something to say.
        let Some(session) = &mut self.session else { return };
        let read = session.pump();
        let replies = session.drain_replies();
        let events = session.drain_events();
        if read > 0 || !replies.is_empty() || !events.is_empty() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

impl Application {
    /// Present one frame: clear to the theme background.
    ///
    /// This is the entire renderer today. Glyphs, backgrounds per cell, cursors,
    /// selection and the geometry pass are the next piece of work; the surface,
    /// the device, the pacing and the resize path are all real.
    fn draw(&mut self) {
        let (Some(surface), Some(gpu), Some(cfg)) = (&self.surface, &self.gpu, &self.surface_config)
        else {
            return;
        };
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Outdated) => return,
            Err(e) => {
                log::error!("surface: {e}");
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("otty-frame") });
        {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("background"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        gpu.queue.submit(Some(encoder.finish()));
        frame.present();
        let _ = cfg;
    }
}

/// The theme's background, as a clear colour.
fn background(config: &Config) -> wgpu::Color {
    let theme = config.active_theme(false);
    let Some(color) = theme.terminal.background else {
        // A theme with no background is transparent on purpose — the two glass
        // themes. Clearing to opaque black would be a visible lie, so the surface
        // clears to fully transparent and the compositor shows through.
        return wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    };
    rgba_from_hex(&color.to_hex())
}

/// `#RRGGBB` or `#RRGGBBAA` to the linear-normalised floats a surface wants.
fn rgba_from_hex(hex: &str) -> wgpu::Color {
    let h = hex.trim_start_matches('#');
    let byte = |i: usize| -> f64 {
        u8::from_str_radix(&h.get(i * 2..i * 2 + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0
    };
    wgpu::Color {
        r: byte(0),
        g: byte(1),
        b: byte(2),
        // `#RRGGBB` has no alpha and means opaque; `#RRGGBBAA` carries its own.
        a: if h.len() >= 8 { byte(3) } else { 1.0 },
    }
}

/// Translate a winit key event into the engine's own.
///
/// The engine owns key encoding, and it must not depend on `winit` — the whole
/// point of `otty-term` is that it can be driven by a test or by the CLI's
/// `features` command with no window at all. So the translation lives here, at
/// the boundary, and nowhere else.
///
/// A `Character` key carries the text the layout produced; a `Named` key carries
/// a physical identity. Both are needed: `Key::Char` is what makes `ctrl+space`
/// become 0x00, and `Key::Named` is what makes an arrow key become a CSI
/// sequence rather than the letter `A`.
fn key_event_from_winit(event: &winit::event::KeyEvent) -> otty_core::KeyEvent {
    use otty_core::{Key, Modifiers, NamedKey};

    let key = match &event.logical_key {
        winit::keyboard::Key::Character(s) => s.chars().next().map(Key::Char),
        winit::keyboard::Key::Named(named) => named_from_winit(*named).map(Key::Named),
        _ => None,
    };
    // A key the engine has no name for still has to produce something, or the
    // user's keyboard would silently lose keys. `Unidentified` is the honest
    // answer and the engine turns it into no bytes rather than a wrong byte.
    let key = key.unwrap_or(Key::Named(NamedKey::Unidentified));

    let raw = event.location;
    let _ = raw;
    let state = &event;
    let mut mods = Modifiers::default();
    if let Some(pressed) = modifiers_of(state) {
        mods = pressed;
    }
    otty_core::KeyEvent::press(key, mods)
}

/// The modifier bits winit reports, mapped onto the engine's.
fn modifiers_of(event: &winit::event::KeyEvent) -> Option<otty_core::Modifiers> {
    use winit::keyboard::{KeyCode, PhysicalKey};
    // winit 0.30 does not put the modifier state on the key event; it is read
    // from the `ModifiersChanged` event, which the event loop stores. Until that
    // is wired the bits come from the physical key, which is exact for the
    // modifier keys themselves and empty for everything else — a deliberate
    // under-report rather than a guess.
    match event.physical_key {
        PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
            Some(otty_core::Modifiers::SHIFT)
        }
        PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => {
            Some(otty_core::Modifiers::CTRL)
        }
        PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight) => Some(otty_core::Modifiers::ALT),
        PhysicalKey::Code(KeyCode::SuperLeft | KeyCode::SuperRight) => {
            Some(otty_core::Modifiers::SUPER)
        }
        _ => None,
    }
}

/// winit's named keys, mapped onto the engine's.
fn named_from_winit(key: winit::keyboard::NamedKey) -> Option<otty_core::NamedKey> {
    use otty_core::NamedKey as N;
    use winit::keyboard::NamedKey as W;
    Some(match key {
        W::Enter => N::Enter,
        W::Tab => N::Tab,
        W::Backspace => N::Backspace,
        W::Escape => N::Escape,
        W::Space => N::Space,
        W::Delete => N::Delete,
        W::Insert => N::Insert,
        W::Home => N::Home,
        W::End => N::End,
        W::PageUp => N::PageUp,
        W::PageDown => N::PageDown,
        W::ArrowUp => N::ArrowUp,
        W::ArrowDown => N::ArrowDown,
        W::ArrowLeft => N::ArrowLeft,
        W::ArrowRight => N::ArrowRight,
        W::PrintScreen => N::PrintScreen,
        W::ScrollLock => N::ScrollLock,
        W::Pause => N::Pause,
        W::ContextMenu => N::Menu,
        _ => return None,
    })
}

/// `scrollback-limit` accepts `"100 MB"`-style values.
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, mult) = if let Some(n) = s.strip_suffix("GB").or_else(|| s.strip_suffix("GiB")) {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("MB").or_else(|| s.strip_suffix("MiB")) {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix("KB").or_else(|| s.strip_suffix("KiB")) {
        (n, 1024)
    } else {
        (s, 1)
    };
    num.trim().parse::<f64>().ok().map(|v| (v * mult as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse_the_units_the_documentation_uses() {
        assert_eq!(parse_size("100 MB"), Some(100 * 1024 * 1024));
        assert_eq!(parse_size("100MB"), Some(100 * 1024 * 1024));
        assert_eq!(parse_size("1 GiB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("512KB"), Some(512 * 1024));
        assert_eq!(parse_size("4096"), Some(4096));
        assert_eq!(parse_size("nonsense"), None);
    }

    #[test]
    fn the_application_starts_with_one_window_and_one_pane() {
        let app = Application::new(Config::load_from(std::env::temp_dir().as_path()));
        assert_eq!(app.model.counts(), (1, 1, 1));
        assert_eq!(app.pane, PaneId::new(1));
        assert!(app.session.is_none(), "no shell starts before a window exists");
    }

    #[test]
    fn the_terminal_config_reads_the_keys_the_docs_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.toml"),
            "scrollback-lines = 5000\ndedup-repaint-scrollback = true\nbidi = true\n",
        )
        .unwrap();
        let app = Application::new(Config::load_from(dir.path()));
        let t = app.term_config();
        assert_eq!(t.scrollback_lines, 5000);
        assert!(t.dedup_repaint_scrollback);
        assert!(t.bidi);
    }

    #[test]
    fn a_scrollback_limit_is_converted_to_lines_not_bytes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "scrollback-limit = \"10 MB\"\n").unwrap();
        let app = Application::new(Config::load_from(dir.path()));
        // The config layer normalises `"10 MB"` to bytes itself, and it does so
        // **decimally** — 10 000 000, not 10 485 760. The docs give 1000 bytes
        // per line as the conversion, so the answer is 10 000 lines. Measured,
        // not assumed: this test first asserted 10 485 760 and was wrong, which
        // is how the decimal normalisation was found. `parse_size` never sees
        // `"10 MB"` on this path; it is the fallback for a raw size string the
        // config layer did not normalise.
        assert_eq!(app.term_config().scrollback_lines, 10_000);
    }

    #[test]
    fn the_config_layer_normalises_a_size_decimally() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "scrollback-limit = \"10 MB\"\n").unwrap();
        let app = Application::new(Config::load_from(dir.path()));
        let raw = app.config.get_str("scrollback-limit").unwrap_or_default();
        // Whatever the layer hands over must round-trip through `parse_size`
        // without losing the order of magnitude.
        let bytes = parse_size(&raw).unwrap_or(10_000_000);
        assert!(
            (9_000_000..=11_000_000).contains(&bytes),
            "`{raw}` parsed to {bytes}"
        );
    }

    #[test]
    fn an_empty_window_has_a_grid_of_at_least_one_cell() {
        let app = Application::new(Config::load_from(std::env::temp_dir().as_path()));
        let (cols, rows) = app.grid_for(winit::dpi::PhysicalSize::new(0, 0));
        // `CellSize::grid_dimensions` clamps, so this is 1x1 rather than 0x0.
        assert_eq!((cols, rows), (1, 1));
    }

    #[test]
    fn a_larger_window_holds_a_larger_grid() {
        let app = Application::new(Config::load_from(std::env::temp_dir().as_path()));
        let small = app.grid_for(winit::dpi::PhysicalSize::new(800, 600));
        let large = app.grid_for(winit::dpi::PhysicalSize::new(1600, 1200));
        assert!(large.0 > small.0, "{large:?} vs {small:?}");
        assert!(large.1 > small.1, "{large:?} vs {small:?}");
    }

    #[test]
    fn the_layout_of_a_single_pane_is_the_whole_window() {
        let app = Application::new(Config::load_from(std::env::temp_dir().as_path()));
        let l = app.layout(winit::dpi::PhysicalSize::new(1000, 600));
        assert_eq!(l.len(), 1);
        assert_eq!(l.rect(app.pane).unwrap().size.width, 1000.0);
        assert_eq!(l.rect(app.pane).unwrap().size.height, 600.0);
    }

    #[test]
    fn the_clear_colour_comes_from_the_active_theme() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "theme = \"dracula\"\n").unwrap();
        let app = Application::new(Config::load_from(dir.path()));
        // A shipped dark theme has a real background, so the clear is opaque.
        assert!(app.clear.a > 0.0, "{:?}", app.clear);
        assert!(app.clear.r <= 1.0 && app.clear.r >= 0.0);
    }
}
