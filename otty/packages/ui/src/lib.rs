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

pub mod layout;
pub mod session;
pub mod view;
pub mod model;

pub use layout::{cell_at, grid_dimensions, layout, layout_single, Divider, Layout, LayoutOptions};
pub use session::Session;
pub use view::{pty_config, term_config, TerminalView, ViewError};
pub use model::{App, Axis, Badge, Node, Pane, SplitDirection, Tab, Window};
