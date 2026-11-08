//! The war councils' env server (#100): newline-delimited JSON on stdin/stdout. See
//! `protocol.rs` for the messages.
//!
//! ```text
//! cargo build --release -p moc-civ --example civ_gym
//! ```

mod protocol;

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut out = io::BufWriter::new(io::stdout().lock());
    let mut server = protocol::Server::default();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        match server.line(&line) {
            Some(reply) => {
                let _ = writeln!(out, "{reply}");
                let _ = out.flush();
            }
            None => break,
        }
    }
}
