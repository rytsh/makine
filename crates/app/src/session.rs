//! Shared worlds on the local network: the app side of `makine-net`.
//!
//! The session moves edits between the world view and the network every
//! frame. Free chips placed as blocks travel with the edits as packages,
//! so every player can simulate them.

use std::collections::{BTreeMap, BTreeSet};

use egui::Color32;
use makine_lang::{tr, trf};
use makine_net::{Client, Event, Host, Player};
use makine_pack::Package;
use makine_sim::Library;
use makine_world::{Edit, Pos, World};

use crate::sandbox::{self, Sandbox};
use crate::worldview::WorldView;

pub enum Link {
    Host(Host),
    Client(Client),
}

pub struct Session {
    pub link: Link,
    /// Status line for the panel.
    pub status: String,
    last_cursor: Option<Pos>,
    cursors: BTreeMap<u32, Pos>,
    /// Whether the joined world has arrived.
    pub ready: bool,
}

/// A colour per player, the same on every screen.
pub fn color(id: u32) -> Color32 {
    const C: [Color32; 6] = [
        Color32::from_rgb(255, 120, 120),
        Color32::from_rgb(120, 200, 255),
        Color32::from_rgb(140, 230, 140),
        Color32::from_rgb(255, 200, 90),
        Color32::from_rgb(220, 140, 255),
        Color32::from_rgb(120, 230, 220),
    ];
    C[id as usize % C.len()]
}

/// Free chips placed by `e` (with their own dependencies), as packages.
fn chips_in_edit(e: &Edit, lib: &Library, sandbox: &Sandbox) -> Vec<Package> {
    fn names(e: &Edit, out: &mut BTreeSet<String>) {
        match e {
            Edit::Part { part: Some(p), .. } => {
                if let Some(n) = p.kind.chip_name() {
                    out.insert(n.to_string());
                }
            }
            Edit::Batch(v) => v.iter().for_each(|e| names(e, out)),
            _ => {}
        }
    }
    let mut n = BTreeSet::new();
    names(e, &mut n);
    n.into_iter()
        .filter(|n| sandbox.chips.contains_key(n))
        .filter_map(|n| sandbox::export(lib, sandbox, &n).ok())
        .collect()
}

/// Free chips used anywhere in the world.
pub fn chips_in_world(w: &World, lib: &Library, sandbox: &Sandbox) -> Vec<Package> {
    let edit = Edit::Batch(
        w.parts()
            .map(|(id, p)| Edit::Part {
                id,
                part: Some(p.clone()),
            })
            .collect(),
    );
    chips_in_edit(&edit, lib, sandbox)
}

/// Install received chips; returns whether any were new. Chips that clash
/// with the player's own (same name, different contents) are skipped and
/// reported.
fn install(chips: &[Package], lib: &Library, sandbox: &mut Sandbox, status: &mut String) -> bool {
    let mut added = false;
    for p in chips {
        let before = sandbox.chips.len();
        match sandbox::install(lib, sandbox, p) {
            Ok(_) => added |= sandbox.chips.len() > before,
            Err(e) => *status = trf!("Bir çip eklenemedi: {e}", "Could not add a chip: {e}"),
        }
    }
    added
}

impl Session {
    pub fn host(name: &str, port: u16) -> std::io::Result<Session> {
        let h = Host::start(name, port)?;
        let ip = makine_net::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| {
                tr!("bu bilgisayarın IP adresi", "this computer's IP address").into()
            });
        let port = h.port();
        let addr = if port == makine_net::PORT {
            ip
        } else {
            format!("{ip}:{port}")
        };
        Ok(Session {
            link: Link::Host(h),
            status: trf!(
                "Paylaşımda. Arkadaşların şu adrese bağlansın: {addr}",
                "Sharing. Your friends should connect to: {addr}"
            ),
            last_cursor: None,
            cursors: BTreeMap::new(),
            ready: true,
        })
    }

    /// Open the hosted session on an internet relay too.
    pub fn open_room(&mut self, relay: &str) -> std::io::Result<String> {
        let Link::Host(h) = &mut self.link else {
            return Err(std::io::Error::other("not hosting"));
        };
        let code = h.open_room(relay)?;
        self.status = trf!(
            "İnternette de açık. Oda kodu: {code} (sunucu: {relay})",
            "Also open on the internet. Room code: {code} (server: {relay})"
        );
        Ok(code)
    }

    pub fn room(&self) -> Option<&str> {
        match &self.link {
            Link::Host(h) => h.room(),
            Link::Client(_) => None,
        }
    }

    /// Join room `code` on the relay at `relay`.
    pub fn join_room(relay: &str, code: &str, name: &str) -> std::io::Result<Session> {
        let c = Client::join_room(relay, code, name)?;
        Ok(Session {
            link: Link::Client(c),
            status: tr!("Bağlanılıyor…", "Connecting…").into(),
            last_cursor: None,
            cursors: BTreeMap::new(),
            ready: false,
        })
    }

    pub fn join(addr: &str, name: &str) -> std::io::Result<Session> {
        let c = Client::connect(addr, name)?;
        Ok(Session {
            link: Link::Client(c),
            status: tr!("Bağlanılıyor…", "Connecting…").into(),
            last_cursor: None,
            cursors: BTreeMap::new(),
            ready: false,
        })
    }

    pub fn is_host(&self) -> bool {
        matches!(self.link, Link::Host(_))
    }

    pub fn players(&self) -> Vec<Player> {
        match &self.link {
            Link::Host(h) => h.players(),
            Link::Client(c) => c.players(),
        }
    }

    pub fn my_id(&self) -> Option<u32> {
        match &self.link {
            Link::Host(_) => Some(makine_net::HOST_ID),
            Link::Client(c) => c.id(),
        }
    }

    /// Exchange edits and cursors. Returns `Some(reason)` when the session
    /// ended, and whether the library must be rebuilt (new chips).
    pub fn tick(
        &mut self,
        view: &mut WorldView,
        lib: &Library,
        sandbox: &mut Sandbox,
    ) -> (Option<String>, bool) {
        let mut new_chips = false;
        // Our edits out, once joined. Before the shared world arrives the
        // view shows our own world, and edits to it are not shared.
        view.sharing = self.ready;
        let outgoing = std::mem::take(&mut view.outbox);
        if self.ready {
            for e in outgoing {
                let chips = chips_in_edit(&e, lib, sandbox);
                match &mut self.link {
                    Link::Host(h) => h.send_edit(e, chips),
                    Link::Client(c) => c.send_edit(e, chips),
                }
            }
            // Only moves of a few points are sent.
            let cursor = view.hover().map(|p| Pos::new(p.x.round(), p.y.round()));
            if cursor != self.last_cursor {
                self.last_cursor = cursor;
                match &mut self.link {
                    Link::Host(h) => h.send_cursor(cursor),
                    Link::Client(c) => c.send_cursor(cursor),
                }
            }
        }

        let host = matches!(self.link, Link::Host(_));
        let events = match &mut self.link {
            Link::Host(h) => {
                let shared: &Sandbox = sandbox;
                // Players' edits are applied to the world inside poll.
                let rev = view.world.revision();
                let events = h.poll(&mut view.world, |w| chips_in_world(w, lib, shared));
                if view.world.revision() != rev {
                    view.after_remote_edits();
                }
                events
            }
            Link::Client(c) => c.poll(),
        };
        let mut closed = None;
        for e in events {
            match e {
                Event::Welcome { world, chips } => {
                    install(&chips, lib, sandbox, &mut self.status);
                    let name = world.name.clone();
                    *view = WorldView::new(world.checked());
                    view.sharing = true;
                    // The app refreshes the library (with the new chips)
                    // and hands it to the new view.
                    new_chips = true;
                    self.ready = true;
                    self.status = trf!(
                        "'{name}' dünyasına bağlandın.",
                        "You joined the world '{name}'."
                    );
                }
                Event::Edit { edit, chips, .. } => {
                    new_chips |= install(&chips, lib, sandbox, &mut self.status);
                    if !host {
                        view.apply_remote(edit);
                    }
                }
                Event::Joined(p) => self.status = trf!("{} katıldı.", "{} joined.", p.name),
                Event::Left(p) => {
                    self.cursors.remove(&p.id);
                    self.status = trf!("{} ayrıldı.", "{} left.", p.name);
                }
                Event::Cursor { id, pos } => match pos {
                    Some(p) => {
                        self.cursors.insert(id, p);
                    }
                    None => {
                        self.cursors.remove(&id);
                    }
                },
                Event::Closed(reason) => closed = Some(reason),
            }
        }
        let names: BTreeMap<u32, String> =
            self.players().into_iter().map(|p| (p.id, p.name)).collect();
        view.others = self
            .cursors
            .iter()
            .filter(|(id, _)| Some(**id) != self.my_id())
            .filter_map(|(id, p)| Some((names.get(id)?.clone(), color(*id), *p)))
            .collect();
        (closed, new_chips)
    }
}

/// A default player name: the user's login name.
pub fn default_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "oyuncu".into())
}
