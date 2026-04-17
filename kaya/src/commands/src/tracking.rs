//! RESP3 CLIENT TRACKING* command handlers and push-frame builder.
//!
//! This module implements:
//!
//! * `CLIENT TRACKING ON|OFF [REDIRECT client-id] [BCAST] [PREFIX prefix ...] [OPTIN] [OPTOUT] [NOLOOP]`
//! * `CLIENT TRACKINGINFO`
//! * `CLIENT CACHING YES|NO`
//! * `CLIENT NO-EVICT ON|OFF`
//! * `CLIENT NO-TOUCH ON|OFF`
//!
//! It exposes a [`TrackingCommandHandler`] that wraps an `Arc<TrackingTable>`
//! and a per-connection `ClientId`. The main `CommandHandler::client_cmd`
//! forwards `CLIENT TRACKING*` sub-commands here after looking up the right
//! `TrackingCommandHandler` for the current connection.
//!
//! The `build_invalidation_push` free function is the single source of truth
//! for the RESP3 `>invalidate` push frame emitted to clients when a key is
//! mutated.

use std::sync::Arc;

use bytes::Bytes;
use kaya_network::tracking::{ClientId, ClientOptions, TrackingTable};
use kaya_protocol::Frame;
use tracing::instrument;

// ---------------------------------------------------------------------------
// Public push-frame builder
// ---------------------------------------------------------------------------

/// Build a RESP3 `> invalidate <keys>` push frame from a slice of raw key
/// bytes. This is the canonical format sent on the client's outbound channel
/// when a tracked key is mutated.
///
/// Wire encoding (RESP3 Push):
///
/// ```text
/// >2\r\n
/// $10\r\ninvalidate\r\n
/// *N\r\n
/// $len\r\n<key1>\r\n
/// ...
/// ```
pub fn build_invalidation_push(keys: &[Vec<u8>]) -> Frame {
    let key_frames: Vec<Frame> = keys
        .iter()
        .map(|k| Frame::BulkString(Bytes::copy_from_slice(k)))
        .collect();
    Frame::Push(vec![
        Frame::BulkString(Bytes::from_static(b"invalidate")),
        Frame::Array(key_frames),
    ])
}

// ---------------------------------------------------------------------------
// Per-connection tracking command handler
// ---------------------------------------------------------------------------

/// Error type for CLIENT TRACKING* parse/state failures.
#[derive(Debug, thiserror::Error)]
pub enum TrackingCmdError {
    #[error("syntax error: {0}")]
    Syntax(String),
    #[error("unknown subcommand: {0}")]
    UnknownSubcommand(String),
}

impl TrackingCmdError {
    pub fn to_frame(&self) -> Frame {
        Frame::Error(format!("ERR {self}"))
    }
}

/// Handles `CLIENT TRACKING*` sub-commands for a single connection.
///
/// Callers should create one per connection and keep it alive for the
/// connection's lifetime.
#[derive(Clone)]
pub struct TrackingCommandHandler {
    pub client_id: ClientId,
    pub table: Arc<TrackingTable>,
}

impl std::fmt::Debug for TrackingCommandHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackingCommandHandler")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

impl TrackingCommandHandler {
    /// Construct a handler bound to a specific connection and table.
    pub fn new(client_id: ClientId, table: Arc<TrackingTable>) -> Self {
        Self { client_id, table }
    }

    // -----------------------------------------------------------------------
    // CLIENT TRACKING ON|OFF ...
    // -----------------------------------------------------------------------

    /// Handle `CLIENT TRACKING ON|OFF [options...]`.
    ///
    /// Grammar (case-insensitive):
    /// ```text
    /// CLIENT TRACKING ON|OFF
    ///   [REDIRECT <client-id>]
    ///   [BCAST]
    ///   [PREFIX <prefix> [PREFIX <prefix> ...]]
    ///   [OPTIN]
    ///   [OPTOUT]
    ///   [NOLOOP]
    /// ```
    #[instrument(level = "debug", skip(self, args))]
    pub fn handle_tracking(&self, args: &[Bytes]) -> Result<Frame, TrackingCmdError> {
        // args[0] is expected to be ON or OFF (the caller already consumed
        // "TRACKING" from the CLIENT command's argument list, so args[0]
        // is the ON|OFF token).
        let toggle = args
            .first()
            .ok_or_else(|| TrackingCmdError::Syntax("missing ON|OFF".into()))?;

        let toggle_upper = std::str::from_utf8(toggle)
            .map_err(|_| TrackingCmdError::Syntax("non-UTF8 toggle".into()))?
            .to_ascii_uppercase();

        match toggle_upper.as_str() {
            "OFF" => {
                self.table.disable(self.client_id);
                return Ok(Frame::ok());
            }
            "ON" => {}
            other => {
                return Err(TrackingCmdError::Syntax(format!(
                    "expected ON or OFF, got {other}"
                )));
            }
        }

        // Parse optional flags from args[1..].
        let mut bcast = false;
        let mut prefixes: Vec<Bytes> = Vec::new();
        let mut opt_in = false;
        let mut opt_out = false;
        let mut no_loop = false;
        let mut redirect: Option<ClientId> = None;

        let mut i = 1usize;
        while i < args.len() {
            let flag = std::str::from_utf8(&args[i])
                .map_err(|_| TrackingCmdError::Syntax("non-UTF8 flag".into()))?
                .to_ascii_uppercase();

            match flag.as_str() {
                "BCAST" => {
                    bcast = true;
                    i += 1;
                }
                "OPTIN" => {
                    opt_in = true;
                    i += 1;
                }
                "OPTOUT" => {
                    opt_out = true;
                    i += 1;
                }
                "NOLOOP" => {
                    no_loop = true;
                    i += 1;
                }
                "PREFIX" => {
                    i += 1;
                    let p = args.get(i).ok_or_else(|| {
                        TrackingCmdError::Syntax("PREFIX requires an argument".into())
                    })?;
                    prefixes.push(p.clone());
                    i += 1;
                }
                "REDIRECT" => {
                    i += 1;
                    let raw = args.get(i).ok_or_else(|| {
                        TrackingCmdError::Syntax("REDIRECT requires a client-id".into())
                    })?;
                    let s = std::str::from_utf8(raw)
                        .map_err(|_| TrackingCmdError::Syntax("non-UTF8 redirect id".into()))?;
                    let cid: ClientId = s.parse().map_err(|_| {
                        TrackingCmdError::Syntax(format!("invalid client-id: {s}"))
                    })?;
                    redirect = Some(cid);
                    i += 1;
                }
                unknown => {
                    return Err(TrackingCmdError::Syntax(format!(
                        "unknown TRACKING option: {unknown}"
                    )));
                }
            }
        }

        if opt_in && opt_out {
            return Err(TrackingCmdError::Syntax(
                "OPTIN and OPTOUT are mutually exclusive".into(),
            ));
        }

        let options = ClientOptions {
            opt_in,
            opt_out,
            no_loop,
            no_evict: false,
            caching_override: None,
            redirect,
        };

        if bcast {
            self.table
                .enable_bcast_with_options(self.client_id, prefixes.into_iter().collect(), options);
        } else {
            self.table
                .enable_default_with_options(self.client_id, options);
        }

        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // CLIENT TRACKINGINFO
    // -----------------------------------------------------------------------

    /// Handle `CLIENT TRACKINGINFO`.
    ///
    /// Returns a RESP3 Map with keys: `flags`, `redirect`, `prefixes`.
    pub fn handle_trackinginfo(&self) -> Frame {
        let info = self.table.info(self.client_id);

        // Build `flags` array.
        let mut flags: Vec<Frame> = Vec::new();
        if info.mode.is_on() {
            flags.push(Frame::BulkString(Bytes::from_static(b"on")));
        } else {
            flags.push(Frame::BulkString(Bytes::from_static(b"off")));
        }
        match &info.mode {
            kaya_network::tracking::TrackingMode::Bcast(_) => {
                flags.push(Frame::BulkString(Bytes::from_static(b"bcast")));
            }
            kaya_network::tracking::TrackingMode::Default => {}
            kaya_network::tracking::TrackingMode::Off => {}
        }
        if info.options.opt_in {
            flags.push(Frame::BulkString(Bytes::from_static(b"optin")));
        }
        if info.options.opt_out {
            flags.push(Frame::BulkString(Bytes::from_static(b"optout")));
        }
        if info.options.no_loop {
            flags.push(Frame::BulkString(Bytes::from_static(b"noloop")));
        }
        if info.options.no_evict {
            flags.push(Frame::BulkString(Bytes::from_static(b"noevict")));
        }

        // Redirect.
        let redirect_frame = match info.options.redirect {
            Some(cid) => Frame::Integer(cid as i64),
            None => Frame::Integer(-1),
        };

        // Prefixes (only meaningful in BCAST mode).
        let prefixes_frame = match &info.mode {
            kaya_network::tracking::TrackingMode::Bcast(pfxs) => Frame::Array(
                pfxs.iter()
                    .map(|p| Frame::BulkString(p.clone()))
                    .collect(),
            ),
            _ => Frame::Array(vec![]),
        };

        Frame::Map(vec![
            (
                Frame::BulkString(Bytes::from_static(b"flags")),
                Frame::Array(flags),
            ),
            (
                Frame::BulkString(Bytes::from_static(b"redirect")),
                redirect_frame,
            ),
            (
                Frame::BulkString(Bytes::from_static(b"prefixes")),
                prefixes_frame,
            ),
        ])
    }

    // -----------------------------------------------------------------------
    // CLIENT CACHING YES|NO
    // -----------------------------------------------------------------------

    /// Handle `CLIENT CACHING YES|NO`.
    pub fn handle_caching(&self, args: &[Bytes]) -> Result<Frame, TrackingCmdError> {
        let value_raw = args
            .first()
            .ok_or_else(|| TrackingCmdError::Syntax("missing YES|NO".into()))?;
        let value_str = std::str::from_utf8(value_raw)
            .map_err(|_| TrackingCmdError::Syntax("non-UTF8 YES|NO".into()))?
            .to_ascii_uppercase();

        let enabled = match value_str.as_str() {
            "YES" => true,
            "NO" => false,
            other => {
                return Err(TrackingCmdError::Syntax(format!(
                    "expected YES or NO, got {other}"
                )));
            }
        };

        self.table.set_caching_override(self.client_id, enabled);
        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // CLIENT NO-EVICT ON|OFF
    // -----------------------------------------------------------------------

    /// Handle `CLIENT NO-EVICT ON|OFF`.
    pub fn handle_no_evict(&self, args: &[Bytes]) -> Result<Frame, TrackingCmdError> {
        let value_raw = args
            .first()
            .ok_or_else(|| TrackingCmdError::Syntax("missing ON|OFF".into()))?;
        let value_str = std::str::from_utf8(value_raw)
            .map_err(|_| TrackingCmdError::Syntax("non-UTF8 ON|OFF".into()))?
            .to_ascii_uppercase();

        let enabled = match value_str.as_str() {
            "ON" => true,
            "OFF" => false,
            other => {
                return Err(TrackingCmdError::Syntax(format!(
                    "expected ON or OFF, got {other}"
                )));
            }
        };

        self.table.set_no_evict(self.client_id, enabled);
        Ok(Frame::ok())
    }

    // -----------------------------------------------------------------------
    // CLIENT NO-TOUCH ON|OFF
    // -----------------------------------------------------------------------

    /// Handle `CLIENT NO-TOUCH ON|OFF`.
    ///
    /// `NO-TOUCH` prevents the server from updating the LRU/LFU clock for
    /// keys touched by this client. We store the flag in the connection's
    /// metadata (not in `TrackingTable`), so here we simply acknowledge it
    /// and return OK — the flag is honoured by the store layer when provided
    /// via the command context.
    pub fn handle_no_touch(&self, args: &[Bytes]) -> Result<Frame, TrackingCmdError> {
        let value_raw = args
            .first()
            .ok_or_else(|| TrackingCmdError::Syntax("missing ON|OFF".into()))?;
        let value_str = std::str::from_utf8(value_raw)
            .map_err(|_| TrackingCmdError::Syntax("non-UTF8 ON|OFF".into()))?
            .to_ascii_uppercase();

        match value_str.as_str() {
            "ON" | "OFF" => Ok(Frame::ok()),
            other => Err(TrackingCmdError::Syntax(format!(
                "expected ON or OFF, got {other}"
            ))),
        }
    }

    // -----------------------------------------------------------------------
    // Unified dispatch for CLIENT sub-commands handled here
    // -----------------------------------------------------------------------

    /// Dispatch a `CLIENT <subcommand> [args...]` call where `subcommand_upper`
    /// is already uppercased and `rest` are the remaining args bytes.
    ///
    /// Returns `None` if the sub-command is not a tracking-related one (so
    /// the caller can fall through to its own dispatch table).
    pub fn dispatch(
        &self,
        subcommand_upper: &str,
        rest: &[Bytes],
    ) -> Option<Result<Frame, TrackingCmdError>> {
        match subcommand_upper {
            "TRACKING" => Some(self.handle_tracking(rest)),
            "TRACKINGINFO" => Some(Ok(self.handle_trackinginfo())),
            "CACHING" => Some(self.handle_caching(rest)),
            "NO-EVICT" => Some(self.handle_no_evict(rest)),
            "NO-TOUCH" => Some(self.handle_no_touch(rest)),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dashmap::DashMap;
    use kaya_network::tracking::TrackingTable;
    use kaya_protocol::{Encoder, Frame};
    use tokio::sync::mpsc;

    // Helper: build a handler for a synthetic client.
    fn make_handler(client_id: ClientId) -> (TrackingCommandHandler, Arc<TrackingTable>) {
        let table = Arc::new(TrackingTable::new());
        let handler = TrackingCommandHandler::new(client_id, table.clone());
        (handler, table)
    }

    // -----------------------------------------------------------------------
    // 1. build_invalidation_push format
    // -----------------------------------------------------------------------

    #[test]
    fn build_invalidation_push_produces_correct_resp3_push_frame() {
        let keys: Vec<Vec<u8>> = vec![b"user:1".to_vec(), b"user:2".to_vec()];
        let frame = build_invalidation_push(&keys);

        // Encode and check wire bytes.
        let mut buf = bytes::BytesMut::new();
        Encoder::encode(&frame, &mut buf);
        let wire = std::str::from_utf8(&buf).unwrap();

        assert!(wire.starts_with(">2\r\n"), "push header '>2'");
        assert!(wire.contains("$10\r\ninvalidate\r\n"), "invalidate label");
        assert!(wire.contains("*2\r\n"), "array of 2 keys");
        assert!(wire.contains("$6\r\nuser:1\r\n"), "key user:1");
        assert!(wire.contains("$6\r\nuser:2\r\n"), "key user:2");

        // Structural check.
        match frame {
            Frame::Push(ref parts) => {
                assert_eq!(parts.len(), 2);
                assert!(
                    matches!(&parts[0], Frame::BulkString(b) if b.as_ref() == b"invalidate")
                );
                match &parts[1] {
                    Frame::Array(keys) => assert_eq!(keys.len(), 2),
                    other => panic!("expected Array, got {other:?}"),
                }
            }
            other => panic!("expected Push, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 2. CLIENT TRACKING ON registers the client
    // -----------------------------------------------------------------------

    #[test]
    fn client_tracking_on_registers_client() {
        let (h, table) = make_handler(10);
        let args: Vec<Bytes> = vec![Bytes::from_static(b"ON")];
        let resp = h.handle_tracking(&args).unwrap();
        assert_eq!(resp, Frame::ok());
        assert_eq!(table.client_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 3. CLIENT TRACKING OFF unregisters the client
    // -----------------------------------------------------------------------

    #[test]
    fn client_tracking_off_unregisters_client() {
        let (h, table) = make_handler(11);

        // First register.
        h.handle_tracking(&[Bytes::from_static(b"ON")]).unwrap();
        assert_eq!(table.client_count(), 1);

        // Then disable.
        h.handle_tracking(&[Bytes::from_static(b"OFF")]).unwrap();
        assert_eq!(table.client_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 4. CLIENT TRACKING ON BCAST PREFIX registers prefix in BCAST mode
    //    and invalidation reaches the client without a prior read.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn bcast_prefix_invalidation_without_prior_read() {
        let (h, table) = make_handler(20);

        let args: Vec<Bytes> = vec![
            Bytes::from_static(b"ON"),
            Bytes::from_static(b"BCAST"),
            Bytes::from_static(b"PREFIX"),
            Bytes::from_static(b"sess:"),
        ];
        h.handle_tracking(&args).unwrap();

        // Build sender map.
        let senders: DashMap<ClientId, mpsc::Sender<Frame>> = DashMap::new();
        let (tx, mut rx) = mpsc::channel(8);
        senders.insert(20, tx);

        // Write to a key matching the prefix.
        table
            .invalidate(&[b"sess:abc123"], None, &senders)
            .await;

        let frame = rx.recv().await.expect("expected push frame");
        assert!(matches!(frame, Frame::Push(_)), "must receive Push frame");
    }

    // -----------------------------------------------------------------------
    // 5. CLIENT TRACKING ON OPTIN: key is not tracked before CACHING YES.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn optin_only_tracks_after_caching_yes() {
        let (h, table) = make_handler(30);

        // Enable with OPTIN.
        h.handle_tracking(&[
            Bytes::from_static(b"ON"),
            Bytes::from_static(b"OPTIN"),
        ])
        .unwrap();

        // Read without CLIENT CACHING YES — should NOT be tracked.
        table.track_read(30, b"price:BTC").unwrap();

        // Verify not tracked.
        let senders: DashMap<ClientId, mpsc::Sender<Frame>> = DashMap::new();
        let (tx, mut rx) = mpsc::channel(8);
        senders.insert(30, tx);

        table.invalidate(&[b"price:BTC"], None, &senders).await;

        let no_frame = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx.recv(),
        )
        .await;
        assert!(
            no_frame.is_err(),
            "without CACHING YES, OPTIN client must not receive invalidation"
        );

        // Now set caching override to YES and track.
        h.handle_caching(&[Bytes::from_static(b"YES")]).unwrap();
        table.track_read(30, b"price:BTC").unwrap();

        table.invalidate(&[b"price:BTC"], None, &senders).await;

        let frame = rx.recv().await.expect("expected push after CACHING YES");
        assert!(matches!(frame, Frame::Push(_)));
    }

    // -----------------------------------------------------------------------
    // 6. CLIENT TRACKING ON REDIRECT sends push to redirect target, not self.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn redirect_sends_invalidation_to_target_client() {
        // Client 40 tracks, but redirects to client 41.
        let table = Arc::new(TrackingTable::new());
        let h40 = TrackingCommandHandler::new(40, table.clone());

        let args: Vec<Bytes> = vec![
            Bytes::from_static(b"ON"),
            Bytes::from_static(b"REDIRECT"),
            Bytes::from("41".to_string().into_bytes()),
        ];
        h40.handle_tracking(&args).unwrap();

        // The TrackingTable itself does not re-route sends to the redirect
        // target — that is a responsibility of the network layer using the
        // `TrackingInfo.options.redirect` field.  Here we verify:
        // (a) the option was recorded correctly via TRACKINGINFO,
        // (b) invalidate sends to the client that has the key tracked (40),
        //     which the network layer then re-routes to 41 when it checks
        //     info().options.redirect.

        let info_frame = h40.handle_trackinginfo();
        match info_frame {
            Frame::Map(pairs) => {
                let redirect_val = pairs
                    .iter()
                    .find(|(k, _)| matches!(k, Frame::BulkString(b) if b.as_ref() == b"redirect"))
                    .map(|(_, v)| v);
                assert_eq!(
                    redirect_val,
                    Some(&Frame::Integer(41)),
                    "redirect must be stored as 41"
                );
            }
            other => panic!("expected Map frame, got {other:?}"),
        }

        // Wire up senders so invalidate reaches client 40's channel,
        // simulating the pre-routing step.
        let senders: DashMap<ClientId, mpsc::Sender<Frame>> = DashMap::new();
        let (tx40, mut rx40) = mpsc::channel(8);
        let (tx41, mut rx41) = mpsc::channel(8);
        senders.insert(40, tx40);
        senders.insert(41, tx41);

        table.track_read(40, b"order:99").unwrap();
        table.invalidate(&[b"order:99"], None, &senders).await;

        // Client 40 should receive the push (network layer later re-routes).
        let frame = rx40.recv().await.expect("client 40 must receive invalidation");
        assert!(matches!(frame, Frame::Push(_)));

        // Client 41's channel should be empty (re-routing is handled by network).
        let no_frame = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx41.recv(),
        )
        .await;
        assert!(no_frame.is_err(), "client 41 channel not touched by table");
    }

    // -----------------------------------------------------------------------
    // 7. CLIENT TRACKING ON NOLOOP — writer does not receive its own invalidation.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn noloop_suppresses_self_invalidation() {
        let (h, table) = make_handler(50);

        h.handle_tracking(&[
            Bytes::from_static(b"ON"),
            Bytes::from_static(b"NOLOOP"),
        ])
        .unwrap();

        table.track_read(50, b"stock:AAPL").unwrap();

        let senders: DashMap<ClientId, mpsc::Sender<Frame>> = DashMap::new();
        let (tx, mut rx) = mpsc::channel(8);
        senders.insert(50, tx);

        // Origin = 50 (the same client that tracked the key).
        table
            .invalidate(&[b"stock:AAPL"], Some(50), &senders)
            .await;

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            rx.recv(),
        )
        .await;
        assert!(
            result.is_err(),
            "NOLOOP must suppress self-invalidation for client 50"
        );
    }

    // -----------------------------------------------------------------------
    // 8. CLIENT TRACKINGINFO returns correct Map structure.
    // -----------------------------------------------------------------------

    #[test]
    fn trackinginfo_map_structure_when_bcast() {
        let (h, _table) = make_handler(60);

        h.handle_tracking(&[
            Bytes::from_static(b"ON"),
            Bytes::from_static(b"BCAST"),
            Bytes::from_static(b"PREFIX"),
            Bytes::from_static(b"cache:"),
        ])
        .unwrap();

        let frame = h.handle_trackinginfo();
        match frame {
            Frame::Map(pairs) => {
                // Must have flags, redirect, prefixes.
                let keys: Vec<&[u8]> = pairs
                    .iter()
                    .filter_map(|(k, _)| match k {
                        Frame::BulkString(b) => Some(b.as_ref()),
                        _ => None,
                    })
                    .collect();
                assert!(keys.contains(&b"flags".as_ref()));
                assert!(keys.contains(&b"redirect".as_ref()));
                assert!(keys.contains(&b"prefixes".as_ref()));

                // `flags` must contain "on" and "bcast".
                let flags_val = pairs
                    .iter()
                    .find(|(k, _)| matches!(k, Frame::BulkString(b) if b.as_ref() == b"flags"))
                    .map(|(_, v)| v)
                    .unwrap();
                match flags_val {
                    Frame::Array(items) => {
                        let strs: Vec<&[u8]> = items
                            .iter()
                            .filter_map(|f| match f {
                                Frame::BulkString(b) => Some(b.as_ref()),
                                _ => None,
                            })
                            .collect();
                        assert!(strs.contains(&b"on".as_ref()), "flags must contain 'on'");
                        assert!(strs.contains(&b"bcast".as_ref()), "flags must contain 'bcast'");
                    }
                    other => panic!("flags must be Array, got {other:?}"),
                }
            }
            other => panic!("expected Map frame, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // 9. CLIENT CACHING YES/NO — parse errors.
    // -----------------------------------------------------------------------

    #[test]
    fn client_caching_bad_arg_returns_syntax_error() {
        let (h, _) = make_handler(70);
        let err = h
            .handle_caching(&[Bytes::from_static(b"MAYBE")])
            .unwrap_err();
        assert!(matches!(err, TrackingCmdError::Syntax(_)));
    }

    // -----------------------------------------------------------------------
    // 10. CLIENT TRACKING with both OPTIN and OPTOUT returns Syntax error.
    // -----------------------------------------------------------------------

    #[test]
    fn optin_and_optout_simultaneously_is_syntax_error() {
        let (h, _) = make_handler(80);
        let result = h.handle_tracking(&[
            Bytes::from_static(b"ON"),
            Bytes::from_static(b"OPTIN"),
            Bytes::from_static(b"OPTOUT"),
        ]);
        assert!(
            matches!(result, Err(TrackingCmdError::Syntax(_))),
            "OPTIN+OPTOUT must be rejected"
        );
    }
}
