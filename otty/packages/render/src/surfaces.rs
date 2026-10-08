//! One render target per pane, so a split/PiP/freeze can act on a single layer.

use std::collections::BTreeMap;

use otty_core::PaneId;

#[derive(Debug, Clone)]
pub struct PaneSurface {
    pub id: PaneId,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub frozen: bool,
}

#[derive(Debug, Default)]
pub struct PaneSurfaces {
    surfaces: BTreeMap<u64, PaneSurface>,
}

impl PaneSurfaces {
    pub fn ensure(&mut self, id: PaneId, width: u32, height: u32) -> &mut PaneSurface {
        self.surfaces.entry(id.get()).or_insert_with(|| PaneSurface {
            id,
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
            frozen: false,
        })
    }

    pub fn freeze(&mut self, id: PaneId) -> bool {
        if let Some(s) = self.surfaces.get_mut(&id.get()) {
            s.frozen = true;
            true
        } else {
            false
        }
    }

    /// Lift a pane out (PiP). The tab keeps the other surfaces.
    pub fn extract(&mut self, id: PaneId) -> Option<PaneSurface> {
        self.surfaces.remove(&id.get())
    }

    pub fn get(&self, id: PaneId) -> Option<&PaneSurface> {
        self.surfaces.get(&id.get())
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// `freeze-inactive-tab=true`: drop GPU pixels for every pane except `active`.
    pub fn freeze_inactive(&mut self, active: PaneId) {
        for s in self.surfaces.values_mut() {
            if s.id != active {
                s.frozen = true;
                s.pixels.clear();
            }
        }
    }

    /// Recreate the surface so the first frame after a tab switch can draw.
    pub fn thaw(&mut self, id: PaneId) {
        if let Some(s) = self.surfaces.get_mut(&id.get()) {
            s.frozen = false;
            let n = (s.width * s.height * 4) as usize;
            if s.pixels.len() != n {
                s.pixels = vec![0; n];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_splits_have_their_own_targets_and_one_can_freeze_or_extract() {
        let mut s = PaneSurfaces::default();
        let a = PaneId::new(1);
        let b = PaneId::new(2);
        s.ensure(a, 40, 20);
        s.ensure(b, 40, 20);
        assert_eq!(s.len(), 2);
        assert!(!s.get(a).unwrap().frozen);
        assert!(s.freeze(a));
        assert!(s.get(a).unwrap().frozen);
        assert!(!s.get(b).unwrap().frozen);
        let pip = s.extract(a).expect("lift pane a");
        assert!(pip.frozen);
        assert_eq!(s.len(), 1);
        assert!(s.get(b).is_some());
    }

    #[test]
    fn freeze_inactive_releases_gpu_and_thaw_draws_the_first_frame() {
        let mut s = PaneSurfaces::default();
        let a = PaneId::new(1);
        let b = PaneId::new(2);
        s.ensure(a, 8, 8);
        s.ensure(b, 8, 8);
        s.freeze_inactive(a);
        assert!(s.get(b).unwrap().frozen);
        assert!(s.get(b).unwrap().pixels.is_empty(), "idle tab must not hold a GPU surface");
        assert!(!s.get(a).unwrap().pixels.is_empty());
        s.thaw(b);
        assert!(!s.get(b).unwrap().frozen);
        assert_eq!(s.get(b).unwrap().pixels.len(), 8 * 8 * 4, "first frame can draw");
    }
}
