//! Chips as Verilog, so a design built in makine can leave the simulator:
//! run in Icarus Verilog, be synthesized by Yosys, go onto an FPGA or be
//! made into a real chip through Tiny Tapeout.
//!
//! Every chip becomes one module, keeping the hierarchy the player built.
//! NAND gates become `~(a & b)`; splits, joins, constants and repeated parts
//! become bit slices. Nothing is simplified: the netlist is the one makine
//! simulates.
//!
//! - [`verilog`]: the modules of a chip and everything it uses
//! - [`bench`]: testbenches, from a level's tests or from makine's own
//!   simulation
//! - [`project`]: folders to hand to the tools (Icarus/Yosys, Tiny Tapeout)
//! - [`fpga`]: a whole RISC-V computer for an FPGA board around the
//!   player's CPU
//!
//! **Storage.** A flip-flop built from NAND gates is a combinational loop.
//! Simulators handle that, but synthesis and timing tools for FPGAs and
//! silicon do not. With [`Options::primitives`], the storage chips of the
//! learning path (`Latch`, `DLatch`, `DFF`) are written as real latches and
//! flip-flops instead of their gates. The player's version passed the same
//! level test, so the circuit around them behaves the same.

pub mod bench;
pub mod fpga;
mod ident;
pub mod project;
pub mod verilog;

pub use bench::{golden_steps, testbench};
pub use fpga::{fpga_project, hex_image};
pub use project::{tiny_tapeout, verilog_project, Project, TinyTapeout};
pub use verilog::{verilog, Export};

/// How to write a chip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Write `Latch`, `DLatch` and `DFF` as real storage elements instead
    /// of NAND loops. Needed for synthesis; see the crate documentation.
    pub primitives: bool,
    /// Put before every module name, so the modules do not clash with
    /// others in a larger design.
    pub prefix: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            primitives: true,
            prefix: "mk_".into(),
        }
    }
}
