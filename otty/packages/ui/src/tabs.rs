//! Sidebar tab list: grouping and hand-inserted dividers.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabGroupMode {
    None,
    Project,
    Date,
}

impl TabGroupMode {
    pub fn from_action(action: &str) -> Option<Self> {
        Some(match action {
            "tabs_group_none" => Self::None,
            "tabs_group_project" => Self::Project,
            "tabs_group_date" => Self::Date,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabListRow {
    Group(String),
    Tab { title: String },
    Divider,
}

/// Walk up until `.git` exists. No git → keep `cwd`.
pub fn git_root(cwd: &str, has_git: impl Fn(&str) -> bool) -> String {
    let mut cur = cwd.trim_end_matches(['/', '\\']).to_string();
    if cur.is_empty() {
        return cur;
    }
    let start = cur.clone();
    loop {
        if has_git(&cur) {
            return cur;
        }
        let Some(parent) = Path::new(&cur).parent() else {
            return start;
        };
        let parent = parent.to_string_lossy().into_owned();
        if parent == cur {
            return start;
        }
        cur = parent;
    }
}

/// `items` are (title, cwd). `dividers_after` are 0-based tab indices.
pub fn group_tabs(
    items: &[(String, String)],
    mode: TabGroupMode,
    has_git: impl Fn(&str) -> bool,
    dividers_after: &[usize],
) -> Vec<TabListRow> {
    let mut out = Vec::new();
    match mode {
        TabGroupMode::None | TabGroupMode::Date => {
            for (i, (title, _)) in items.iter().enumerate() {
                out.push(TabListRow::Tab {
                    title: title.clone(),
                });
                if dividers_after.contains(&i) {
                    out.push(TabListRow::Divider);
                }
            }
        }
        TabGroupMode::Project => {
            let mut last: Option<String> = None;
            for (i, (title, cwd)) in items.iter().enumerate() {
                let root = git_root(cwd, &has_git);
                if last.as_deref() != Some(root.as_str()) {
                    let name = Path::new(&root)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or(root.clone());
                    out.push(TabListRow::Group(name));
                    last = Some(root);
                }
                out.push(TabListRow::Tab {
                    title: title.clone(),
                });
                if dividers_after.contains(&i) {
                    out.push(TabListRow::Divider);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    fn git(dir: &str) -> bool {
        dir == "/work/otty" || dir == "/work/other"
    }

    #[test]
    fn none_vs_project_and_dividers() {
        let items = [
            ("ui".into(), "/work/otty/packages/ui".into()),
            ("cli".into(), "/work/otty/packages/cli".into()),
            ("tmp".into(), "/tmp/scratch".into()),
        ];
        let none = group_tabs(&items, TabGroupMode::None, git, &[]);
        assert!(none.iter().all(|r| !matches!(r, TabListRow::Group(_))));
        assert_eq!(none.len(), 3);

        let by = group_tabs(&items, TabGroupMode::Project, git, &[]);
        assert_eq!(
            by.iter()
                .filter_map(|r| match r {
                    TabListRow::Group(n) => Some(n.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            ["otty", "scratch"]
        );
        assert_eq!(git_root("/work/otty/packages/ui", git), "/work/otty");

        let with_div = group_tabs(&items, TabGroupMode::None, git, &[0]);
        assert_eq!(with_div[1], TabListRow::Divider);

        let mut d = Dispatcher::new();
        assert!(d.activate("tabs_group_project"));
        assert_eq!(d.tab_group, TabGroupMode::Project);
        assert!(d.activate("tabs_group_none"));
        assert_eq!(d.tab_group, TabGroupMode::None);
        assert!(d.activate("tabs_insert_divider"));
        assert!(!d.dividers.is_empty());
        assert!(d.activate("tabs_remove_dividers"));
        assert!(d.dividers.is_empty());
    }
}
