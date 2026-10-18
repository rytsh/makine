//! Building worlds together over a local network.
//!
//! One player *hosts*: their world is the shared one and their app decides
//! the order of all edits. Others *join* by address, receive the world and
//! then every edit, in the host's order.
//!
//! Each player applies their own edits at once, so building feels instant.
//! Edits are plain "set this part" or "set the wire into this pin" changes,
//! where the last write wins, so applying the host's order on top makes everyone converge: when
//! another player's edit arrived in between, a player's own edit is applied
//! again when the host echoes it back (see [`Client::poll`]).
//!
//! The protocol is one JSON message per line over TCP. Simulation runs on
//! each machine separately; only the parts and wires are shared.

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub mod relay;

use makine_pack::Package;
use makine_world::{Edit, Pos, World};
use serde::{Deserialize, Serialize};

/// Default TCP port.
pub const PORT: u16 = 7878;
/// Bumped when messages change; mismatched players are refused.
pub const PROTOCOL: u32 = 5;
/// The host's player id.
pub const HOST_ID: u32 = 0;
/// Longest message accepted (a large world with its chips).
const MAX_LINE: usize = 64 << 20;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub id: u32,
    pub name: String,
}

/// Messages from a joined player to the host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToHost {
    Hello {
        name: String,
        protocol: u32,
    },
    /// An edit, with packages for free chips it places (so others can
    /// simulate them).
    Edit {
        edit: Edit,
        chips: Vec<Package>,
    },
    Cursor {
        pos: Option<Pos>,
    },
}

/// Messages from the host to joined players.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToClient {
    Welcome {
        you: u32,
        players: Vec<Player>,
        world: World,
        chips: Vec<Package>,
    },
    Refused {
        reason: String,
    },
    Edit {
        from: u32,
        edit: Edit,
        chips: Vec<Package>,
    },
    Joined(Player),
    Left {
        id: u32,
    },
    Cursor {
        id: u32,
        pos: Option<Pos>,
    },
}

/// What happened since the last poll, for the app to act on.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The shared world (client only, once, on joining).
    Welcome {
        world: World,
        chips: Vec<Package>,
    },
    /// Another player's edit. On a client, apply it to the world; on the
    /// host it has already been applied (in order, see [`Host::poll`]).
    Edit {
        from: u32,
        edit: Edit,
        chips: Vec<Package>,
    },
    Joined(Player),
    Left(Player),
    Cursor {
        id: u32,
        pos: Option<Pos>,
    },
    /// The session ended; the reason is for the player.
    Closed(String),
}

/// Write lines to `stream` from a channel, on its own thread, so a slow
/// peer never blocks the app.
fn writer(mut stream: TcpStream) -> Sender<String> {
    let (tx, rx) = channel::<String>();
    thread::spawn(move || {
        for line in rx {
            if stream.write_all(line.as_bytes()).is_err() || stream.write_all(b"\n").is_err() {
                break;
            }
        }
        let _ = stream.shutdown(Shutdown::Both);
    });
    tx
}

/// Read JSON lines from `stream` on its own thread; `on` gets each parsed
/// message and `None` at the end.
fn reader<T: for<'de> Deserialize<'de>>(
    stream: TcpStream,
    on: impl Fn(Option<T>) -> bool + Send + 'static,
) {
    thread::spawn(move || {
        let mut r = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match (&mut r).take(MAX_LINE as u64).read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) if !line.ends_with('\n') => break, // too long or cut off
                Ok(_) => {}
            }
            let Ok(msg) = serde_json::from_str::<T>(&line) else {
                break;
            };
            if !on(Some(msg)) {
                return;
            }
        }
        on(None);
    });
}

fn json<T: Serialize>(m: &T) -> String {
    serde_json::to_string(m).expect("messages serialize")
}

fn clean_name(name: &str, id: u32) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
    let n = n.trim();
    if n.is_empty() {
        format!("oyuncu {id}")
    } else {
        n.to_string()
    }
}

enum Incoming {
    Connected(u32, TcpStream),
    Message(u32, ToHost),
    Closed(u32),
}

struct Conn {
    tx: Sender<String>,
    stream: TcpStream,
}

/// A new connection: give it an id and start reading it.
fn admit(tx: &Sender<Incoming>, next: &std::sync::atomic::AtomicU32, stream: TcpStream) -> bool {
    let id = next.fetch_add(1, Ordering::Relaxed);
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_nodelay(true);
    let Ok(read) = stream.try_clone() else {
        return true;
    };
    if tx.send(Incoming::Connected(id, stream)).is_err() {
        return false;
    }
    let tx = tx.clone();
    reader::<ToHost>(read, move |m| {
        let msg = match m {
            Some(m) => Incoming::Message(id, m),
            None => Incoming::Closed(id),
        };
        tx.send(msg).is_ok()
    });
    true
}

/// A hosted session.
pub struct Host {
    port: u16,
    /// The room code, when also open on a relay.
    room: Option<String>,
    relay_control: Option<TcpStream>,
    tx: Sender<Incoming>,
    next: Arc<std::sync::atomic::AtomicU32>,
    stop: Arc<AtomicBool>,
    incoming: Receiver<Incoming>,
    /// Connected but not yet said hello.
    pending: BTreeMap<u32, Conn>,
    joined: BTreeMap<u32, Conn>,
    players: BTreeMap<u32, Player>,
}

impl Host {
    /// Start listening on `port` (0 picks a free one).
    pub fn start(name: &str, port: u16) -> io::Result<Host> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, incoming) = channel();
        let next = Arc::new(std::sync::atomic::AtomicU32::new(HOST_ID + 1));
        let s = stop.clone();
        let (t, n) = (tx.clone(), next.clone());
        thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if !admit(&t, &n, stream) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(30));
                    }
                    Err(_) => thread::sleep(Duration::from_millis(100)),
                }
            }
        });
        let mut players = BTreeMap::new();
        players.insert(
            HOST_ID,
            Player {
                id: HOST_ID,
                name: clean_name(name, HOST_ID),
            },
        );
        Ok(Host {
            port,
            room: None,
            relay_control: None,
            tx,
            next,
            stop,
            incoming,
            pending: BTreeMap::new(),
            joined: BTreeMap::new(),
            players,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Also take players over the internet, through the relay at `relay`.
    /// Returns the room code to give them.
    pub fn open_room(&mut self, relay: &str) -> io::Result<String> {
        let (code, control) = relay::open_room(relay)?;
        let (tx, next, stop) = (self.tx.clone(), self.next.clone(), self.stop.clone());
        let (relay, room) = (relay.to_string(), code.clone());
        self.relay_control = Some(control.get_ref().try_clone()?);
        thread::spawn(move || {
            relay::guests(control, |id| {
                if stop.load(Ordering::Relaxed) {
                    return false;
                }
                match relay::accept(&relay, &room, id) {
                    Ok(s) => admit(&tx, &next, s),
                    Err(_) => true,
                }
            });
        });
        self.room = Some(code.clone());
        Ok(code)
    }

    pub fn room(&self) -> Option<&str> {
        self.room.as_deref()
    }

    pub fn players(&self) -> Vec<Player> {
        self.players.values().cloned().collect()
    }

    fn broadcast(&self, msg: &ToClient, except: Option<u32>) {
        let line = json(msg);
        for (id, c) in &self.joined {
            if Some(*id) != except {
                let _ = c.tx.send(line.clone());
            }
        }
    }

    /// Handle what arrived, in order: players' edits are applied to
    /// `world` here, so a player who joins in the same poll gets the world
    /// with every edit broadcast before them. `chips` gives the free chips
    /// a world uses (sent to joining players).
    pub fn poll(
        &mut self,
        world: &mut World,
        chips: impl Fn(&World) -> Vec<Package>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        while let Ok(m) = self.incoming.try_recv() {
            match m {
                Incoming::Connected(id, stream) => {
                    let Ok(w) = stream.try_clone() else { continue };
                    self.pending.insert(
                        id,
                        Conn {
                            tx: writer(w),
                            stream,
                        },
                    );
                }
                Incoming::Message(id, ToHost::Hello { name, protocol }) => {
                    let Some(conn) = self.pending.remove(&id) else {
                        continue;
                    };
                    if protocol != PROTOCOL {
                        let reason = makine_lang::tr!(
                            "makine sürümleriniz farklı; ikiniz de aynı sürümü kullanın",
                            "your makine versions differ; both of you should use the same version"
                        );
                        let _ = conn.tx.send(json(&ToClient::Refused {
                            reason: reason.into(),
                        }));
                        continue;
                    }
                    let player = Player {
                        id,
                        name: clean_name(&name, id),
                    };
                    self.players.insert(id, player.clone());
                    let welcome = ToClient::Welcome {
                        you: id,
                        players: self.players(),
                        world: world.clone(),
                        chips: chips(world),
                    };
                    let _ = conn.tx.send(json(&welcome));
                    self.broadcast(&ToClient::Joined(player.clone()), None);
                    self.joined.insert(id, conn);
                    events.push(Event::Joined(player));
                }
                Incoming::Message(id, ToHost::Edit { edit, chips }) => {
                    if !self.joined.contains_key(&id) {
                        continue;
                    }
                    // Everyone, the sender too: the echo tells it the order.
                    self.broadcast(
                        &ToClient::Edit {
                            from: id,
                            edit: edit.clone(),
                            chips: chips.clone(),
                        },
                        None,
                    );
                    world.apply(edit.clone());
                    events.push(Event::Edit {
                        from: id,
                        edit,
                        chips,
                    });
                }
                Incoming::Message(id, ToHost::Cursor { pos }) => {
                    if self.joined.contains_key(&id) {
                        self.broadcast(&ToClient::Cursor { id, pos }, Some(id));
                        events.push(Event::Cursor { id, pos });
                    }
                }
                Incoming::Closed(id) => {
                    self.pending.remove(&id);
                    if self.joined.remove(&id).is_some() {
                        if let Some(p) = self.players.remove(&id) {
                            self.broadcast(&ToClient::Left { id }, None);
                            events.push(Event::Left(p));
                        }
                    }
                }
            }
        }
        events
    }

    /// Send the host's own edit (already applied to the world).
    pub fn send_edit(&mut self, edit: Edit, chips: Vec<Package>) {
        self.broadcast(
            &ToClient::Edit {
                from: HOST_ID,
                edit,
                chips,
            },
            None,
        );
    }

    pub fn send_cursor(&mut self, pos: Option<Pos>) {
        self.broadcast(&ToClient::Cursor { id: HOST_ID, pos }, None);
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(c) = &self.relay_control {
            let _ = c.shutdown(Shutdown::Both);
        }
        for c in self.pending.values().chain(self.joined.values()) {
            let _ = c.stream.shutdown(Shutdown::Both);
        }
    }
}

/// A joined session.
pub struct Client {
    tx: Sender<String>,
    rx: Receiver<Option<ToClient>>,
    stream: TcpStream,
    you: Option<u32>,
    players: BTreeMap<u32, Player>,
    /// Own edits sent and not yet echoed; `true` once another player's edit
    /// was applied after it, so it must be applied again when echoed.
    pending: VecDeque<bool>,
    closed: bool,
}

/// `addr` with the default port if it has none.
fn with_port(addr: &str) -> String {
    let a = addr.trim();
    if a.parse::<std::net::SocketAddr>().is_ok()
        || a.rsplit_once(':')
            .is_some_and(|(h, p)| !h.contains(':') && p.parse::<u16>().is_ok())
    {
        a.to_string()
    } else {
        format!("{a}:{PORT}")
    }
}

impl Client {
    /// Connect to a host at `addr` (`192.168.1.20` or `192.168.1.20:7878`).
    pub fn connect(addr: &str, name: &str) -> io::Result<Client> {
        let addr = with_port(addr);
        let target = addr.to_socket_addrs()?.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                makine_lang::tr!("adres bulunamadı", "address not found"),
            )
        })?;
        let stream = TcpStream::connect_timeout(&target, Duration::from_secs(3))?;
        Self::over(stream, name)
    }

    /// Join room `code` on the relay at `relay`.
    pub fn join_room(relay: &str, code: &str, name: &str) -> io::Result<Client> {
        Self::over(relay::join(relay, code)?, name)
    }

    /// Say hello over a connected stream.
    fn over(stream: TcpStream, name: &str) -> io::Result<Client> {
        stream.set_nodelay(true)?;
        let tx = writer(stream.try_clone()?);
        let (mtx, rx) = channel();
        reader::<ToClient>(stream.try_clone()?, move |m| mtx.send(m).is_ok());
        let _ = tx.send(json(&ToHost::Hello {
            name: name.to_string(),
            protocol: PROTOCOL,
        }));
        Ok(Client {
            tx,
            rx,
            stream,
            you: None,
            players: BTreeMap::new(),
            pending: VecDeque::new(),
            closed: false,
        })
    }

    /// Our player id, once welcomed.
    pub fn id(&self) -> Option<u32> {
        self.you
    }

    pub fn players(&self) -> Vec<Player> {
        self.players.values().cloned().collect()
    }

    pub fn poll(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        while !self.closed {
            let Ok(m) = self.rx.try_recv() else { break };
            match m {
                None => {
                    self.closed = true;
                    events.push(Event::Closed(
                        makine_lang::tr!("bağlantı koptu", "connection lost").into(),
                    ));
                }
                Some(ToClient::Welcome {
                    you,
                    players,
                    world,
                    chips,
                }) => {
                    self.you = Some(you);
                    self.players = players.into_iter().map(|p| (p.id, p)).collect();
                    events.push(Event::Welcome { world, chips });
                }
                Some(ToClient::Refused { reason }) => {
                    self.closed = true;
                    events.push(Event::Closed(reason));
                }
                Some(ToClient::Edit { from, edit, chips }) => {
                    if Some(from) == self.you {
                        // Our own edit, now in the host's order. Apply it
                        // again only if someone else's came in between.
                        if self.pending.pop_front() == Some(true) {
                            events.push(Event::Edit { from, edit, chips });
                        }
                    } else {
                        for p in &mut self.pending {
                            *p = true;
                        }
                        events.push(Event::Edit { from, edit, chips });
                    }
                }
                Some(ToClient::Joined(p)) => {
                    if Some(p.id) != self.you {
                        self.players.insert(p.id, p.clone());
                        events.push(Event::Joined(p));
                    }
                }
                Some(ToClient::Left { id }) => {
                    if let Some(p) = self.players.remove(&id) {
                        events.push(Event::Left(p));
                    }
                }
                Some(ToClient::Cursor { id, pos }) => events.push(Event::Cursor { id, pos }),
            }
        }
        events
    }

    /// Send our own edit (already applied locally).
    pub fn send_edit(&mut self, edit: Edit, chips: Vec<Package>) {
        self.pending.push_back(false);
        let _ = self.tx.send(json(&ToHost::Edit { edit, chips }));
    }

    pub fn send_cursor(&mut self, pos: Option<Pos>) {
        let _ = self.tx.send(json(&ToHost::Cursor { pos }));
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

/// This computer's address on the local network, to tell other players.
pub fn local_ip() -> Option<std::net::IpAddr> {
    // Connecting a UDP socket sends nothing; it only picks the interface.
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.168.0.1:9")
        .or_else(|_| s.connect("10.0.0.1:9"))
        .ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports() {
        assert_eq!(with_port("192.168.1.5"), "192.168.1.5:7878");
        assert_eq!(with_port(" 192.168.1.5:9000 "), "192.168.1.5:9000");
        assert_eq!(with_port("ev-pc"), "ev-pc:7878");
        assert_eq!(with_port("ev-pc:80"), "ev-pc:80");
        assert_eq!(with_port("[::1]:80"), "[::1]:80");
    }

    #[test]
    fn names() {
        assert_eq!(clean_name("  ", 3), "oyuncu 3");
        assert_eq!(clean_name("ali\n", 1), "ali");
        assert_eq!(clean_name(&"x".repeat(50), 1).len(), 24);
    }
}

#[cfg(test)]
mod relay_tests {
    use super::*;
    use makine_world::{Kind, Placed};
    use std::time::Instant;

    fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
        let start = Instant::now();
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(start.elapsed() < Duration::from_secs(5), "timed out");
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// A host opens a room on a relay; a guest joins with the code and the
    /// two share a world as on a local network.
    #[test]
    fn worlds_are_shared_through_a_relay() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let relay = listener.local_addr().unwrap().to_string();
        thread::spawn(move || relay::run(listener));

        let mut host = Host::start("ev", 0).unwrap();
        let code = host.open_room(&relay).unwrap();
        assert_eq!(code.len(), 6);
        let mut world = World::new("ortak");
        world.apply(Edit::Part {
            id: 1,
            part: Some(Placed {
                kind: Kind::Lamp,
                pos: Pos::new(0.0, 0.0),
            }),
        });

        let mut guest = Client::join_room(&relay, &code.to_uppercase(), "misafir").unwrap();
        let got = wait(|| {
            host.poll(&mut world, |_| vec![]);
            guest.poll().into_iter().find_map(|e| match e {
                Event::Welcome { world, .. } => Some(world),
                _ => None,
            })
        });
        assert_eq!(got.len(), 1);
        assert_eq!(host.players().len(), 2);

        // An edit from the guest reaches the host's world.
        guest.send_edit(
            Edit::Part {
                id: 2,
                part: Some(Placed {
                    kind: Kind::Lamp,
                    pos: Pos::new(50.0, 0.0),
                }),
            },
            vec![],
        );
        wait(|| {
            host.poll(&mut world, |_| vec![]);
            (world.len() == 2).then_some(())
        });

        assert!(Client::join_room(&relay, "yokyok", "x").is_err());
    }
}
