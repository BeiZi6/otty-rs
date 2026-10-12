//! Open Quickly: one fuzzy field over nine filters.
//!
//! Filter keys that are not in the table return an empty list. A query that
//! names an existing directory pins a Path row at the top; ↩ opens it and
//! records the visit in frecency.

use std::path::Path;

use otty_core::expand_path;
use otty_ssh::SshConfig;
use otty_state::Store;

/// The nine FEATURES §1 filters, plus `None` for an unknown key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFilter {
    All,
    Opened,
    Recent,
    Folders,
    Ssh,
    Agents,
    Current,
    Recipes,
    Files,
}

impl OpenFilter {
    /// Names (`folders`) and shortcut letters (`z` = ⌘Z). Unknown → `None`.
    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key.trim().to_ascii_lowercase().as_str() {
            "all" | "0" => Self::All,
            "opened" | "w" => Self::Opened,
            "recent" | "r" => Self::Recent,
            "folders" | "z" => Self::Folders,
            "ssh" | "s" => Self::Ssh,
            "agents" | "g" => Self::Agents,
            "current" | "j" => Self::Current,
            "recipes" | "e" => Self::Recipes,
            "files" | "p" => Self::Files,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Opened => "opened",
            Self::Recent => "recent",
            Self::Folders => "folders",
            Self::Ssh => "ssh",
            Self::Agents => "agents",
            Self::Current => "current",
            Self::Recipes => "recipes",
            Self::Files => "files",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuicklyKind {
    Path,
    Folder,
    Opened,
    Recent,
    Ssh,
    Agent,
    Current,
    Recipe,
    File,
}

impl QuicklyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Folder => "folder",
            Self::Opened => "opened",
            Self::Recent => "recent",
            Self::Ssh => "ssh",
            Self::Agent => "agent",
            Self::Current => "current",
            Self::Recipe => "recipe",
            Self::File => "file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuicklyEntry {
    pub kind: QuicklyKind,
    pub title: String,
    pub path: String,
}

#[derive(Debug, Clone, Default)]
pub struct QuicklySources {
    pub opened: Vec<String>,
    pub recent: Vec<String>,
    pub folders: Vec<String>,
    pub ssh: Vec<String>,
    pub agents: Vec<String>,
    pub current: Vec<String>,
    pub recipes: Vec<String>,
    pub files: Vec<String>,
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl QuicklySources {
    /// Folders from the default frecency store, SSH hosts from `~/.ssh/config`.
    pub fn live(opened: Vec<String>) -> Self {
        let mut s = Self {
            opened,
            ..Default::default()
        };
        if let Ok(store) = Store::open_default() {
            s.folders = Self::folders_from_store(&store, 12, now_secs());
        }
        if let Some(path) = SshConfig::default_path() {
            if let Ok(cfg) = SshConfig::read_file(&path) {
                s.ssh = Self::ssh_hosts(&cfg);
            }
        }
        s
    }

    pub fn folders_from_store(store: &Store, limit: usize, now: i64) -> Vec<String> {
        store
            .ranked_directories(limit, now)
            .unwrap_or_default()
            .into_iter()
            .map(|f| f.path)
            .collect()
    }

    pub fn ssh_hosts(cfg: &SshConfig) -> Vec<String> {
        cfg.blocks
            .iter()
            .flat_map(|b| b.patterns.iter())
            .filter(|p| !p.chars().any(|c| matches!(c, '*' | '?' | '!')))
            .cloned()
            .collect()
    }
}

/// Pin a Path row when the query is an existing directory. `/` and `~` prefixes
/// are expanded first; a bare existing path (Windows drive letters) also pins.
pub fn pin_path(query: &str) -> Option<QuicklyEntry> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let looks_like_path = q.starts_with('/')
        || q.starts_with('~')
        || q.starts_with('\\')
        || q.contains('/')
        || q.contains('\\');
    if !looks_like_path {
        return None;
    }
    let expanded = expand_path(q);
    if Path::new(&expanded).is_dir() {
        Some(QuicklyEntry {
            kind: QuicklyKind::Path,
            title: expanded.clone(),
            path: expanded,
        })
    } else {
        None
    }
}

/// `filter == None` is an unknown key: empty list, no panic.
pub fn query_quickly(
    filter: Option<OpenFilter>,
    query: &str,
    sources: &QuicklySources,
) -> Vec<QuicklyEntry> {
    let Some(filter) = filter else {
        return Vec::new();
    };
    let mut rows = match filter {
        OpenFilter::All => {
            let mut all = Vec::new();
            extend_kind(&mut all, QuicklyKind::Opened, &sources.opened);
            extend_kind(&mut all, QuicklyKind::Folder, &sources.folders);
            extend_kind(&mut all, QuicklyKind::Recent, &sources.recent);
            extend_kind(&mut all, QuicklyKind::Ssh, &sources.ssh);
            extend_kind(&mut all, QuicklyKind::Agent, &sources.agents);
            extend_kind(&mut all, QuicklyKind::Current, &sources.current);
            extend_kind(&mut all, QuicklyKind::Recipe, &sources.recipes);
            extend_kind(&mut all, QuicklyKind::File, &sources.files);
            all
        }
        OpenFilter::Opened => kind_rows(QuicklyKind::Opened, &sources.opened),
        OpenFilter::Recent => kind_rows(QuicklyKind::Recent, &sources.recent),
        OpenFilter::Folders => kind_rows(QuicklyKind::Folder, &sources.folders),
        OpenFilter::Ssh => kind_rows(QuicklyKind::Ssh, &sources.ssh),
        OpenFilter::Agents => kind_rows(QuicklyKind::Agent, &sources.agents),
        OpenFilter::Current => kind_rows(QuicklyKind::Current, &sources.current),
        OpenFilter::Recipes => kind_rows(QuicklyKind::Recipe, &sources.recipes),
        OpenFilter::Files => kind_rows(QuicklyKind::File, &sources.files),
    };
    let q = query.trim().to_ascii_lowercase();
    if !q.is_empty() {
        rows.retain(|e| {
            e.title.to_ascii_lowercase().contains(&q) || e.path.to_ascii_lowercase().contains(&q)
        });
    }
    if let Some(pin) = pin_path(query) {
        rows.retain(|e| e.path != pin.path);
        rows.insert(0, pin);
    }
    rows
}

fn kind_rows(kind: QuicklyKind, paths: &[String]) -> Vec<QuicklyEntry> {
    paths
        .iter()
        .map(|p| QuicklyEntry {
            kind,
            title: p.clone(),
            path: p.clone(),
        })
        .collect()
}

fn extend_kind(out: &mut Vec<QuicklyEntry>, kind: QuicklyKind, paths: &[String]) {
    out.extend(kind_rows(kind, paths));
}

/// Open an existing directory. Records the visit when a store is given.
pub fn open_directory(path: &str, store: Option<&Store>, now: i64) -> Result<String, String> {
    let expanded = expand_path(path.trim());
    if !Path::new(&expanded).is_dir() {
        return Err(format!("{expanded} is not a directory"));
    }
    if let Some(s) = store {
        let _ = s.record_visit(&expanded, now);
    }
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn cmd_shift_o_opens_an_existing_directory_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        let store = Store::in_memory().unwrap();
        let now = 1_700_000_000;

        let mut d = Dispatcher::new();
        assert!(d.activate("open_quickly"));
        assert!(d.quickly_open);

        let pin = pin_path(&path).expect("existing dir must pin a Path row");
        assert_eq!(pin.kind, QuicklyKind::Path);
        assert!(d.run_quickly(&pin.path, Some(&store), now));
        assert_eq!(d.last_opened.as_deref(), Some(path.as_str()));
        assert!(!d.quickly_open);
        let ranked = store.ranked_directories(10, now).unwrap();
        assert!(ranked.iter().any(|f| f.path == otty_state::normalize_path(&path)
            || f.path == path));
    }

    #[test]
    fn cmd_z_lists_frecency_folders() {
        let store = Store::in_memory().unwrap();
        let now = 1_700_000_000;
        store.record_visit("/work/otty", now).unwrap();
        store.record_visit("/tmp/scratch", now - 60).unwrap();
        let mut sources = QuicklySources::default();
        sources.folders = QuicklySources::folders_from_store(&store, 12, now);

        let rows = query_quickly(OpenFilter::from_key("z"), "", &sources);
        assert!(
            rows.iter().any(|e| e.path.contains("otty")),
            "⌘Z must list frecency: {rows:?}"
        );
        assert_eq!(rows[0].kind, QuicklyKind::Folder);
        assert!(OpenFilter::from_key("folders") == Some(OpenFilter::Folders));
    }

    #[test]
    fn unknown_filter_key_returns_empty_and_does_not_panic() {
        let sources = QuicklySources {
            folders: vec!["/a".into()],
            ..Default::default()
        };
        let rows = query_quickly(OpenFilter::from_key("nope"), "a", &sources);
        assert!(rows.is_empty());
        let rows = query_quickly(OpenFilter::from_key(""), "", &sources);
        assert!(rows.is_empty());

        let mut d = Dispatcher::new();
        d.activate("open_quickly");
        d.set_quickly_filter("not-a-filter");
        assert!(d.quickly_filter.is_none());
        let listed = query_quickly(d.quickly_filter, "a", &sources);
        assert!(listed.is_empty());
    }

    #[test]
    fn tilde_and_slash_pin_when_the_path_exists() {
        if let Some(home) = otty_core::dirs_home() {
            if home.is_dir() {
                let pin = pin_path("~").expect("~ must pin $HOME");
                assert_eq!(pin.kind, QuicklyKind::Path);
            }
        }
        assert!(pin_path("~/otty-s05-missing-xyzzy-dir").is_none());
        assert!(pin_path("not-a-path").is_none());
    }

    #[test]
    fn ssh_hosts_skip_wildcards() {
        let cfg = SshConfig::parse("Host web1 *.internal\n  User git\nHost *\n  Port 22\n", None);
        let hosts = QuicklySources::ssh_hosts(&cfg);
        assert_eq!(hosts, vec!["web1".to_string()]);
    }
}
