"""Scaffold the Otty workspace package manifests and stub source files.

Run once. After this the packages are filled in by hand; the script is kept so
the manifest set can be regenerated if a package is added.
"""
from pathlib import Path

ROOT = Path(r"E:\study_serve\otty-re\otty")

COMMON = """version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
authors.workspace = true
"""

# name, description, [dependencies block], is_binary
PKGS = [
    ("config", "otty-config",
     "Configuration: the 236-key config.toml schema, themes, recipes and import/export.",
     """otty-core = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
toml_edit = { workspace = true }
thiserror = { workspace = true }
regex = { workspace = true }
dirs = { workspace = true }
log = { workspace = true }
aho-corasick = { workspace = true }
glob = { workspace = true }
""", False),

    ("term", "otty-term",
     "The VT engine: escape-sequence parser, grid, scrollback, reflow and selection.",
     """otty-core = { workspace = true }
vte = { workspace = true }
unicode-width = { workspace = true }
unicode-segmentation = { workspace = true }
unicode-bidi = { workspace = true }
log = { workspace = true }
""", False),

    ("pty", "otty-pty",
     "Pseudo-terminal and child-process management on top of portable-pty.",
     """otty-core = { workspace = true }
portable-pty = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }
base64 = { workspace = true }

[target.'cfg(unix)'.dependencies]
libc = { workspace = true }
""", False),

    ("state", "otty-state",
     "SQLite-backed persistence: session history, frecency, agent conversations.",
     """otty-core = { workspace = true }
rusqlite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
jiff = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }
""", False),

    ("ipc", "otty-ipc",
     "The control channel between the CLI, agent hooks and the running app.",
     """otty-core = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }

[target.'cfg(unix)'.dependencies]
libc = { workspace = true }
""", False),

    ("fonts", "otty-fonts",
     "Font discovery, the Anthropic type stack, shaping and glyph rasterisation.",
     """otty-core = { workspace = true }
fontdb = { workspace = true }
swash = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }
serde = { workspace = true }
""", False),

    ("agent", "otty-agent",
     "Coding-agent integrations: hooks, lifecycle state, history and resume.",
     """otty-core = { workspace = true }
otty-config = { workspace = true }
otty-state = { workspace = true }
otty-ipc = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
log = { workspace = true }
jiff = { workspace = true }
""", False),

    ("autocomplete", "otty-autocomplete",
     "Fig-compatible spec database, inline suggestions and on-device learning.",
     """otty-core = { workspace = true }
otty-config = { workspace = true }
otty-state = { workspace = true }
rusqlite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
regex = { workspace = true }
log = { workspace = true }
""", False),

    ("ssh", "otty-ssh",
     "SSH integration: argv scanning, ssh_config resolution, remote helper install.",
     """otty-core = { workspace = true }
otty-config = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }
regex = { workspace = true }
log = { workspace = true }
""", False),

    ("render", "otty-render",
     "The GPU renderer: wgpu pipelines, glyph atlas, box-drawing geometry.",
     """otty-core = { workspace = true }
otty-term = { workspace = true }
otty-fonts = { workspace = true }
wgpu = { workspace = true }
bytemuck = { workspace = true }
pollster = { workspace = true }
image = { workspace = true }
log = { workspace = true }
""", False),

    ("ui", "otty-ui",
     "The application shell: windows, tabs, panes, details panel and settings.",
     """otty-core = { workspace = true }
otty-config = { workspace = true }
otty-term = { workspace = true }
otty-pty = { workspace = true }
otty-state = { workspace = true }
otty-ipc = { workspace = true }
otty-fonts = { workspace = true }
otty-agent = { workspace = true }
otty-autocomplete = { workspace = true }
otty-ssh = { workspace = true }
otty-render = { workspace = true }
winit = { workspace = true }
pollster = { workspace = true }
arboard = { workspace = true }
rfd = { workspace = true }
image = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
jiff = { workspace = true }
notify = { workspace = true }
log = { workspace = true }
crossbeam-channel = { workspace = true }
which = { workspace = true }
walkdir = { workspace = true }
regex = { workspace = true }
unicode-segmentation = { workspace = true }
""", False),

    ("cli", "otty-cli",
     "The `otty` command-line tool: config, fonts, themes, window/tab/pane control.",
     """otty-core = { workspace = true }
otty-config = { workspace = true }
otty-term = { workspace = true }
otty-state = { workspace = true }
otty-ipc = { workspace = true }
otty-fonts = { workspace = true }
otty-agent = { workspace = true }
otty-autocomplete = { workspace = true }
otty-ssh = { workspace = true }
clap = { workspace = true }
clap_complete = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml_edit = { workspace = true }
rusqlite = { workspace = true }
qrcode = { workspace = true }
regex = { workspace = true }
anyhow = { workspace = true }
log = { workspace = true }
env_logger = { workspace = true }
which = { workspace = true }
url = { workspace = true }
walkdir = { workspace = true }
tempfile = { workspace = true }
unicode-width = { workspace = true }
""", True),

    ("helper", "otty-helper",
     "The POSIX helper Otty uploads to remote hosts for stats and file transfer.",
     """otty-core = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
log = { workspace = true }
""", True),
]

for dirname, crate, desc, deps, is_bin in PKGS:
    pkg = ROOT / "packages" / dirname
    (pkg / "src").mkdir(parents=True, exist_ok=True)
    manifest = f"""[package]
name = "{crate}"
description = "{desc}"
{COMMON}
[dependencies]
{deps}
"""
    (pkg / "Cargo.toml").write_text(manifest, encoding="utf-8")
    entry = "main.rs" if is_bin else "lib.rs"
    target = pkg / "src" / entry
    if not target.exists():
        if is_bin:
            body = "//! " + desc + "\n\nfn main() {\n"
            body += '    println!("' + crate + ' {}", otty_core::VERSION);\n}\n'
            target.write_text(body, encoding="utf-8")
        else:
            target.write_text(f"//! {desc}\n", encoding="utf-8")
    print(f"{'bin ' if is_bin else 'lib '} packages/{dirname}  ->  {crate}")

print(f"\n{len(PKGS)} packages scaffolded under {ROOT / 'packages'}")
