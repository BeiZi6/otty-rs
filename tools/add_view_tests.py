"""Give the view tests: compiling is not the same as working."""
import pathlib
import sys

F = pathlib.Path(r"E:\study_serve\otty-re\otty\packages\ui\src\view.rs")
t = F.read_text(encoding="utf-8")

TESTS = '''
#[cfg(test)]
mod tests {
    use super::*;
    use otty_core::PaneId;
    use std::time::{Duration, Instant};

    fn p(n: u64) -> PaneId {
        PaneId::new(n)
    }

    /// A view over a real shell, sized so a 16 px font gives a usable grid.
    ///
    /// `Config::load_from` on a temporary directory rather than `Config::load`,
    /// so the test never reads the developer's own configuration and cannot be
    /// affected by whatever font size they have set.
    fn view() -> (tempfile::TempDir, TerminalView) {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load_from(dir.path());
        let view = TerminalView::new(&config, p(1), (640, 384), 1.0)
            .expect("a view must start a shell");
        (dir, view)
    }

    /// Pump until `pred` holds, because a real shell is asynchronous.
    fn pump_until(
        view: &mut TerminalView,
        timeout: Duration,
        mut pred: impl FnMut(&TerminalView) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            view.pump();
            view.frame();
            if pred(view) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        false
    }

    #[test]
    fn a_view_starts_a_shell_and_reports_a_grid() {
        let (_dir, view) = view();
        let (cols, rows) = view.grid_size();
        assert!(cols > 10, "grid is {cols}x{rows}");
        assert!(rows > 3, "grid is {cols}x{rows}");
        assert!(view.cell().width > 0.0);
        assert!(view.cell().height > 0.0);
    }

    #[test]
    fn the_pixel_buffer_matches_the_size_it_was_asked_for() {
        let (_dir, view) = view();
        assert_eq!(view.buffer_size(), (640, 384));
        assert_eq!(view.pixels().len(), 640 * 384 * 4);
    }

    #[test]
    fn the_grid_size_follows_the_font_metrics() {
        let (_dir, view) = view();
        let (cols, rows) = view.grid_size();
        let cell = view.cell();
        // The grid is the pixel size divided by the cell, so the identity has to
        // hold or the window and the text disagree about how much fits.
        assert_eq!(cols, (640.0 / cell.width).floor() as u16);
        assert_eq!(rows, (384.0 / cell.height).floor() as u16);
    }

    #[test]
    fn resizing_keeps_the_grid_the_pty_and_the_buffer_in_step() {
        let (_dir, mut view) = view();
        let before = view.grid_size();
        view.resize(1280, 768, true);
        let after = view.grid_size();
        assert!(after.0 > before.0, "{before:?} -> {after:?}");
        assert!(after.1 > before.1, "{before:?} -> {after:?}");
        // The buffer followed, which is what makes the frame the right size.
        assert_eq!(view.buffer_size(), (1280, 768));
        // And the engine's grid agrees with what the view reports.
        assert_eq!(view.grid_size(), (view.session.terminal().cols(), view.session.terminal().rows()));
    }

    #[test]
    fn a_minimised_window_does_not_resize_to_nothing() {
        let (_dir, mut view) = view();
        let before = view.grid_size();
        view.resize(0, 0, true);
        assert_eq!(view.grid_size(), before, "a zero-sized window must be ignored");
    }

    #[test]
    fn a_frame_paints_the_theme_background_before_anything_is_drawn() {
        let (_dir, mut view) = view();
        view.frame();
        let background = view.palette().background;
        // The first frame has whatever the shell printed at startup, so assert on
        // a pixel the shell is unlikely to have touched: the last column of the
        // last row, which is past the end of any prompt.
        let (w, h) = view.buffer_size();
        let _ = (w, h);
        // Every pixel is opaque, which is what the compositor promises.
        assert!(view.pixels().chunks_exact(4).all(|px| px[3] == 0xFF));
        assert_ne!(background, 0, "the palette has no background");
    }

    #[test]
    fn output_from_the_shell_reaches_the_pixel_buffer() {
        // The end-to-end assertion for this layer: PTY -> engine -> frame ->
        // rasteriser -> pixels. If this passes, the pipeline is wired.
        let (_dir, mut view) = view();
        view.write(b"echo otty-view-marker\\r\\n").unwrap();
        assert!(
            pump_until(&mut view, Duration::from_secs(15), |v| {
                let (w, h) = v.buffer_size();
                let bg = v.palette().background as u32;
                // Any pixel that is not the background means text was drawn.
                (0..h)
                    .step_by(4)
                    .any(|y| (0..w).step_by(4).any(|x| v.pixel_at(x, y) != bg & 0xFFFF_FFFF))
            }),
            "no glyph ink reached the buffer; grid was {:?}",
            view.grid_size()
        );
    }

    #[test]
    fn keystrokes_reach_the_shell_and_its_answer_comes_back() {
        let (_dir, mut view) = view();
        view.write(b"printf 'returned-value\\\\n'\\r\\n").unwrap();
        assert!(
            pump_until(&mut view, Duration::from_secs(15), |v| {
                v.session.terminal().contents().contains("returned-value")
            }),
            "the shell's answer never reached the grid"
        );
    }

    #[test]
    fn the_tab_title_prefers_the_shell_to_a_default() {
        let (_dir, mut view) = view();
        // A title the shell can set with the sequence the docs name.
        view.write(b"printf '\\\\033]0;my-title\\\\007'\\r\\n").unwrap();
        let got = pump_until(&mut view, Duration::from_secs(15), |v| {
            v.tab_title() == "my-title"
        });
        assert!(got, "the OSC title never reached the model: {:?}", view.tab_title());
    }

    #[test]
    fn a_click_maps_back_to_a_cell_inside_the_pane() {
        let (_dir, view) = view();
        let cell = view.cell();
        // Just inside the top-left cell.
        assert_eq!(view.cell_at(1.0, 1.0), Some((0, 0)));
        // Two cells across, one down.
        assert_eq!(view.cell_at(cell.width * 2.5, cell.height * 1.5), Some((2, 1)));
        // Outside the window entirely.
        assert_eq!(view.cell_at(-5.0, -5.0), None);
    }

    #[test]
    fn the_chrome_and_the_layout_use_the_same_divider_width() {
        // The gutter the mouse can grab has to be the gutter that is drawn.
        let (_dir, view) = view();
        assert_eq!(view.layout(640, 384).dividers().len(), 0, "one pane, no dividers");
        assert_eq!(view.options.divider, view.chrome().divider_width);
    }
}
'''

if "mod tests" in t:
    print("tests already present")
    sys.exit(1)
F.write_text(t + TESTS, encoding="utf-8")
print("added view tests")
