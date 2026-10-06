//! SSH integration: what Otty wraps, and what it must never get wrong.
//!
//! Two jobs.
//!
//! **Scan an `ssh` command line.** Otty's `ssh` is a wrapper: it has to learn the
//! target host, user and port *before* ssh runs, so it can upload the remote
//! helper, forward the shell integration, and label the tab. The tempting
//! shortcut — "the first argument that does not start with `-` is the host" —
//! is wrong in ways that matter. `ssh -o User=git -p 2222 host` has two
//! non-option tokens; `scp`-style bare paths are not hosts; and, as the product's
//! own comment records, a prefix match of `-c` against the flag list also hits
//! `-C`, which is why the scanner here is a real table lookup rather than a
//! `starts_with`.
//!
//! **Read `~/.ssh/config`.** The wrapper must agree with ssh about which host a
//! name resolves to, because it uses that answer to decide whether a
//! `ControlPath` is honoured or minted, and to describe itself in
//! `otty ssh doctor`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What an `ssh` command line resolved to.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SshArgvScan {
    /// The option-style flags seen, without their values.
    pub flags: Vec<String>,
    /// `-o`/`-O` style settings, key and value.
    pub options: Vec<(String, String)>,
    /// The destination exactly as it was written — `user@host` or `host`.
    pub destination: Option<String>,
    pub user: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    /// `-J` / `-o ProxyJump`.
    pub jump: Option<String>,
    /// Arguments that follow the destination and are therefore the *remote
    /// command*, which must not be reinterpreted as local options.
    pub remote_command: Vec<String>,
    /// True when `-N` or `-W` was given, i.e. no shell will be started.
    pub no_shell: bool,
    /// The `ssh` executable the wrapper was invoked as, if it was not `ssh`.
    pub invoked_as: Option<String>,
}

/// Options that consume the following argument.
///
/// Ordering does not matter; the lookup is exact, which is the point. `-c` takes
/// a cipher and `-C` does not take anything, and a prefix match cannot tell them
/// apart.
const VALUE_TAKING: &[&str] = &[
    "-B", "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o",
    "-p", "-Q", "-R", "-S", "-W", "-w",
];

/// Options that stand alone.
const STANDALONE: &[&str] = &[
    "-4", "-6", "-A", "-a", "-C", "-f", "-G", "-g", "-K", "-k", "-M", "-N", "-n", "-q", "-s",
    "-T", "-t", "-V", "-v", "-X", "-x", "-Y", "-y",
];

/// Long options that take a value, spelled `--name=value` or `--name value`.
const LONG_WITH_VALUE: &[&str] = &[
    "--ciphers", "--config-file", "--escape-char", "--identity-file", "--identity-agent",
    "--jump-host", "--local-forward", "--login-name", "--mac", "--option", "--port",
    "--remote-forward", "--syslog-facility", "--tag", "--version",
];

/// `-o` settings that carry the information the wrapper needs, in lower case.
const INTERESTING_OPTIONS: &[&str] = &[
    "user", "hostname", "port", "proxyjump", "proxycommand", "controlmaster", "controlpath",
    "controlpersist", "sendenv", "requesttty", "stricthostkeychecking",
];

/// Scan an `ssh` argv (excluding `argv[0]`).
pub fn scan(args: &[String]) -> SshArgvScan {
    let mut out = SshArgvScan::default();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();

        // Once the destination has been read, ssh stops parsing options: the
        // rest of the line is the remote command and belongs to the far end.
        // `ssh web1 --version` asks web1 for its ssh version, not ours.
        if out.destination.is_some() {
            out.remote_command.push(a.to_owned());
            i += 1;
            continue;
        }

        if a == "--" {
            // Everything after `--` is positional.
            i += 1;
            break;
        }

        if let Some(long) = a.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((n, v)) => (format!("--{n}"), Some(v.to_owned())),
                None => (a.to_owned(), None),
            };
            let takes_value = LONG_WITH_VALUE.iter().any(|l| l.eq_ignore_ascii_case(&name));
            if takes_value {
                let value = inline.or_else(|| {
                    i += 1;
                    args.get(i).cloned()
                });
                if let Some(v) = value {
                    // A long option carries its own meaning, so it is mapped
                    // directly rather than through the `-o key=value` path.
                    match name.to_ascii_lowercase().as_str() {
                        "--port" => out.port = v.parse().ok(),
                        "--login-name" => out.user = Some(v.clone()),
                        "--jump-host" => out.jump = Some(v.clone()),
                        "--option" => record_option(&mut out, "--option", &v),
                        _ => out.options.push((name.clone(), v.clone())),
                    }
                }
                out.flags.push(name);
            } else {
                out.flags.push(name);
            }
            i += 1;
            continue;
        }

        if a.len() >= 2 && a.starts_with('-') {
            // A cluster like `-4vq` is a run of standalone flags; a cluster like
            // `-p2222` carries its value inline.
            let (head, tail) = a.split_at(2);
            let takes_value = VALUE_TAKING.contains(&head);
            if takes_value {
                let value = if tail.is_empty() {
                    i += 1;
                    args.get(i).cloned()
                } else {
                    Some(tail.to_owned())
                };
                if let Some(v) = value {
                    match head {
                        "-p" => out.port = v.parse().ok(),
                        "-l" => out.user = Some(v.clone()),
                        "-J" => out.jump = Some(v.clone()),
                        "-o" | "-O" => record_option(&mut out, "-o", &v),
                        "-W" => out.no_shell = true,
                        _ => {}
                    }
                }
                out.flags.push(head.to_owned());
                i += 1;
                continue;
            }
            // Not a known value-taking flag: treat it as a cluster of
            // standalone flags, so `-4vq` reports all three.
            if a.chars().skip(1).all(|c| STANDALONE.iter().any(|f| f.len() == 2 && f.ends_with(c)))
            {
                for c in a.chars().skip(1) {
                    let f = format!("-{c}");
                    if f == "-N" || f == "-W" {
                        out.no_shell = true;
                    }
                    out.flags.push(f);
                }
                i += 1;
                continue;
            }
            // Unknown. Keep it as a flag but do not consume the next token:
            // guessing wrong would swallow the destination.
            out.flags.push(a.to_owned());
            i += 1;
            continue;
        }

        // The first positional token is the destination.
        if out.destination.is_none() {
            out.destination = Some(a.to_owned());
            if let Some((user, host)) = a.split_once('@') {
                if !user.is_empty() {
                    out.user = Some(user.to_owned());
                }
                out.host = Some(host.to_owned());
            } else {
                out.host = Some(a.to_owned());
            }
        } else {
            out.remote_command.push(a.to_owned());
        }
        i += 1;
    }

    // Anything left after `--` is positional too.
    while i < args.len() {
        if out.destination.is_none() {
            out.destination = Some(args[i].clone());
            out.host = Some(args[i].clone());
        } else {
            out.remote_command.push(args[i].clone());
        }
        i += 1;
    }

    out
}

fn record_option(out: &mut SshArgvScan, name: &str, value: &str) {
    let (key, val) = value
        .split_once('=')
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .unwrap_or_else(|| (String::new(), value.trim().to_owned()));
    let key = if key.is_empty() {
        // `-o User` without `=` is not valid ssh, but `-o` alone is a config
        // file directive in some dialects; keep it plainly.
        String::new()
    } else {
        key
    };
    if key.is_empty() {
        return;
    }
    let lower = key.to_ascii_lowercase();
    if INTERESTING_OPTIONS.contains(&lower.as_str()) {
        match lower.as_str() {
            "user" => out.user = Some(val.clone()),
            "hostname" => out.host = Some(val.clone()),
            "port" => out.port = val.parse().ok(),
            "proxyjump" => out.jump = Some(val.clone()),
            _ => {}
        }
    }
    out.options.push((key.clone(), val.clone()));
    let _ = name;
}

/// One `Host` block from `~/.ssh/config`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HostBlock {
    /// The patterns the block matches, in order.
    pub patterns: Vec<String>,
    /// The directives, key lower-cased, first occurrence winning exactly as ssh
    /// does ("the first obtained value for each parameter is used").
    pub directives: BTreeMap<String, String>,
    /// The file the block came from, for `ssh doctor` to cite.
    pub source: Option<PathBuf>,
    pub line: usize,
}

impl HostBlock {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.directives.get(&key.to_ascii_lowercase()).map(String::as_str)
    }
}

/// A parsed `ssh_config`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SshConfig {
    pub blocks: Vec<HostBlock>,
    /// Files pulled in by `Include`.
    pub includes: Vec<PathBuf>,
}

/// Split an ssh_config value into its whitespace-separated parts, honouring
/// double quotes — which is how a `ProxyCommand` or a `ControlPath` containing
/// spaces survives.
pub fn split_values(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    let mut escaped = false;
    for c in raw.chars() {
        if escaped {
            cur.push(c);
            escaped = false;
            continue;
        }
        match c {
            '\\' if quote => escaped = true,
            '"' => quote = !quote,
            c if c.is_whitespace() && !quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

impl SshConfig {
    /// Parse the text of an ssh_config file.
    pub fn parse(text: &str, source: Option<PathBuf>) -> Self {
        let mut cfg = SshConfig::default();
        let mut current: Option<HostBlock> = None;
        for (n, raw_line) in text.lines().enumerate() {
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once(|c: char| c.is_whitespace() || c == '=')
            else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "host" => {
                    if let Some(b) = current.take() {
                        cfg.blocks.push(b);
                    }
                    current = Some(HostBlock {
                        patterns: split_values(value),
                        directives: BTreeMap::new(),
                        source: source.clone(),
                        line: n + 1,
                    });
                }
                "include" => {
                    for v in split_values(value) {
                        cfg.includes.push(PathBuf::from(v));
                    }
                }
                "match" => {
                    // A `Match` block is host-conditional in ways this wrapper
                    // does not evaluate; close the current Host block so its
                    // directives cannot leak into the next one.
                    if let Some(b) = current.take() {
                        cfg.blocks.push(b);
                    }
                }
                _ => {
                    if let Some(b) = current.as_mut() {
                        // First value wins, as ssh documents.
                        b.directives.entry(key).or_insert_with(|| value.to_owned());
                    }
                }
            }
        }
        if let Some(b) = current.take() {
            cfg.blocks.push(b);
        }
        cfg
    }

    /// Read a file, or an empty config when it does not exist.
    pub fn read_file(path: &Path) -> std::io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(t) => Ok(Self::parse(&t, Some(path.to_path_buf()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// The default config path, honouring `$HOME`.
    pub fn default_path() -> Option<PathBuf> {
        otty_core::dirs_home().map(|h| h.join(".ssh").join("config"))
    }

    /// Resolve `host` the way ssh would: the *first* block whose pattern
    /// matches wins, and `*` matches anything.
    pub fn resolve(&self, host: &str) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        for block in &self.blocks {
            if block.patterns.iter().any(|p| pattern_matches(p, host)) {
                for (k, v) in &block.directives {
                    out.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        out
    }

    /// The block a `host` matched first, for `ssh doctor` to cite.
    pub fn matching_block(&self, host: &str) -> Option<&HostBlock> {
        self.blocks
            .iter()
            .find(|b| b.patterns.iter().any(|p| pattern_matches(p, host)))
    }
}

fn strip_comment(line: &str) -> &str {
    let mut quote = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quote = !quote,
            '#' if !quote => return &line[..i],
            _ => {}
        }
    }
    line
}

/// ssh's host patterns: `*` and `?` wildcards, negated with `!`.
pub fn pattern_matches(pattern: &str, host: &str) -> bool {
    if let Some(neg) = pattern.strip_prefix('!') {
        return !pattern_matches(neg, host);
    }
    let (p, h) = (pattern.to_ascii_lowercase(), host.to_ascii_lowercase());
    glob(&p.as_bytes(), h.as_bytes())
}

fn glob(pat: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = Some(p);
            mark = t;
            p += 1;
        } else if let Some(sp) = star {
            p = sp + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

/// What the wrapper decided for one host — the substance of `otty ssh doctor`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HostPlan {
    pub host: String,
    pub user: Option<String>,
    pub port: u16,
    pub resolved_hostname: Option<String>,
    pub jump: Option<String>,
    /// Whether the user configured a `ControlPath` we honour, or whether Otty
    /// mints one.
    pub control_path: Option<String>,
    pub control_path_is_ours: bool,
    pub control_master: Option<String>,
    pub control_persist: Option<String>,
    /// The remote shell answering at the far end, when known.
    pub remote_login_shell: Option<String>,
    /// Why the plan says what it says, one line each.
    pub reasons: Vec<String>,
}

/// Build the plan for `scanned`, merged with the user's ssh_config.
///
/// The command line wins over the config file, which is what ssh itself does.
pub fn plan(scanned: &SshArgvScan, config: &SshConfig) -> HostPlan {
    let host = scanned.host.clone().unwrap_or_default();
    let resolved = config.resolve(&host);
    let mut reasons = Vec::new();

    if let Some(block) = config.matching_block(&host) {
        reasons.push(match (&block.source, block.line) {
            (Some(p), l) => format!(
                "`{host}` matched `{}` at {}:{}",
                block.patterns.join(" "),
                p.display(),
                l
            ),
            _ => format!("`{host}` matched `{}`", block.patterns.join(" ")),
        });
    } else {
        reasons.push(format!("`{host}` matched no Host block; using built-in defaults"));
    }

    let resolved_hostname = resolved.get("hostname").cloned();
    let user = scanned
        .user
        .clone()
        .or_else(|| resolved.get("user").cloned())
        .or_else(|| std::env::var("USER").ok().or_else(|| std::env::var("USERNAME").ok()));
    let port = scanned
        .port
        .or_else(|| resolved.get("port").and_then(|p| p.parse().ok()))
        .unwrap_or(22);
    let jump = scanned.jump.clone().or_else(|| resolved.get("proxyjump").cloned());

    // A configured ControlPath that contains no `%` placeholder is a fixed
    // socket; one with a placeholder is per-host and still usable. Only an
    // absent value means Otty has to mint its own.
    let configured = resolved.get("controlpath").cloned();
    let (control_path, control_path_is_ours) = match configured {
        Some(p) if !p.trim().is_empty() => {
            reasons.push(format!("honouring the configured ControlPath `{p}`"));
            (Some(p), false)
        }
        _ => {
            let minted = minted_control_path(&host, port, user.as_deref());
            reasons.push(format!(
                "no ControlPath configured; minting `{minted}` so a second connection reuses the first"
            ));
            (Some(minted), true)
        }
    };

    let control_master = resolved.get("controlmaster").cloned();
    let control_persist = resolved.get("controlpersist").cloned();
    if control_master.as_deref() == Some("no") {
        reasons.push(
            "ControlMaster is `no`, so Otty will not multiplex this host and will not reuse a socket"
                .to_owned(),
        );
    }

    HostPlan {
        host,
        user,
        port,
        resolved_hostname,
        jump,
        control_path,
        control_path_is_ours,
        control_master,
        control_persist,
        // The far end's login shell can only be learnt by asking it, which
        // `ssh doctor` does at run time.
        remote_login_shell: None,
        reasons,
    }
}

/// Otty's own control socket, placed under the app-support directory so two
/// panes connecting to the same host share one transport.
pub fn minted_control_path(host: &str, port: u16, user: Option<&str>) -> String {
    let safe: String = host
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
        .collect();
    let who = user.unwrap_or("default");
    otty_core::app_support_dir()
        .join("ssh")
        .join(format!("{who}@{safe}:{port}"))
        .display()
        .to_string()
}

/// The tokens `otty-helper` uses to identify the host it is running on.
///
/// A helper report is trusted only when it names the host the wrapper expected;
/// otherwise a command that ran locally would be recorded as remote.
pub fn host_tokens(machine: &str) -> Vec<String> {
    let lower = machine.to_ascii_lowercase();
    let mut out = vec![lower.clone()];
    if let Some(short) = lower.split('.').next() {
        if short != lower {
            out.push(short.to_owned());
        }
    }
    out
}

/// Whether a host reported by the helper is the host the wrapper expected.
///
/// Either name may be the short or the fully-qualified form — a host the user
/// types as `web1` reports itself as `web1.internal.example.com` and vice versa —
/// so the two names are compared as sets of aliases rather than as strings.
pub fn host_matches(expected: &str, reported: &str) -> bool {
    let want = host_tokens(expected);
    host_tokens(reported).iter().any(|t| want.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn a_bare_host_is_the_destination() {
        let s = scan(&argv("myserver"));
        assert_eq!(s.host.as_deref(), Some("myserver"));
        assert_eq!(s.destination.as_deref(), Some("myserver"));
        assert_eq!(s.port, None);
        assert!(s.remote_command.is_empty());
    }

    #[test]
    fn user_and_host_split_on_the_at_sign() {
        let s = scan(&argv("deploy@web1.example.com"));
        assert_eq!(s.user.as_deref(), Some("deploy"));
        assert_eq!(s.host.as_deref(), Some("web1.example.com"));
    }

    #[test]
    fn a_short_option_does_not_match_a_longer_one() {
        // `-c` takes a cipher; `-C` takes nothing. A prefix match confuses them.
        let s = scan(&argv("-C web1"));
        assert_eq!(s.host.as_deref(), Some("web1"), "-C must not eat the host");
        assert!(s.flags.contains(&"-C".to_owned()));

        let s = scan(&argv("-c aes128-ctr web1"));
        assert_eq!(s.host.as_deref(), Some("web1"));
        assert!(s.flags.contains(&"-c".to_owned()));
    }

    #[test]
    fn value_taking_options_consume_exactly_one_token() {
        let s = scan(&argv("-p 2222 -i ~/.ssh/id_ed25519 deploy@web1"));
        assert_eq!(s.port, Some(2222));
        assert_eq!(s.user.as_deref(), Some("deploy"));
        assert_eq!(s.host.as_deref(), Some("web1"));
        assert!(s.remote_command.is_empty());
    }

    #[test]
    fn an_inline_value_is_not_a_separate_token() {
        let s = scan(&argv("-p2222 web1"));
        assert_eq!(s.port, Some(2222));
        assert_eq!(s.host.as_deref(), Some("web1"));

        let s = scan(&argv("-lwww web1"));
        assert_eq!(s.user.as_deref(), Some("www"));
        assert_eq!(s.host.as_deref(), Some("web1"));
    }

    #[test]
    fn o_settings_are_parsed_and_understood() {
        let s = scan(&argv("-o User=git -o Port=2222 -o StrictHostKeyChecking=no host"));
        assert_eq!(s.user.as_deref(), Some("git"));
        assert_eq!(s.port, Some(2222));
        assert_eq!(s.host.as_deref(), Some("host"));
        assert!(s
            .options
            .iter()
            .any(|(k, v)| k == "StrictHostKeyChecking" && v == "no"));
    }

    #[test]
    fn a_long_option_wins_over_the_config_file() {
        let s = scan(&argv("--port=2200 host"));
        assert_eq!(s.port, Some(2200));
        let s = scan(&argv("--port 2201 host"));
        assert_eq!(s.port, Some(2201));
    }

    #[test]
    fn a_clustered_flag_run_reports_every_flag() {
        let s = scan(&argv("-4vq host"));
        assert!(s.flags.contains(&"-4".to_owned()));
        assert!(s.flags.contains(&"-v".to_owned()));
        assert!(s.flags.contains(&"-q".to_owned()));
        assert_eq!(s.host.as_deref(), Some("host"));
    }

    #[test]
    fn everything_after_the_destination_is_the_remote_command() {
        let s = scan(&argv("web1 --version"));
        assert_eq!(s.host.as_deref(), Some("web1"));
        assert_eq!(s.remote_command, ["--version"]);
    }

    #[test]
    fn a_double_dash_ends_the_option_scan() {
        let s = scan(&argv("-- -weird-host"));
        assert_eq!(s.host.as_deref(), Some("-weird-host"));
    }

    #[test]
    fn no_shell_flags_are_recognised() {
        assert!(scan(&argv("-N web1")).no_shell);
        assert!(scan(&argv("-W %h:%p jump")).no_shell);
        assert!(!scan(&argv("web1")).no_shell);
    }

    #[test]
    fn a_jump_host_is_recorded_from_either_spelling() {
        assert_eq!(scan(&argv("-J bastion web1")).jump.as_deref(), Some("bastion"));
        let s = scan(&argv("-o ProxyJump=bastion web1"));
        assert_eq!(s.jump.as_deref(), Some("bastion"));
    }

    #[test]
    fn values_split_on_whitespace_but_respect_quotes() {
        assert_eq!(split_values("a b c"), ["a", "b", "c"]);
        assert_eq!(
            split_values("ProxyCommand ssh -W %h:%p \"jump box\""),
            ["ProxyCommand", "ssh", "-W", "%h:%p", "jump box"]
        );
        assert_eq!(split_values("  ").len(), 0);
    }

    #[test]
    fn host_patterns_support_wildcards_and_negation() {
        assert!(pattern_matches("*", "anything"));
        assert!(pattern_matches("*.example.com", "web1.example.com"));
        assert!(!pattern_matches("*.example.com", "web1.example.org"));
        assert!(pattern_matches("web?", "web1"));
        assert!(pattern_matches("!bad", "good"));
        assert!(!pattern_matches("!bad", "bad"));
    }

    const CONFIG: &str = "\
# a comment
Host web1
    HostName 10.0.0.11
    User deploy
    Port 2222

Host *.internal
    User ops
    ControlPath ~/.ssh/cm-%r@%h:%p
    ControlMaster auto

Host *
    ServerAliveInterval 30

Include ~/.ssh/extra.conf
";

    #[test]
    fn the_first_matching_block_wins_for_each_directive() {
        let cfg = SshConfig::parse(CONFIG, None);
        assert_eq!(cfg.blocks.len(), 3);
        let r = cfg.resolve("web1");
        assert_eq!(r.get("hostname").map(String::as_str), Some("10.0.0.11"));
        assert_eq!(r.get("user").map(String::as_str), Some("deploy"));
        assert_eq!(r.get("port").map(String::as_str), Some("2222"));
        // `Host *` fills in what the specific block did not.
        assert_eq!(r.get("serveraliveinterval").map(String::as_str), Some("30"));
        // …and never overrides it.
        assert_eq!(r.get("user").map(String::as_str), Some("deploy"));
    }

    #[test]
    fn comments_are_stripped_outside_quotes() {
        let cfg = SshConfig::parse("Host h # trailing\n  User u # and here\n", None);
        assert_eq!(cfg.blocks[0].patterns, ["h"]);
        assert_eq!(cfg.blocks[0].get("user"), Some("u"));
    }

    #[test]
    fn includes_are_collected() {
        let cfg = SshConfig::parse(CONFIG, None);
        assert_eq!(cfg.includes, [PathBuf::from("~/.ssh/extra.conf")]);
    }

    #[test]
    fn a_match_block_closes_the_host_block() {
        let cfg = SshConfig::parse("Host a\n  User x\nMatch host b\n  User y\n", None);
        assert_eq!(cfg.blocks.len(), 1);
        assert_eq!(cfg.blocks[0].get("user"), Some("x"));
    }

    #[test]
    fn the_plan_prefers_the_command_line_over_the_config() {
        let cfg = SshConfig::parse(CONFIG, None);
        let s = scan(&argv("-p 9999 web1"));
        let p = plan(&s, &cfg);
        assert_eq!(p.port, 9999);
        assert_eq!(p.user.as_deref(), Some("deploy"), "the config still supplies the user");
        assert_eq!(p.resolved_hostname.as_deref(), Some("10.0.0.11"));
    }

    #[test]
    fn a_configured_control_path_is_honoured_rather_than_minted() {
        let cfg = SshConfig::parse(CONFIG, None);
        let p = plan(&scan(&argv("box.internal")), &cfg);
        assert_eq!(p.control_path.as_deref(), Some("~/.ssh/cm-%r@%h:%p"));
        assert!(!p.control_path_is_ours);
        assert!(p.reasons.iter().any(|r| r.contains("honouring")));
    }

    #[test]
    fn a_host_without_a_control_path_gets_one_that_is_ours() {
        let cfg = SshConfig::parse(CONFIG, None);
        let p = plan(&scan(&argv("solo.example.com")), &cfg);
        assert!(p.control_path_is_ours);
        assert!(p.control_path.is_some());
        assert!(p.reasons.iter().any(|r| r.contains("no ControlPath configured")));
    }

    #[test]
    fn control_master_no_is_called_out() {
        let cfg = SshConfig::parse("Host h\n  ControlMaster no\n", None);
        let p = plan(&scan(&argv("h")), &cfg);
        assert!(p.reasons.iter().any(|r| r.contains("ControlMaster is `no`")));
    }

    #[test]
    fn a_minted_path_is_filesystem_safe() {
        let p = minted_control_path("web1.example.com", 22, Some("deploy"));
        assert!(p.ends_with("deploy@web1.example.com:22"), "{p}");
        let p = minted_control_path("we₪rd/host", 22, None);
        assert!(!p.contains('₪') && !p.contains("//"), "{p}");
    }

    #[test]
    fn helper_reports_are_matched_on_either_name() {
        assert!(host_matches("web1", "web1.internal.example.com"));
        assert!(host_matches("web1.internal.example.com", "web1"));
        assert!(!host_matches("web2", "web1.internal.example.com"));
        assert_eq!(host_tokens("web1"), ["web1"]);
        assert_eq!(host_tokens("Web1.Example.COM"), ["web1.example.com", "web1"]);
    }

    #[test]
    fn a_missing_config_file_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = SshConfig::read_file(&dir.path().join("nope")).unwrap();
        assert!(cfg.blocks.is_empty());
    }
}
