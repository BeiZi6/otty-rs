"""Final four errors in main.rs."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\main.rs")
t = F.read_text(encoding="utf-8")
before = t

SUBS = [
    # `face()` returns the id, not a struct holding one.
    ("""        let Some(answered) = resolved.face() else {
            return otty_core::CellSize::default();
        };
        catalog
            .metrics(answered.id, &MetricsOptions::new(size))""",
     """        let Some(face) = resolved.face() else {
            return otty_core::CellSize::default();
        };
        catalog
            .metrics(face, &MetricsOptions::new(size))"""),

    # `encode_key` takes the event by value and the options by reference.
    ("""                if let Some(bytes) = otty_term::input::encode_key(&event, opts) {""",
     """                if let Some(bytes) = otty_term::input::encode_key(
                    otty_term::KeyEvent::from_winit(&event),
                    &opts,
                ) {"""),

    # wgpu 26 added `depth_slice` to the colour attachment.
    ("""                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,""",
     """                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,"""),

    # `Color` exposes hex, so the clear colour is read back from it. A theme
    # background may be `none` (the two glass themes), and that is a transparent
    # clear rather than a black one.
    ("""    let c = theme
        .terminal
        .background
        .map(|c| c.to_rgba())
        // A theme with no background is transparent-on-purpose (the two glass
        // themes); presenting opaque black would be a visible lie, so the surface
        // clears to fully transparent and the compositor shows through.
        .unwrap_or((0.0, 0.0, 0.0, 0.0));
    wgpu::Color { r: c.0 as f64, g: c.1 as f64, b: c.2 as f64, a: c.3 as f64 }""",
     """    let Some(color) = theme.terminal.background else {
        // A theme with no background is transparent on purpose — the two glass
        // themes. Clearing to opaque black would be a visible lie, so the surface
        // clears to fully transparent and the compositor shows through.
        return wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    };
    rgba_from_hex(&color.to_hex())"""),

    # The hex parser, added next to `background`.
    ("""/// `scrollback-limit` accepts `"100 MB"`-style values.""",
     """/// `#RRGGBB` or `#RRGGBBAA` to the linear-normalised floats a surface wants.
fn rgba_from_hex(hex: &str) -> wgpu::Color {
    let h = hex.trim_start_matches('#');
    let byte = |i: usize| -> f64 {
        u8::from_str_radix(&h.get(i * 2..i * 2 + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 / 255.0
    };
    wgpu::Color {
        r: byte(0),
        g: byte(1),
        b: byte(2),
        // `#RRGGBB` has no alpha and means opaque; `#RRGGBBAA` carries its own.
        a: if h.len() >= 8 { byte(3) } else { 1.0 },
    }
}

/// `scrollback-limit` accepts `"100 MB"`-style values."""),
]

for old, new in SUBS:
    if old not in t:
        print(f"MISS: {old.strip().splitlines()[0][:70]}")
        sys.exit(1)
    t = t.replace(old, new, 1)

if t == before:
    print("nothing changed")
    sys.exit(1)
F.write_text(t, encoding="utf-8")
print("patched main.rs")
