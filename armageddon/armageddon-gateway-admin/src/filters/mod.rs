// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! Admin-route filter chain for the ARMAGEDDON Pingora gateway.
//!
//! Filter registration order (must be respected in `routes::build_admin_filters`):
//!
//! 1. **JWT** (existing `armageddon-forge` filter) — validates Bearer token,
//!    populates `ctx.user_id`, `ctx.roles`, `ctx.trace_id`.
//! 2. **KetoAuthzFilter** — checks `AdminRole` namespace in Keto; 403 on deny.
//! 3. **OtpRateLimitFilter** — enforces 3 req/5 min on `POST /api/admin/otp/issue`.
//! 4. **SecurityHeadersFilter** — injects HSTS / X-Frame-Options / Cache-Control.
//! 5. **AdminAccessLogFilter** — Prometheus counter in `on_logging` hook.

pub mod access_log;
pub mod keto_authz;
pub mod otp_rate_limit;
pub mod security_headers;
pub mod websocket_proxy;

pub use access_log::AdminAccessLogFilter;
pub use keto_authz::{KetoAuthzConfig, KetoAuthzFilter};
pub use otp_rate_limit::OtpRateLimitFilter;
pub use security_headers::SecurityHeadersFilter;
pub use websocket_proxy::{WebSocketProxyFilter, WsProxyConfig};
