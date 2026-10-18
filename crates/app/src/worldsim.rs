//! Running a world: its compiled circuit, the clock and the speed.
//!
//! The circuit is rebuilt whenever the world's circuit changes (not when a
//! part is only moved or a switch flipped), keeping what devices hold
//! (memory, screen, keys).

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use makine_lang::tr;
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
            SimError::Oscillates => f.write_str(tr!(
                "devre salınıyor: bir çıkış kendi girişini ters çeviriyor (ör. kendine bağlı NOT)",
                "the circuit oscillates: an output flips its own input (e.g. a NOT wired to itself)"
            )),
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
    pub traces: BTreeMap<End, VecDeque<(u64, Option<u64>)>>,
    pub break_on_change: Option<End>,
    cycle_target: Option<u64>,
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
            traces: BTreeMap::new(),
            break_on_change: None,
            cycle_target: None,
        }
    }
}

impl WorldSim {
    /// At most sixteen probes, each holding the latest 256 simulation
    /// steps. Samples are taken per evaluation, not per rendered frame.
    pub fn watch(&mut self, end: End) {
        if self.traces.len() < 16 {
            self.traces.entry(end).or_default();
        }
    }

    pub fn reset(&mut self) {
        self.circuit = None;
        self.revision = u64::MAX;
        self.tick = 0;
        self.values.clear();
        self.keys.clear();
        self.oscillating = false;
        self.rate = 0.0;
        self.rate_window = (Instant::now(), 0);
        self.step_once = false;
        self.cycle_target = None;
        for t in self.traces.values_mut() {
            t.clear();
        }
    }

    fn sample(&mut self) -> bool {
        let mut hit = false;
        let readings: Vec<_> = self.traces.keys().map(|e| (*e, self.value(*e))).collect();
        for (end, value) in readings {
            let trace = self.traces.get_mut(&end).unwrap();
            if let Some((tick, old)) = trace.back() {
                if self.break_on_change == Some(end)
                    && *tick != self.tick
                    && old.is_some()
                    && value.is_some()
                    && *old != value
                {
                    hit = true;
                }
                if *tick == self.tick {
                    trace.pop_back();
                }
            }
            trace.push_back((self.tick, value));
            if trace.len() > 256 {
                trace.pop_front();
            }
        }
        if hit {
            self.running = false;
        }
        hit
    }
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
                        // Establish the current clock phase before restoring
                        // device state. A layout-independent rebuild while
                        // the clock is high must not invent a rising edge.
                        for (i, (id, src)) in c.inputs.iter().enumerate() {
                            sim.set_input(i, input_value(world, *id, src, self.tick));
                        }
                        let _ = sim.eval();
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

    /// Pause and run to the next rising edge of the first clock. Long
    /// periods are advanced across frames within the usual time budget.
    pub fn clock_cycle(&mut self, world: &World) -> bool {
        let Some(period) = world.parts().find_map(|(_, p)| match p.kind {
            Kind::Clock { period } => Some(u64::from(period.max(1))),
            _ => None,
        }) else {
            return false;
        };
        let phases = if (self.tick / period).is_multiple_of(2) {
            1
        } else {
            2
        };
        self.running = false;
        self.cycle_target = Some(self.tick + phases * period - self.tick % period);
        true
    }

    pub fn advancing_cycle(&self) -> bool {
        self.cycle_target.is_some()
    }

    /// Run this frame's steps: `speed` of them, or as many as fit in a few
    /// milliseconds. While paused the circuit is still evaluated (so
    /// switches respond) without advancing the clock.
    pub fn run_frame(&mut self, world: &World) {
        let advance =
            self.running || std::mem::take(&mut self.step_once) || self.cycle_target.is_some();
        if !advance {
            self.step(world, false);
            self.sample();
            return;
        }
        let start = Instant::now();
        let budget = Duration::from_millis(8);
        let n = if self.cycle_target.is_some() {
            0
        } else if self.running {
            self.speed
        } else {
            1
        };
        let mut done = 0u64;
        loop {
            if !self.step(world, true) {
                self.sample();
                self.cycle_target = None;
                break;
            }
            done += 1;
            if self.sample() {
                self.cycle_target = None;
                break;
            }
            if self.cycle_target.is_some_and(|target| self.tick >= target) {
                self.cycle_target = None;
                break;
            }
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
            let v = input_value(world, *id, src, tick);
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

fn input_value(world: &World, id: Id, src: &InputSource, tick: u64) -> u64 {
    match (src, world.part(id).map(|p| &p.kind)) {
        (InputSource::Clock { period }, _) => (tick / u64::from((*period).max(1))) % 2,
        (_, Some(Kind::Switch { on })) => *on as u64,
        (_, Some(Kind::Input { value, .. })) => *value,
        _ => 0,
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

    #[test]
    fn probes_sample_each_step_and_trigger_stops_fast_run() {
        let mut w = World::new("scope");
        let clk = put(&mut w, Kind::Clock { period: 2 });
        let mut sim = WorldSim::default();
        let end = End::new(clk, 0);
        sim.watch(end);
        sim.speed = 10;
        sim.run_frame(&w);
        assert_eq!(sim.traces[&end].len(), 10);
        assert_eq!(sim.traces[&end][0], (1, Some(0)));
        assert_eq!(sim.traces[&end][1], (2, Some(1)));
        sim.break_on_change = Some(end);
        sim.run_frame(&w);
        assert!(!sim.running);
        assert_eq!(sim.tick, 12);
        sim.run_frame(&w);
        assert_eq!(
            sim.traces[&end].len(),
            12,
            "paused frames replace the sample"
        );
        sim.break_on_change = None;
        sim.running = true;
        sim.speed = 1000;
        sim.run_frame(&w);
        assert_eq!(sim.traces[&end].len(), 256);
        sim.reset();
        assert_eq!(sim.tick, 0);
        assert!(sim.traces[&end].is_empty());
    }

    #[test]
    fn workshop_cycles_and_live_edit_keep_storage() {
        let project = makine_world::projects::mcs4::project(makine_world::projects::mcs4::DEMO);
        let mut world = project.world;
        let a = project.watches[0];
        let pc = project.watches[2];
        let mut sim = WorldSim {
            running: false,
            ..Default::default()
        };
        sim.run_frame(&world);
        assert_eq!(sim.value(pc), Some(0));
        for (want_pc, want_a) in [(1, 0), (2, 0), (3, 3), (4, 3), (5, 3), (6, 3), (2, 3)] {
            assert!(sim.clock_cycle(&world));
            sim.run_frame(&world);
            assert_eq!(sim.value(pc), Some(want_pc));
            assert_eq!(sim.value(a), Some(want_a));
        }
        // Rebuilding at a high clock level must not overwrite the saved
        // registers with the new natives' initially empty input buffers.
        put(&mut world, Kind::Lamp);
        sim.run_frame(&world);
        assert_eq!(sim.value(pc), Some(2));
        assert_eq!(sim.value(a), Some(3));
    }
}

/// The example worlds do what their descriptions say.
#[cfg(test)]
mod example_tests {
    use super::*;
    use makine_world::projects::examples::all;
    use makine_world::projects::Project;

    fn open(id: &str) -> (Project, WorldSim) {
        let e = all().into_iter().find(|e| e.id == id).unwrap();
        ((e.build)(), WorldSim::default())
    }

    fn find(p: &Project, f: impl Fn(&Kind) -> bool) -> Vec<Id> {
        p.world
            .parts()
            .filter(|(_, x)| f(&x.kind))
            .map(|(id, _)| id)
            .collect()
    }

    fn run(s: &mut WorldSim, w: &World, steps: u32) {
        s.speed = steps;
        s.run_frame(w);
    }

    #[test]
    fn half_adder_adds() {
        let (mut p, mut s) = open("half_adder");
        let sw = find(&p, |k| matches!(k, Kind::Switch { .. }));
        let outs = find(&p, |k| matches!(k, Kind::Output { .. }));
        for id in &sw {
            p.world.apply(makine_world::Edit::Part {
                id: *id,
                part: Some(makine_world::Placed {
                    kind: Kind::Switch { on: true },
                    pos: p.world.part(*id).unwrap().pos,
                }),
            });
        }
        run(&mut s, &p.world, 1);
        let v: Vec<u64> = outs
            .iter()
            .map(|o| s.input_value(&p.world, End::new(*o, 0)).unwrap())
            .collect();
        assert_eq!(v, vec![0, 1], "1 + 1: sum 0, carry 1");
    }

    #[test]
    fn counter_counts_and_adder_adds() {
        let (p, mut s) = open("counter");
        let reg = find(&p, |k| {
            matches!(k, Kind::Device(makine_world::Device::Register { .. }))
        })[0];
        run(&mut s, &p.world, 15 * 2 * 5 + 1);
        assert_eq!(s.value(End::new(reg, 0)), Some(5));

        let (p, mut s) = open("adder4");
        let out = find(&p, |k| matches!(k, Kind::Output { .. }))[0];
        run(&mut s, &p.world, 1);
        assert_eq!(s.input_value(&p.world, End::new(out, 0)), Some(14), "5 + 9");
    }

    #[test]
    fn traffic_light_cycles() {
        let (p, mut s) = open("traffic");
        let outs = find(&p, |k| matches!(k, Kind::Output { .. }));
        let lights = |s: &WorldSim| -> Vec<u64> {
            outs.iter()
                .map(|o| s.input_value(&p.world, End::new(*o, 0)).unwrap())
                .collect()
        };
        // Order of the outputs: red, amber, green.
        let mut seen = Vec::new();
        for _ in 0..4 {
            run(&mut s, &p.world, 80);
            seen.push(lights(&s));
        }
        assert_eq!(
            seen,
            vec![vec![1, 1, 0], vec![0, 0, 1], vec![0, 1, 0], vec![1, 0, 0]]
        );
    }

    #[test]
    fn screen_line_draws_the_diagonal() {
        let (p, mut s) = open("screen_line");
        run(&mut s, &p.world, 4 * 32 + 4);
        let id = find(&p, |k| {
            matches!(k, Kind::Device(makine_world::Device::Screen { .. }))
        })[0];
        let (c, name) = s.device(id).unwrap();
        let px = makine_world::device::screen(c, &name).unwrap();
        for i in 0..32 {
            assert!(px[i * 32 + i], "pixel {i},{i}");
        }
        assert!(!px[1]);
    }

    #[test]
    fn typewriter_stores_keys() {
        let (p, mut s) = open("typewriter");
        let kbd = find(&p, |k| {
            matches!(k, Kind::Device(makine_world::Device::Keyboard))
        })[0];
        let cursor = find(&p, |k| {
            matches!(k, Kind::Device(makine_world::Device::Register { .. }))
        })[0];
        run(&mut s, &p.world, 1);
        for k in *b"hi!" {
            s.press(kbd, k);
            run(&mut s, &p.world, 1);
        }
        run(&mut s, &p.world, 40);
        assert_eq!(s.value(End::new(cursor, 0)), Some(3));
    }

    fn set_input(p: &mut Project, name: &str, v: u64) {
        let (id, kind) = p
            .world
            .parts()
            .find(|(_, x)| matches!(&x.kind, Kind::Input { name: n, .. } if n == name))
            .map(|(id, x)| (id, x.clone()))
            .unwrap();
        let Kind::Input { name, width, .. } = kind.kind else {
            unreachable!()
        };
        p.world.apply(makine_world::Edit::Part {
            id,
            part: Some(makine_world::Placed {
                kind: Kind::Input {
                    name,
                    width,
                    value: v,
                },
                pos: kind.pos,
            }),
        });
    }

    #[test]
    fn sr_latch_remembers() {
        let (mut p, mut s) = open("sr_latch");
        let lamp = find(&p, |k| matches!(k, Kind::Lamp))[0];
        let on = |s: &WorldSim, p: &Project| s.input_value(&p.world, End::new(lamp, 0)).unwrap();
        run(&mut s, &p.world, 2);
        set_input(&mut p, "set", 1);
        run(&mut s, &p.world, 2);
        assert_eq!(on(&s, &p), 1);
        set_input(&mut p, "set", 0);
        run(&mut s, &p.world, 2);
        assert_eq!(on(&s, &p), 1, "still on");
        set_input(&mut p, "reset", 1);
        run(&mut s, &p.world, 2);
        set_input(&mut p, "reset", 0);
        run(&mut s, &p.world, 2);
        assert_eq!(on(&s, &p), 0);
    }

    #[test]
    fn ram_bench_writes_and_reads() {
        let (mut p, mut s) = open("ram_bench");
        let ram = find(&p, |k| {
            matches!(k, Kind::Device(makine_world::Device::Ram { .. }))
        })[0];
        let (a, d, w) = ("adres", "veri", "yaz");
        set_input(&mut p, a, 3);
        set_input(&mut p, d, 77);
        run(&mut s, &p.world, 1);
        set_input(&mut p, w, 1);
        run(&mut s, &p.world, 1);
        set_input(&mut p, w, 0);
        set_input(&mut p, a, 4);
        run(&mut s, &p.world, 1);
        assert_eq!(s.value(End::new(ram, 0)), Some(0));
        set_input(&mut p, a, 3);
        run(&mut s, &p.world, 1);
        assert_eq!(s.value(End::new(ram, 0)), Some(77));
    }

    #[test]
    fn rom_benches_match_level_tables_and_split_instruction_fields() {
        for lang in makine_lang::Lang::ALL {
            makine_lang::set_for_thread(Some(lang));
            for (id, words) in [
                ("rom_bench", makine_levels::memory::DATA.as_slice()),
                ("program_rom", makine_levels::memory::PROGRAM.as_slice()),
            ] {
                let (mut p, mut s) = open(id);
                let rom = find(&p, |k| {
                    matches!(k, Kind::Device(makine_world::Device::Rom { .. }))
                })[0];
                let outputs = find(&p, |k| matches!(k, Kind::Output { .. }));
                for addr in (0..words.len()).rev() {
                    set_input(&mut p, "addr", addr as u64);
                    run(&mut s, &p.world, 1);
                    let value = words[addr] as u64;
                    assert_eq!(s.value(End::new(rom, 0)), Some(value));
                    if id == "program_rom" {
                        assert_eq!(
                            s.input_value(&p.world, End::new(outputs[0], 0)),
                            Some(value & 15)
                        );
                        assert_eq!(
                            s.input_value(&p.world, End::new(outputs[1], 0)),
                            Some(value >> 4)
                        );
                    }
                }
            }
        }
        makine_lang::set_for_thread(None);
    }

    #[test]
    fn byte_ram_bench_separates_enable_edges_and_combinational_reads() {
        let (mut p, mut s) = open("byte_ram_bench");
        let ram = find(&p, |k| {
            matches!(k, Kind::Device(makine_world::Device::Ram { .. }))
        })[0];
        run(&mut s, &p.world, 1);
        set_input(&mut p, "addr", 1);
        set_input(&mut p, "we", 1);
        run(&mut s, &p.world, 1);
        assert_eq!(s.value(End::new(ram, 0)), Some(0), "no rising edge yet");
        set_input(&mut p, "clk", 1);
        run(&mut s, &p.world, 1);
        assert_eq!(s.value(End::new(ram, 0)), Some(0xA5));
        set_input(&mut p, "d", 0x3C);
        set_input(&mut p, "addr", 2);
        run(&mut s, &p.world, 1);
        assert_eq!(
            s.value(End::new(ram, 0)),
            Some(0),
            "high clock must not write new address"
        );
        set_input(&mut p, "clk", 0);
        run(&mut s, &p.world, 1);
        set_input(&mut p, "clk", 1);
        run(&mut s, &p.world, 1);
        assert_eq!(s.value(End::new(ram, 0)), Some(0x3C));
        set_input(&mut p, "we", 0);
        set_input(&mut p, "addr", 1);
        run(&mut s, &p.world, 1);
        assert_eq!(
            s.value(End::new(ram, 0)),
            Some(0xA5),
            "we=0 must allow reads"
        );
        set_input(&mut p, "d", 0xFF);
        set_input(&mut p, "clk", 0);
        run(&mut s, &p.world, 1);
        set_input(&mut p, "clk", 1);
        run(&mut s, &p.world, 1);
        assert_eq!(
            s.value(End::new(ram, 0)),
            Some(0xA5),
            "we=0 must prevent writes"
        );
    }
}
