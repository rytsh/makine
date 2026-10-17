//! Gate-level circuit simulator for makine.
//!
//! - [`chip`]: the editable data model (chips, pins, parts, wires, library)
//! - [`circuit`]: flattening to NAND gates and evaluation
//! - [`native`]: parts implemented in Rust (memory, fast models)
//! - [`verify`]: checking a circuit against expected behaviour

pub mod chip;
pub mod circuit;
pub mod native;
pub mod verify;

pub use chip::{
    into, mask, out, parse_pins, ChipBuilder, ChipDef, Library, Part, PartId, PartKind, PartPins,
    Pin, Sink, Source, Wire, MAX_WIDTH, NAND,
};
pub use circuit::{Circuit, CompileError, Oscillation};
pub use native::Level as LevelSensitive;
pub use native::{Behavior, ClockEdge, Clocked, NativeDef};
pub use verify::{
    check_combinational, check_sequence, function_table, partial_table, Failure, Rng, TestStep,
    Verdict,
};
