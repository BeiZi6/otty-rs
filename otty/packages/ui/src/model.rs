//! The window / tab / pane tree.
//!
//! Deliberately free of `winit`, `wgpu` and threads: this is the part of the
//! shell that decides *what* exists and *where* it goes, and it is the part a
//! test can drive. The event loop and the GPU only ask it questions.
//!
//! # The shape
//!
//! A window holds tabs; a tab holds a **split tree** of panes. The tree is the
//! thing that makes `otty pane split --direction right` mean something precise:
//!
//! ```text
//! Split(Horizontal)
//! ├── Leaf(pane 1)          the left half
//! └── Split(Vertical)       the right half, itself split
//!     ├── Leaf(pane 2)      top-right
//!     └── Leaf(pane 3)      bottom-right
//! ```
//!
//! Closing a pane collapses its parent split, promoting the sibling — which is
//! why a tree rather than a list of columns: with a list, closing the last pane
//! of a column has to be special-cased, and the split ratios on either side have
//! to be renegotiated by hand.

use std::collections::BTreeMap;
use std::time::Instant;

use otty_core::{AgentState, PaneId, TabId, WindowId};

/// A direction to split in. Named for where the **new** pane goes, which is what
/// a user means by `--direction right`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitDirection {
    Right,
    Left,
    Up,
    Down,
}

impl SplitDirection {
    /// The axis the split divides.
    pub const fn axis(self) -> Axis {
        match self {
            Self::Right | Self::Left => Axis::Horizontal,
            Self::Up | Self::Down => Axis::Vertical,
        }
    }

    /// Whether the new pane takes the second slot rather than the first.
    pub const fn new_is_second(self) -> bool {
        matches!(self, Self::Right | Self::Down)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Left => "left",
            Self::Up => "up",
            Self::Down => "down",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "right" => Self::Right,
            "left" => Self::Left,
            "up" | "top" => Self::Up,
            "down" | "bottom" => Self::Down,
            _ => return None,
        })
    }
}

/// Which way a split divides the space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Panes side by side; the divider is vertical.
    Horizontal,
    /// Panes stacked; the divider is horizontal.
    Vertical,
}

/// A node of a tab's split tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Leaf(PaneId),
    Split {
        axis: Axis,
        /// The first child's share, 10–90. Kept as an integer percent because
        /// that is what `--size` takes and what the config stores; storing a
        /// float would introduce a rounding difference between what the user
        /// asked for and what a save-then-load produces.
        ratio: u8,
        first: Box<Node>,
        second: Box<Node>,
    },
}

impl Node {
    /// Every pane under this node, in layout order (left-to-right,
    /// top-to-bottom), which is the order `otty pane list` prints.
    pub fn panes(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.walk(&mut |id| out.push(id));
        out
    }

    /// How many panes the node holds.
    pub fn len(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.len() + second.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn contains(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf(id) => *id == pane,
            Self::Split { first, second, .. } => first.contains(pane) || second.contains(pane),
        }
    }

    fn walk(&self, f: &mut impl FnMut(PaneId)) {
        match self {
            Self::Leaf(id) => f(*id),
            Self::Split { first, second, .. } => {
                first.walk(f);
                second.walk(f);
            }
        }
    }

    /// Insert `new_pane` by splitting `target`.
    ///
    /// Returns `false` when `target` is not in this tree, so the caller can
    /// report "no such pane" rather than silently creating an orphan.
    pub fn split(&mut self, target: PaneId, new_pane: PaneId, direction: SplitDirection, ratio: u8) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                let leaf = Node::Leaf(target);
                let fresh = Node::Leaf(new_pane);
                let (first, second) = if direction.new_is_second() {
                    (leaf, fresh)
                } else {
                    (fresh, leaf)
                };
                *self = Node::Split {
                    axis: direction.axis(),
                    ratio: ratio.clamp(10, 90),
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split(target, new_pane, direction, ratio)
                    || second.split(target, new_pane, direction, ratio)
            }
        }
    }

    /// Remove `pane`, collapsing its parent split so the sibling takes the space.
    ///
    /// Returns the replacement tree, or `None` when this node *was* that pane —
    /// which means the tab has no panes left, and the caller has to decide that
    /// a tab with no panes closes.
    pub fn remove(&mut self, pane: PaneId) -> Option<Node> {
        match self {
            Self::Leaf(id) if *id == pane => None,
            Self::Leaf(_) => Some(self.clone()),
            Self::Split { first, second, .. } => {
                if first.contains(pane) {
                    match first.remove(pane) {
                        Some(replacement) => {
                            *first = Box::new(replacement);
                            Some(self.clone())
                        }
                        // `first` was the pane itself: the sibling takes over.
                        None => Some((**second).clone()),
                    }
                } else if second.contains(pane) {
                    match second.remove(pane) {
                        Some(replacement) => {
                            *second = Box::new(replacement);
                            Some(self.clone())
                        }
                        None => Some((**first).clone()),
                    }
                } else {
                    // The pane is not in this subtree; nothing to do.
                    Some(self.clone())
                }
            }
        }
    }

    /// Set the split ratio of the split that directly contains `pane`.
    pub fn set_ratio_for(&mut self, pane: PaneId, ratio: u8) -> bool {
        match self {
            Self::Leaf(_) => false,
            Self::Split { first, second, ratio: r, .. } => {
                if first.contains(pane) || second.contains(pane) {
                    // Only the innermost split that holds it counts.
                    if (first.contains(pane) && first.set_ratio_for(pane, ratio))
                        || (second.contains(pane) && second.set_ratio_for(pane, ratio))
                    {
                        return true;
                    }
                    *r = ratio.clamp(10, 90);
                    return true;
                }
                false
            }
        }
    }
}

/// A pane's live session.
#[derive(Debug)]
pub struct Pane {
    pub id: PaneId,
    pub tty: Option<String>,
    /// The agent session reporting through this pane, and its state.
    pub agent: Option<(String, AgentState)>,
    /// The user-set title, if any. `None` means the shell's title wins.
    pub custom_title: Option<String>,
    /// The title the shell last reported via OSC 0/2.
    pub shell_title: Option<String>,
    /// Whether the pane was created zoomed to fill its tab.
    pub zoomed: bool,
    /// Last OSC 7 working directory, which Details → Info shows.
    pub cwd: Option<String>,
    /// Terminal unless `otty view` / `otty edit` opened a file, folder or URL.
    pub kind: PaneKind,
}

/// What a pane shows. `tab-rule` `file=` matches File / Folder / Url only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneKind {
    Terminal,
    File { path: String, body: String },
    Folder { path: String, names: Vec<String> },
    Url { url: String },
}

impl Pane {
    pub fn new(id: PaneId) -> Self {
        Self {
            id,
            tty: None,
            agent: None,
            custom_title: None,
            shell_title: None,
            zoomed: false,
            cwd: None,
            kind: PaneKind::Terminal,
        }
    }

    /// The title to display: a user-set one beats the shell's.
    pub fn title(&self) -> Option<&str> {
        self.custom_title.as_deref().or(self.shell_title.as_deref())
    }
}

/// A tab.
#[derive(Debug)]
pub struct Tab {
    pub id: TabId,
    pub tree: Node,
    /// The focused pane. Always a member of `tree`.
    pub focused: PaneId,
    /// The pane filling the tab, when the user zoomed one.
    pub zoomed: Option<PaneId>,
    /// The badge the tab is showing, if any.
    pub badge: Option<Badge>,
}

/// A tab's progress badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    Running,
    Success,
    Error,
    /// A determinate progress reading, 0–100.
    Progress(u8),
}

/// A window.
#[derive(Debug)]
pub struct Window {
    pub id: WindowId,
    tabs: Vec<Tab>,
    active: usize,
}

impl Window {
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    /// Find a tab by id.
    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    /// Append a tab and make it active, which is what "new tab" does.
    pub fn push_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }

    /// Make `id` active. Returns `false` when there is no such tab, so a script
    /// naming a stale id gets "no such tab" rather than a silent change.
    pub fn focus_tab(&mut self, id: TabId) -> bool {
        match self.tabs.iter().position(|t| t.id == id) {
            Some(i) => {
                self.active = i;
                true
            }
            None => false,
        }
    }

    /// Close a tab. The last tab cannot be closed on its own — closing it closes
    /// the window — so the caller is told rather than left with an empty window.
    pub fn close_tab(&mut self, id: TabId) -> bool {
        let Some(i) = self.tabs.iter().position(|t| t.id == id) else { return false };
        self.tabs.remove(i);
        if self.active >= self.tabs.len() && !self.tabs.is_empty() {
            self.active = self.tabs.len() - 1;
        }
        !self.tabs.is_empty()
    }

    /// Pull `id` out of this window. The window may be left with no tabs.
    pub fn take_tab(&mut self, id: TabId) -> Option<Tab> {
        let i = self.tabs.iter().position(|t| t.id == id)?;
        let tab = self.tabs.remove(i);
        if self.active >= self.tabs.len() && !self.tabs.is_empty() {
            self.active = self.tabs.len() - 1;
        } else if self.active > i && self.active > 0 {
            self.active -= 1;
        }
        Some(tab)
    }

    /// Move a tab to `position`, clamped into range.
    pub fn move_tab(&mut self, id: TabId, position: usize) -> bool {
        let Some(i) = self.tabs.iter().position(|t| t.id == id) else { return false };
        let tab = self.tabs.remove(i);
        let at = position.min(self.tabs.len());
        self.tabs.insert(at, tab);
        self.active = at;
        true
    }
}

/// Everything the shell owns.
#[derive(Debug)]
pub struct App {
    windows: Vec<Window>,
    active: usize,
    panes: BTreeMap<PaneId, Pane>,
    started: Instant,
    /// The agent sessions seen so far, for the sidebar.
    pub agents: BTreeMap<String, AgentState>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            active: 0,
            panes: BTreeMap::new(),
            started: Instant::now(),
            agents: BTreeMap::new(),
        }
    }

    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    pub fn active_window(&self) -> Option<&Window> {
        self.windows.get(self.active)
    }

    pub fn active_window_mut(&mut self) -> Option<&mut Window> {
        self.windows.get_mut(self.active)
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.get(&id)
    }

    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(&id)
    }

    /// Every pane across every window, in window → tab → layout order. This is
    /// the order `otty pane list` prints and the order `Selector::Index` counts.
    pub fn all_panes(&self) -> Vec<PaneId> {
        self.windows
            .iter()
            .flat_map(|w| w.tabs.iter())
            .flat_map(|t| t.tree.panes())
            .collect()
    }

    /// Find a pane's tab, in any window.
    pub fn tab_of(&self, pane: PaneId) -> Option<TabId> {
        self.windows
            .iter()
            .flat_map(|w| w.tabs.iter())
            .find(|t| t.tree.contains(pane))
            .map(|t| t.id)
    }

    pub fn window_of(&self, pane: PaneId) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|w| w.tabs.iter().any(|t| t.tree.contains(pane)))
            .map(|w| w.id)
    }

    /// Register a new window with one tab holding one pane.
    pub fn add_window(&mut self, window: WindowId, tab: TabId, pane: PaneId) -> PaneId {
        self.panes.insert(pane, Pane::new(pane));
        self.windows.push(Window {
            id: window,
            tabs: vec![Tab {
                id: tab,
                tree: Node::Leaf(pane),
                focused: pane,
                zoomed: None,
                badge: None,
            }],
            active: 0,
        });
        self.active = self.windows.len() - 1;
        pane
    }

    /// Add a tab to a window.
    pub fn add_tab(&mut self, window: WindowId, tab: TabId, pane: PaneId) -> bool {
        let Some(w) = self.windows.iter_mut().find(|w| w.id == window) else { return false };
        self.panes.insert(pane, Pane::new(pane));
        w.push_tab(Tab {
            id: tab,
            tree: Node::Leaf(pane),
            focused: pane,
            zoomed: None,
            badge: None,
        });
        true
    }

    /// Tear `tab` into a new window that keeps its split tree.
    pub fn move_tab_to_new_window(&mut self, tab: TabId, new_window: WindowId) -> bool {
        let Some(src) = self.window_of_tab(tab) else { return false };
        if self.windows.len() == 1 {
            let w = &self.windows[0];
            if w.tabs.len() == 1 {
                return false;
            }
        }
        let Some(src_w) = self.windows.iter_mut().find(|w| w.id == src) else { return false };
        let Some(t) = src_w.take_tab(tab) else { return false };
        self.windows.retain(|w| !w.tabs.is_empty());
        self.windows.push(Window {
            id: new_window,
            tabs: vec![t],
            active: 0,
        });
        self.active = self.windows.len() - 1;
        true
    }

    /// Pull every tab into the active window and drop the empty ones.
    pub fn merge_all_windows(&mut self) -> bool {
        if self.windows.len() < 2 {
            return false;
        }
        let dest = self.active.min(self.windows.len() - 1);
        let dest_id = self.windows[dest].id;
        let mut extras: Vec<Tab> = Vec::new();
        for w in self.windows.iter_mut() {
            if w.id == dest_id {
                continue;
            }
            extras.append(&mut w.tabs);
        }
        if let Some(d) = self.windows.iter_mut().find(|w| w.id == dest_id) {
            d.tabs.append(&mut extras);
        }
        self.windows.retain(|w| w.id == dest_id);
        self.active = 0;
        true
    }

    /// Close a window. Returns whether any windows remain.
    pub fn close_window(&mut self, id: WindowId) -> bool {
        let Some(i) = self.windows.iter().position(|w| w.id == id) else { return !self.windows.is_empty() };
        let panes: Vec<_> = self.windows[i]
            .tabs
            .iter()
            .flat_map(|t| t.tree.panes())
            .collect();
        for p in panes {
            self.panes.remove(&p);
        }
        self.windows.remove(i);
        if self.active >= self.windows.len() && !self.windows.is_empty() {
            self.active = self.windows.len() - 1;
        }
        !self.windows.is_empty()
    }

    fn window_of_tab(&self, tab: TabId) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|w| w.tabs.iter().any(|t| t.id == tab))
            .map(|w| w.id)
    }

    /// Split a pane. The new pane becomes focused, which is what every terminal
    /// does and what `--no-focus` opts out of.
    pub fn split_pane(
        &mut self,
        target: PaneId,
        new_pane: PaneId,
        direction: SplitDirection,
        ratio: u8,
    ) -> bool {
        let Some(tab_id) = self.tab_of(target) else { return false };
        self.panes.insert(new_pane, Pane::new(new_pane));
        for w in &mut self.windows {
            if let Some(t) = w.tab_mut(tab_id) {
                if !t.tree.split(target, new_pane, direction, ratio) {
                    self.panes.remove(&new_pane);
                    return false;
                }
                t.focused = new_pane;
                return true;
            }
        }
        false
    }

    /// Close a pane. Returns the tab that closed, when this emptied it.
    pub fn close_pane(&mut self, pane: PaneId) -> Option<TabId> {
        let tab_id = self.tab_of(pane)?;
        self.panes.remove(&pane);
        for w in &mut self.windows {
            let Some(t) = w.tab_mut(tab_id) else { continue };
            // A zoomed pane being closed un-zooms the tab.
            if t.zoomed == Some(pane) {
                t.zoomed = None;
            }
            let emptied = match t.tree.remove(pane) {
                // The tree *was* that pane: nothing left in the tab.
                None => return Some(tab_id),
                Some(replacement) => {
                    t.tree = replacement;
                    false
                }
            };
            let _ = emptied;
            if t.focused == pane {
                // Focus the first remaining pane, the way a close does.
                t.focused = t.tree.panes()[0];
            }
            return None;
        }
        None
    }

    /// Focus a pane, un-zooming any other pane in its tab.
    pub fn focus_pane(&mut self, pane: PaneId) -> bool {
        let Some(tab_id) = self.tab_of(pane) else { return false };
        for w in &mut self.windows {
            if let Some(t) = w.tab_mut(tab_id) {
                t.focused = pane;
                return true;
            }
        }
        false
    }

    /// Zoom a pane to fill its tab, or un-zoom it if it already is.
    pub fn toggle_zoom(&mut self, pane: PaneId) -> bool {
        let Some(tab_id) = self.tab_of(pane) else { return false };
        for w in &mut self.windows {
            if let Some(t) = w.tab_mut(tab_id) {
                let zoomed = t.zoomed == Some(pane);
                t.zoomed = if zoomed { None } else { Some(pane) };
                let state = t.zoomed;
                if let Some(p) = self.panes.get_mut(&pane) {
                    p.zoomed = state.is_some();
                }
                return true;
            }
        }
        false
    }

    /// How long the app has been running, for the About panel.
    pub fn uptime(&self) -> std::time::Duration {
        self.started.elapsed()
    }

    /// The pane count and window count a status line shows.
    pub fn counts(&self) -> (usize, usize, usize) {
        let panes = self.panes.len();
        let tabs: usize = self.windows.iter().map(|w| w.tabs.len()).sum();
        (self.windows.len(), tabs, panes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: u64) -> PaneId {
        PaneId::new(n)
    }
    fn t(n: u64) -> TabId {
        TabId::new(n)
    }
    fn w(n: u64) -> WindowId {
        WindowId::new(n)
    }

    fn app_with_one_pane() -> App {
        let mut app = App::new();
        app.add_window(w(1), t(1), p(1));
        app
    }

    #[test]
    fn a_fresh_window_has_one_tab_and_one_pane() {
        let app = app_with_one_pane();
        assert_eq!(app.counts(), (1, 1, 1));
        assert_eq!(app.all_panes(), [p(1)]);
        assert_eq!(app.active_window().unwrap().active_tab().unwrap().focused, p(1));
    }

    #[test]
    fn a_direction_is_named_for_where_the_new_pane_goes() {
        let mut tree = Node::Leaf(p(1));
        assert!(tree.split(p(1), p(2), SplitDirection::Right, 50));
        // Right means the new pane is second in layout order.
        assert_eq!(tree.panes(), [p(1), p(2)]);

        let mut tree = Node::Leaf(p(1));
        assert!(tree.split(p(1), p(2), SplitDirection::Left, 50));
        assert_eq!(tree.panes(), [p(2), p(1)]);
    }

    #[test]
    fn splitting_right_then_down_nests_the_way_the_arrow_keys_suggest() {
        let mut tree = Node::Leaf(p(1));
        assert!(tree.split(p(1), p(2), SplitDirection::Right, 50));
        assert!(tree.split(p(2), p(3), SplitDirection::Down, 50));
        assert_eq!(tree.len(), 3);
        // Left-to-right, then top-to-bottom within each column.
        assert_eq!(tree.panes(), [p(1), p(2), p(3)]);
        match &tree {
            Node::Split { axis, first, second, .. } => {
                assert_eq!(*axis, Axis::Horizontal);
                assert!(matches!(**first, Node::Leaf(id) if id == p(1)));
                match &**second {
                    Node::Split { axis, .. } => assert_eq!(*axis, Axis::Vertical),
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn splitting_a_pane_that_is_not_there_is_refused_rather_than_orphaned() {
        let mut tree = Node::Leaf(p(1));
        assert!(!tree.split(p(99), p(2), SplitDirection::Right, 50));
        assert_eq!(tree.panes(), [p(1)]);

        let mut app = app_with_one_pane();
        assert!(!app.split_pane(p(99), p(2), SplitDirection::Right, 50));
        // The rejected pane must not have been left in the map.
        assert_eq!(app.counts(), (1, 1, 1));
        assert!(app.pane(p(2)).is_none());
    }

    #[test]
    fn closing_a_pane_promotes_its_sibling_and_collapses_the_split() {
        let mut app = app_with_one_pane();
        app.split_pane(p(1), p(2), SplitDirection::Right, 50);
        assert_eq!(app.counts().2, 2);

        let closed_tab = app.close_pane(p(1));
        assert_eq!(closed_tab, None, "one pane left, so the tab survives");
        let tab = app.active_window().unwrap().active_tab().unwrap();
        // The tree collapsed back to a single leaf, not a split with a hole.
        assert!(matches!(tab.tree, Node::Leaf(id) if id == p(2)));
        assert_eq!(app.counts().2, 1);
        // Focus moved to the survivor.
        assert_eq!(tab.focused, p(2));
    }

    #[test]
    fn closing_the_last_pane_reports_the_tab_that_emptied() {
        let mut app = app_with_one_pane();
        assert_eq!(app.close_pane(p(1)), Some(t(1)));
        assert!(app.pane(p(1)).is_none());
    }

    #[test]
    fn closing_the_focused_pane_of_three_leaves_a_connected_tree() {
        let mut app = app_with_one_pane();
        app.split_pane(p(1), p(2), SplitDirection::Right, 50);
        app.split_pane(p(2), p(3), SplitDirection::Down, 50);
        app.close_pane(p(2));
        let tab = app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(tab.tree.panes(), [p(1), p(3)]);
        assert_eq!(tab.tree.len(), 2);
        assert!(tab.tree.contains(tab.focused));
    }

    #[test]
    fn a_split_ratio_is_clamped_to_the_documented_range() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 99);
        match tree {
            Node::Split { ratio, .. } => assert_eq!(ratio, 90),
            other => panic!("{other:?}"),
        }
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 1);
        match tree {
            Node::Split { ratio, .. } => assert_eq!(ratio, 10),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn splitting_focuses_the_new_pane() {
        let mut app = app_with_one_pane();
        app.split_pane(p(1), p(2), SplitDirection::Down, 50);
        assert_eq!(app.active_window().unwrap().active_tab().unwrap().focused, p(2));
    }

    #[test]
    fn zoom_is_a_toggle_and_a_second_pane_does_not_keep_a_stale_flag() {
        let mut app = app_with_one_pane();
        app.split_pane(p(1), p(2), SplitDirection::Right, 50);
        assert!(app.toggle_zoom(p(1)));
        assert_eq!(app.active_window().unwrap().active_tab().unwrap().zoomed, Some(p(1)));
        assert!(app.pane(p(1)).unwrap().zoomed);
        assert!(app.toggle_zoom(p(1)));
        assert_eq!(app.active_window().unwrap().active_tab().unwrap().zoomed, None);
        assert!(!app.pane(p(1)).unwrap().zoomed);
    }

    #[test]
    fn closing_a_zoomed_pane_un_zooms_its_tab() {
        let mut app = app_with_one_pane();
        app.split_pane(p(1), p(2), SplitDirection::Right, 50);
        app.toggle_zoom(p(2));
        app.close_pane(p(2));
        let tab = app.active_window().unwrap().active_tab().unwrap();
        assert_eq!(tab.zoomed, None, "a zoomed pane that is gone must not stay zoomed");
    }

    #[test]
    fn a_custom_title_beats_the_shell_s_title_and_an_empty_one_restores_it() {
        let mut pane = Pane::new(p(1));
        pane.shell_title = Some("zsh".into());
        assert_eq!(pane.title(), Some("zsh"));
        pane.custom_title = Some("Build".into());
        assert_eq!(pane.title(), Some("Build"));
        pane.custom_title = Some(String::new());
        // An empty string is a title, not an absence: the API says "set to empty
        // text to restore the shell-provided title", so the caller clears it.
        assert_eq!(pane.title(), Some(""));
        pane.custom_title = None;
        assert_eq!(pane.title(), Some("zsh"));
    }

    #[test]
    fn tabs_focus_move_and_close_the_way_the_cli_describes() {
        let mut app = app_with_one_pane();
        app.add_tab(w(1), t(2), p(2));
        app.add_tab(w(1), t(3), p(3));
        let win = app.active_window_mut().unwrap();
        assert_eq!(win.tabs().len(), 3);
        assert_eq!(win.active_index(), 2);

        assert!(win.focus_tab(t(1)));
        assert_eq!(win.active_index(), 0);
        // Focusing a tab that is gone is refused rather than silently ignored.
        assert!(!win.focus_tab(t(99)));

        // Moving to position 0 puts it first.
        assert!(win.move_tab(t(1), 0));
        assert_eq!(win.tabs()[0].id, t(1));

        assert!(win.close_tab(t(1)));
        assert_eq!(win.tabs().len(), 2);
        // The last tab cannot be closed on its own.
        win.close_tab(t(2));
        assert!(!win.close_tab(t(3)), "closing the last tab is the window's business");
    }

    #[test]
    fn moving_a_tab_past_the_end_clamps_instead_of_panicking() {
        let mut app = app_with_one_pane();
        app.add_tab(w(1), t(2), p(2));
        let win = app.active_window_mut().unwrap();
        assert!(win.move_tab(t(1), 99));
        assert_eq!(win.tabs().last().unwrap().id, t(1));
    }

    #[test]
    fn panes_are_found_from_either_direction() {
        let mut app = app_with_one_pane();
        app.add_tab(w(1), t(2), p(2));
        app.split_pane(p(2), p(3), SplitDirection::Right, 50);

        assert_eq!(app.tab_of(p(3)), Some(t(2)));
        assert_eq!(app.tab_of(p(1)), Some(t(1)));
        assert_eq!(app.tab_of(p(99)), None);
        assert_eq!(app.window_of(p(3)), Some(w(1)));
        assert_eq!(app.window_of(p(99)), None);
    }

    #[test]
    fn all_panes_is_in_layout_order_across_tabs() {
        let mut app = app_with_one_pane();
        app.add_tab(w(1), t(2), p(2));
        app.split_pane(p(2), p(3), SplitDirection::Right, 50);
        assert_eq!(app.all_panes(), [p(1), p(2), p(3)]);
    }

    #[test]
    fn adding_a_tab_to_a_window_that_does_not_exist_is_refused() {
        let mut app = app_with_one_pane();
        assert!(!app.add_tab(w(99), t(2), p(2)));
        assert!(app.pane(p(2)).is_none(), "no pane may be registered for a rejected tab");
    }

    #[test]
    fn the_split_direction_names_round_trip() {
        for d in [
            SplitDirection::Right,
            SplitDirection::Left,
            SplitDirection::Up,
            SplitDirection::Down,
        ] {
            assert_eq!(SplitDirection::parse(d.as_str()), Some(d));
        }
        assert_eq!(SplitDirection::parse("top"), Some(SplitDirection::Up));
        assert_eq!(SplitDirection::parse("sideways"), None);
    }

    #[test]
    fn the_axis_follows_the_direction() {
        assert_eq!(SplitDirection::Right.axis(), Axis::Horizontal);
        assert_eq!(SplitDirection::Left.axis(), Axis::Horizontal);
        assert_eq!(SplitDirection::Up.axis(), Axis::Vertical);
        assert_eq!(SplitDirection::Down.axis(), Axis::Vertical);
    }

    #[test]
    fn set_ratio_targets_the_innermost_split_that_holds_the_pane() {
        let mut tree = Node::Leaf(p(1));
        tree.split(p(1), p(2), SplitDirection::Right, 50);
        tree.split(p(2), p(3), SplitDirection::Down, 50);
        assert!(tree.set_ratio_for(p(1), 30));
        assert!(tree.set_ratio_for(p(3), 70));
        match &tree {
            Node::Split { ratio, second, .. } => {
                assert_eq!(*ratio, 30, "the outer split took the p(1) change");
                match &**second {
                    Node::Split { ratio, .. } => {
                        assert_eq!(*ratio, 70, "the inner split took the p(3) change");
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
        // A pane that is not in the tree changes nothing.
        let before = tree.clone();
        assert!(!tree.set_ratio_for(p(99), 20));
        assert_eq!(tree, before);
    }

    #[test]
    fn uptime_and_counts_are_cheap_and_consistent() {
        let app = app_with_one_pane();
        assert!(app.uptime().as_nanos() > 0);
        let (wins, tabs, panes) = app.counts();
        assert_eq!((wins, tabs, panes), (1, 1, 1));
        assert_eq!(panes, app.all_panes().len());
    }
}
