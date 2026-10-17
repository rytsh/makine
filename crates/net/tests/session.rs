//! Real sockets on this machine: a host and two players edit the same
//! cells at the same time and must end with identical worlds.

use std::time::{Duration, Instant};

use makine_net::{Client, Event, Host};
use makine_world::{Block, Edit, Pos, World};

fn set(x: i32, b: Block) -> Edit {
    Edit::Set {
        pos: Pos::new(x, 0, 0),
        block: Some(b),
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
    let events = host.poll(world, |_| vec![]);
    for e in &events {
        if let Event::Edit { edit, .. } = e {
            world.apply(edit.clone());
        }
    }
    events
}

fn cells(w: &World) -> Vec<(Pos, Block)> {
    let mut v: Vec<(Pos, Block)> = w.blocks().map(|(p, b)| (p, b.clone())).collect();
    v.sort_by_key(|(p, _)| *p);
    v
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
    hw.apply(set(0, Block::Lamp));
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

    // Everyone writes cell 1 (and some others) at once, without waiting.
    for round in 0..20 {
        a.edit(set(1, Block::Wire));
        b.edit(set(1, Block::Via));
        let e = set(1, Block::Cross);
        hw.apply(e.clone());
        host.send_edit(e, vec![]);
        a.edit(set(10 + round, Block::Wire));
        b.edit(set(10 + round, Block::Lamp));
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

    // Cursors reach the others.
    a.client.send_cursor(Some(Pos::new(3, 4, 0)));
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

#[test]
fn nobody_there() {
    // Port 1 on localhost: nothing listens.
    assert!(Client::connect("127.0.0.1:1", "x").is_err());
}
