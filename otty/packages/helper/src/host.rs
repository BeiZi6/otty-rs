//! What the helper can learn about the machine it is running on.
//!
//! Everything here reads files or runs a read-only command. Nothing writes,
//! nothing needs root, and nothing is cached: the helper is short-lived, so the
//! cost of reading `/proc` once is smaller than the cost of keeping state.

use serde::{Deserialize, Serialize};

/// Who is on the far end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostIdentity {
    /// The machine's own name, as it reports it.
    pub hostname: String,
    /// `linux`, `macos`, `freebsd`, …
    pub os: String,
    pub kernel: Option<String>,
    /// `x86_64`, `aarch64`, …
    pub arch: String,
    /// The login the helper was run as.
    pub user: String,
    /// The helper build that answered, so the local side can re-upload when it
    /// is older than the app.
    pub helper_version: String,
    pub protocol_version: u32,
}

impl HostIdentity {
    /// Learn the identity of the machine this process is running on.
    pub fn detect() -> Self {
        Self {
            hostname: hostname(),
            os: std::env::consts::OS.to_owned(),
            kernel: kernel_version(),
            arch: std::env::consts::ARCH.to_owned(),
            user: whoami(),
            helper_version: crate::VERSION.to_owned(),
            protocol_version: crate::PROTOCOL_VERSION,
        }
    }
}

/// The names a host answers to.
///
/// A helper report is trusted only when it names the host the wrapper expected —
/// otherwise a command that ran locally would be recorded as remote. A host the
/// user types as `web1` reports itself as `web1.internal.example.com`, so the
/// comparison is a set of aliases rather than a string equality. This is the
/// remote half of `otty_remote_shim::host_tokens`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostTokens(pub Vec<String>);

impl HostTokens {
    /// Derive the alias set from a machine name.
    pub fn of(machine: &str) -> Self {
        let lower = machine.trim().to_ascii_lowercase();
        let mut tokens = vec![lower.clone()];
        // The short name before the first dot is how a user types it.
        if let Some(short) = lower.split('.').next() {
            if short != lower && !short.is_empty() {
                tokens.push(short.to_owned());
            }
        }
        // A trailing dot is a fully-qualified name, not a different host.
        let trimmed = lower.trim_end_matches('.').to_owned();
        if trimmed != lower && !trimmed.is_empty() && !tokens.contains(&trimmed) {
            tokens.push(trimmed);
        }
        tokens.retain(|t| !t.is_empty());
        tokens.sort();
        tokens.dedup();
        Self(tokens)
    }

    /// Whether this set contains `name`.
    pub fn contains(&self, name: &str) -> bool {
        let n = name.trim().to_ascii_lowercase();
        self.0.iter().any(|t| *t == n)
    }

    /// Whether two machines are the same host, either spelling.
    pub fn matches(&self, other: &HostTokens) -> bool {
        self.0.iter().any(|t| other.0.contains(t))
    }
}

/// Resource usage at one instant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostStats {
    /// CPU busy fraction across all cores, 0.0–1.0, when it can be measured.
    pub cpu_fraction: Option<f32>,
    pub cpu_cores: Option<u32>,
    pub load_average: Option<[f32; 3]>,
    pub memory_total_bytes: Option<u64>,
    pub memory_available_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub disk_available_bytes: Option<u64>,
    /// The filesystem the reading is for.
    pub disk_path: String,
    pub network_rx_bytes: Option<u64>,
    pub network_tx_bytes: Option<u64>,
    pub uptime_seconds: Option<u64>,
}

impl HostStats {
    /// Gather what this platform can answer.
    pub fn collect() -> Self {
        let mut s = Self {
            cpu_fraction: None,
            cpu_cores: std::thread::available_parallelism().ok().map(|n| n.get() as u32),
            load_average: None,
            memory_total_bytes: None,
            memory_available_bytes: None,
            disk_total_bytes: None,
            disk_available_bytes: None,
            disk_path: otty_core::dirs_home()
                .unwrap_or_else(|| "/".into())
                .display()
                .to_string(),
            network_rx_bytes: None,
            network_tx_bytes: None,
            uptime_seconds: None,
        };
        match std::env::consts::OS {
            "linux" => s.fill_from_proc(),
            "macos" => s.fill_from_sysctl(),
            _ => {}
        }
        s
    }

    /// Linux: `/proc` answers all of it, and none of it needs a syscall wrapper.
    fn fill_from_proc(&mut self) {
        if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
            if let Some(line) = stat.lines().next() {
                let fields: Vec<u64> =
                    line.split_whitespace().skip(1).filter_map(|f| f.parse().ok()).collect();
                if fields.len() >= 4 {
                    let idle = fields[3] + fields.get(4).copied().unwrap_or(0);
                    let total: u64 = fields.iter().sum();
                    if total > 0 {
                        self.cpu_fraction = Some(1.0 - idle as f32 / total as f32);
                    }
                }
            }
        }
        if let Ok(mem) = std::fs::read_to_string("/proc/meminfo") {
            for line in mem.lines() {
                let mut it = line.split_whitespace();
                let (Some(key), Some(value)) = (it.next(), it.next()) else { continue };
                let Ok(kib) = value.parse::<u64>() else { continue };
                match key.trim_end_matches(':') {
                    "MemTotal" => self.memory_total_bytes = Some(kib * 1024),
                    "MemAvailable" => self.memory_available_bytes = Some(kib * 1024),
                    _ => {}
                }
            }
        }
        if let Ok(loadavg) = std::fs::read_to_string("/proc/loadavg") {
            let parts: Vec<f32> =
                loadavg.split_whitespace().take(3).filter_map(|f| f.parse().ok()).collect();
            if parts.len() == 3 {
                self.load_average = Some([parts[0], parts[1], parts[2]]);
            }
        }
        if let Ok(uptime) = std::fs::read_to_string("/proc/uptime") {
            self.uptime_seconds =
                uptime.split_whitespace().next().and_then(|s| s.parse::<f32>().ok()).map(|f| f as u64);
        }
        if let Ok(net) = std::fs::read_to_string("/proc/net/dev") {
            let (mut rx, mut tx) = (0u64, 0u64);
            for line in net.lines().skip(2) {
                let Some((_, rest)) = line.split_once(':') else { continue };
                let fields: Vec<&str> = rest.split_whitespace().collect();
                // Received bytes are field 0, transmitted bytes field 8.
                if let (Some(r), Some(t)) = (fields.first(), fields.get(8)) {
                    rx += r.parse::<u64>().unwrap_or(0);
                    tx += t.parse::<u64>().unwrap_or(0);
                }
            }
            if rx > 0 || tx > 0 {
                self.network_rx_bytes = Some(rx);
                self.network_tx_bytes = Some(tx);
            }
        }
        self.fill_disk_from_df();
    }

    /// macOS: the same numbers, from `sysctl` and `df`.
    fn fill_from_sysctl(&mut self) {
        let sysctl = |name: &str| -> Option<String> {
            let out = std::process::Command::new("sysctl").args(["-n", name]).output().ok()?;
            let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            (!s.is_empty()).then_some(s)
        };
        if let Some(total) = sysctl("hw.memsize").and_then(|s| s.parse::<u64>().ok()) {
            self.memory_total_bytes = Some(total);
        }
        self.load_average = sysctl("vm.loadavg").and_then(|s| {
            let parts: Vec<f32> = s
                .trim_matches(|c| c == '{' || c == '}')
                .split_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect();
            (parts.len() >= 3).then(|| [parts[0], parts[1], parts[2]])
        });
        // `vm_stat` reports pages; the page size is fixed at 16 KiB on Apple
        // silicon and 4 KiB on Intel, and `hw.pagesize` says which.
        let page = sysctl("hw.pagesize").and_then(|s| s.parse::<u64>().ok()).unwrap_or(4096);
        if let Ok(out) = std::process::Command::new("vm_stat").output() {
            let text = String::from_utf8_lossy(&out.stdout);
            let free_pages: u64 = text
                .lines()
                .filter(|l| l.starts_with("Pages free") || l.starts_with("Pages inactive"))
                .filter_map(|l| l.split(':').nth(1))
                .filter_map(|v| v.trim().trim_end_matches('.').parse::<u64>().ok())
                .sum();
            if free_pages > 0 {
                self.memory_available_bytes = Some(free_pages * page);
            }
        }
        self.fill_disk_from_df();
    }

    /// `df -kP` is the one reading that is the same on every POSIX system.
    fn fill_disk_from_df(&mut self) {
        let Ok(out) = std::process::Command::new("df").args(["-kP", &self.disk_path]).output() else {
            return;
        };
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(line) = text.lines().nth(1) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() >= 4 {
                let total = f[1].parse::<u64>().ok();
                let available = f[3].parse::<u64>().ok();
                self.disk_total_bytes = total.map(|v| v * 1024);
                self.disk_available_bytes = available.map(|v| v * 1024);
            }
        }
    }

    /// Memory used, derived from the two readings.
    pub fn memory_used_bytes(&self) -> Option<u64> {
        Some(self.memory_total_bytes?.saturating_sub(self.memory_available_bytes?))
    }

    /// Disk used, derived.
    pub fn disk_used_bytes(&self) -> Option<u64> {
        Some(self.disk_total_bytes?.saturating_sub(self.disk_available_bytes?))
    }
}

// ------------------------------------------------------------------ platform

/// The machine's name.
pub fn hostname() -> String {
    if let Ok(h) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
        let h = h.trim().to_owned();
        if !h.is_empty() {
            return h;
        }
    }
    // `hostname` exists on every POSIX system and is a single fork; a
    // short-lived helper can afford it and avoids a libc binding.
    if let Ok(out) = std::process::Command::new("hostname").output() {
        let h = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !h.is_empty() {
            return h;
        }
    }
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_owned())
}

/// The kernel release string.
pub fn kernel_version() -> Option<String> {
    if let Ok(u) = std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        let u = u.trim().to_owned();
        if !u.is_empty() {
            return Some(u);
        }
    }
    let out = std::process::Command::new("uname").arg("-r").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// The login the helper is running as.
pub fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| {
            std::process::Command::new("id")
                .arg("-un")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .unwrap_or_else(|_| "unknown".to_owned())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_name_and_a_fully_qualified_name_are_the_same_host() {
        let a = HostTokens::of("web1");
        let b = HostTokens::of("web1.internal.example.com");
        assert!(a.matches(&b));
        assert!(b.matches(&a));
        assert!(a.contains("web1"));
        assert!(b.contains("web1"));
        assert!(b.contains("web1.internal.example.com"));
    }

    #[test]
    fn two_different_hosts_do_not_match() {
        let a = HostTokens::of("web1.example.com");
        let b = HostTokens::of("web2.example.com");
        assert!(!a.matches(&b));
        // A shared domain is not a match: only the first label counts.
        assert!(!a.contains("example.com"));
    }

    #[test]
    fn matching_is_case_insensitive_and_tolerates_a_trailing_dot() {
        assert!(HostTokens::of("WEB1").contains("web1"));
        assert!(HostTokens::of("web1.example.com.").contains("web1.example.com"));
    }

    #[test]
    fn the_token_set_is_sorted_and_deduplicated() {
        let t = HostTokens::of("web1");
        assert_eq!(t.0, ["web1"]);
        let mut sorted = t.0.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(t.0, sorted);
    }

    #[test]
    fn a_blank_name_yields_no_tokens() {
        assert!(HostTokens::of("   ").0.is_empty());
    }

    #[test]
    fn identity_is_populated_on_this_machine() {
        let id = HostIdentity::detect();
        assert!(!id.hostname.is_empty());
        assert!(!id.os.is_empty());
        assert!(!id.arch.is_empty());
        assert!(!id.user.is_empty());
        assert_eq!(id.helper_version, crate::VERSION);
        assert_eq!(id.protocol_version, crate::PROTOCOL_VERSION);
    }

    #[test]
    fn stats_collect_without_panicking() {
        let s = HostStats::collect();
        assert!(!s.disk_path.is_empty());
        // Cores are known on every platform Rust supports.
        assert!(s.cpu_cores.is_some_and(|c| c > 0));
        // Whatever is available must be consistent with what is total.
        if let (Some(total), Some(avail)) = (s.disk_total_bytes, s.disk_available_bytes) {
            assert!(avail <= total, "available {avail} > total {total}");
        }
        if let (Some(total), Some(avail)) = (s.memory_total_bytes, s.memory_available_bytes) {
            assert!(avail <= total, "available {avail} > total {total}");
        }
        if let Some(f) = s.cpu_fraction {
            assert!((0.0..=1.0).contains(&f), "cpu fraction {f} out of range");
        }
    }

    #[test]
    fn derived_usage_never_underflows() {
        let s = HostStats {
            cpu_fraction: None,
            cpu_cores: None,
            load_average: None,
            memory_total_bytes: Some(100),
            memory_available_bytes: Some(40),
            disk_total_bytes: Some(1000),
            disk_available_bytes: Some(1000),
            disk_path: "/".into(),
            network_rx_bytes: None,
            network_tx_bytes: None,
            uptime_seconds: None,
        };
        assert_eq!(s.memory_used_bytes(), Some(60));
        assert_eq!(s.disk_used_bytes(), Some(0));
        // A reading that is missing gives no answer rather than a wrong one.
        let empty = HostStats {
            memory_total_bytes: None,
            ..s.clone()
        };
        assert_eq!(empty.memory_used_bytes(), None);
    }

    #[test]
    fn identity_serialises_with_stable_field_names() {
        let id = HostIdentity::detect();
        let v = serde_json::to_value(&id).unwrap();
        for key in [
            "hostname",
            "os",
            "kernel",
            "arch",
            "user",
            "helper_version",
            "protocol_version",
        ] {
            assert!(v.get(key).is_some(), "missing {key}");
        }
    }
}
