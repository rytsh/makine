//! Reproducible workshop projects: real circuits, not decorative pictures.
pub mod examples;
pub mod mcs4;

use crate::{End, Id, World};
use makine_sim::ChipDef;

pub struct Project {
    pub world: World,
    pub chips: Vec<ChipDef>,
    pub watches: Vec<End>,
    pub collapsed: Vec<Id>,
}
