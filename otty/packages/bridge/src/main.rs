use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use base64::Engine;
use otty_config::Config;
use otty_core::PaneId;
use otty_ui::TerminalView;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Command {
    Input { text: String },
    Resize { width: u32, height: u32 },
    Close,
}

fn main() {
    if let Err(error) = run() {
        emit(&serde_json::json!({"type":"error", "message":error.to_string()}));
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let scale = std::env::var("OTTY_SCALE_FACTOR")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(1.0);
    let config = Config::load();
    let mut view = TerminalView::new(&config, PaneId::new(1), (1200, 700), scale)?;
    let (commands, _reader) = command_channel();
    let chrome = serde_json::to_value(view.chrome())?;
    let layout = config.get_str("window-layout").unwrap_or_else(|| "sidebar-left".to_owned());
    let sidebar_visible = config.get_bool("sidebar-visible").unwrap_or(true);

    emit(&serde_json::json!({
        "type":"ready",
        "title":view.tab_title(),
        "cwd":std::env::current_dir()?.to_string_lossy(),
        "grid": {"cols":view.grid_size().0, "rows":view.grid_size().1},
        "chrome":chrome,
        "layout":layout,
        "sidebar_visible":sidebar_visible,
        "icon":view.tab_icon(),
    }));
    view.frame();
    emit_frame(&view)?;

    let mut last_title = view.tab_title();
    let mut last_cwd = None;
    let mut last_blink = false;
    let mut last_icon = view.tab_icon().to_owned();
    let mut last_icon_check = std::time::Instant::now();
    loop {
        let mut changed = false;
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Input { text } => {
                    view.handle_key(&text)?;
                    changed = true;
                }
                Command::Resize { width, height } => {
                    let width = width.clamp(80, 8192);
                    let height = height.clamp(80, 8192);
                    view.resize(width, height, true);
                    emit(&serde_json::json!({
                        "type":"grid",
                        "cols":view.grid_size().0,
                        "rows":view.grid_size().1,
                    }));
                    changed = true;
                }
                Command::Close => {
                    view.close();
                    return Ok(());
                }
            }
        }

        changed |= view.pump();
        let title = view.tab_title();
        if title != last_title {
            emit(&serde_json::json!({"type":"title", "value":title}));
            last_title = title;
        }
        if let Some(cwd) = view.working_directory() {
            if last_cwd.as_deref() != Some(cwd) {
                emit(&serde_json::json!({"type":"cwd", "value":cwd.to_string_lossy()}));
                last_cwd = Some(cwd.to_path_buf());
            }
        }
        if last_icon_check.elapsed() >= Duration::from_millis(400) {
            last_icon_check = std::time::Instant::now();
            let icon = view.tab_icon();
            if icon != last_icon {
                emit(&serde_json::json!({"type":"icon", "value":icon}));
                last_icon = icon.to_owned();
            }
        }
        let blink = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|time| (time.as_millis() / 500) % 2 == 0)
            .unwrap_or(false);
        let blink_changed = blink != last_blink;
        last_blink = blink;
        if changed || blink_changed {
            view.set_cursor_blink(blink);
            view.frame();
            emit_frame(&view)?;
        }
        thread::sleep(Duration::from_millis(16));
    }
}

fn command_channel() -> (Receiver<Command>, thread::JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let reader = thread::Builder::new()
        .name("otty-electron-stdin".to_owned())
        .spawn(move || {
            for line in io::stdin().lock().lines() {
                match line {
                    Ok(line) => match serde_json::from_str::<Command>(&line) {
                        Ok(command) => {
                            let closing = matches!(command, Command::Close);
                            if sender.send(command).is_err() || closing {
                                return;
                            }
                        }
                        Err(error) => emit(&serde_json::json!({
                            "type":"error", "message":error.to_string()
                        })),
                    },
                    Err(error) => {
                        emit(&serde_json::json!({"type":"error", "message":error.to_string()}));
                        return;
                    }
                }
            }
            let _ = sender.send(Command::Close);
        })
        .expect("stdin reader thread must start");
    (receiver, reader)
}

fn emit_frame(view: &TerminalView) -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = view.buffer_size();
    let pixels = base64::engine::general_purpose::STANDARD.encode(view.pixels());
    emit(&serde_json::json!({
        "type":"frame", "width":width, "height":height, "pixels":pixels
    }));
    Ok(())
}

fn emit(value: &serde_json::Value) {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if serde_json::to_writer(&mut output, value).is_ok() {
        let _ = output.write_all(b"\n");
        let _ = output.flush();
    }
}
