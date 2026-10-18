//! Real sockets on this machine: a host and two players edit the same
//! cells at the same time and must end with identical worlds.

use std::time::{Duration, Instant};

use makine_net::{Client, Event, Host};
use makine_world::{Edit, Kind, Placed, Pos, World};

/// Put part `id` (a lamp at `x`, or a switch).
fn set(id: u64, x: f32, switch: bool) -> Edit {
    let kind = if switch {
        Kind::Switch { on: true }
    } else {
        Kind::Lamp
    };
    Edit::Part {
        id,
        part: Some(Placed {
            kind,
            pos: Pos::new(x, 0.0),
        }),
    }
}

struct Peer {
    world: World,
    client: Client,
    log: Vec<Event>,
}

impl Peer {
    fn poll(&mut self) {
        for e in self.client.poll() {
            match &e {
                Event::Welcome { world, .. } => self.world = world.clone(),
                Event::Edit { edit, .. } => {
                    self.world.apply(edit.clone());
                }
                _ => {}
            }
            self.log.push(e);
        }
    }

    fn edit(&mut self, e: Edit) {
        self.world.apply(e.clone());
        self.client.send_edit(e, vec![]);
    }
}

fn host_poll(host: &mut Host, world: &mut World) -> Vec<Event> {
    host.poll(world, |_| vec![])
}

fn cells(w: &World) -> Vec<(u64, Placed)> {
    w.parts().map(|(id, p)| (id, p.clone())).collect()
}

/// Poll everyone until `done` or a timeout.
fn settle(
    host: &mut Host,
    hw: &mut World,
    peers: &mut [&mut Peer],
    done: impl Fn(&World, &[&mut Peer]) -> bool,
) {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(5) {
        host_poll(host, hw);
        for p in peers.iter_mut() {
            p.poll();
        }
        if done(hw, peers) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out");
}

#[test]
fn players_converge() {
    let mut hw = World::new("ortak");
    hw.apply(set(1000, 0.0, false));
    let mut host = Host::start("ev sahibi", 0).unwrap();
    let addr = format!("127.0.0.1:{}", host.port());
    let mut a = Peer {
        world: World::new("?"),
        client: Client::connect(&addr, "ayşe").unwrap(),
        log: vec![],
    };
    let mut b = Peer {
        world: World::new("?"),
        client: Client::connect(&addr, "burak").unwrap(),
        log: vec![],
    };
    settle(&mut host, &mut hw, &mut [&mut a, &mut b], |_, p| {
        p.iter().all(|p| p.client.id().is_some())
    });
    assert_eq!(cells(&a.world), cells(&hw), "welcome carries the world");
    assert_eq!(host.players().len(), 3);
    settle(&mut host, &mut hw, &mut [&mut a, &mut b], |_, p| {
        p.iter().all(|p| p.client.players().len() == 3)
    });

    // Everyone writes part 1 (and some others) at once, without waiting.
    for round in 0..20 {
        a.edit(set(1, 1.0, false));
        b.edit(set(1, 2.0, true));
        let e = set(1, 3.0, false);
        hw.apply(e.clone());
        host.send_edit(e, vec![]);
        a.edit(set(10 + round, 0.0, false));
        b.edit(set(10 + round, 5.0, true));
        if round % 3 == 0 {
            host_poll(&mut host, &mut hw);
            a.poll();
        }
    }
    settle(&mut host, &mut hw, &mut [&mut a, &mut b], |hw, p| {
        p.iter().all(|p| cells(&p.world) == cells(hw))
    });
    // And it stays that way (no late echoes undo anything).
    std::thread::sleep(Duration::from_millis(100));
    host_poll(&mut host, &mut hw);
    a.poll();
    b.poll();
    assert_eq!(cells(&a.world), cells(&hw));
    assert_eq!(cells(&b.world), cells(&hw));

    // Presentation metadata uses exactly the same ordered edit stream.
    // No player may lose a module name or a manual route to a late echo.
    a.edit(Edit::Module {
        id: 999,
        module: Some(makine_world::Module {
            name: "ALU".into(),
            members: vec![1],
            pos: None,
        }),
    });
    b.edit(Edit::Style {
        to: makine_world::End::new(1, 0),
        style: Some(makine_world::WireStyle {
            name: "data".into(),
            points: vec![Pos::new(40.0, 80.0)],
        }),
    });
    settle(&mut host, &mut hw, &mut [&mut a, &mut b], |hw, p| {
        hw.module(999).is_some()
            && hw.wire_style(makine_world::End::new(1, 0)).is_some()
            && p.iter().all(|p| p.world.to_json() == hw.to_json())
    });

    // Cursors reach the others.
    a.client.send_cursor(Some(Pos::new(3.0, 4.0)));
    let aid = a.client.id().unwrap();
    settle(&mut host, &mut hw, &mut [&mut a, &mut b], |_, p| {
        p[1].log
            .iter()
            .any(|e| matches!(e, Event::Cursor { id, pos: Some(_) } if *id == aid))
    });

    // A player leaves: the others hear it.
    drop(a);
    let t = Instant::now();
    loop {
        host_poll(&mut host, &mut hw);
        b.poll();
        if b.log
            .iter()
            .any(|e| matches!(e, Event::Left(p) if p.name == "ayşe"))
        {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "no Left event");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(host.players().len(), 2);

    // The host stops: the player is told.
    drop(host);
    let t = Instant::now();
    loop {
        b.poll();
        if b.log.iter().any(|e| matches!(e, Event::Closed(_))) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "no Closed event");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A player who joins while others are editing gets every edit: those
/// before the welcome are in the world, those after arrive as edits.
#[test]
fn late_joiner_misses_nothing() {
    let mut hw = World::new("ortak");
    let mut host = Host::start("ev", 0).unwrap();
    let addr = format!("127.0.0.1:{}", host.port());
    let mut a = Peer {
        world: World::new("?"),
        client: Client::connect(&addr, "a").unwrap(),
        log: vec![],
    };
    settle(&mut host, &mut hw, &mut [&mut a], |_, p| {
        p[0].client.id().is_some()
    });
    // a floods edits; b connects in the middle of them.
    for x in 0..200 {
        a.edit(set(x, 0.0, false));
    }
    let mut b = Peer {
        world: World::new("?"),
        client: Client::connect(&addr, "b").unwrap(),
        log: vec![],
    };
    for x in 200..400 {
        a.edit(set(x, 1.0, true));
    }
    settle(&mut host, &mut hw, &mut [&mut a, &mut b], |hw, p| {
        hw.len() == 400 && p.iter().all(|p| cells(&p.world) == cells(hw))
    });
}

#[test]
fn nobody_there() {
    // Port 1 on localhost: nothing listens.
    assert!(Client::connect("127.0.0.1:1", "x").is_err());
}
