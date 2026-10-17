//! Running a world: its compiled circuit, the clock and the speed.
//!
//! The circuit is rebuilt whenever the world changes, keeping what devices
//! hold (memory, screen, keys). Large worlds are rebuilt only when asked
//! (at the end of a drag), see [`WorldSim::defer`].

use std::time::{Duration, Instant};

use makine_sim::{Circuit, Library};
use makine_world::compile::{check_chips, compile, CompileError, Compiled, InputSource};
use makine_world::device;
use makine_world::{Block, Pos, World};

/// Why a world cannot run.
#[derive(Clone, Debug, PartialEq)]
pub enum SimError {
    /// The world itself is wrong; short circuits point at their drivers.
    Compile(CompileError),
    /// Chip blocks do not match the library, or the circuit cannot be built.
    Chips(String),
    /// The circuit never settles (e.g. a NOT feeding itself).
    Oscillates,
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::Compile(e) => write!(f, "{e}"),
            SimError::Chips(e) => write!(f, "{e}"),
            SimError::Oscillates => write!(
                f,
                "devre salınıyor: bir çıkış kendi girişini ters çeviriyor (ör. kendine bağlı NOT)"
            ),
        }
    }
}

pub struct WorldSim {
    circuit: Option<Result<(Compiled, Circuit), SimError>>,
    /// World revision the circuit was built from.
    revision: u64,
    /// The player's chips, for chip blocks.
    lib: Library,
    /// Switch positions, refreshed with the circuit (switches only change
    /// through edits).
    switches: Vec<(usize, Pos)>,
    /// Steps since start, for clocks.
    pub tick: u64,
    pub running: bool,
    /// Steps per frame; 0 runs as many as fit in the frame budget.
    pub speed: u32,
    /// Steps per second, measured.
    pub rate: f64,
    rate_window: (Instant, u64),
    step_once: bool,
    /// Every chip output after the last step.
    values: Vec<u64>,
    /// The circuit stopped settling at the last step.
    oscillating: bool,
    compile_cost: Duration,
    /// Keys to deliver to a keyboard device: (native name, key).
    keys: Vec<(String, u8)>,
}

impl Default for WorldSim {
    fn default() -> Self {
        Self {
            circuit: None,
            revision: u64::MAX,
            lib: Library::new(),
            switches: Vec::new(),
            tick: 0,
            running: true,
            speed: 1,
            rate: 0.0,
            rate_window: (Instant::now(), 0),
            step_once: false,
            values: Vec::new(),
            oscillating: false,
            compile_cost: Duration::ZERO,
            keys: Vec::new(),
        }
    }
}

impl WorldSim {
    pub fn set_library(&mut self, lib: Library) {
        self.lib = lib;
        self.revision = u64::MAX;
    }

    /// Whether rebuilding is slow enough to wait for the end of a drag.
    pub fn slow(&self) -> bool {
        self.compile_cost > Duration::from_millis(15)
    }

    /// The circuit for `world`, rebuilt if the world changed (unless
    /// `defer`). Device contents carry over to the new circuit.
    pub fn circuit(&mut self, world: &World, defer: bool) -> Option<&mut (Compiled, Circuit)> {
        if self.revision != world.revision() && !(defer && self.circuit.is_some()) {
            let started = Instant::now();
            self.revision = world.revision();
            let saved = self.saved_devices();
            self.circuit = Some(self.build(world).map(|(c, mut sim)| {
                for (name, s) in saved {
                    device::restore(&mut sim, &name, s);
                }
                (c, sim)
            }));
            self.switches = self
                .compiled()
                .map(|c| {
                    c.inputs
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| match s {
                            InputSource::Switch(p) => Some((i, *p)),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default();
            self.compile_cost = started.elapsed();
        }
        self.circuit.as_mut()?.as_mut().ok()
    }

    fn build(&self, world: &World) -> Result<(Compiled, Circuit), SimError> {
        let c = compile(world).map_err(SimError::Compile)?;
        check_chips(world, &self.lib).map_err(SimError::Chips)?;
        let sim = Circuit::compile_def(&c.library_with(&self.lib), &c.def)
            .map_err(|e| SimError::Chips(e.to_string()))?;
        Ok((c, sim))
    }

    fn saved_devices(&mut self) -> Vec<(String, device::Saved)> {
        let Some(Ok((c, sim))) = &mut self.circuit else {
            return Vec::new();
        };
        c.devices
            .iter()
            .filter_map(|d| d.native.as_ref())
            .filter_map(|n| Some((n.clone(), device::save(sim, n)?)))
            .collect()
    }

    /// The compiled world, if it compiles.
    pub fn compiled(&self) -> Option<&Compiled> {
        match &self.circuit {
            Some(Ok((c, _))) => Some(c),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<SimError> {
        match &self.circuit {
            Some(Err(e)) => Some(e.clone()),
            _ if self.oscillating => Some(SimError::Oscillates),
            _ => None,
        }
    }

    /// Every chip output after the last step (empty if not running).
    pub fn values(&self) -> &[u64] {
        &self.values
    }

    /// The value of a chip output.
    pub fn output(&self, o: usize) -> Option<bool> {
        self.values.get(o).map(|v| *v == 1)
    }

    /// Whether the lamp at `pos` is lit.
    pub fn lamp_lit(&self, pos: Pos) -> bool {
        self.compiled()
            .is_some_and(|c| c.lamp_lit(pos, &self.values))
    }

    /// Deliver a key to the keyboard device at `anchor` at the next step.
    pub fn press(&mut self, anchor: Pos, key: u8) {
        if let Some(n) = self.native_at(anchor) {
            self.keys.push((n, key));
        }
    }

    /// The native part name of the device at `anchor`.
    pub fn native_at(&self, anchor: Pos) -> Option<String> {
        let c = self.compiled()?;
        c.devices
            .iter()
            .find(|d| d.anchor == anchor)?
            .native
            .clone()
    }

    /// Mutable access to the running circuit (to read devices).
    pub fn running_circuit(&mut self) -> Option<&mut Circuit> {
        match &mut self.circuit {
            Some(Ok((_, sim))) => Some(sim),
            _ => None,
        }
    }

    pub fn single_step(&mut self) {
        self.step_once = true;
    }

    /// Run this frame's steps: `speed` of them, or as many as fit in a few
    /// milliseconds. While paused the circuit is still evaluated (so
    /// switches respond) without advancing the clock.
    pub fn run_frame(&mut self, world: &World, defer: bool) {
        let advance = self.running || std::mem::take(&mut self.step_once);
        if !advance {
            self.step(world, defer, false);
            return;
        }
        let start = Instant::now();
        let budget = Duration::from_millis(8);
        let n = if self.running { self.speed } else { 1 };
        let mut done = 0u64;
        loop {
            if !self.step(world, defer, true) {
                break;
            }
            done += 1;
            let more = if n == 0 {
                start.elapsed() < budget
            } else {
                done < n as u64 && start.elapsed() < budget * 4
            };
            if !more {
                break;
            }
        }
        let (since, count) = self.rate_window;
        let count = count + done;
        let dt = since.elapsed().as_secs_f64();
        if dt >= 0.5 {
            self.rate = count as f64 / dt;
            self.rate_window = (Instant::now(), 0);
        } else {
            self.rate_window = (since, count);
        }
    }

    /// One evaluation; `advance` takes a step first, so the values shown
    /// are always those of step `tick`. Returns whether the circuit ran.
    fn step(&mut self, world: &World, defer: bool, advance: bool) -> bool {
        if self.circuit(world, defer).is_none() {
            self.values.clear();
            return false;
        }
        if advance {
            self.tick += 1;
        }
        let tick = self.tick;
        let keys = std::mem::take(&mut self.keys);
        let Some(Ok((c, sim))) = &mut self.circuit else {
            unreachable!("built above")
        };
        for (name, key) in keys {
            device::press(sim, &name, key);
        }
        for &(i, p) in &self.switches {
            let on = matches!(world.get(p), Some(Block::Switch { on: true, .. }));
            sim.set_input(i, on as u64);
        }
        for (i, src) in c.inputs.iter().enumerate() {
            if let InputSource::Clock { period, .. } = src {
                sim.set_input(i, (tick / u64::from((*period).max(1))) % 2);
            }
        }
        self.oscillating = sim.eval().is_err();
        self.values.clear();
        if self.oscillating {
            return false;
        }
        self.values
            .extend((0..c.def.outputs.len()).map(|o| sim.output(o)));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use makine_world::{Dir, Edit, Gate};

    fn set(w: &mut World, x: i32, b: Block) {
        w.apply(Edit::Set {
            pos: Pos::new(x, 0, 0),
            block: Some(b),
        });
    }

    fn lamp(sim: &WorldSim) -> bool {
        sim.lamp_lit(Pos::new(1, 0, 0))
    }

    #[test]
    fn speed_and_single_step() {
        let mut w = World::new("s");
        set(
            &mut w,
            0,
            Block::Clock {
                period: 1,
                facing: Dir::East,
            },
        );
        set(&mut w, 1, Block::Lamp);
        let mut s = WorldSim::default();
        s.run_frame(&w, false);
        assert_eq!(s.tick, 1);
        s.speed = 100;
        s.run_frame(&w, false);
        assert_eq!(s.tick, 101);
        // As fast as possible: more than one step, however busy the machine.
        s.speed = 0;
        let t = s.tick;
        s.run_frame(&w, false);
        assert!(s.tick > t + 1, "as fast as possible: {}", s.tick - t);
        // Paused: frames do not advance, a single step does, and the lamp
        // follows the clock (period 1: toggles every step).
        s.running = false;
        let t = s.tick;
        s.run_frame(&w, false);
        s.run_frame(&w, false);
        assert_eq!(s.tick, t);
        let before = lamp(&s);
        s.single_step();
        s.run_frame(&w, false);
        assert_eq!(s.tick, t + 1);
        assert_ne!(lamp(&s), before);
    }

    #[test]
    fn switches_and_oscillation() {
        let mut w = World::new("s");
        set(
            &mut w,
            0,
            Block::Switch {
                on: true,
                facing: Dir::East,
            },
        );
        set(&mut w, 1, Block::Lamp);
        let mut s = WorldSim::default();
        s.run_frame(&w, false);
        assert!(lamp(&s));
        set(
            &mut w,
            0,
            Block::Switch {
                on: false,
                facing: Dir::East,
            },
        );
        s.run_frame(&w, false);
        assert!(!lamp(&s));

        // A NOT whose output loops back to its input never settles.
        let mut w = World::new("o");
        w.apply(Edit::Set {
            pos: Pos::new(1, 0, 0),
            block: Some(Block::Gate {
                gate: Gate::Not,
                facing: Dir::East,
            }),
        });
        for p in [(2, 0), (2, 1), (1, 1), (0, 1), (0, 0)] {
            w.apply(Edit::Set {
                pos: Pos::new(p.0, p.1, 0),
                block: Some(Block::Wire),
            });
        }
        s.run_frame(&w, false);
        assert_eq!(s.error(), Some(SimError::Oscillates));
    }

    #[test]
    fn deferred_rebuild() {
        let mut w = World::new("d");
        set(
            &mut w,
            0,
            Block::Switch {
                on: true,
                facing: Dir::East,
            },
        );
        set(&mut w, 1, Block::Lamp);
        let mut s = WorldSim::default();
        s.run_frame(&w, false);
        set(&mut w, 2, Block::Lamp);
        s.run_frame(&w, true);
        assert_eq!(s.compiled().unwrap().lamps.len(), 1, "waits while deferred");
        s.run_frame(&w, false);
        assert_eq!(s.compiled().unwrap().lamps.len(), 2);
    }
}
