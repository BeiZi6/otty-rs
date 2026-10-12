//! Recipes: tabs, split trees and working directories as one TOML document.
//!
//! The format is the `.ottyrecipe` of `spec/FEATURES.md` §5:
//!
//! ```toml
//! [recipe]
//! name = "deploy-prod-debug"
//! version = 1
//! scope = "window"          # "tab" | "window" | "commands"
//!
//! [[window.tabs]]
//! title = "API"
//!
//! [[window.tabs.panes]]
//! cwd = "{{current_folder}}/api"
//! commands = ["tail -F log/prod.log"]
//!
//! [[window.tabs.panes]]
//! cwd = "{{current_folder}}/api"
//! split = "right"           # relative to the previous pane
//! size = 0.5                # 0.0–1.0 of the parent
//! ```
//!
//! Two things here are this rebuild's, because the documentation shows neither:
//! a pane may say `of = N` to split the tab's N-th pane rather than the previous
//! one (without it a tree like "two rows on the left, one column on the right"
//! cannot be written), and the `commands` scope keeps its list in
//! `[recipe] commands`.
//!
//! Session restore is the same document with every window in `[[windows]]`:
//! what comes back after a relaunch and what a saved recipe opens are built by
//! one function, [`apply`].

use std::path::{Path, PathBuf};

use otty_core::{PaneId, WindowId};
use serde::{Deserialize, Serialize};

use crate::dispatch::Dispatcher;
use crate::model::{App, Axis, Node, SplitDirection, Tab};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Recipe {
    pub recipe: Meta,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowSpec>,
    /// Session restore keeps every window; a recipe has `window` alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<WindowSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub version: u32,
    /// `tab`, `window` or `commands`.
    pub scope: String,
    /// The `commands` scope: replayed into the focused pane.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    /// Session restore only: the cwds of closed tabs, newest last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closed: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowSpec {
    #[serde(default)]
    pub tabs: Vec<TabSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TabSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub panes: Vec<PaneSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PaneSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Where this pane goes relative to the one it splits: right, left, up, down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split: Option<String>,
    /// This pane's share of the split, 0.0–1.0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// The tab's pane this one splits, counted from 0. Omitted: the previous one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    /// The live pane this was captured from, so a caller can add its commands.
    #[serde(skip)]
    pub pane: Option<PaneId>,
}

fn leftmost(node: &Node) -> PaneId {
    match node {
        Node::Leaf(id) => *id,
        Node::Split { first, .. } => leftmost(first),
    }
}

/// A tab as a recipe: its leftmost pane, then one pane per split, each
/// splitting the leftmost pane of the side it was cut from.
pub fn capture_tab(app: &App, tab: &Tab) -> TabSpec {
    fn walk(app: &App, node: &Node, panes: &mut Vec<PaneSpec>) {
        let Node::Split { axis, ratio, first, second } = node else { return };
        let at = panes.iter().position(|p| p.pane == Some(leftmost(first))).unwrap_or(0);
        let new = leftmost(second);
        panes.push(PaneSpec {
            cwd: app.pane(new).and_then(|p| p.cwd.clone()),
            split: Some(if *axis == Axis::Horizontal { "right" } else { "down" }.to_owned()),
            size: Some(f64::from(100 - ratio) / 100.0),
            of: (at + 1 != panes.len()).then_some(at),
            pane: Some(new),
            ..Default::default()
        });
        walk(app, first, panes);
        walk(app, second, panes);
    }
    let root = leftmost(&tab.tree);
    let mut panes = vec![PaneSpec { cwd: app.pane(root).and_then(|p| p.cwd.clone()), pane: Some(root), ..Default::default() }];
    walk(app, &tab.tree, &mut panes);
    TabSpec { title: app.pane(tab.focused).and_then(|p| p.custom_title.clone()), panes }
}

pub fn capture_window(app: &App, window: WindowId) -> WindowSpec {
    let tabs = app.windows().iter().find(|w| w.id == window).map(|w| w.tabs()).unwrap_or_default();
    WindowSpec { tabs: tabs.iter().map(|t| capture_tab(app, t)).collect() }
}

/// Every window, for session restore.
pub fn capture_session(app: &App, closed: Vec<String>) -> Recipe {
    Recipe {
        recipe: Meta { name: "session".to_owned(), version: 1, scope: "window".to_owned(), closed, ..Default::default() },
        window: None,
        windows: app.windows().iter().map(|w| capture_window(app, w.id)).collect(),
    }
}

/// Where a recipe's first tab goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Every tab is a new tab of the active window.
    NewTabs,
    /// A new window holds them.
    NewWindow,
    /// The focused tab — a fresh one — becomes the first; the rest follow it.
    Here,
}

/// The `{{…}}` variables a recipe's paths may start with.
#[derive(Debug, Clone, Default)]
pub struct Vars {
    /// The directory the recipe is opened from.
    pub current_folder: String,
    pub home: String,
    /// The folder holding the `.ottyrecipe`.
    pub recipe_location: String,
}

impl Vars {
    pub fn expand(&self, path: &str) -> String {
        path.replace("{{current_folder}}", &self.current_folder)
            .replace("{{home}}", &self.home)
            .replace("{{recipe_location}}", &self.recipe_location)
    }

    /// The reverse, for a recipe that is handed to someone else: a path under
    /// the current folder or the home folder starts with its variable.
    pub fn portable(&self, path: &str) -> String {
        let slashes = |s: &str| s.replace('\\', "/");
        let path = slashes(path);
        for (var, prefix) in [("{{current_folder}}", &self.current_folder), ("{{home}}", &self.home)] {
            let prefix = slashes(prefix);
            match path.strip_prefix(prefix.trim_end_matches('/')) {
                Some(rest) if !prefix.is_empty() && (rest.is_empty() || rest.starts_with('/')) => {
                    return format!("{var}{rest}");
                }
                _ => {}
            }
        }
        path
    }
}

/// Build `window` in the model: tabs, splits, working directories, titles.
/// Returns the new panes, each with the commands the recipe has for it.
pub fn apply(d: &mut Dispatcher, window: &WindowSpec, place: Place, vars: &Vars) -> Vec<(PaneId, Vec<String>)> {
    let mut out: Vec<(PaneId, Vec<String>)> = Vec::new();
    for (n, tab) in window.tabs.iter().enumerate() {
        match (n, place) {
            (0, Place::Here) => {}
            (0, Place::NewWindow) => drop(d.activate("new_window")),
            _ => drop(d.activate("new_tab")),
        }
        let first = out.len();
        for (i, spec) in tab.panes.iter().enumerate() {
            let split = (i > 0).then(|| {
                let made = &out[first..];
                let target = made[spec.of.filter(|of| *of < made.len()).unwrap_or(made.len() - 1)].0;
                (target, spec.split.as_deref().and_then(SplitDirection::parse).unwrap_or(SplitDirection::Right))
            });
            if let Some((target, direction)) = split {
                d.app.reveal_pane(target);
                d.activate(&format!("split_{}", direction.as_str()));
            }
            // A split that could not be made leaves the focus where it was.
            let Some(pane) = d.focused_pane_id().filter(|p| out.iter().all(|(made, _)| made != p)) else { continue };
            if let (Some((target, direction)), Some(t)) = (split, d.app.tab_mut_of(pane)) {
                let share = (spec.size.unwrap_or(0.5).clamp(0.1, 0.9) * 100.0).round() as u8;
                t.tree.set_ratio_between(target, pane, if direction.new_is_second() { 100 - share } else { share });
            }
            if let Some(p) = d.app.pane_mut(pane) {
                p.cwd = spec.cwd.as_deref().map(|cwd| vars.expand(cwd));
                if i == 0 {
                    p.custom_title.clone_from(&tab.title);
                }
            }
            out.push((pane, spec.commands.clone()));
        }
    }
    // Leave the first pane of the first tab in front.
    if let Some((pane, _)) = out.first() {
        d.app.reveal_pane(*pane);
    }
    out
}

/// Every command the recipe would run, as `(tab, pane, command)` counted
/// from 1 — what the trust prompt shows before anything executes.
pub fn commands_of(recipe: &Recipe) -> Vec<(usize, usize, String)> {
    let mut out: Vec<_> = recipe.recipe.commands.iter().map(|c| (0, 0, c.clone())).collect();
    for window in recipe.window.iter().chain(&recipe.windows) {
        for (t, tab) in window.tabs.iter().enumerate() {
            for (p, pane) in tab.panes.iter().enumerate() {
                out.extend(pane.commands.iter().map(|c| (t + 1, p + 1, c.clone())));
            }
        }
    }
    out
}

/// Whether `command` hands the terminal to another shell — `ssh`, `tmux
/// attach`, `docker exec -it`, `su` — so that what follows it in a recipe must
/// not be typed until that shell is back. Errs toward yes.
pub fn hands_off(command: &str) -> bool {
    let words: Vec<&str> = command.split_whitespace().collect();
    let interactive = || words.iter().any(|w| w.starts_with('-') && !w.starts_with("--") && w.contains(['i', 't']));
    match words.as_slice() {
        ["ssh" | "mosh" | "su" | "telnet" | "screen" | "tmux", ..] => true,
        ["sudo", "-i" | "-s" | "su", ..] => true,
        ["docker" | "podman" | "kubectl", "exec" | "run", ..] => interactive(),
        _ => false,
    }
}

pub fn parse(bytes: &[u8]) -> Result<Recipe, String> {
    toml::from_str(&String::from_utf8_lossy(bytes)).map_err(|e| e.to_string())
}

pub fn render(recipe: &Recipe) -> String {
    toml::to_string(recipe).unwrap_or_default()
}

/// `~/.config/otty/recipes`, the library saved recipes live in.
pub fn library_dir() -> PathBuf {
    otty_core::config_dir().join("recipes")
}

/// A recipe's file in the library `dir`. The name is kept as written, minus
/// what a file name cannot hold.
pub fn library_path(dir: &Path, name: &str) -> PathBuf {
    let stem: String = name.trim().chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '-' } else { c }).collect();
    dir.join(format!("{}.ottyrecipe", if stem.is_empty() { "recipe" } else { &stem }))
}

/// Write `recipe` to `path`, making its folder.
pub fn write(path: &Path, recipe: &Recipe) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, render(recipe))
}

/// The recipes in the library `dir`, by name: `(name, path, scope)`.
pub fn list(dir: &Path) -> Vec<(String, PathBuf, String)> {
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ottyrecipe"))
        .filter_map(|path| {
            let recipe = parse(&std::fs::read(&path).ok()?).ok()?;
            Some((recipe.recipe.name, path, recipe.recipe.scope))
        })
        .collect();
    out.sort();
    out
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn trusted(store: &Path) -> Vec<String> {
    std::fs::read(store).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

/// Whether the user chose Always Trust for exactly these bytes. `store` is
/// `trusted_recipes.json`; an edited recipe hashes differently and asks again.
pub fn is_trusted(store: &Path, bytes: &[u8]) -> bool {
    trusted(store).contains(&sha256(bytes))
}

pub fn trust(store: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut hashes = trusted(store);
    hashes.push(sha256(bytes));
    hashes.dedup();
    if let Some(dir) = store.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(store, serde_json::to_vec_pretty(&hashes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (1 over 2) | 3, the tree "previous pane" alone cannot describe.
    fn three_panes() -> Dispatcher {
        let mut d = Dispatcher::new();
        d.activate("split_right");
        let left = d.app.active_window().unwrap().active_tab().unwrap().tree.panes()[0];
        d.app.reveal_pane(left);
        d.activate("split_down");
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        let panes = tab.tree.panes();
        for (pane, cwd) in panes.iter().zip(["/home/me/api", "/home/me/api/log", "/srv/web"]) {
            d.app.pane_mut(*pane).unwrap().cwd = Some(cwd.to_owned());
        }
        d.app.tab_mut_of(panes[0]).unwrap().tree.set_ratio_between(panes[0], panes[2], 70);
        d.app.pane_mut(panes[0]).unwrap().custom_title = Some("API".to_owned());
        d.app.reveal_pane(panes[0]);
        d
    }

    fn tree_of(d: &Dispatcher) -> (Node, Vec<Option<String>>) {
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        (tab.tree.clone(), tab.tree.panes().iter().map(|p| d.app.pane(*p).unwrap().cwd.clone()).collect())
    }

    /// Two trees with the same shape and ratios, whatever their pane ids.
    fn same_shape(a: &Node, b: &Node) -> bool {
        match (a, b) {
            (Node::Leaf(_), Node::Leaf(_)) => true,
            (Node::Split { axis, ratio, first, second }, Node::Split { axis: a2, ratio: r2, first: f2, second: s2 }) => {
                axis == a2 && ratio == r2 && same_shape(first, f2) && same_shape(second, s2)
            }
            _ => false,
        }
    }

    #[test]
    fn a_saved_window_opens_as_the_same_tree_with_the_same_directories() {
        let d = three_panes();
        let window = d.app.active_window().unwrap().id;
        let recipe = Recipe {
            recipe: Meta { name: "api".into(), version: 1, scope: "window".into(), ..Default::default() },
            window: Some(capture_window(&d.app, window)),
            windows: Vec::new(),
        };
        let text = render(&recipe);
        // The pane that splits the first one, not the previous one, says so.
        assert!(text.contains("of = 0") && text.contains("split = \"down\"") && text.contains("size = 0.3"), "{text}");
        assert!(!text.contains("pane ="), "live pane ids are not part of the file: {text}");

        let back = parse(text.as_bytes()).unwrap();
        let mut fresh = Dispatcher::new();
        let made = apply(&mut fresh, back.window.as_ref().unwrap(), Place::NewWindow, &Vars::default());
        assert_eq!(made.len(), 3);
        assert_eq!(fresh.app.counts().0, 2, "a window recipe opens a window");
        let ((tree, cwds), (was, were)) = (tree_of(&fresh), tree_of(&d));
        assert!(same_shape(&tree, &was), "{tree:?}\nwas\n{was:?}");
        assert_eq!(cwds, were);
        let tab = fresh.app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(fresh.app.pane(tab.focused).unwrap().custom_title.as_deref(), Some("API"));
    }

    #[test]
    fn the_documented_file_parses_and_its_variables_expand() {
        let text = r#"
[recipe]
name = "deploy-prod-debug"
version = 1
scope = "window"

[[window.tabs]]
title = "API"

[[window.tabs.panes]]
cwd = "{{current_folder}}/api"
commands = ["tail -F log/prod.log"]

[[window.tabs.panes]]
cwd = "{{home}}/notes"
split = "right"
size = 0.25
commands = ["ssh web1", "make deploy"]
"#;
        let recipe = parse(text.as_bytes()).unwrap();
        assert_eq!(commands_of(&recipe).len(), 3);
        assert_eq!(commands_of(&recipe)[1], (1, 2, "ssh web1".to_owned()));
        let vars = Vars { current_folder: "/work".into(), home: "/home/me".into(), recipe_location: "/dl".into() };
        let mut d = Dispatcher::new();
        let made = apply(&mut d, recipe.window.as_ref().unwrap(), Place::Here, &vars);
        assert_eq!(d.app.counts(), (1, 1, 2), "Here fills the tab that is already there");
        let cwd = |i: usize| d.app.pane(made[i].0).unwrap().cwd.clone().unwrap();
        assert_eq!((cwd(0), cwd(1)), ("/work/api".to_owned(), "/home/me/notes".to_owned()));
        assert_eq!(made[1].1, ["ssh web1", "make deploy"]);
        let tab = d.app.active_window().unwrap().active_tab().unwrap();
        assert!(matches!(tab.tree, Node::Split { axis: Axis::Horizontal, ratio: 75, .. }), "{:?}", tab.tree);

        assert_eq!(vars.portable("/work/api"), "{{current_folder}}/api");
        assert_eq!(vars.portable("/home/me"), "{{home}}");
        assert_eq!(vars.portable("/home/meow"), "/home/meow", "a prefix must end at a separator");
        assert_eq!(Vars::default().portable("C:\\x"), "C:/x");
    }

    #[test]
    fn a_recipe_file_is_trusted_by_its_bytes_and_the_library_lists_what_it_holds() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("support").join("trusted_recipes.json");
        let recipe = Recipe {
            recipe: Meta { name: "部署: prod".into(), version: 1, scope: "commands".into(), commands: vec!["make".into()], ..Default::default() },
            ..Default::default()
        };
        let path = library_path(dir.path(), &recipe.recipe.name);
        assert_eq!(path.file_name().unwrap().to_string_lossy(), "部署- prod.ottyrecipe");
        write(&path, &recipe).unwrap();
        assert_eq!(list(dir.path()), [("部署: prod".to_owned(), path.clone(), "commands".to_owned())]);

        let bytes = std::fs::read(&path).unwrap();
        assert!(!is_trusted(&store, &bytes));
        trust(&store, &bytes).unwrap();
        assert!(is_trusted(&store, &bytes));
        // Editing the file changes its hash, so it asks again.
        assert!(!is_trusted(&store, format!("{}\n# edited", String::from_utf8_lossy(&bytes)).as_bytes()));
    }

    #[test]
    fn commands_that_hand_the_terminal_over_are_recognised() {
        for yes in ["ssh web1", "tmux attach -t main", "docker exec -it api sh", "sudo -i", "su - deploy", "kubectl exec -ti pod -- sh"] {
            assert!(hands_off(yes), "{yes}");
        }
        for no in ["make deploy", "docker ps", "docker exec api ls", "sudo make install", "echo ssh"] {
            assert!(!hands_off(no), "{no}");
        }
    }
}
