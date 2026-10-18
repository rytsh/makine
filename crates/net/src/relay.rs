//! Playing over the internet through a relay: a small server both sides
//! connect *out* to, so nobody needs to open a port on their router.
//!
//! The host opens a control connection and gets a room code:
//!
//! ```text
//! host → relay   HOST
//! relay → host   ROOM k7qx2m
//! ```
//!
//! A guest asks for the room; the relay tells the host on the control
//! connection, the host opens a new connection for that guest, and the
//! relay joins the two. From then on the bytes pass through untouched, so
//! the game's own protocol is the same as on a local network.
//!
//! ```text
//! guest → relay  JOIN k7qx2m
//! relay → host   GUEST 3
//! host → relay   ACCEPT k7qx2m 3
//! relay → guest  OK            (then the two streams are spliced)
//! ```

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Default relay port.
pub const RELAY_PORT: u16 = 7879;
const MAX_ROOMS: usize = 256;
/// Longest a guest waits for the host to pick up.
const ACCEPT_WAIT: Duration = Duration::from_secs(10);

struct Room {
    control: TcpStream,
    next_guest: u32,
    waiting: HashMap<u32, TcpStream>,
}

#[derive(Clone, Default)]
struct Rooms(Arc<Mutex<HashMap<String, Room>>>);

fn code() -> String {
    const LETTERS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    let mut v = h.finish();
    (0..6)
        .map(|_| {
            let c = LETTERS[(v % LETTERS.len() as u64) as usize] as char;
            v /= LETTERS.len() as u64;
            c
        })
        .collect()
}

/// Read one short line (the handshake), without buffering past it.
fn read_line(s: &mut TcpStream) -> io::Result<String> {
    let mut line = Vec::new();
    let mut b = [0u8; 1];
    while line.len() < 128 {
        if s.read(&mut b)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        if b[0] == b'\n' {
            return String::from_utf8(line).map_err(|_| io::ErrorKind::InvalidData.into());
        }
        line.push(b[0]);
    }
    Err(io::ErrorKind::InvalidData.into())
}

/// Copy both ways until either side closes.
fn splice(a: TcpStream, b: TcpStream) {
    let (a2, b2) = match (a.try_clone(), b.try_clone()) {
        (Ok(a2), Ok(b2)) => (a2, b2),
        _ => return,
    };
    let pipe = |mut from: TcpStream, mut to: TcpStream| {
        thread::spawn(move || {
            let _ = io::copy(&mut from, &mut to);
            let _ = to.shutdown(Shutdown::Both);
            let _ = from.shutdown(Shutdown::Both);
        });
    };
    pipe(a, b2);
    pipe(b, a2);
}

impl Rooms {
    fn handle(&self, mut s: TcpStream) -> io::Result<()> {
        s.set_read_timeout(Some(Duration::from_secs(10)))?;
        let line = read_line(&mut s)?;
        s.set_read_timeout(None)?;
        let mut words = line.split_whitespace();
        match (words.next(), words.next(), words.next()) {
            (Some("HOST"), None, _) => {
                let mut rooms = self.0.lock().unwrap();
                if rooms.len() >= MAX_ROOMS {
                    writeln!(s, "ERR full")?;
                    return Ok(());
                }
                let mut c = code();
                while rooms.contains_key(&c) {
                    c = code();
                }
                writeln!(s, "ROOM {c}")?;
                let watch = s.try_clone()?;
                rooms.insert(
                    c.clone(),
                    Room {
                        control: s,
                        next_guest: 1,
                        waiting: HashMap::new(),
                    },
                );
                drop(rooms);
                // The room lives as long as its control connection.
                let me = self.clone();
                thread::spawn(move || {
                    let mut buf = [0u8; 64];
                    let mut w = watch;
                    while matches!(w.read(&mut buf), Ok(n) if n > 0) {}
                    me.0.lock().unwrap().remove(&c);
                });
            }
            (Some("JOIN"), Some(room), None) => {
                let id = {
                    let mut rooms = self.0.lock().unwrap();
                    let Some(r) = rooms.get_mut(room) else {
                        writeln!(s, "ERR room")?;
                        return Ok(());
                    };
                    let id = r.next_guest;
                    r.next_guest += 1;
                    if writeln!(r.control, "GUEST {id}").is_err() {
                        writeln!(s, "ERR room")?;
                        return Ok(());
                    }
                    r.waiting.insert(id, s.try_clone()?);
                    id
                };
                // Give up if the host does not pick up in time.
                let me = self.clone();
                let room = room.to_string();
                thread::spawn(move || {
                    thread::sleep(ACCEPT_WAIT);
                    let left =
                        me.0.lock()
                            .unwrap()
                            .get_mut(&room)
                            .and_then(|r| r.waiting.remove(&id));
                    if let Some(mut g) = left {
                        let _ = writeln!(g, "ERR timeout");
                        let _ = g.shutdown(Shutdown::Both);
                    }
                });
            }
            (Some("ACCEPT"), Some(room), Some(id)) => {
                let guest = id.parse::<u32>().ok().and_then(|id| {
                    self.0
                        .lock()
                        .unwrap()
                        .get_mut(room)
                        .and_then(|r| r.waiting.remove(&id))
                });
                let Some(mut g) = guest else {
                    return Ok(());
                };
                writeln!(g, "OK")?;
                splice(s, g);
            }
            _ => writeln!(s, "ERR hello")?,
        }
        Ok(())
    }
}

/// Run a relay on `addr` until the process ends.
pub fn serve(addr: &str) -> io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!("makine relay on {}", listener.local_addr()?);
    run(listener);
    Ok(())
}

/// Serve on an already bound listener (for tests: port 0).
pub fn run(listener: TcpListener) {
    let rooms = Rooms::default();
    for s in listener.incoming().flatten() {
        let _ = s.set_nodelay(true);
        let rooms = rooms.clone();
        thread::spawn(move || {
            let _ = rooms.handle(s);
        });
    }
}

/// `addr` with the default relay port if it has none.
pub fn relay_addr(addr: &str) -> String {
    let a = addr.trim();
    if a.rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
    {
        a.to_string()
    } else {
        format!("{a}:{RELAY_PORT}")
    }
}

pub(crate) fn connect(relay: &str) -> io::Result<TcpStream> {
    let addr = relay_addr(relay);
    let target = addr.to_socket_addrs()?.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            makine_lang::tr!("sunucu bulunamadı", "server not found"),
        )
    })?;
    let s = TcpStream::connect_timeout(&target, Duration::from_secs(5))?;
    s.set_nodelay(true)?;
    Ok(s)
}

/// Open a room: returns its code and the control connection, whose lines
/// are `GUEST n` for each guest asking in.
pub(crate) fn open_room(relay: &str) -> io::Result<(String, BufReader<TcpStream>)> {
    let mut s = connect(relay)?;
    writeln!(s, "HOST")?;
    let line = read_line(&mut s)?;
    match line.strip_prefix("ROOM ") {
        Some(code) => Ok((code.to_string(), BufReader::new(s))),
        None => Err(io::Error::other(makine_lang::tr!(
            "sunucu oda açamadı (dolu olabilir)",
            "the server could not open a room (it may be full)"
        ))),
    }
}

/// Pick up guest `id` of `room`: a stream to them, ready for the game.
pub(crate) fn accept(relay: &str, room: &str, id: u32) -> io::Result<TcpStream> {
    let mut s = connect(relay)?;
    writeln!(s, "ACCEPT {room} {id}")?;
    Ok(s)
}

/// Ask to join `room`: once the host picks up, the stream is theirs.
pub(crate) fn join(relay: &str, room: &str) -> io::Result<TcpStream> {
    let mut s = connect(relay)?;
    writeln!(s, "JOIN {}", room.trim().to_lowercase())?;
    s.set_read_timeout(Some(ACCEPT_WAIT + Duration::from_secs(2)))?;
    let line = read_line(&mut s)?;
    s.set_read_timeout(None)?;
    match line.as_str() {
        "OK" => Ok(s),
        "ERR room" => Err(io::Error::new(
            io::ErrorKind::NotFound,
            makine_lang::tr!("bu kodla bir oda yok", "there is no room with this code"),
        )),
        _ => Err(io::Error::other(makine_lang::tr!(
            "oyunu açan cevap vermedi",
            "the host did not answer"
        ))),
    }
}

/// Read `GUEST n` lines from a room's control connection.
pub(crate) fn guests(control: BufReader<TcpStream>, mut on: impl FnMut(u32) -> bool) {
    for line in control.lines() {
        let Ok(line) = line else { break };
        if let Some(id) = line.strip_prefix("GUEST ").and_then(|n| n.parse().ok()) {
            if !on(id) {
                break;
            }
        }
    }
}
