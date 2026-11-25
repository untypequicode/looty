//! looty-bridge: agent of the Looty PC tunnel (protocol v1, docs/tunnel-protocol.md).

pub mod config;
pub mod latex_service;
pub mod proto;
pub mod session;

use std::time::{SystemTime, UNIX_EPOCH};

/// Status line on stderr, with a UTC time. The token is never passed here.
pub fn log(msg: impl AsRef<str>) {
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day = s % 86_400;
    eprintln!(
        "[{:02}:{:02}:{:02}Z] {}",
        day / 3600,
        day % 3600 / 60,
        day % 60,
        msg.as_ref()
    );
}
