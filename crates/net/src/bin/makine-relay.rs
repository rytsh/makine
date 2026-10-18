//! The relay server for playing makine over the internet. Run it on any
//! machine both players can reach (a small cloud server is enough):
//!
//! ```sh
//! makine-relay              # listens on 0.0.0.0:7879
//! makine-relay 0.0.0.0:9000
//! ```
//!
//! It only passes bytes between players who agreed on a room code; it
//! does not look at or keep the worlds.

fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("0.0.0.0:{}", makine_net::relay::RELAY_PORT));
    if let Err(e) = makine_net::relay::serve(&addr) {
        eprintln!("makine-relay: {addr}: {e}");
        std::process::exit(1);
    }
}
