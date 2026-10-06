//! File / folder / URL panes opened by `otty view`.
//!
//! A file is read-only text in a new tab. A directory lists names. URLs are
//! recorded as a pane kind; the shell draws them later.

use std::fs;
use std::path::Path;

use otty_core::expand_path;

use crate::model::PaneKind;

/// Load a path into the pane kind `otty view` would open.
pub fn load_view(path: &str) -> Result<PaneKind, String> {
    let raw = path.trim();
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Ok(PaneKind::Url {
            url: raw.to_owned(),
        });
    }
    let expanded = expand_path(raw);
    let p = Path::new(&expanded);
    if p.is_dir() {
        let mut names: Vec<String> = fs::read_dir(p)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        Ok(PaneKind::Folder {
            path: expanded,
            names,
        })
    } else if p.is_file() {
        let body = fs::read_to_string(p).map_err(|e| e.to_string())?;
        Ok(PaneKind::File {
            path: expanded,
            body,
        })
    } else {
        Err(format!("{expanded} is not a file or directory"))
    }
}

/// `otty view PATH` / `otty-app -e view PATH --mode view`.
pub fn view_target_from_args(args: &[String]) -> Option<&str> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-e" {
            i += 1;
            continue;
        }
        if args[i] == "view" || args[i] == "edit" {
            let t = args.get(i + 1)?;
            if t.starts_with('-') {
                return None;
            }
            return Some(t.as_str());
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn view_opens_readme_readonly_and_lists_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "# hello\n").unwrap();
        std::fs::write(dir.path().join("a.txt"), "x").unwrap();

        let mut d = Dispatcher::new();
        assert!(d.open_view(&readme.display().to_string()));
        let pane = d.app.pane(d.focused_pane_id().unwrap()).unwrap();
        match &pane.kind {
            PaneKind::File { path, body } => {
                assert!(path.ends_with("README.md"), "{path}");
                assert_eq!(body, "# hello\n");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(d.app.counts().1, 2, "opens in a new tab");

        assert!(d.open_view(&dir.path().display().to_string()));
        let pane = d.app.pane(d.focused_pane_id().unwrap()).unwrap();
        match &pane.kind {
            PaneKind::Folder { names, .. } => {
                assert!(names.iter().any(|n| n == "README.md"));
                assert!(names.iter().any(|n| n == "a.txt"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn otty_view_argv_names_the_file() {
        let args = ["-e".into(), "view".into(), "README.md".into(), "--mode".into(), "view".into()];
        assert_eq!(view_target_from_args(&args), Some("README.md"));
        assert_eq!(view_target_from_args(&["open".into()]), None);
    }
}
