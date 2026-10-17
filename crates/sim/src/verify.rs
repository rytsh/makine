//! Checking a circuit against expected behaviour.

use crate::chip::{mask, Pin};
use crate::circuit::{Circuit, Oscillation};

/// One row of a test: a value for each input pin and the expected value of
/// each output pin. `None` in `expect` means "don't care".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestStep {
    pub inputs: Vec<u64>,
    pub expect: Vec<Option<u64>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// Index of the failing step.
    pub step: usize,
    pub inputs: Vec<u64>,
    pub expected: Vec<Option<u64>>,
    pub actual: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail(Failure),
    Oscillates {
        step: usize,
    },
    /// A failure explained in words, for tests that are not a table of pin
    /// values (e.g. running a program on a CPU).
    Wrong {
        step: usize,
        message: String,
    },
}

impl Verdict {
    pub fn passed(&self) -> bool {
        matches!(self, Verdict::Pass)
    }
}

/// Inputs with at most this many bits in total are tested exhaustively.
pub const EXHAUSTIVE_BITS: u32 = 18;
const SAMPLES: usize = 4096;

/// Test vectors for a combinational chip, with expected outputs computed by
/// `reference`. Small chips get every input combination, in textbook order
/// (first pin most significant). Larger ones get edge cases plus a fixed
/// pseudo-random sample.
pub fn function_table(inputs: &[Pin], reference: impl Fn(&[u64]) -> Vec<u64>) -> Vec<TestStep> {
    partial_table(inputs, |i| reference(i).into_iter().map(Some).collect())
}

/// Like [`function_table`], but the reference may answer "don't care"
/// (`None`) for some outputs.
pub fn partial_table(
    inputs: &[Pin],
    reference: impl Fn(&[u64]) -> Vec<Option<u64>>,
) -> Vec<TestStep> {
    let total: u32 = inputs.iter().map(|p| p.width as u32).sum();
    let vectors: Vec<Vec<u64>> = if total <= EXHAUSTIVE_BITS {
        (0..1u64 << total)
            .map(|row| {
                let mut shift = total;
                inputs
                    .iter()
                    .map(|p| {
                        shift -= p.width as u32;
                        row >> shift & p.mask()
                    })
                    .collect()
            })
            .collect()
    } else {
        sample_inputs(inputs)
    };
    vectors
        .into_iter()
        .map(|ins| {
            let expect = reference(&ins);
            TestStep {
                inputs: ins,
                expect,
            }
        })
        .collect()
}

fn sample_inputs(inputs: &[Pin]) -> Vec<Vec<u64>> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    // Values that tend to expose carry and sign bugs.
    let edges = |p: &Pin| {
        let m = p.mask();
        [0, 1, m, m >> 1, (m >> 1) + 1, 2].map(|v| v & m)
    };
    let mut out = Vec::with_capacity(SAMPLES);
    for i in 0..6 {
        for j in 0..6 {
            out.push(
                inputs
                    .iter()
                    .enumerate()
                    .map(|(k, p)| edges(p)[if k % 2 == 0 { i } else { j }])
                    .collect(),
            );
        }
    }
    while out.len() < SAMPLES {
        // Half the samples use small numbers: a random value cut to a random
        // number of bits. Plain random 32-bit values are almost never small,
        // so division by 3 or a shift by 1 would otherwise go untested.
        let small = out.len() % 2 == 0;
        out.push(
            inputs
                .iter()
                .map(|p| {
                    let v = rng.next_u64() & p.mask();
                    if small {
                        v & mask((rng.next_u64() % (p.width as u64 + 1)) as u8)
                    } else {
                        v
                    }
                })
                .collect(),
        );
    }
    // Every single bit on its own (and every bit cleared), so a circuit that
    // ignores some bit cannot slip through on random values alone.
    for (k, p) in inputs.iter().enumerate() {
        for bit in 0..p.width {
            for flip in [false, true] {
                let mut v: Vec<u64> = inputs.iter().map(|q| rng.next_u64() & q.mask()).collect();
                v[k] = if flip {
                    p.mask() & !(1 << bit)
                } else {
                    1 << bit
                };
                out.push(v);
            }
        }
    }
    out
}

/// Small deterministic PRNG (splitmix64), so tests are reproducible.
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn bit(&mut self) -> u64 {
        self.next_u64() & 1
    }
}

/// Check combinational behaviour. Steps are independent, so they are packed
/// into the 64 simulation lanes and checked 64 at a time. State is reset
/// before each batch.
pub fn check_combinational(circuit: &mut Circuit, inputs: &[Pin], steps: &[TestStep]) -> Verdict {
    circuit.set_native_lanes(64);
    for (batch_idx, batch) in steps.chunks(64).enumerate() {
        let base = batch_idx * 64;
        circuit.reset();
        for (pin, p) in inputs.iter().enumerate() {
            for bit in 0..p.width as usize {
                let lanes = batch.iter().enumerate().fold(0u64, |acc, (lane, s)| {
                    acc | (s.inputs[pin] >> bit & 1) << lane
                });
                circuit.set_input_lanes(pin, bit, lanes);
            }
        }
        if circuit.eval().is_err() {
            return Verdict::Oscillates { step: base };
        }
        for (lane, step) in batch.iter().enumerate() {
            let actual: Vec<u64> = (0..circuit.output_count())
                .map(|p| circuit.output_in_lane(p, lane as u32))
                .collect();
            if !matches(&step.expect, &actual) {
                return Verdict::Fail(Failure {
                    step: base + lane,
                    inputs: step.inputs.clone(),
                    expected: step.expect.clone(),
                    actual,
                });
            }
        }
    }
    Verdict::Pass
}

/// Check sequential behaviour: steps run in order and state carries over.
/// The circuit is reset once at the start.
pub fn check_sequence(circuit: &mut Circuit, steps: &[TestStep]) -> Verdict {
    circuit.set_native_lanes(1);
    circuit.reset();
    for (i, step) in steps.iter().enumerate() {
        let actual = match circuit.eval_values(&step.inputs) {
            Ok(a) => a,
            Err(Oscillation) => return Verdict::Oscillates { step: i },
        };
        if !matches(&step.expect, &actual) {
            return Verdict::Fail(Failure {
                step: i,
                inputs: step.inputs.clone(),
                expected: step.expect.clone(),
                actual,
            });
        }
    }
    Verdict::Pass
}

fn matches(expect: &[Option<u64>], actual: &[u64]) -> bool {
    expect.len() == actual.len()
        && expect
            .iter()
            .zip(actual)
            .all(|(e, a)| e.is_none_or(|e| e == *a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chip::parse_pins;

    #[test]
    fn exhaustive_order() {
        let pins = parse_pins(&["a", "b[2]"]);
        let t = function_table(&pins, |i| vec![i[0] + i[1]]);
        assert_eq!(t.len(), 8);
        assert_eq!(t[0].inputs, vec![0, 0]);
        assert_eq!(t[1].inputs, vec![0, 1]);
        assert_eq!(t[4].inputs, vec![1, 0]);
        assert_eq!(t[7].inputs, vec![1, 3]);
        assert_eq!(t[7].expect, vec![Some(4)]);
    }

    #[test]
    fn sampled_when_large() {
        let pins = parse_pins(&["a[32]", "b[32]"]);
        let t = function_table(&pins, |i| vec![i[0] ^ i[1]]);
        assert_eq!(t.len(), SAMPLES + 2 * 64);
        assert!(t
            .iter()
            .all(|s| s.inputs.iter().all(|&v| v <= u32::MAX as u64)));
        assert!(t.iter().any(|s| s.inputs == vec![u32::MAX as u64, 1]));
    }
}
