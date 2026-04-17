// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Response cache for ARMAGEDDON backed by KAYA. Implements HTTP caching
//! semantics (ETag, If-None-Match, Vary, Cache-Control) with blake3 cache keys.

pub mod error;
pub mod key;
pub mod policy;
pub mod store;
pub mod vary;

pub use error::CacheError;
pub use policy::{CacheControl, CachePolicy};
pub use store::{
    AsyncKeyValue, CachedResponse, ConditionalResponse, InMemoryKv, KayaAdapter, ResponseCache,
};
pub use vary::{is_wildcard, parse_vary, project_vary_headers};
