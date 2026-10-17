//! The block world: a layered grid of wires, gates and chips that compiles
//! to a circuit.
//!
//! Design notes, with multiplayer and linked worlds in mind:
//!
//! - The world is a sparse map of chunks (16×16 cells per layer), so it can
//!   grow without bounds and be sent piece by piece.
//! - Every change is an [`Edit`]. Edits are plain data (serializable), can
//!   be undone, and are what a server would broadcast.
//! - [`Block::Port`] marks a named connection to the outside of the world:
//!   the world's own pins when it is packaged as a chip, and later the
//!   points where worlds link to each other.

pub mod block;
pub mod chip;
pub mod compile;
pub mod device;
pub mod draw;
pub mod region;
pub mod world;

pub use block::{Block, Dir, Gate};
pub use compile::{compile, CompileError, Compiled};
pub use device::Device;
pub use world::{Change, Edit, Pos, World};
