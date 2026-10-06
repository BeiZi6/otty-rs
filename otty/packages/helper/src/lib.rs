//! `otty-helper` — the small static binary Otty uploads to a remote host.
//!
//! The SSH integration works by copying this file to the far end and running it
//! over the existing connection, so it has three jobs and a hard constraint:
//!
//! * **report** what the host looks like — CPU, memory, disk, network, load;
//! * **move files** for the remote file pane, in both directions;
//! * **install the shell integration** on the far end, and undo it cleanly;
//! * and it must be **statically linked against musl** on Linux, because the
//!   server's libc is whatever the distribution happens to ship. That is exactly
//!   why the shipped bundle carries `aarch64-unknown-linux-musl` and
//!   `x86_64-unknown-linux-musl` builds of this program and nothing else for
//!   Linux.
//!
//! # Protocol
//!
//! One JSON object per line, request then response, over stdin/stdout. It is
//! deliberately boring: the transport is an SSH exec channel, so the only things
//! available are two byte streams, and a self-describing line protocol survives
//! a login shell banner, a `$PS1`, and a `motd`. Verbose logs go to stderr, never
//! stdout, so a chatty remote rc file cannot corrupt the stream.

pub mod host;
pub mod protocol;

pub use host::{HostIdentity, HostStats, HostTokens};
pub use protocol::{Request, Response, RpcError, PROTOCOL_VERSION};

/// The helper's own version, so the local side can tell it needs re-uploading.
pub const VERSION: &str = otty_core::VERSION;
