//! The application shell: windows, tabs, panes, and the event loop that drives
//! them.
//!
//! Split in two on purpose:
//!
//! * [`model`] is the tree of what exists and where it goes. It has no
//!   dependency on `winit`, `wgpu`, or threads, so it is the part a test drives.
//! * The event loop and the GPU only ever *ask* the model questions; they do not
//!   own its invariants.
//!
//! That separation is what makes the split/close/zoom rules testable at all —
//! "closing the focused pane of three leaves a connected tree" is a property of
//! the model, and asserting it against a windowing system would be a worse test
//! in every way.

pub mod context;
pub mod details;
pub mod dispatch;
pub mod palette;
pub mod pane_view;
pub mod quickly;
pub mod settings;
pub mod layout;
pub mod session;
pub mod syntax;
pub mod tabs;
pub mod view;
pub mod model;

pub use context::{context_items, context_rows, pane_hit, tab_hit, ContextHit, ContextItem};
pub use details::{details_info, DetailsInfo, DetailsTab};
pub use dispatch::{shell_menu, shell_menu_models, Dispatcher};
pub use palette::{filter_palette, palette_entries, palette_rows, PaletteEntry};
pub use quickly::{
    open_directory, query_quickly, quickly_rows, OpenFilter, QuicklyEntry, QuicklySources,
};
pub use settings::{set_setting, show_origin};
pub use view::cell_size_for_font;
pub use layout::{cell_at, grid_dimensions, layout, layout_single, Divider, Layout, LayoutOptions};
pub use session::Session;
pub use view::{pty_config, term_config, TerminalView, ViewError};
pub use model::{App, Axis, Badge, Node, Pane, PaneKind, SplitDirection, Tab, Window};
pub use pane_view::{load_view, view_target_from_args};
