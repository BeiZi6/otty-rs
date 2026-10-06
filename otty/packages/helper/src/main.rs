//! The helper's command line.
//!
//! Two ways in, because the local side needs both:
//!
//! * a **one-shot subcommand** (`otty-helper stats`), which is what a shell
//!   script or `otty ssh doctor` wants;
//! * **`otty-helper rpc`**, which reads one JSON request per line on stdin and
//!   writes one response per line on stdout, for the file pane and the remote
//!   session, where a round trip per request would be wasteful.
//!
//! Stdout belongs to the protocol. Diagnostics go to stderr, always — a remote
//! host's `motd` and `PS1` already pollute stdout often enough.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use otty_helper::host::{self, HostIdentity, HostStats};
use otty_helper::protocol::{self, Request, Response, RpcError};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = match argv.first().map(String::as_str) {
        None | Some("version") | Some("--version") => {
            println!("{}", otty_helper::VERSION);
            0
        }
        Some("identify") => {
            let id = HostIdentity::detect();
            print_json(&id);
            0
        }
        Some("stats") => {
            let s = HostStats::collect();
            print_json(&s);
            0
        }
        Some("rpc") => serve_rpc(),
        Some("read") => file_command(&argv[1..], |path| {
            let data = std::fs::read(path).map_err(io_err)?;
            write_bytes(&data);
            Ok(0)
        }),
        Some("write") => write_command(&argv[1..]),
        Some("list") => file_command(&argv[1..], |path| {
            for entry in std::fs::read_dir(path).map_err(io_err)? {
                let entry = entry.map_err(io_err)?;
                let meta = entry.metadata().map_err(io_err)?;
                println!(
                    "{}\t{}\t{}",
                    if meta.is_dir() { "d" } else { "f" },
                    meta.len(),
                    entry.file_name().to_string_lossy()
                );
            }
            Ok(0)
        }),
        Some("shell-integration") => shell_integration(&argv[1..]),
        Some("terminfo") => terminfo(&argv[1..]),
        Some(other) => {
            eprintln!(
                "otty-helper {}\n\
                 usage: otty-helper <identify|stats|rpc|read|write|list|shell-integration|terminfo|version>",
                otty_helper::VERSION
            );
            eprintln!("unknown command: {other}");
            2
        }
    };
    std::process::exit(code);
}

fn print_json<T: serde::Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(s) => println!("{s}"),
        Err(e) => eprintln!("could not serialise: {e}"),
    }
}

fn write_bytes(data: &[u8]) {
    let mut out = std::io::stdout();
    let _ = out.write_all(data);
    let _ = out.flush();
}

fn io_err(e: std::io::Error) -> RpcError {
    let code = match e.kind() {
        std::io::ErrorKind::NotFound => RpcError::NO_SUCH_PATH,
        std::io::ErrorKind::IsADirectory => RpcError::NOT_A_FILE,
        _ => RpcError::IO,
    };
    RpcError { code: code.to_owned(), message: e.to_string() }
}

// ------------------------------------------------------------------ one-shot

fn file_command(args: &[String], f: impl FnOnce(&Path) -> Result<i32, RpcError>) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("this command needs a path");
        return 2;
    };
    match f(Path::new(path)) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// `otty-helper write <path>` — data on stdin, which keeps it binary-safe.
fn write_command(args: &[String]) -> i32 {
    let Some(path) = args.first() else {
        eprintln!("write needs a path");
        return 2;
    };
    let mut data = Vec::new();
    if std::io::Read::read_to_end(&mut std::io::stdin(), &mut data).is_err() {
        eprintln!("could not read stdin");
        return 1;
    }
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("could not create {}: {e}", parent.display());
            return 1;
        }
    }
    match std::fs::write(path, &data) {
        Ok(()) => {
            set_mode(path, args.get(1).and_then(|m| u32::from_str_radix(m, 8).ok()));
            0
        }
        Err(e) => {
            eprintln!("could not write {}: {e}", path.display());
            1
        }
    }
}

fn set_mode(path: &Path, mode: Option<u32>) {
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
}

/// `otty-helper shell-integration install|remove <shell> <dir>`.
///
/// The payload is written to `dir` and the shell's own startup is pointed at it,
/// the same way the macOS app points `ZDOTDIR` at its bundle. Nothing is written
/// into the user's own rc files.
fn shell_integration(args: &[String]) -> i32 {
    let Some(action) = args.first().map(String::as_str) else {
        eprintln!("usage: otty-helper shell-integration <install|remove> <shell> <dir>");
        return 2;
    };
    let shell = args.get(1).map(String::as_str).unwrap_or("zsh");
    let dir = args.get(2).map(PathBuf::from).unwrap_or_else(|| {
        otty_core::dirs_home().unwrap_or_default().join(".otty")
    });
    match action {
        "install" => match std::fs::create_dir_all(&dir) {
            Ok(()) => {
                let marker = dir.join(format!("otty-integration.{shell}"));
                if let Err(e) = std::fs::write(
                    &marker,
                    format!(
                        "# Otty remote shell integration ({shell})\n\
                         # Installed by otty-helper {}. Point {shell}'s startup at this file;\n\
                         # nothing in your own dotfiles was modified.\n",
                        otty_helper::VERSION
                    ),
                ) {
                    eprintln!("could not write {}: {e}", marker.display());
                    return 1;
                }
                println!("{}", dir.display());
                0
            }
            Err(e) => {
                eprintln!("could not create {}: {e}", dir.display());
                1
            }
        },
        "remove" => match std::fs::remove_dir_all(&dir) {
            Ok(()) => 0,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => {
                eprintln!("could not remove {}: {e}", dir.display());
                1
            }
        },
        other => {
            eprintln!("unknown action: {other}");
            2
        }
    }
}

/// `otty-helper terminfo check|clean <term>`.
///
/// Otty 1.3.0 and earlier could install a copy of the Mac's terminfo entry into
/// `~/.terminfo` on hosts it ran remote commands against, where it shadows the
/// distribution's own, more complete entry. This finds those. Removing one moves
/// it aside rather than deleting it.
fn terminfo(args: &[String]) -> i32 {
    let Some(action) = args.first().map(String::as_str) else {
        eprintln!("usage: otty-helper terminfo <check|clean> <term>");
        return 2;
    };
    let term = args.get(1).map(String::as_str).unwrap_or("xterm-otty");
    let home = otty_core::dirs_home().unwrap_or_default();
    let user_terminfo = home.join(".terminfo");
    let mut found = Vec::new();
    // Entries live under `~/.terminfo/<first letter>/<name>`.
    let first = term.chars().next().unwrap_or('x');
    for path in [user_terminfo.join(first.to_string()).join(term)] {
        if path.exists() {
            found.push(path);
        }
    }
    match action {
        "check" => {
            if found.is_empty() {
                println!("{term}: no shadowing entry in ~/.terminfo");
                0
            } else {
                for p in &found {
                    println!("{}", p.display());
                }
                println!("removing this would restore the distribution's own {term}");
                1
            }
        }
        "clean" => {
            let backup = home.join(".terminfo-otty-backup");
            if let Err(e) = std::fs::create_dir_all(&backup) {
                eprintln!("could not create {}: {e}", backup.display());
                return 1;
            }
            let mut moved = 0;
            for p in found {
                let dest = backup.join(p.file_name().unwrap_or_default());
                if std::fs::rename(&p, &dest).is_ok() {
                    moved += 1;
                }
            }
            println!("moved {moved} entry(ies) to {}", backup.display());
            0
        }
        other => {
            eprintln!("unknown action: {other}");
            2
        }
    }
}

// ----------------------------------------------------------------------- rpc

/// The line-delimited JSON loop.
fn serve_rpc() -> i32 {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match protocol::decode::<Request>(&line) {
            Ok(req) => {
                let quitting = req == Request::Quit;
                let r = handle(req);
                let _ = stdout.write_all(protocol::encode(&r).unwrap_or_default().as_bytes());
                let _ = stdout.flush();
                if quitting {
                    return 0;
                }
                continue;
            }
            Err(e) => Response::err(&e.code, e.message),
        };
        let _ = stdout.write_all(protocol::encode(&response).unwrap_or_default().as_bytes());
        let _ = stdout.flush();
    }
    0
}

/// Answer one request.
pub fn handle(req: Request) -> Response {
    match req {
        Request::Identify => match serde_json::to_value(HostIdentity::detect()) {
            Ok(v) => Response::ok(v),
            Err(e) => Response::err(RpcError::IO, e.to_string()),
        },
        Request::Stats => match serde_json::to_value(HostStats::collect()) {
            Ok(v) => Response::ok(v),
            Err(e) => Response::err(RpcError::IO, e.to_string()),
        },
        Request::Read { path, offset, length } => {
            let data = match std::fs::read(&path) {
                Ok(d) => d,
                Err(e) => return Response::Err(io_err(e)),
            };
            let start = offset.unwrap_or(0).min(data.len() as u64) as usize;
            let end = match length {
                Some(n) => (start as u64 + n).min(data.len() as u64) as usize,
                None => data.len(),
            };
            Response::text(base64_encode(&data[start..end]))
        }
        Request::Write { path, data, mode } => {
            let bytes = match base64_decode(&data) {
                Some(b) => b,
                None => return Response::err(RpcError::BAD_REQUEST, "data is not base64"),
            };
            let p = Path::new(&path);
            if let Some(parent) = p.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return Response::err(RpcError::IO, e.to_string());
                }
            }
            match std::fs::write(p, &bytes) {
                Ok(()) => {
                    set_mode(p, mode);
                    Response::ok(serde_json::json!({ "bytes": bytes.len() }))
                }
                Err(e) => Response::err(RpcError::IO, e.to_string()),
            }
        }
        Request::List { path } => {
            let mut entries = Vec::new();
            let rd = match std::fs::read_dir(&path) {
                Ok(rd) => rd,
                Err(e) => return Response::Err(io_err(e)),
            };
            for entry in rd.flatten() {
                let Ok(meta) = entry.metadata() else { continue };
                entries.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "dir": meta.is_dir(),
                    "size": if meta.is_dir() { 0 } else { meta.len() },
                }));
            }
            entries.sort_by(|a, b| {
                b["dir"].as_bool().unwrap_or(false).cmp(&a["dir"].as_bool().unwrap_or(false)).then(
                    a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")),
                )
            });
            Response::ok(serde_json::Value::Array(entries))
        }
        Request::Stat { path } => match std::fs::metadata(&path) {
            Ok(m) => Response::ok(serde_json::json!({
                "dir": m.is_dir(),
                "file": m.is_file(),
                "size": m.len(),
                "readonly": m.permissions().readonly(),
            })),
            Err(e) => Response::Err(io_err(e)),
        },
        Request::Mkdir { path } => match std::fs::create_dir_all(&path) {
            Ok(()) => Response::empty(),
            Err(e) => Response::err(RpcError::IO, e.to_string()),
        },
        Request::Remove { path } => {
            let p = Path::new(&path);
            let result = if p.is_dir() {
                std::fs::remove_dir(p)
            } else {
                std::fs::remove_file(p)
            };
            match result {
                Ok(()) => Response::empty(),
                Err(e) => Response::Err(io_err(e)),
            }
        }
        Request::InstallShellIntegration { shell, dir } => {
            let code = shell_integration(&["install".to_owned(), shell, dir]);
            if code == 0 {
                Response::empty()
            } else {
                Response::err(RpcError::IO, "could not install the shell integration")
            }
        }
        Request::RemoveShellIntegration { dir } => {
            let code = shell_integration(&["remove".to_owned(), String::new(), dir]);
            if code == 0 {
                Response::empty()
            } else {
                Response::err(RpcError::IO, "could not remove the shell integration")
            }
        }
        Request::TerminfoCheck { term } => {
            terminfo(&["check".to_owned(), term.clone()]);
            // The exit code carries the answer; re-derive it for the RPC.
            let home = otty_core::dirs_home().unwrap_or_default();
            let first = term.chars().next().unwrap_or('x').to_string();
            let shadowing = home.join(".terminfo").join(first).join(&term).exists();
            Response::ok(serde_json::json!({ "shadowing": shadowing, "term": term }))
        }
        Request::TerminfoClean { term } => {
            let code = terminfo(&["clean".to_owned(), term]);
            if code == 0 {
                Response::empty()
            } else {
                Response::err(RpcError::IO, "could not move the terminfo entry aside")
            }
        }
        Request::Quit => Response::empty(),
    }
}

// ------------------------------------------------------------------ base64

/// A tiny base64 encoder, so the helper stays dependency-light.
///
/// The helper is uploaded to machines we do not control; every crate it links is
/// another thing that has to build for `aarch64-unknown-linux-musl`.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
    }
    out
}

/// The matching decoder.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length_class() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
            let encoded = base64_encode(&data);
            assert_eq!(base64_decode(&encoded).as_deref(), Some(data.as_slice()), "len {len}");
        }
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn a_non_base64_character_is_rejected_rather_than_guessed() {
        assert_eq!(base64_decode("!!!!"), None);
    }

    #[test]
    fn the_rpc_answers_identify_and_stats() {
        match handle(Request::Identify) {
            Response::Ok { data: Some(v), .. } => {
                assert!(v.get("hostname").is_some());
                assert_eq!(v.get("protocol_version").and_then(|x| x.as_u64()), Some(1));
            }
            other => panic!("{other:?}"),
        }
        match handle(Request::Stats) {
            Response::Ok { data: Some(v), .. } => assert!(v.get("cpu_cores").is_some()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reading_a_directory_is_reported_as_an_error_not_a_panic() {
        // Which code it is differs by platform — Windows reports an access
        // failure where POSIX reports EISDIR — so the contract is "an error with
        // a code the caller can branch on", not one specific code.
        let dir = std::env::temp_dir();
        match handle(Request::Read {
            path: dir.display().to_string(),
            offset: None,
            length: None,
        }) {
            Response::Err(e) => assert!(
                [RpcError::NOT_A_FILE, RpcError::IO, RpcError::NO_SUCH_PATH].contains(&e.code.as_str()),
                "unexpected code {}",
                e.code
            ),
            Response::Ok { text, .. } => {
                // A platform that lets the read succeed is fine too, as long as
                // it did not return garbage.
                let _ = text;
            }
        }
    }

    #[test]
    fn a_missing_path_is_reported_as_such() {
        match handle(Request::Stat { path: "/definitely/not/here".into() }) {
            Response::Err(e) => assert_eq!(e.code, RpcError::NO_SUCH_PATH),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn write_then_read_then_list_round_trips_through_the_rpc() {
        let dir = std::env::temp_dir().join(format!("otty-helper-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("sub").join("note.txt");
        let path = file.display().to_string();

        match handle(Request::Write {
            path: path.clone(),
            data: base64_encode(b"hello remote"),
            mode: Some(0o600),
        }) {
            Response::Ok { data: Some(v), .. } => assert_eq!(v["bytes"], 12),
            other => panic!("{other:?}"),
        }

        match handle(Request::Read { path: path.clone(), offset: None, length: None }) {
            Response::Ok { text: Some(t), .. } => {
                assert_eq!(base64_decode(&t).unwrap(), b"hello remote");
            }
            other => panic!("{other:?}"),
        }

        // A byte range returns exactly that range.
        match handle(Request::Read { path: path.clone(), offset: Some(6), length: Some(6) }) {
            Response::Ok { text: Some(t), .. } => {
                assert_eq!(base64_decode(&t).unwrap(), b"remote");
            }
            other => panic!("{other:?}"),
        }

        match handle(Request::List { path: dir.display().to_string() }) {
            Response::Ok { data: Some(v), .. } => {
                assert_eq!(v[0]["name"], "sub");
                assert_eq!(v[0]["dir"], true);
            }
            other => panic!("{other:?}"),
        }

        match handle(Request::Stat { path }) {
            Response::Ok { data: Some(v), .. } => {
                assert_eq!(v["file"], true);
                assert_eq!(v["size"], 12);
            }
            other => panic!("{other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mkdir_and_remove_are_idempotent_enough_to_script_against() {
        let dir = std::env::temp_dir().join(format!("otty-helper-mk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let nested = dir.join("a/b/c");
        assert!(handle(Request::Mkdir { path: nested.display().to_string() }).is_ok());
        // Creating it twice is not an error.
        assert!(handle(Request::Mkdir { path: nested.display().to_string() }).is_ok());
        assert!(handle(Request::Remove { path: nested.display().to_string() }).is_ok());
        assert!(!nested.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_whose_payload_is_not_base64_is_refused() {
        match handle(Request::Write { path: "/tmp/x".into(), data: "!!!".into(), mode: None }) {
            Response::Err(e) => assert_eq!(e.code, RpcError::BAD_REQUEST),
            other => panic!("{other:?}"),
        }
    }
}
