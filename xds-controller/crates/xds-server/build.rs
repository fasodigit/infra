// Build script for xds-server: compiles all xDS v3 proto files into Rust types and tonic services.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../../proto";

    // All proto files to compile
    let protos = &[
        // Discovery (core)
        "envoy/service/discovery/v3/discovery.proto",
        "envoy/service/discovery/v3/ads.proto",
        // Per-type discovery services
        "envoy/service/cluster/v3/cds.proto",
        "envoy/service/endpoint/v3/eds.proto",
        "envoy/service/route/v3/rds.proto",
        "envoy/service/listener/v3/lds.proto",
        "envoy/service/secret/v3/sds.proto",
        // Config types
        "envoy/config/cluster/v3/cluster.proto",
        "envoy/config/endpoint/v3/endpoint.proto",
        "envoy/config/route/v3/route.proto",
        "envoy/config/listener/v3/listener.proto",
        "envoy/extensions/transport_sockets/tls/v3/secret.proto",
        // Dependencies
        "google/rpc/status.proto",
    ];

    // Prepend proto_root to each proto file path
    let proto_files: Vec<String> = protos
        .iter()
        .map(|p| format!("{proto_root}/{p}"))
        .collect();

    tonic_build::configure()
        .build_server(true)
        .build_client(false) // ARMAGEDDON has its own client
        .out_dir("src/generated")
        .compile_protos(
            &proto_files.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            &[proto_root],
        )?;

    Ok(())
}
