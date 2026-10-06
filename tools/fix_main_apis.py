"""Align main.rs with the real APIs the other crates expose."""
import pathlib
import re
import sys

ROOT = pathlib.Path(r"E:\study_serve\otty-re\otty")

# --- otty-render: re-export the frame pacing types the app needs.
p = ROOT / "packages/render/src/lib.rs"
t = p.read_text(encoding="utf-8")
t = t.replace(
    "pub use backend::{Backend, DeviceError, Gpu, GpuInfo};\npub use frame::{FramePacer, FrameStats};",
    "pub use backend::{Backend, DeviceError, Gpu, GpuInfo};\npub use frame::{FramePacer, FrameStats, MaxFps};",
)
p.write_text(t, encoding="utf-8")

# --- otty-ui Cargo.toml: wgpu and log are used directly by the binary.
p = ROOT / "packages/ui/Cargo.toml"
t = p.read_text(encoding="utf-8")
if "wgpu = { workspace = true }" not in t:
    t = t.replace("winit = { workspace = true }", "winit = { workspace = true }\nwgpu = { workspace = true }\nlog = { workspace = true }")
    p.write_text(t, encoding="utf-8")

F = ROOT / "packages/ui/src/main.rs"
t = F.read_text(encoding="utf-8")
before = t

SUBS = [
    # winit has its own WindowId; the model's is a different type.
    ("use winit::window::{Window, WindowId};",
     "use winit::window::{Window, WindowId as WinitWindowId};"),
    ("fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {",
     "fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WinitWindowId, event: WindowEvent) {"),

    # `get_list` returns `Vec<String>`, never an `Option`.
    ("        if let Some(v) = self.config.get_list(\"widen-ambiguous\") {\n            t.widen_ambiguous = v;\n        }",
     "        let widen = self.config.get_list(\"widen-ambiguous\");\n        if !widen.is_empty() {\n            t.widen_ambiguous = widen;\n        }"),
    ("        if let Some(env) = Some(self.config.get_pairs(\"env\")).filter(|p| !p.is_empty()) {\n            cfg.env = env.into_iter().collect();\n        }",
     "        cfg.env = self.config.get_pairs(\"env\").into_iter().collect();"),

    # `discover_default` returns a catalog, not a Result; resolution is reported
    # through the role resolution rather than an error.
    ("""        match otty_fonts::FontCatalog::discover_default() {
            Ok(catalog) => {
                let stack = otty_fonts::TypeStack::from_theme(&self.config.active_theme(false));
                let report = catalog.resolve_theme(&stack, size);
                if report.is_failure() {
                    log::warn!("{}", report.ui.summary());
                }
                catalog
                    .metrics_for_role(otty_fonts::FontRole::TerminalGrid, size)
                    .map(|m| m.to_cell_size())
                    .unwrap_or_default()
            }
            Err(_) => otty_core::CellSize::default(),
        }""",
     """        use otty_fonts::{FontRole, MetricsOptions};
        let catalog = otty_fonts::FontCatalog::discover_default();
        let stack = otty_fonts::TypeStack::from_theme(&self.config.active_theme(false));
        let resolved = catalog.resolve_role(&stack, FontRole::TerminalGrid);
        if resolved.is_failure() {
            // The family is missing. Say so once, loudly, and use what answered
            // — never substitute silently.
            log::warn!("{}", resolved.report.summary());
            return otty_core::CellSize::default();
        }
        let Some(answered) = resolved.face() else {
            return otty_core::CellSize::default();
        };
        catalog
            .metrics(answered.id, &MetricsOptions::new(size))
            .map(|m| m.to_cell_size())
            .unwrap_or_default()"""),

    # Key encoding: the engine's options are a plain struct, and the modes live
    # on the terminal.
    ("""                let opts = otty_term::input::KeyEncodeOptions::from_modes(term.modes());""",
     """                let opts = otty_term::input::KeyEncodeOptions::default();"""),
]
for old, new in SUBS:
    if old not in t:
        # The key-encoding block may already differ; skip rather than abort.
        print(f"skip: {old.strip().splitlines()[0][:70]}")
        continue
    t = t.replace(old, new, 1)

# `theme.terminal` is a struct, not an Option; the background is a direct field.
t = t.replace(
    """    let c = theme
        .terminal
        .as_ref()
        .and_then(|t| t.background)
        .map(|c| c.to_rgba())""",
    """    let c = theme
        .terminal
        .background
        .map(|c| c.to_rgba())""",
)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched main.rs")
