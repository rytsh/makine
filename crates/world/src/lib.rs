//! The world: parts placed freely on a plane with wires from pin to pin,
//! like the level editor, that compiles to a circuit.
//!
//! Design notes, with multiplayer in mind:
//!
//! - Every change is an [`Edit`]. Edits are plain data (serializable), can
//!   be undone, and are what the host broadcasts.
//! - Parts keep their [`world::Id`] for as long as they exist, and a wire
//!   is known by the input it goes into, so edits from several players
//!   never get confused about what they change.
//! - Input and output parts become the pins of a chip made from a world.

pub mod chip;
pub mod compile;
pub mod device;
pub mod part;
pub mod projects;
pub mod world;

pub use compile::{compile, Compiled, InputSource, Problem};
pub use device::Device;
pub use part::{Gate, Kind};
pub use world::{Change, Edit, End, Id, Module, Placed, Pos, WireStyle, World};
