//! Native parts: chips whose behaviour is written in Rust instead of gates.
//!
//! They serve two purposes: parts that are impractical to build from gates
//! (memory with millions of bits), and fast models of chips the player has
//! already built and verified, so large circuits stay quick to simulate.
//!
//! A clocked native must behave like a master-slave flip-flop: while the
//! clock is 0 it keeps track of its inputs (in [`Behavior::commit`]), and on
//! the evaluation where the clock rises it updates its state from what it
//! saw last, before the edge. That way a register's output can feed back into
//! its own input without forming a combinational loop.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::chip::{parse_pins, Pin};

/// Behaviour of one native part in one simulation lane.
pub trait Behavior: Any + Send {
    /// Compute outputs from inputs. Inputs marked combinational in the
    /// [`NativeDef`] are up to date; the others may be stale. May be called
    /// more than once per evaluation (when the part sits on a feedback loop),
    /// so calling it again with the same inputs must give the same outputs.
    fn eval(&mut self, inputs: &[u64], outputs: &mut [u64]);

    /// Called once at the end of every evaluation, with all inputs settled.
    /// Only called for stateful parts (see [`NativeDef::stateful`]); this is
    /// where state changes, so `eval` stays repeatable.
    fn commit(&mut self, _inputs: &[u64]) {}

    /// Return to the power-on state.
    fn reset(&mut self);
}

pub type Factory = Arc<dyn Fn() -> Box<dyn Behavior> + Send + Sync>;
type Func = Arc<dyn Fn(&[u64]) -> Vec<u64> + Send + Sync>;

#[derive(Clone)]
pub struct NativeDef {
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    /// For each input: whether it affects the outputs within the same
    /// evaluation. Inputs that are only sampled on a clock edge must be
    /// `false`, otherwise every register would close a loop.
    pub comb: Vec<bool>,
    /// Whether the part has state, i.e. needs [`Behavior::commit`].
    pub stateful: bool,
    pub make: Factory,
}

impl fmt::Debug for NativeDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeDef")
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("comb", &self.comb)
            .finish_non_exhaustive()
    }
}

impl NativeDef {
    /// A combinational part computing `f`. Pins are specs, see [`Pin::parse`].
    pub fn function(
        inputs: &[&str],
        outputs: &[&str],
        f: impl Fn(&[u64]) -> Vec<u64> + Send + Sync + 'static,
    ) -> Self {
        let inputs = parse_pins(inputs);
        let outputs = parse_pins(outputs);
        let masks: Vec<u64> = outputs.iter().map(|p| p.mask()).collect();
        let f: Func = Arc::new(f);
        Self {
            comb: vec![true; inputs.len()],
            stateful: false,
            inputs,
            outputs,
            make: Arc::new(move || {
                Box::new(Function {
                    f: f.clone(),
                    masks: masks.clone(),
                })
            }),
        }
    }

    /// A part with state. `comb` names the inputs that affect outputs
    /// immediately (see [`NativeDef::comb`]).
    pub fn stateful(
        inputs: &[&str],
        outputs: &[&str],
        comb: &[&str],
        make: impl Fn() -> Box<dyn Behavior> + Send + Sync + 'static,
    ) -> Self {
        let inputs = parse_pins(inputs);
        for c in comb {
            assert!(
                inputs.iter().any(|p| &p.name == c),
                "unknown comb input {c}"
            );
        }
        let comb = inputs
            .iter()
            .map(|p| comb.contains(&p.name.as_str()))
            .collect();
        Self {
            inputs,
            outputs: parse_pins(outputs),
            comb,
            stateful: true,
            make: Arc::new(make),
        }
    }
}

struct Function {
    f: Func,
    masks: Vec<u64>,
}

impl Behavior for Function {
    fn eval(&mut self, inputs: &[u64], outputs: &mut [u64]) {
        for ((o, v), m) in outputs.iter_mut().zip((self.f)(inputs)).zip(&self.masks) {
            *o = v & m;
        }
    }

    fn reset(&mut self) {}
}

/// Edge detection for clocked natives, see the module docs.
#[derive(Default, Clone, Debug)]
pub struct ClockEdge {
    /// Clock level at the last commit.
    high: bool,
    /// Inputs at the last commit while the clock was low.
    latched: Vec<u64>,
    fired: bool,
}

impl ClockEdge {
    /// True on the first call of an evaluation in which the clock rose.
    pub fn rising(&mut self, clk: u64) -> bool {
        if clk & 1 == 1 && !self.high && !self.fired {
            self.fired = true;
            true
        } else {
            false
        }
    }

    /// Input `i` as it was just before the edge.
    pub fn before(&self, i: usize) -> u64 {
        self.latched.get(i).copied().unwrap_or(0)
    }

    /// All `n` inputs as they were just before the edge.
    pub fn before_all(&self, n: usize) -> Vec<u64> {
        (0..n).map(|i| self.before(i)).collect()
    }

    pub fn commit(&mut self, inputs: &[u64], clk: usize) {
        let high = inputs[clk] & 1 == 1;
        if !high {
            self.latched.clear();
            self.latched.extend_from_slice(inputs);
        }
        self.high = high;
        self.fired = false;
    }
}

/// A clocked part with state `S`: on each rising clock edge
/// `update(state, inputs_before_edge)` runs; outputs are always
/// `output(state, current_inputs)`.
pub struct Clocked<S> {
    pub state: S,
    init: S,
    edge: ClockEdge,
    clk: usize,
    n_inputs: usize,
    update: fn(&mut S, &[u64]),
    output: fn(&S, &[u64], &mut [u64]),
}

impl<S: Clone + Send + 'static> Behavior for Clocked<S> {
    fn eval(&mut self, inputs: &[u64], outputs: &mut [u64]) {
        if self.edge.rising(inputs[self.clk]) {
            let before = self.edge.before_all(self.n_inputs);
            (self.update)(&mut self.state, &before);
        }
        (self.output)(&self.state, inputs, outputs);
    }

    fn commit(&mut self, inputs: &[u64]) {
        self.edge.commit(inputs, self.clk);
    }

    fn reset(&mut self) {
        self.state = self.init.clone();
        self.edge = ClockEdge::default();
    }
}

impl NativeDef {
    /// A [`Clocked`] part. The clock is the input named `clk`; `comb` lists
    /// the inputs `output` reads (e.g. register file read addresses).
    pub fn clocked<S: Clone + Send + Sync + 'static>(
        inputs: &[&str],
        outputs: &[&str],
        comb: &[&str],
        init: S,
        update: fn(&mut S, &[u64]),
        output: fn(&S, &[u64], &mut [u64]),
    ) -> Self {
        let pins = parse_pins(inputs);
        let clk = pins
            .iter()
            .position(|p| p.name == "clk")
            .expect("clocked part needs clk");
        let n_inputs = pins.len();
        Self::stateful(inputs, outputs, comb, move || {
            Box::new(Clocked {
                state: init.clone(),
                init: init.clone(),
                edge: ClockEdge::default(),
                clk,
                n_inputs,
                update,
                output,
            })
        })
    }
}

/// A level-sensitive part with state `S` (a latch): outputs are
/// `output(state, inputs)` and after each evaluation `state =
/// next(state, inputs)`. All inputs are combinational.
pub struct Level<S> {
    pub state: S,
    init: S,
    output: fn(&S, &[u64], &mut [u64]),
    next: fn(&mut S, &[u64]),
}

impl<S: Clone + Send + 'static> Behavior for Level<S> {
    fn eval(&mut self, inputs: &[u64], outputs: &mut [u64]) {
        (self.output)(&self.state, inputs, outputs);
    }

    fn commit(&mut self, inputs: &[u64]) {
        (self.next)(&mut self.state, inputs);
    }

    fn reset(&mut self) {
        self.state = self.init.clone();
    }
}

impl NativeDef {
    pub fn level<S: Clone + Send + Sync + 'static>(
        inputs: &[&str],
        outputs: &[&str],
        init: S,
        output: fn(&S, &[u64], &mut [u64]),
        next: fn(&mut S, &[u64]),
    ) -> Self {
        let comb: Vec<&str> = inputs
            .iter()
            .map(|s| s.split('[').next().unwrap())
            .collect();
        Self::stateful(inputs, outputs, &comb, move || {
            Box::new(Level {
                state: init.clone(),
                init: init.clone(),
                output,
                next,
            })
        })
    }
}

/// Native register with pins `d[width] load clk -> q[width]`.
pub fn register(width: u8) -> NativeDef {
    let d = format!("d[{width}]");
    let q = format!("q[{width}]");
    NativeDef::clocked(
        &[&d, "load", "clk"],
        &[&q],
        &[],
        0u64,
        |q, i| {
            if i[1] & 1 == 1 {
                *q = i[0];
            }
        },
        |q, _, o| o[0] = *q,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chip::{into, out, ChipBuilder, Library, Part, PartKind};
    use crate::circuit::Circuit;

    /// A counter: native register whose output feeds back through a native
    /// adder. No combinational loop, and it counts once per rising edge.
    #[test]
    fn native_counter() {
        let mut lib = Library::new();
        lib.insert_native("Reg8", register(8));
        lib.insert_native(
            "Inc8",
            NativeDef::function(&["a[8]"], &["out[8]"], |i| vec![(i[0] + 1) & 0xFF]),
        );
        let mut b = ChipBuilder::new("C", &["clk"], &["q[8]"]);
        let clk = b.input("clk");
        let reg = b.part(Part::new(PartKind::Chip("Reg8".into())), &[]);
        let inc = b.chip1("Inc8", &[out(reg, 0)]);
        let one = b.constant(1, 1);
        b.wire(inc, into(reg, 0));
        b.wire(one, into(reg, 1));
        b.wire(clk, into(reg, 2));
        b.output("q", out(reg, 0));
        let mut c = Circuit::compile_def(&lib, &b.build()).unwrap();
        assert!(!c.has_feedback());
        for expect in 0..5u64 {
            assert_eq!(c.eval_values(&[0]).unwrap(), vec![expect]);
            assert_eq!(c.eval_values(&[1]).unwrap(), vec![expect + 1]);
            assert_eq!(c.eval_values(&[1]).unwrap(), vec![expect + 1]);
        }
        let reg: &mut Clocked<u64> = c.native_mut("Reg8").unwrap();
        assert_eq!(reg.state, 5);
        c.reset();
        assert_eq!(c.eval_values(&[0]).unwrap(), vec![0]);
    }

    /// The clock reaches the register through gates (NOT NOT clk), so it is
    /// only valid after the gates run. Edges must still be seen once.
    #[test]
    fn clock_through_gates() {
        let mut lib = Library::new();
        lib.insert_native("Reg8", register(8));
        let mut b = ChipBuilder::new("C", &["d[8]", "clk"], &["q[8]"]);
        let (d, clk) = (b.input("d"), b.input("clk"));
        let reg = b.part(Part::new(PartKind::Chip("Reg8".into())), &[d]);
        let n1 = b.nand(clk, clk);
        let n2 = b.nand(n1, n1);
        let one = b.constant(1, 1);
        b.wire(one, into(reg, 1));
        b.wire(n2, into(reg, 2));
        b.output("q", out(reg, 0));
        let mut c = Circuit::compile_def(&lib, &b.build()).unwrap();
        assert_eq!(c.eval_values(&[5, 0]).unwrap(), vec![0]);
        assert_eq!(c.eval_values(&[5, 1]).unwrap(), vec![5]);
        assert_eq!(c.eval_values(&[7, 1]).unwrap(), vec![5]);
        assert_eq!(c.eval_values(&[7, 0]).unwrap(), vec![5]);
        assert_eq!(c.eval_values(&[9, 1]).unwrap(), vec![7]);
    }

    /// Swapping a gate-level chip for its native model must not change
    /// behaviour, and the fast set controls which one is used.
    #[test]
    fn fast_swap() {
        let mut lib = Library::new();
        let mut b = ChipBuilder::new("Not", &["in"], &["out"]);
        let x = b.input("in");
        let n = b.nand(x, x);
        b.output("out", n);
        lib.insert(b.build());
        lib.insert_native(
            "Not",
            NativeDef::function(&["in"], &["out"], |i| vec![!i[0] & 1]),
        );

        let mut b = ChipBuilder::new("T", &["a"], &["o[4]"]);
        let a = b.input("a");
        let o = b.chip_n("Not", 4, &[a]);
        b.output("o", out(o, 0));
        let top = b.build();

        let slow = Circuit::compile_def(&lib, &top).unwrap();
        assert_eq!((slow.gate_count(), slow.native_count()), (4, 0));
        lib.set_fast("Not", true);
        let mut fast = Circuit::compile_def(&lib, &top).unwrap();
        assert_eq!((fast.gate_count(), fast.native_count()), (0, 4));
        assert_eq!(fast.eval_values(&[0]).unwrap(), vec![0xF]);
        assert_eq!(fast.eval_values(&[1]).unwrap(), vec![0]);
    }

    /// A combinational native inside a NAND feedback loop is iterated with
    /// the loop. Here a native NOT plus a NAND form an SR-style latch.
    #[test]
    fn native_in_loop() {
        let mut lib = Library::new();
        lib.insert_native(
            "Inv",
            NativeDef::function(&["in"], &["out"], |i| vec![!i[0] & 1]),
        );
        // q = NAND(!s, qn); qn = NAND(!r, q) with the inverters native.
        let mut b = ChipBuilder::new("L", &["s", "r"], &["q"]);
        let (s, r) = (b.input("s"), b.input("r"));
        let ns = b.chip1("Inv", &[s]);
        let nr = b.chip1("Inv", &[r]);
        let q = b.part(Part::new(PartKind::Nand), &[ns]);
        let qn = b.nand(nr, out(q, 0));
        b.wire(qn, into(q, 1));
        b.output("q", out(q, 0));
        let mut c = Circuit::compile_def(&lib, &b.build()).unwrap();
        assert!(c.has_feedback());
        assert_eq!(c.eval_values(&[1, 0]).unwrap(), vec![1]);
        assert_eq!(c.eval_values(&[0, 0]).unwrap(), vec![1]);
        assert_eq!(c.eval_values(&[0, 1]).unwrap(), vec![0]);
        assert_eq!(c.eval_values(&[0, 0]).unwrap(), vec![0]);
    }
}
