use soroban_sdk::{Bytes, Env, Map};

use crate::{DrawerDataKey, DrawerState, Store};

pub(crate) struct DrawerCache {
    pub(crate) drawers: Map<u32, Bytes>,
    pub(crate) state: Option<DrawerState>,
    pub(crate) state_dirty: bool,
    pub(crate) dirty_drawers: Map<u32, bool>,
}

impl DrawerCache {
    pub(crate) fn new(e: &Env) -> Self {
        Self {
            drawers: Map::new(e),
            state: None,
            state_dirty: false,
            dirty_drawers: Map::new(e),
        }
    }

    pub(crate) fn commit(&self, e: &Env) {
        if self.state_dirty {
            if let Some(ref state) = self.state {
                e.storage().persistent().set(&DrawerDataKey::State, state);
            }
        }

        for (drawer_id, _) in self.dirty_drawers.iter() {
            if let Some(bitmap) = self.drawers.get(drawer_id) {
                e.storage()
                    .persistent()
                    .set(&Store::drawer_key(drawer_id), &bitmap);
            }
        }
    }
}
