// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Compiles xDS v3 protobuf definitions from the FASO xds-controller proto tree.
// Generates both server and CLIENT stubs so armageddon-xds can open a bidirectional
// ADS stream toward the control plane.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "/home/lyna/Documents/DEVELOPMENT-CLAUDE/INFRA/xds-controller/proto";

    tonic_build::configure()
        // Build both client (for production) and server (for mock ADS server in tests).
        .build_server(true)
        .build_client(true)
        // Emit rerun directives so cargo rebuilds only when protos change.
        .emit_rerun_if_changed(true)
        // Map google.rpc to our crate-local module path so that generated code
        // in envoy.service.discovery.v3 resolves the `super` chain correctly.
        .extern_path(".google.rpc", "crate::proto::google_rpc")
        .compile_protos(
            &[
                // Core ADS / discovery protocol
                &format!("{proto_root}/envoy/service/discovery/v3/ads.proto"),
                &format!("{proto_root}/envoy/service/discovery/v3/discovery.proto"),
                // Resource types
                &format!("{proto_root}/envoy/config/cluster/v3/cluster.proto"),
                &format!("{proto_root}/envoy/config/endpoint/v3/endpoint.proto"),
                &format!("{proto_root}/envoy/config/listener/v3/listener.proto"),
                &format!("{proto_root}/envoy/config/route/v3/route.proto"),
                &format!("{proto_root}/envoy/extensions/transport_sockets/tls/v3/secret.proto"),
                // google.rpc.Status for NACK error_detail
                &format!("{proto_root}/google/rpc/status.proto"),
            ],
            &[proto_root],
        )?;

    Ok(())
}
