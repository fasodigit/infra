// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-mobile-bff build script — compiles proto/core.proto via tonic-build.
// Only client stubs are generated (BFF acts as gRPC client to terroir-core :8730).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&["../proto/core.proto"], &["../proto"])?;
    println!("cargo:rerun-if-changed=../proto/core.proto");
    Ok(())
}
