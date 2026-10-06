//! Drop targets: window-edge zones vs the focused pane.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    Top,
    Bottom,
    Left,
    Right,
    Pane,
}

/// `edge` is the hit strip in the same units as `w`/`h`.
pub fn drop_zone(x: f32, y: f32, w: f32, h: f32, edge: f32) -> DropZone {
    if y >= h - edge {
        DropZone::Bottom
    } else if y <= edge {
        DropZone::Top
    } else if x <= edge {
        DropZone::Left
    } else if x >= w - edge {
        DropZone::Right
    } else {
        DropZone::Pane
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Dispatcher;

    #[test]
    fn drop_folder_on_bottom_opens_a_terminal_at_that_cwd() {
        assert_eq!(drop_zone(50.0, 95.0, 100.0, 100.0, 16.0), DropZone::Bottom);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        let mut d = Dispatcher::new();
        assert_eq!(d.app.counts().1, 1);
        assert!(d.drop_path(&path, DropZone::Bottom));
        assert_eq!(d.app.counts().1, 2);
        let pane = d.app.pane(d.focused_pane_id().unwrap()).unwrap();
        assert_eq!(pane.cwd.as_deref(), Some(path.as_str()));
        assert!(matches!(pane.kind, crate::model::PaneKind::Terminal));
    }
}
