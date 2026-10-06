//! The commands that drive a running Otty.
//!
//! Every one of these turns into an [`otty_ipc::Request`] and comes back as a
//! [`otty_ipc::Response`]. The handlers are deliberately thin: the interesting
//! policy — privilege gates, selector semantics, exit codes — lives in
//! `otty-ipc`, so there is one place to read it rather than one per subcommand.

use std::io::Write;

use otty_ipc::{
    BadgeKind, Client, ClientError, PaneRequest, Request, Response, Selector, SplitDirection,
    TabRequest, WindowRequest,
};
use otty_core::{expand_path, AgentId, AgentState};

use crate::output::{exit, write_json, write_status, CliError, Column, OutputOptions};
use crate::{
    Global, PaneCmd, TabCmd, WatchAgent, WindowCmd,
};

fn client(global: &Global) -> Client {
    Client::new(global.socket.as_deref()).with_timeout(std::time::Duration::from_millis(global.timeout))
}

/// Send one request and hand back the response, turning a refusal into the
/// documented exit code.
pub fn send(global: &Global, req: &Request) -> Result<Response, CliError> {
    let response = client(global).request(req)?;
    match &response {
        Response::Ok { .. } => Ok(response),
        Response::Err { code, message } => Err(CliError::Message {
            code: *code,
            message: message.clone(),
        }),
    }
}

/// Print a response's table or status.
fn emit(
    out: &mut impl Write,
    response: &Response,
    opts: OutputOptions,
) -> Result<(), CliError> {
    match response {
        Response::Ok { data, text } => {
            if let Some(data) = data {
                if opts.format == crate::output::Format::Json {
                    write_json(out, data)?;
                    return Ok(());
                }
                // The app answers with an array of objects; render it as a
                // table keyed by the union of their keys, in first-seen order.
                if let Some(rows) = data.as_array() {
                    let mut headers: Vec<String> = Vec::new();
                    for row in rows {
                        if let Some(obj) = row.as_object() {
                            for k in obj.keys() {
                                if !headers.contains(k) {
                                    headers.push(k.clone());
                                }
                            }
                        }
                    }
                    let columns: Vec<Column> = headers
                        .iter()
                        .map(|h| Column::text(Box::leak(h.replace('_', " ").into_boxed_str())))
                        .collect();
                    let cells: Vec<Vec<String>> = rows
                        .iter()
                        .map(|row| {
                            headers
                                .iter()
                                .map(|h| match row.get(h) {
                                    Some(serde_json::Value::String(s)) => s.clone(),
                                    Some(serde_json::Value::Null) | None => "-".to_owned(),
                                    Some(v) => v.to_string(),
                                })
                                .collect()
                        })
                        .collect();
                    crate::output::render_table(out, &columns, &cells, opts)?;
                    return Ok(());
                }
                writeln!(out, "{data}")?;
            } else if let Some(text) = text {
                if !opts.quiet {
                    writeln!(out, "{text}")?;
                }
            }
        }
        Response::Err { message, .. } => return Err(CliError::usage(message.clone())),
    }
    Ok(())
}

/// `otty ipc <command>` — send a raw control message.
pub fn raw(global: &Global, parts: &[String], opts: OutputOptions) -> Result<i32, CliError> {
    let joined = parts.join(" ");
    let payload = serde_json::from_str::<Request>(&joined)
        .map_err(|e| CliError::usage(format!("`{joined}` is not a request: {e}")))?;
    let mut out = std::io::stdout();
    let response = send(global, &payload)?;
    emit(&mut out, &response, opts)?;
    crate::ok()
}

// ------------------------------------------------------------------ windows

pub fn window_list(global: &Global, opts: OutputOptions) -> Result<i32, CliError> {
    window(global, &WindowCmd::List, opts)
}

pub fn window(global: &Global, cmd: &WindowCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let req = match cmd {
        WindowCmd::List => Request::Window(WindowRequest::List),
        WindowCmd::Show { selector } => Request::Window(WindowRequest::Show(
            selector.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        WindowCmd::New { profile, cwd, command, title } => Request::Window(WindowRequest::New {
            profile: profile.clone(),
            cwd: cwd.as_deref().map(expand_path),
            command: command.clone(),
            title: title.clone(),
        }),
        WindowCmd::Close { selector } => Request::Window(WindowRequest::Close(
            selector.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        WindowCmd::Focus { selector } => {
            Request::Window(WindowRequest::Focus(Selector::parse(selector)))
        }
        WindowCmd::Rename { selector, title } => Request::Window(WindowRequest::Rename {
            window: Selector::parse(selector),
            title: title.clone(),
        }),
        WindowCmd::SelectedTab { selector } => Request::Window(WindowRequest::SelectedTab(
            selector.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
    };
    let response = send(global, &req)?;
    emit(&mut out, &response, opts)?;
    crate::ok()
}

// ---------------------------------------------------------------------- tabs

pub fn tab_list(global: &Global, opts: OutputOptions) -> Result<i32, CliError> {
    tab(global, &TabCmd::List, opts)
}

pub fn tab(global: &Global, cmd: &TabCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let req = match cmd {
        TabCmd::List => Request::Tab(TabRequest::List),
        TabCmd::Show { selector } => Request::Tab(TabRequest::Show(
            selector.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        TabCmd::New { command, title, cwd, profile } => Request::Tab(TabRequest::New {
            window: None,
            profile: profile.clone(),
            cwd: cwd.as_deref().map(expand_path),
            command: command.clone(),
            title: title.clone(),
        }),
        TabCmd::Close { selector } => Request::Tab(TabRequest::Close(
            selector.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        TabCmd::Focus { selector } => Request::Tab(TabRequest::Focus(Selector::parse(selector))),
        TabCmd::Rename { selector, title } => Request::Tab(TabRequest::Rename {
            tab: Selector::parse(selector),
            title: title.clone(),
        }),
        TabCmd::Move { selector, to_window, position } => Request::Tab(TabRequest::Move {
            tab: Selector::parse(selector),
            to_window: to_window.as_deref().map(Selector::parse),
            position: *position,
        }),
        TabCmd::Badge { kind, tab } => {
            // The badge kinds are an enum on both sides; a wrong name is a usage
            // error rather than a silently ignored badge.
            let kind = BadgeKind::parse(kind)
                .ok_or_else(|| CliError::usage(format!("`{kind}` is not a badge kind")))?;
            Request::Tab(TabRequest::Badge {
                tab: tab.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
                kind,
            })
        }
    };
    let response = send(global, &req)?;
    emit(&mut out, &response, opts)?;
    crate::ok()
}

// --------------------------------------------------------------------- panes

pub fn pane_list(global: &Global, opts: OutputOptions) -> Result<i32, CliError> {
    pane(global, &PaneCmd::List, opts)
}

pub fn pane(global: &Global, cmd: &PaneCmd, opts: OutputOptions) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let sel = |s: &Option<String>| s.as_deref().map(Selector::parse);
    let req = match cmd {
        PaneCmd::List => Request::Pane(PaneRequest::List),
        PaneCmd::Show { pane } => Request::Pane(PaneRequest::Show(
            pane.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        PaneCmd::Split { direction, size, command, cwd, pane, no_focus } => {
            let direction = match direction.to_ascii_lowercase().as_str() {
                "right" => SplitDirection::Right,
                "left" => SplitDirection::Left,
                "up" => SplitDirection::Up,
                "down" => SplitDirection::Down,
                other => {
                    return Err(CliError::usage(format!(
                        "`{other}` is not right, left, up or down"
                    )))
                }
            };
            if let Some(s) = size {
                if !(10..=90).contains(s) {
                    return Err(CliError::usage("--size is the new pane's share, 10-90"));
                }
            }
            let _ = no_focus;
            Request::Pane(PaneRequest::Split {
                pane: sel(pane),
                direction,
                size: *size,
                command: command.clone(),
                cwd: cwd.as_deref().map(expand_path),
            })
        }
        PaneCmd::Close { pane } => Request::Pane(PaneRequest::Close(
            pane.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        PaneCmd::Focus { selector } => Request::Pane(PaneRequest::Focus(Selector::parse(selector))),
        PaneCmd::Zoom { pane } => Request::Pane(PaneRequest::Zoom(
            pane.as_deref().map(Selector::parse).unwrap_or(Selector::Index(1)),
        )),
        PaneCmd::Resize { pane, right, down, left, up } => {
            if right.is_none() && down.is_none() && left.is_none() && up.is_none() {
                return Err(CliError::usage(
                    "`otty pane resize` needs at least one of --right/--down/--left/--up",
                ));
            }
            Request::Pane(PaneRequest::Resize {
                pane: sel(pane),
                right: *right,
                down: *down,
                left: *left,
                up: *up,
            })
        }
        PaneCmd::SendKeys { pane, keys } => {
            if keys.is_empty() {
                return Err(CliError::usage("`send-keys` needs something to send"));
            }
            Request::Pane(PaneRequest::SendKeys { pane: sel(pane), keys: keys.clone() })
        }
        PaneCmd::SendText { pane, text } => Request::Pane(PaneRequest::SendText {
            pane: sel(pane),
            text: text.clone(),
        }),
        PaneCmd::Run { pane, argv } => Request::Pane(PaneRequest::Run {
            pane: sel(pane),
            command: shell_join(argv),
        }),
        PaneCmd::Exec { pane, argv } => Request::Pane(PaneRequest::Exec {
            pane: sel(pane),
            command: shell_join(argv),
        }),
        PaneCmd::Capture { pane, lines, escapes } => Request::Pane(PaneRequest::Capture {
            pane: sel(pane),
            lines: *lines,
            escapes: *escapes,
        }),
        PaneCmd::Tty { pane } => Request::Pane(PaneRequest::Tty { pane: sel(pane) }),
        PaneCmd::Processes { pane } => {
            Request::Pane(PaneRequest::Processes { pane: sel(pane) })
        }
        PaneCmd::Busy { pane } => Request::Pane(PaneRequest::Busy { pane: sel(pane) }),
        PaneCmd::Dimensions { pane } => {
            Request::Pane(PaneRequest::Dimensions { pane: sel(pane) })
        }
        PaneCmd::Wait { tab, pane, timeout_secs } => {
            let _ = tab;
            Request::Pane(PaneRequest::Wait {
                pane: sel(pane),
                timeout_ms: timeout_secs.map(|s| s.saturating_mul(1000)),
            })
        }
    };
    let response = send(global, &req)?;
    // `otty pane busy` answers with an exit code as well as a line, because the
    // question it answers is yes/no: 0 busy, 4 idle. A script that only wants the
    // answer never has to parse anything.
    if let PaneCmd::Busy { .. } = cmd {
        emit(&mut out, &response, opts)?;
        let busy = matches!(&response, Response::Ok { data: Some(d), .. }
            if d.get("busy").and_then(|b| b.as_bool()).unwrap_or(false));
        return Ok(if busy { exit::OK } else { exit::NO_MATCH });
    }
    emit(&mut out, &response, opts)?;
    crate::ok()
}

/// Re-join a trailing argv into one command line, quoting what needs it.
///
/// `otty pane run -- cargo test --all` must send `cargo test --all` as one
/// string, because the pane's shell is what parses it.
pub fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.is_empty()
                || a.chars().any(|c| c.is_whitespace() || "\"'\\$`*?[]{}()|&;<>#~!".contains(c))
            {
                format!("'{}'", a.replace('\'', "'\\''"))
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// -------------------------------------------------------------- agent verbs

/// `otty state:<agent> key=value …` — what the bundled hooks call.
///
/// The report is fire-and-forget: a missing app must not break the agent's own
/// session, so a connection failure is silent and the exit code stays 0 unless
/// the *arguments* were wrong.
pub fn report_state(
    global: &Global,
    agent: &str,
    args: &[String],
    opts: OutputOptions,
) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let req = otty_ipc::agent_state_from_args(agent, args).map_err(CliError::usage)?;
    match send(global, &req) {
        Ok(response) => {
            emit(&mut out, &response, opts)?;
            crate::ok()
        }
        Err(CliError::Message { code, message }) if code == exit::NO_APP => {
            // Deliberately silent: the hook runs inside the agent's own process
            // tree and must never be the reason a turn fails.
            let _ = message;
            crate::ok()
        }
        Err(e) => Err(e),
    }
}

/// `otty watch:<agent> <id>` — block until the session reaches idle.
pub fn watch_agent(
    global: &Global,
    agent: &str,
    watch: &WatchAgent,
    opts: OutputOptions,
) -> Result<i32, CliError> {
    let mut out = std::io::stdout();
    let id = otty_core::AgentId::parse(agent);
    let adapter = otty_agent::adapter(&id);

    // Without an integration installed the session can never report anything, so
    // the wait would never end. Say that now rather than after the timeout.
    if adapter.is_none() && otty_agent::is_custom(&id) && agent != "custom" {
        return Err(CliError::Message {
            code: exit::NO_INTEGRATION,
            message: format!("{agent} has no Otty integration installed"),
        });
    }

    let client = client(global);
    let interval = std::time::Duration::from_millis(watch.interval_ms.max(1));
    let deadline = (watch.timeout_secs > 0)
        .then(|| std::time::Duration::from_secs(watch.timeout_secs));
    let unknown_deadline = (watch.unknown_timeout_secs > 0)
        .then(|| std::time::Duration::from_secs(watch.unknown_timeout_secs));

    let started = std::time::Instant::now();
    let mut last = None::<AgentState>;
    loop {
        let req = Request::Status;
        match client.request(&req) {
            Ok(Response::Ok { data, .. }) => {
                let state = data
                    .as_ref()
                    .and_then(|d| d.get("agents"))
                    .and_then(|a| a.get(watch.id.as_str()))
                    .and_then(|s| s.as_str())
                    .and_then(AgentState::parse);
                if let Some(state) = state {
                    if watch.verbose && last != Some(state) {
                        eprintln!("{}: {}", watch.id, state.as_str());
                    }
                    last = Some(state);
                    if state == AgentState::Idle {
                        write_status(&mut out, opts, "idle")?;
                        return crate::ok();
                    }
                } else if unknown_deadline.is_some_and(|d| started.elapsed() >= d) {
                    return Err(CliError::Message {
                        code: exit::NO_STATE,
                        message: format!("no state was ever reported for {}", watch.id),
                    });
                }
            }
            Ok(_) => {}
            Err(ClientError::NoApp) => {
                return Err(CliError::Message {
                    code: exit::NO_APP,
                    message: "Otty is not running".to_owned(),
                })
            }
            Err(e) => return Err(e.into()),
        }
        if deadline.is_some_and(|d| started.elapsed() >= d) {
            return Err(CliError::Message {
                code: exit::TIMEOUT,
                message: format!("{} never became idle", watch.id),
            });
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trailing_argv_is_rejoined_with_quoting_only_where_needed() {
        assert_eq!(shell_join(&["cargo".into(), "test".into()]), "cargo test");
        assert_eq!(
            shell_join(&["cargo".into(), "test".into(), "--all".into()]),
            "cargo test --all"
        );
        // A flag is left alone; a value with a space is quoted.
        assert_eq!(
            shell_join(&["grep".into(), "two words".into(), "f".into()]),
            "grep 'two words' f"
        );
        assert_eq!(
            shell_join(&["echo".into(), "a'b".into()]),
            "echo 'a'\\''b'"
        );
    }

    #[test]
    fn a_split_size_outside_the_documented_range_is_a_usage_error() {
        let global = Global {
            format: None,
            json: false,
            no_headers: false,
            quiet: false,
            socket: None,
            config_file: None,
            timeout: 3000,
            yes: false,
        };
        let cmd = PaneCmd::Split {
            direction: "right".into(),
            size: Some(99),
            command: None,
            cwd: None,
            pane: None,
            no_focus: false,
        };
        let e = pane(&global, &cmd, OutputOptions::default()).unwrap_err();
        assert_eq!(e.code(), exit::USAGE);
        assert!(e.message().contains("10-90"), "{}", e.message());
    }

    #[test]
    fn an_unknown_badge_kind_is_refused() {
        let global = Global {
            format: None,
            json: false,
            no_headers: false,
            quiet: false,
            socket: None,
            config_file: None,
            timeout: 3000,
            yes: false,
        };
        let cmd = TabCmd::Badge { kind: "purple".into(), tab: None };
        let e = tab(&global, &cmd, OutputOptions::default()).unwrap_err();
        assert_eq!(e.code(), exit::USAGE);
    }

    #[test]
    fn an_unknown_split_direction_is_refused() {
        let global = Global {
            format: None,
            json: false,
            no_headers: false,
            quiet: false,
            socket: None,
            config_file: None,
            timeout: 3000,
            yes: false,
        };
        let cmd = PaneCmd::Split {
            direction: "sideways".into(),
            size: None,
            command: None,
            cwd: None,
            pane: None,
            no_focus: false,
        };
        let e = pane(&global, &cmd, OutputOptions::default()).unwrap_err();
        assert!(e.message().contains("right, left, up or down"));
    }
}
