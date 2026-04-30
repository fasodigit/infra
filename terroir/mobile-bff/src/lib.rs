// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-mobile-bff — bibliothèque
//
// BFF orienté app mobile (RN+Expo) : pagination légère, batch sync,
// merge Yjs CRDT côté serveur, broadcast aux clients via WebSocket.

#![forbid(unsafe_code)]

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub const HTTP_PORT: u16 = 8833;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!version().is_empty());
    }
}
