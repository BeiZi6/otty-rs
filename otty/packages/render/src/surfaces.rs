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
}
