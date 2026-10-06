"""Move the slint lines into [workspace.dependencies] exactly once.

Repeated regex rewrites accumulated duplicate keys; this rebuilds the file
deterministically: strip every existing slint line, then insert both immediately
before the first section that follows `[workspace.dependencies]`.
"""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\Cargo.toml")
lines = F.read_text(encoding="utf-8").splitlines()

SLINT = (
    '# The Slint shell\'s UI toolkit. `femtovg` rather than `wgpu`: this build already\n'
    '# composites the grid on the CPU into an RGBA buffer (`otty-render::raster`), so\n'
    '# the toolkit only has to put that buffer on screen — a second graphics stack\n'
    '# would buy nothing, and femtovg\'s software path also works where no GPU exists.\n'
    'slint = { version = "1.8", default-features = false, features = ["compat-1-2", "std", "backend-winit", "renderer-femtovg"] }\n'
    '# The layout compiler, run from `build.rs` so a layout error is a build error.\n'
    'slint-build = "1.8"\n'
)

# Drop every existing slint line, wherever it landed.
body = [l for l in lines if not l.startswith("slint = ") and not l.startswith("slint-build = ")]

# Find the end of [workspace.dependencies] — the first following section header.
try:
    start = body.index("[workspace.dependencies]")
except ValueError:
    print("[workspace.dependencies] not found")
    sys.exit(1)

insert_at = len(body)
for i in range(start + 1, len(body)):
    if body[i].startswith("[") and body[i].endswith("]"):
        insert_at = i
        break

out = body[:insert_at] + SLINT.rstrip("\n").splitlines() + [""] + body[insert_at:]
text = "\n".join(out).rstrip("\n") + "\n"
F.write_text(text, encoding="utf-8")

for i, l in enumerate(text.splitlines(), 1):
    if l.startswith("slint") or l.startswith("[profile"):
        print(i, l[:70])
print("done")
