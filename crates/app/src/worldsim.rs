//! Running a world: its compiled circuit, the clock and the speed.
//!
//! The circuit is rebuilt whenever the world's circuit changes (not when a
//! part is only moved or a switch flipped), keeping what devices hold
//! (memory, screen, keys).

use std::time::{Duration, Instant};

use makine_sim::{Circuit, Library};
use makine_world::compile::{compile, Compiled, InputSource};
use makine_world::{device, End, Id, Kind, World};

/// Why a world cannot run.
#[derive(Clone, Debug, PartialEq)]
pub enum SimError {
    /// The circuit cannot be built (e.g. a chip's insides are broken).
    Build(String),
    /// The circuit never settles (e.g. a NOT feeding itself).
    Oscillates,
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::Build(e) => write!(f, "{e}"),
            SimError::Oscillates => write!(
                f,
                "devre salınıyor: bir çıkış kendi girişini ters çeviriyor (ör. kendine bağlı NOT)"
            ),
        }
    }
}

pub struct WorldSim {
    circuit: Option<Result<(Compiled, Circuit), SimError>>,
    /// World circuit revision the circuit was built from.
    revision: u64,
    /// The player's chips, for library parts.
    lib: Library,
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
    /// Keys to deliver to a keyboard device: (native name, key).
    keys: Vec<(String, u8)>,
}

impl Default for WorldSim {
    fn default() -> Self {
        Self {
            circuit: None,
            revision: u64::MAX,
            lib: Library::new(),
            tick: 0,
            running: true,
            speed: 1,
            rate: 0.0,
            rate_window: (Instant::now(), 0),
            step_once: false,
            values: Vec::new(),
            oscillating: false,
            keys: Vec::new(),
        }
    }
}

impl WorldSim {
    pub fn set_library(&mut self, lib: Library) {
        self.lib = lib;
        self.revision = u64::MAX;
    }

    pub fn library(&self) -> &Library {
        &self.lib
    }

    /// The circuit for `world`, rebuilt if its circuit changed. Device
    /// contents carry over to the new circuit.
    pub fn circuit(&mut self, world: &World) -> Option<&mut (Compiled, Circuit)> {
        if self.revision != world.circuit_revision() {
            self.revision = world.circuit_revision();
            let saved = self.saved_devices();
            let c = compile(world, &self.lib);
            self.circuit = Some(
                Circuit::compile_def(&c.library_with(&self.lib), &c.def)
                    .map_err(|e| SimError::Build(e.to_string()))
                    .map(|mut sim| {
                        for (name, s) in saved {
                            device::restore(&mut sim, &name, s);
                        }
                        (c, sim)
                    }),
            );
        }
        self.circuit.as_mut()?.as_mut().ok()
    }

    fn saved_devices(&mut self) -> Vec<(String, device::Saved)> {
        let Some(Ok((c, sim))) = &mut self.circuit else {
            return Vec::new();
        };
        c.devices
            .iter()
            .filter_map(|(_, n)| Some((n.clone(), device::save(sim, n)?)))
            .collect()
    }

    /// The compiled world, if it builds.
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

    /// The value on output pin `end` after the last step, if it is wired
    /// and the world runs.
    pub fn value(&self, end: End) -> Option<u64> {
        let o = self.compiled()?.probe(end)?;
        self.values.get(o).copied()
    }

    /// The value reaching input pin `to` of `world`, if it is wired.
    pub fn input_value(&self, world: &World, to: End) -> Option<u64> {
        self.value(world.wire_into(to)?)
    }

    /// Deliver a key to keyboard part `id` at the next step.
    pub fn press(&mut self, id: Id, key: u8) {
        if let Some(n) = self.compiled().and_then(|c| c.native(id)) {
            self.keys.push((n.to_string(), key));
        }
    }

    /// Mutable access to the running circuit and the native name of device
    /// part `id` (to read memory or a screen).
    pub fn device(&mut self, id: Id) -> Option<(&mut Circuit, String)> {
        match &mut self.circuit {
            Some(Ok((c, sim))) => {
                let n = c.native(id)?.to_string();
                Some((sim, n))
            }
            _ => None,
        }
    }

    pub fn single_step(&mut self) {
        self.step_once = true;
    }

    /// Run this frame's steps: `speed` of them, or as many as fit in a few
    /// milliseconds. While paused the circuit is still evaluated (so
    /// switches respond) without advancing the clock.
    pub fn run_frame(&mut self, world: &World) {
        let advance = self.running || std::mem::take(&mut self.step_once);
        if !advance {
            self.step(world, false);
            return;
        }
        let start = Instant::now();
        let budget = Duration::from_millis(8);
        let n = if self.running { self.speed } else { 1 };
        let mut done = 0u64;
        loop {
            if !self.step(world, true) {
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
    fn step(&mut self, world: &World, advance: bool) -> bool {
        if self.circuit(world).is_none() {
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
        for (i, (id, src)) in c.inputs.iter().enumerate() {
            let v = match (src, world.part(*id).map(|p| &p.kind)) {
                (InputSource::Clock { period }, _) => (tick / u64::from((*period).max(1))) % 2,
                (_, Some(Kind::Switch { on })) => *on as u64,
                (_, Some(Kind::Input { value, .. })) => *value,
                _ => 0,
            };
            sim.set_input(i, v);
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
    use makine_world::{Edit, Gate, Placed, Pos};

    fn put(w: &mut World, kind: Kind) -> Id {
        let id = w.new_id();
        w.apply(Edit::Part {
            id,
            part: Some(Placed {
                kind,
                pos: Pos::new(0.0, 0.0),
            }),
        });
        id
    }

    fn wire(w: &mut World, from: Id, to: Id) {
        w.apply(Edit::Wire {
            to: End::new(to, 0),
            from: Some(End::new(from, 0)),
        });
    }

    fn lit(s: &WorldSim, w: &World, lamp: Id) -> bool {
        s.input_value(w, End::new(lamp, 0)).unwrap_or(0) != 0
    }

    #[test]
    fn speed_and_single_step() {
        let mut w = World::new("s");
        let clk = put(&mut w, Kind::Clock { period: 1 });
        let lamp = put(&mut w, Kind::Lamp);
        wire(&mut w, clk, lamp);
        let mut s = WorldSim::default();
        s.run_frame(&w);
        assert_eq!(s.tick, 1);
        s.speed = 100;
        s.run_frame(&w);
        assert_eq!(s.tick, 101);
        s.speed = 0;
        let t = s.tick;
        s.run_frame(&w);
        assert!(s.tick > t + 1, "as fast as possible: {}", s.tick - t);
        // Paused: frames do not advance, a single step does, and the lamp
        // follows the clock (period 1: toggles every step).
        s.running = false;
        let t = s.tick;
        s.run_frame(&w);
        s.run_frame(&w);
        assert_eq!(s.tick, t);
        let before = lit(&s, &w, lamp);
        s.single_step();
        s.run_frame(&w);
        assert_eq!(s.tick, t + 1);
        assert_ne!(lit(&s, &w, lamp), before);
    }

    #[test]
    fn switches_without_rebuilding_and_oscillation() {
        let mut w = World::new("s");
        let sw = put(&mut w, Kind::Switch { on: true });
        let lamp = put(&mut w, Kind::Lamp);
        wire(&mut w, sw, lamp);
        let mut s = WorldSim::default();
        s.run_frame(&w);
        assert!(lit(&s, &w, lamp));
        let rev = w.circuit_revision();
        w.apply(Edit::Part {
            id: sw,
            part: Some(Placed {
                kind: Kind::Switch { on: false },
                pos: Pos::new(0.0, 0.0),
            }),
        });
        assert_eq!(w.circuit_revision(), rev);
        s.run_frame(&w);
        assert!(!lit(&s, &w, lamp));

        // A NOT whose output loops back to its input never settles.
        let mut w = World::new("o");
        let not = put(&mut w, Kind::Gate(Gate::Not));
        wire(&mut w, not, not);
        s.run_frame(&w);
        assert_eq!(s.error(), Some(SimError::Oscillates));
    }
}
