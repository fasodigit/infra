//! KAYA Commands: Command router and handler implementations.
//!
//! Routes parsed RESP3 commands to the appropriate store/stream operations
//! and produces RESP3 response frames.

pub mod functions;
pub mod geo;
pub mod handler;
pub mod probabilistic;
pub mod pubsub;
pub mod router;
pub mod tracking;
pub mod json;
pub mod vector;
pub mod timeseries;
pub mod fulltext;
pub mod tiered;

use std::sync::Arc;

use thiserror::Error;

use kaya_protocol::Frame;
use kaya_scripting::ScriptEngine;
use kaya_scripting::functions::FunctionRegistry;
use kaya_store::{Store, BloomManager};
use kaya_store::probabilistic::ProbabilisticStore;
use kaya_streams::StreamManager;
use kaya_network::tracking::TrackingTable;
use kaya_pubsub::{PubSubBroker, ShardedPubSub};
use kaya_json::JsonStore;
use kaya_vector::VectorStore;
use kaya_timeseries::TimeSeriesStore;
use kaya_fulltext::FtStore;
use kaya_tiered::TieredStore;

pub use handler::CommandHandler;
pub use router::CommandRouter;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    #[error("wrong number of arguments for '{command}' command")]
    WrongArity { command: String },

    #[error("store error: {0}")]
    Store(#[from] kaya_store::StoreError),

    #[error("stream error: {0}")]
    Stream(#[from] kaya_streams::StreamError),

    #[error("protocol error: {0}")]
    Protocol(#[from] kaya_protocol::ProtocolError),

    #[error("syntax error: {0}")]
    Syntax(String),

    #[error("NOAUTH Authentication required")]
    AuthRequired,

    #[error("script error: {0}")]
    Script(String),
}

impl CommandError {
    pub fn to_frame(&self) -> Frame {
        match self {
            CommandError::AuthRequired => Frame::Error("NOAUTH Authentication required".into()),
            _ => Frame::Error(format!("ERR {self}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Command context: shared state passed to every command handler
// ---------------------------------------------------------------------------

/// Shared state available to all command handlers.
pub struct CommandContext {
    pub store: Arc<Store>,
    pub streams: Arc<StreamManager>,
    pub blooms: Arc<BloomManager>,
    /// Probabilistic data structures (Cuckoo, HLL, CMS, TopK).
    pub prob: Arc<ProbabilisticStore>,
    pub scripting: Option<Arc<ScriptEngine>>,
    /// Optional password for AUTH. If None, no authentication is required.
    pub password: Option<String>,
    /// Optional Pub/Sub broker (exact + pattern subscriptions).
    pub pubsub: Option<Arc<PubSubBroker>>,
    /// Optional sharded Pub/Sub broker (SSUBSCRIBE / SPUBLISH).
    pub sharded_pubsub: Option<Arc<ShardedPubSub>>,
    /// Optional Functions registry (FUNCTION / FCALL).
    pub functions: Option<Arc<FunctionRegistry>>,
    /// Optional client-side caching tracking table (CLIENT TRACKING).
    pub tracking: Option<Arc<TrackingTable>>,
    /// Optional JSON document store (JSON.*).
    pub json: Option<Arc<JsonStore>>,
    /// Optional vector HNSW store (FT.* with VECTOR schema).
    pub vector: Option<Arc<VectorStore>>,
    /// Optional time-series store (TS.*).
    pub timeseries: Option<Arc<TimeSeriesStore>>,
    /// Optional full-text Tantivy store (FT.* text).
    pub fulltext: Option<Arc<FtStore>>,
    /// Optional tiered hot/cold store (KAYA.TIERED.*).
    pub tiered: Option<Arc<TieredStore>>,
}

impl CommandContext {
    pub fn new(
        store: Arc<Store>,
        streams: Arc<StreamManager>,
        blooms: Arc<BloomManager>,
    ) -> Self {
        Self {
            store,
            streams,
            blooms,
            prob: Arc::new(ProbabilisticStore::new()),
            scripting: None,
            password: None,
            pubsub: None,
            sharded_pubsub: None,
            functions: None,
            tracking: None,
            json: None,
            vector: None,
            timeseries: None,
            fulltext: None,
            tiered: None,
        }
    }

    /// Override the default ProbabilisticStore with an explicit instance.
    pub fn with_prob(mut self, prob: Arc<ProbabilisticStore>) -> Self {
        self.prob = prob;
        self
    }

    pub fn with_scripting(mut self, engine: Arc<ScriptEngine>) -> Self {
        self.scripting = Some(engine);
        self
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.password = password;
        self
    }

    /// Attach a Pub/Sub broker for exact / pattern subscriptions.
    pub fn with_pubsub(mut self, broker: Arc<PubSubBroker>) -> Self {
        self.pubsub = Some(broker);
        self
    }

    /// Attach a sharded Pub/Sub broker for SSUBSCRIBE / SPUBLISH.
    pub fn with_sharded_pubsub(mut self, sharded: Arc<ShardedPubSub>) -> Self {
        self.sharded_pubsub = Some(sharded);
        self
    }

    /// Attach a Functions registry for FUNCTION / FCALL commands.
    pub fn with_functions(mut self, registry: Arc<FunctionRegistry>) -> Self {
        self.functions = Some(registry);
        self
    }

    /// Attach a tracking table for CLIENT TRACKING commands.
    pub fn with_tracking(mut self, table: Arc<TrackingTable>) -> Self {
        self.tracking = Some(table);
        self
    }

    pub fn with_json(mut self, store: Arc<JsonStore>) -> Self {
        self.json = Some(store);
        self
    }

    pub fn with_vector(mut self, store: Arc<VectorStore>) -> Self {
        self.vector = Some(store);
        self
    }

    pub fn with_timeseries(mut self, store: Arc<TimeSeriesStore>) -> Self {
        self.timeseries = Some(store);
        self
    }

    pub fn with_fulltext(mut self, store: Arc<FtStore>) -> Self {
        self.fulltext = Some(store);
        self
    }

    pub fn with_tiered(mut self, store: Arc<TieredStore>) -> Self {
        self.tiered = Some(store);
        self
    }
}
