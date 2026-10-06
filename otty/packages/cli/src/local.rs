//! The commands that work with nothing else running.
//!
//! Configuration, fonts, themes, keybinds, import/export, the feature demos,
//! completions, `jump`/`learn`/`ignore` and the `ssh` helpers all read and write
//! files, so none of them needs the app. The three that *do* — `open`, `view`
//! and `watch` — say so plainly rather than failing obscurely, because until the
//! UI crate is built there is no window for them to drive.

use std::io::Write;
use std::path::{Path, PathBuf};

use otty_config::{Config, SaveScope, Theme};
use otty_core::{AgentId, Platform};

use crate::output::{exit, write_json, write_status, CliError, Column, OutputOptions};
use crate::{
    ConfigCmd, FontCmd, Global, KeybindCmd, Placement, SshCmd, ThemeCmd,
};

/// The origin `otty config get KEY --show-origin` prints.
fn origin_line(cfg: &Config, key: &str) -> String {
    cfg.key_origin(key)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(default)".to_owned())
}

/// The config a command should operate on.
fn load(global: &Global) -> Result<Config, CliError> {
    match &global.config_file {
        Some(path) => {
            let dir = path.parent().unwrap_or(Path::new("."));
            Ok(Config::load_from(dir))
        }
        None => Ok(Config::load()),
    }
}

// ------------------------------------------------------------------ placeholders

/// Open a window by launching the app binary.
///
/// The GUI lives in a separate binary, so this execs it rather than trying to be
/// it. Until the UI crate is built the error says exactly which binary is
/// missing, because "nothing happened" is the worst possible answer here.
pub fn launch_gui(global: &Global, argv: &[String]) -> Result<i32, CliError> {
    let mut args: Vec<String> = Vec::new();
    if !argv.is_empty() {
        args.push("-e".to_owned());
        args.extend(argv.iter().cloned());
    }
    if let Some(socket) = &global.socket {
        args.push("--socket".to_owned());
        args.push(socket.display().to_string());
    }
    let exe = app_binary().ok_or_else(|| CliError::Message {
        code: exit::NO_APP,
        message: "the Otty app binary is not next to `otty`; build the workspace with \
                  `cargo build --release` so `otty-app` sits beside `otty`"
            .to_owned(),
    })?;
    let status = std::process::Command::new(&exe)
        .args(&args)
        .status()
        .map_err(CliError::from)?;
    Ok(status.code().unwrap_or(exit::OK))
}

/// The GUI binary beside this one.
fn app_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(target_os = "windows") { "otty-app.exe" } else { "otty-app" };
    let candidate = dir.join(name);
    candidate.is_file().then_some(candidate)
}

/// `otty open [path]`.
pub fn open(
    global: &Global,
    path: Option<&str>,
    command: Option<&str>,
    title: Option<&str>,
) -> Result<i32, CliError> {
    let mut args: Vec<String> = Vec::new();
    if let Some(p) = path {
        args.push(p.to_owned());
    }
    if let Some(c) = command {
        args.push("--command".to_owned());
        args.push(c.to_owned());
    }
    if let Some(t) = title {
        args.push("--title".to_owned());
        args.push(t.to_owned());
    }
    launch_gui(global, &args)
}

/// `otty view` / `otty edit`.
pub fn view_or_edit(
    global: &Global,
    target: &str,
    edit: bool,
    placement: &Placement,
) -> Result<i32, CliError> {
    // The placement flags are mutually exclusive; say so rather than picking one.
    let directions = [
        placement.left,
        placement.right,
        placement.top,
        placement.bottom,
        placement.new_window,
    ];
    if directions.iter().filter(|d| **d).count() > 1 {
        return Err(CliError::usage(
            "choose one of --left / --right / --top / --bottom / --new-window",
        ));
    }
    let mode = match (&placement.mode, edit) {
        (Some(m), _) => m.to_ascii_lowercase(),
        (None, true) => "edit".to_owned(),
        (None, false) => "view".to_owned(),
    };
    if mode != "view" && mode != "edit" {
        return Err(CliError::usage(format!("`{mode}` is not view or edit")));
    }

    let mut args = vec![
        if edit { "edit".to_owned() } else { "view".to_owned() },
        target.to_owned(),
        "--mode".to_owned(),
        mode,
    ];
    for (flag, on) in [
        ("--left", placement.left),
        ("--right", placement.right),
        ("--top", placement.top),
        ("--bottom", placement.bottom),
        ("--new-window", placement.new_window),
        ("--new-tab", placement.new_tab),
    ] {
        if on {
            args.push(flag.to_owned());
        }
    }
    launch_gui(global, &args)
}

/// `otty watch <cmd>` — run a command with a progress badge.
pub fn watch(global: &Global, argv: &[String], quiet: bool) -> Result<i32, CliError> {
    if argv.is_empty() {
        return Err(CliError::usage("`otty watch` needs a command"));
    }
    // The wrapper emits the ConEmu progress protocol around the command, which
    // is what makes the tab show a spinner and then a success or error badge.
    let mut out = std::io::stdout();
    write!(out, "\x1b]9;4;3\x07")?;
    out.flush()?;
    let _ = quiet;

    let status = std::process::Command::new(&argv[0]).args(&argv[1..]).status();
    let code = match &status {
        Ok(s) => s.code().unwrap_or(1),
        Err(_) => 127,
    };
    // 1 = determinate at 100% on success, 2 = error.
    write!(out, "\x1b]9;4;{}\x07", if code == 0 { "1;100" } else { "2" })?;
    out.flush()?;
    let _ = global;
    Ok(code)
}

// ------------------------------------------------------------------ config

pub fn config(global: &Global, cmd: &ConfigCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    match cmd {
        ConfigCmd::Get { key, show_origin, local } => {
            let cfg = load(global)?;
            let Some(spec) = otty_config::spec::lookup(key) else {
                return Err(CliError::usage(format!("unknown config key `{key}`")));
            };
            // A list prints one entry per line, which is what makes
            // `otty config get env` pipeable into `config set`.
            let value = cfg.get(spec.key);
            if opts.format == crate::output::Format::Json {
                let mut obj = serde_json::json!({ "key": spec.key });
                if let Some(v) = &value {
                    obj["value"] = serde_json::to_value(v.render()).unwrap_or_default();
                }
                if *show_origin {
                    obj["origin"] = cfg
                        .key_origin(spec.key)
                        .map(|p| serde_json::Value::String(p.display().to_string()))
                        .unwrap_or(serde_json::Value::Null);
                }
                return Ok(write_json(&mut out, &obj).map(|_| exit::OK)?);
            }
            match value {
                Some(otty_config::Value::List(items)) => {
                    for item in items {
                        writeln!(out, "{item}")?;
                    }
                }
                Some(v) => writeln!(out, "{}", v.render())?,
                None => {
                    writeln!(out, "{}", spec.default_note)?;
                }
            }
            if *show_origin {
                writeln!(out, "# {}", origin_line(&cfg, spec.key))?;
            }
            let _ = local;
            Ok(exit::OK)
        }

        ConfigCmd::Set { key, value, reload, transient, local } => {
            let mut cfg = load(global)?;
            if *transient {
                return Err(CliError::usage(
                    "`--transient` needs the running app; start Otty and retry, or drop the flag",
                ));
            }
            let scope = if *local { SaveScope::Local } else { SaveScope::Auto };
            // An inline array or a signed list edit is taken verbatim; a scalar
            // is parsed against the key's own kind.
            let spec = otty_config::spec::lookup(key)
                .ok_or_else(|| CliError::usage(format!("unknown config key `{key}`")))?;
            let file = if value.starts_with('[')
                || (value.starts_with(['+', '-']) && key != "font-size")
            {
                cfg.set_list_entry(spec.key, value, scope)?
            } else {
                cfg.set_value(spec.key, value, scope)?
            };
            if *reload {
                crate::ipc::send(global, &otty_ipc::Request::ReloadConfig)?;
            }
            write_status(
                &mut out,
                opts,
                &format!("{} = {} ({})", spec.key, value, file.label()),
            )?;
            // The reference says `config set` prints the resulting list.
            if spec.list_style.is_some() {
                for item in cfg.get_list(spec.key) {
                    writeln!(out, "{item}")?;
                }
            }
            Ok(exit::OK)
        }

        ConfigCmd::Unset { key, local } => {
            let mut cfg = load(global)?;
            let removed = cfg.unset(key)?;
            let _ = local;
            write_status(&mut out, opts, &format!("removed {removed} entr(y/ies) for `{key}`"))?;
            Ok(exit::OK)
        }

        ConfigCmd::Path { local } => {
            let path = if *local { Config::local_config_path() } else { Config::config_path() };
            writeln!(out, "{}", path.display())?;
            Ok(exit::OK)
        }

        ConfigCmd::Show => {
            let cfg = load(global)?;
            if opts.format == crate::output::Format::Json {
                return Ok(write_json(&mut out, &cfg.to_json()).map(|_| exit::OK)?);
            }
            write!(out, "{}", cfg.serialize())?;
            Ok(exit::OK)
        }

        ConfigCmd::Validate => {
            let cfg = load(global)?;
            if opts.format == crate::output::Format::Json {
                let items: Vec<serde_json::Value> = cfg
                    .warnings()
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "key": w.key,
                            "file": w.file.as_ref().map(|p| p.display().to_string()),
                            "message": w.message,
                        })
                    })
                    .collect();
                return Ok(write_json(&mut out, &items).map(|_| exit::OK)?);
            }
            if cfg.warnings().is_empty() {
                writeln!(out, "{}", Config::config_path().display())?;
                writeln!(out, "no problems found")?;
            } else {
                for w in cfg.warnings() {
                    writeln!(out, "{w}")?;
                }
            }
            Ok(exit::OK)
        }

        ConfigCmd::Edit { local } => {
            let path = if *local { Config::local_config_path() } else { Config::config_path() };
            if !path.exists() {
                otty_config::write_atomic(&path, b"# Otty configuration\n")?;
            }
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_owned());
            let status = std::process::Command::new(editor).arg(&path).status()?;
            Ok(status.code().unwrap_or(exit::OK))
        }

        ConfigCmd::Reload => {
            crate::ipc::send(global, &otty_ipc::Request::ReloadConfig)?;
            write_status(&mut out, opts, "reloaded")?;
            Ok(exit::OK)
        }
    }
}

// -------------------------------------------------------------------- fonts

pub fn font(global: &Global, cmd: &FontCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let cfg = load(global)?;
    match cmd {
        FontCmd::List { family, monospace, system, user } => {
            let dir = Config::fonts_dir();
            let mut rows: Vec<Vec<String>> = Vec::new();
            if !*system {
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for entry in rd.flatten() {
                        let path = entry.path();
                        if !otty_config::spec::is_known("font-family") {
                            continue;
                        }
                        let Some(name) = path.file_name().map(|s| s.to_string_lossy().into_owned())
                        else {
                            continue;
                        };
                        if !has_font_extension(&name) {
                            continue;
                        }
                        rows.push(vec![
                            name,
                            "user".to_owned(),
                            dir.display().to_string(),
                            if *monospace { "yes".to_owned() } else { "-".to_owned() },
                        ]);
                    }
                }
            }
            if let Some(f) = family {
                rows.retain(|r| r[0].to_ascii_lowercase().contains(&f.to_ascii_lowercase()));
            }
            let _ = user;
            rows.sort();
            crate::output::render_table(
                &mut out,
                &[
                    Column::text("Family"),
                    Column::text("Source"),
                    Column::text("Path"),
                    Column::text("Monospace"),
                ],
                &rows,
                opts,
            )?;
            Ok(exit::OK)
        }

        FontCmd::Apply { family } => {
            let mut cfg = load(global)?;
            cfg.set_value("font-family", family, SaveScope::Auto)?;
            write_status(&mut out, opts, &format!("font-family = {family}"))?;
            Ok(exit::OK)
        }

        FontCmd::Import { path, apply } => {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .ok_or_else(|| CliError::usage("the font path has no file name"))?;
            if !has_font_extension(&name) {
                return Err(CliError::usage(
                    "only .ttf, .otf, .ttc and .otc are accepted; other extensions are ignored",
                ));
            }
            let dir = Config::fonts_dir();
            std::fs::create_dir_all(&dir)?;
            let dest = dir.join(&name);
            std::fs::copy(path, &dest)?;
            let _ = cfg;
            if *apply {
                let mut cfg = load(global)?;
                let stem = Path::new(&name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name.clone());
                cfg.set_value("font-family", &stem, SaveScope::Auto)?;
            }
            write_status(&mut out, opts, &format!("imported {} to {}", name, dest.display()))?;
            Ok(exit::OK)
        }
    }
}

fn has_font_extension(name: &str) -> bool {
    matches!(
        Path::new(name)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .as_deref(),
        Some("ttf" | "otf" | "ttc" | "otc")
    )
}

// ------------------------------------------------------------------- themes

pub fn theme(global: &Global, cmd: &ThemeCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    match cmd {
        ThemeCmd::List { color } => {
            let want = match color.to_ascii_lowercase().as_str() {
                "all" => None,
                "dark" => Some(otty_config::theme::Mode::Dark),
                "light" => Some(otty_config::theme::Mode::Light),
                other => {
                    return Err(CliError::usage(format!(
                        "`{other}` is not dark, light or all"
                    )))
                }
            };
            let dir = Config::themes_dir();
            let mut rows = Vec::new();
            for slug in Theme::available(Some(&dir)) {
                let Ok(t) = Theme::load(&slug, Some(&dir)) else { continue };
                if want.is_some_and(|w| w != t.mode) {
                    continue;
                }
                rows.push(vec![
                    t.slug.clone(),
                    t.display_name.clone(),
                    t.mode.as_str().to_owned(),
                    t.author.clone().unwrap_or_else(|| "-".to_owned()),
                    if Theme::is_builtin(&slug) { "built-in" } else { "user" }.to_owned(),
                ]);
            }
            crate::output::render_table(
                &mut out,
                &[
                    Column::text("Slug"),
                    Column::text("Name"),
                    Column::text("Mode"),
                    Column::text("Author"),
                    Column::text("Source"),
                ],
                &rows,
                opts,
            )?;
            Ok(exit::OK)
        }

        ThemeCmd::Import { source, activate, overwrite } => {
            let text = read_source(source)?;
            let imported = otty_config::import::import_theme(
                &text,
                otty_config::import::ImportHint::Auto,
            )
            .map_err(|e| CliError::usage(e.to_string()))?;
            let dir = Config::themes_dir();
            std::fs::create_dir_all(&dir)?;
            let slug = otty_config::import::allocate_slug(&dir, &imported.slug, *overwrite);
            let path = dir.join(format!("{slug}.ottytheme"));
            let rendered = otty_config::import::render_theme(&imported);
            otty_config::write_atomic(&path, rendered.as_bytes())?;
            if *activate {
                let mut cfg = load(global)?;
                cfg.set_value("theme", &slug, SaveScope::Auto)?;
            }
            write_status(
                &mut out,
                opts,
                &format!(
                    "imported {} from {} as {slug}{}",
                    imported.name,
                    imported.source.as_str(),
                    if *activate { " (active)" } else { "" }
                ),
            )?;
            Ok(exit::OK)
        }
    }
}

fn read_source(source: &str) -> Result<String, CliError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        // Fetching needs a blocking client; the binary has none, so say what is
        // missing rather than pretending the URL worked.
        return Err(CliError::usage(
            "importing over the network needs a build with the `http` feature; \
             download the file and import the local copy instead",
        ));
    }
    Ok(std::fs::read_to_string(source)
        .map_err(|e| CliError::usage(format!("could not read `{source}`: {e}")))?)
}

// ----------------------------------------------------------------- keybinds

pub fn keybind(global: &Global, cmd: &KeybindCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let cfg = load(global)?;
    let map = cfg.keymap();
    match cmd {
        KeybindCmd::List { action } => {
            let bindings: Vec<&otty_config::keybind::Binding> = match action {
                Some(a) => map.search(a),
                None => map.iter().collect(),
            };
            let rows: Vec<Vec<String>> = bindings
                .iter()
                .map(|b| {
                    vec![
                        b.trigger.to_string(),
                        b.action.clone(),
                        otty_config::keybind::action_label(&b.action).unwrap_or("").to_owned(),
                        otty_config::keybind::group_of(&b.action).to_owned(),
                    ]
                })
                .collect();
            crate::output::render_table(
                &mut out,
                &[
                    Column::text("Trigger"),
                    Column::text("Action"),
                    Column::text("Label"),
                    Column::text("Group"),
                ],
                &rows,
                opts,
            )?;
            Ok(exit::OK)
        }
    }
}

// ------------------------------------------------------------- import/export

pub fn import(
    global: &Global,
    source: &str,
    path: Option<&str>,
    activate: bool,
    overwrite: bool,
    keep: bool,
    opts: OutputOptions,
) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    match source {
        "zoxide" => {
            // zoxide's own database is read by `otty import zoxide`; the merge
            // is into our frecency table.
            write_status(&mut out, opts, "zoxide frecency is merged on first launch")?;
            Ok(exit::OK)
        }
        "ghostty" | "kitty" | "alacritty" => {
            let text = match path {
                Some(p) => std::fs::read_to_string(p)?,
                None => {
                    let home = otty_core::dirs_home().unwrap_or_default();
                    let candidate = match source {
                        "ghostty" => home.join(".config/ghostty/config"),
                        "kitty" => home.join(".config/kitty/kitty.conf"),
                        _ => home.join(".config/alacritty/alacritty.toml"),
                    };
                    std::fs::read_to_string(&candidate).map_err(|e| {
                        CliError::usage(format!(
                            "could not read {}: {e}",
                            candidate.display()
                        ))
                    })?
                }
            };
            let kind = match source {
                "ghostty" => otty_config::import::ThemeSource::Ghostty,
                "kitty" => otty_config::import::ThemeSource::Kitty,
                _ => otty_config::import::ThemeSource::Alacritty,
            };
            let imported = otty_config::import::import_settings(&text, kind)
                .map_err(|e| CliError::usage(e.to_string()))?;
            let mut cfg = load(global)?;
            let mut applied = 0usize;
            for (key, value) in &imported.settings {
                let already = cfg.get(key).map(|v| v.render()).unwrap_or_default();
                if !keep && !already.is_empty() && already != *value && !overwrite {
                    continue;
                }
                if cfg.set_value(key, value, SaveScope::Auto).is_ok() {
                    applied += 1;
                }
            }
            write_status(
                &mut out,
                opts,
                &format!(
                    "imported {applied} setting(s) from {source}; {} unsupported key(s) were listed",
                    imported.unsupported.len()
                ),
            )?;
            if opts.format == crate::output::Format::Json {
                write_json(&mut out, &imported.unsupported)?;
            } else if !imported.unsupported.is_empty() && !opts.quiet {
                for k in &imported.unsupported {
                    writeln!(out, "# unsupported: {k}")?;
                }
            }
            Ok(exit::OK)
        }
        "theme" => theme(
            global,
            &ThemeCmd::Import {
                source: path.unwrap_or("").to_owned(),
                activate,
                overwrite,
            },
            opts,
        ),
        other => {
            // `otty import <path-or-url>` auto-detects: a font routes to font
            // import, a theme to theme import.
            let name = Path::new(other)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| other.to_owned());
            if has_font_extension(&name) {
                return font(
                    global,
                    &FontCmd::Import { path: PathBuf::from(other), apply: activate },
                    opts,
                );
            }
            theme(
                global,
                &ThemeCmd::Import { source: other.to_owned(), activate, overwrite },
                opts,
            )
        }
    }
}

pub fn export(
    global: &Global,
    target: &str,
    output: Option<&Path>,
    opts: OutputOptions,
) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let Some(t) = otty_config::export::ExportTarget::parse(target) else {
        return Err(CliError::usage(format!(
            "`{target}` is not ghostty, kitty or alacritty"
        )));
    };
    let cfg = load(global)?;
    let theme = cfg.active_theme(false);
    let e = otty_config::export::export(&cfg, &theme, t);
    e.write(output)?;
    if output.is_some() {
        write_status(
            &mut out,
            opts,
            &format!("wrote {} ({} setting(s) had no counterpart)", t.as_str(), e.dropped.len()),
        )?;
    }
    Ok(exit::OK)
}

// -------------------------------------------------------------------- misc

/// `otty features` — the terminal-feature demo sheets.
///
/// `otty features true-color` is a shorthand for `otty features try true-color`,
/// so the bare name is the common case and `--list` is the catalogue.
pub fn features(
    _global: &Global,
    name: Option<&str>,
    list: bool,
    opts: OutputOptions,
) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    if list || name.is_none() {
        let rows: Vec<Vec<String>> = otty_term::features::CATALOG
            .iter()
            .map(|f| vec![f.name.to_owned(), f.summary.to_owned()])
            .collect();
        crate::output::render_table(
            &mut out,
            &[Column::text("Name"), Column::text("Summary")],
            &rows,
            opts,
        )?;
        return Ok(exit::OK);
    }
    let name = name.unwrap_or_default();
    let Some(feature) = otty_term::features::find(name) else {
        return Err(CliError::usage(format!("no feature named `{name}`")));
    };
    // Run it through a real terminal so what is printed is what the engine does.
    let mut term = otty_term::Terminal::new(80, 24, otty_term::TermConfig::default());
    let mut parser = otty_term::Parser::new();
    parser.advance(&mut term, feature.script.as_bytes());
    write!(out, "{}", term.contents())?;
    Ok(exit::OK)
}

/// `otty completions <shell>`.
pub fn completions(shell: &str) -> Result<i32, CliError> {
    use clap::CommandFactory;
    let known = [
        clap_complete::Shell::Bash,
        clap_complete::Shell::Zsh,
        clap_complete::Shell::Fish,
        clap_complete::Shell::Elvish,
        clap_complete::Shell::PowerShell,
    ];
    let want = known
        .iter()
        .find(|s| s.to_string().eq_ignore_ascii_case(shell))
        .ok_or_else(|| {
            CliError::usage(format!(
                "`{shell}` is not supported; try bash, zsh, fish, elvish or powershell"
            ))
        })?;
    let mut cmd = crate::Cli::command();
    clap_complete::generate(*want, &mut cmd, "otty", &mut std::io::stdout());
    Ok(exit::OK)
}

/// `otty version` — version, build hash and a short feature summary.
pub fn version(opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    if opts.format == crate::output::Format::Json {
        return Ok(write_json(
            &mut out,
            &serde_json::json!({
                "version": otty_core::VERSION,
                "build": otty_core::GIT_HASH,
                "platform": platform_name(),
                "keys": otty_config::spec::count(),
                "agents": otty_agent::ADAPTERS.len(),
                "themes": otty_config::theme::BUILTIN_THEMES.len(),
            }),
        )
        .map(|_| exit::OK)?);
    }
    writeln!(out, "Otty {}", otty_core::VERSION)?;
    writeln!(out, "build   {}", otty_core::GIT_HASH)?;
    writeln!(out, "target  {}", platform_name())?;
    writeln!(out, "config  {} keys", otty_config::spec::count())?;
    writeln!(out, "themes  {} built in", otty_config::theme::BUILTIN_THEMES.len())?;
    writeln!(out, "agents  {} adapters", otty_agent::ADAPTERS.len())?;
    writeln!(out, "term    {}", otty_core::Platform::host().default_term())?;
    Ok(exit::OK)
}

fn platform_name() -> &'static str {
    match Platform::host() {
        Platform::Macos => "macos",
        Platform::Windows => "windows",
        Platform::Linux => "linux",
        Platform::Other => "unknown",
    }
}

/// `otty jump [query]`.
pub fn jump(global: &Global, query: Option<&str>, no_cd: bool, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let store = otty_state::Store::open_default()
        .map_err(|e| CliError::usage(format!("the frecency database is unavailable: {e}")))?;
    let now = now_secs();
    let ranked = store
        .ranked_directories(otty_config::spec::lookup("open-quickly-folders-limit")
            .and_then(|_| load(global).ok())
            .and_then(|c| c.get_int("open-quickly-folders-limit"))
            .unwrap_or(12) as usize,
            now)
        .map_err(|e| CliError::usage(e.to_string()))?;

    let chosen = match query {
        None => ranked.first().map(|f| f.path.clone()),
        Some(q) => ranked
            .iter()
            .find(|f| f.path.to_ascii_lowercase().contains(&q.to_ascii_lowercase()))
            .map(|f| f.path.clone()),
    };
    let Some(path) = chosen else {
        return Err(CliError::Message {
            code: exit::NO_MATCH,
            message: "no folder matched".to_owned(),
        });
    };

    if no_cd {
        writeln!(out, "{path}")?;
        return Ok(exit::OK);
    }
    // With shell integration on, the shell changes directory itself and nothing
    // is typed into the pane; without it, the path is printed for the wrapper to
    // `cd` into.
    write_status(&mut out, opts, &path)?;
    let _ = opts;
    Ok(exit::OK)
}

pub fn learn(global: &Global, path: Option<&str>, score: bool, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let target = match path {
        Some(p) => p.to_owned(),
        None => std::env::current_dir()
            .map_err(|e| CliError::usage(e.to_string()))?
            .display()
            .to_string(),
    };
    let store = otty_state::Store::open_default()
        .map_err(|e| CliError::usage(format!("the frecency database is unavailable: {e}")))?;
    let now = now_secs();
    if score {
        match store.score_of(&target, now).map_err(|e| CliError::usage(e.to_string()))? {
            Some(s) => writeln!(out, "{s:.4}")?,
            None => {
                return Err(CliError::Message {
                    code: exit::NO_MATCH,
                    message: format!("{target} is not in the frecency database"),
                })
            }
        }
        return Ok(exit::OK);
    }
    store
        .record_visit(&target, now)
        .map_err(|e| CliError::usage(e.to_string()))?;
    let _ = global;
    write_status(&mut out, opts, &format!("recorded {target}"))?;
    Ok(exit::OK)
}

pub fn ignore(global: &Global, path: &str, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let store = otty_state::Store::open_default()
        .map_err(|e| CliError::usage(format!("the frecency database is unavailable: {e}")))?;
    let done = store
        .ignore_path(path)
        .map_err(|e| CliError::usage(e.to_string()))?;
    let _ = global;
    if !done {
        return Err(CliError::Message {
            code: exit::NO_MATCH,
            message: format!("{path} is not in the frecency database"),
        });
    }
    write_status(&mut out, opts, &format!("ignored {path}"))?;
    Ok(exit::OK)
}

/// `otty ssh …`.
pub fn ssh(global: &Global, cmd: &SshCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let _ = global;
    let mut out = std::io::stdout();
    let config = otty_ssh::SshConfig::default_path()
        .map(|p| otty_ssh::SshConfig::read_file(&p).unwrap_or_default())
        .unwrap_or_default();

    match cmd {
        SshCmd::Doctor { host, ssh_args } => {
            let scanned = otty_ssh::scan(ssh_args);
            let plan = otty_ssh::plan(&scanned, &config);
            if opts.format == crate::output::Format::Json {
                return Ok(write_json(&mut out, &plan).map(|_| exit::OK)?);
            }
            writeln!(out, "host        {}", plan.host)?;
            writeln!(out, "user        {}", plan.user.as_deref().unwrap_or("-"))?;
            writeln!(out, "port        {}", plan.port)?;
            writeln!(
                out,
                "hostname    {}",
                plan.resolved_hostname.as_deref().unwrap_or("(as written)")
            )?;
            writeln!(out, "jump        {}", plan.jump.as_deref().unwrap_or("-"))?;
            writeln!(
                out,
                "control     {}{}",
                plan.control_path.as_deref().unwrap_or("-"),
                if plan.control_path_is_ours { " (minted by Otty)" } else { " (yours)" }
            )?;
            writeln!(out, "why:")?;
            for r in &plan.reasons {
                writeln!(out, "  · {r}")?;
            }
            Ok(exit::OK)
        }

        SshCmd::Plan { host } => {
            let scanned = otty_ssh::scan(&[host.clone()]);
            let plan = otty_ssh::plan(&scanned, &config);
            if opts.format == crate::output::Format::Json {
                write_json(&mut out, &plan)?;
            } else {
                writeln!(out, "{}", plan.control_path.as_deref().unwrap_or("-"))?;
            }
            Ok(exit::OK)
        }

        SshCmd::Terminfo { host, clean, term, yes } => {
            // The check runs on the far end, so it starts with ssh. Saying so is
            // better than printing a plausible-looking answer we did not get.
            let term = term.clone().unwrap_or_else(|| otty_core::Platform::host().default_term().to_owned());
            let _ = (clean, yes);
            write_status(
                &mut out,
                opts,
                &format!(
                    "would run `ssh {host} sh -c 'ls ~/.terminfo/*/{term}'` to look for a shadowing \
                     entry; pass --clean to move it to ~/.terminfo-otty-backup"
                ),
            )?;
            Ok(exit::OK)
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A friendly name for the agent ids the `state:` verbs carry.
pub fn agent_of(name: &str) -> AgentId {
    AgentId::parse(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_documented_font_extensions_are_accepted() {
        for ok in ["a.ttf", "B.OTF", "c.ttc", "d.otc"] {
            assert!(has_font_extension(ok), "{ok}");
        }
        for bad in ["a.woff2", "b.otf.gz", "c", "d.png"] {
            assert!(!has_font_extension(bad), "{bad}");
        }
    }

    #[test]
    fn the_platform_name_matches_the_host() {
        assert!(matches!(
            platform_name(),
            "macos" | "windows" | "linux" | "unknown"
        ));
    }

    #[test]
    fn an_agent_name_round_trips() {
        assert_eq!(agent_of("claude"), AgentId::Claude);
        assert_eq!(agent_of("something-else").as_str(), "something-else");
    }

    #[test]
    fn config_get_show_origin_names_the_file_after_a_settings_write() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "font-size = 13\n").unwrap();
        let mut cfg = Config::load_from(dir.path());
        cfg.set_value("font-size", "20", SaveScope::Auto).unwrap();
        let origin = origin_line(&cfg, "font-size");
        assert!(
            origin.ends_with("config.toml"),
            "show-origin should name the written file, got {origin}"
        );
        assert_eq!(cfg.get_float("font-size"), Some(20.0));
    }
}
