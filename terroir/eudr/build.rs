// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-eudr build script — compiles proto/eudr.proto via tonic-build.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // EUDR service: server (we expose) + we don't need client.
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["../proto/eudr.proto"], &["../proto"])?;
    // Core service: client only (we call terroir-core).
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&["../proto/core.proto"], &["../proto"])?;
    println!("cargo:rerun-if-changed=../proto/eudr.proto");
    println!("cargo:rerun-if-changed=../proto/core.proto");
    Ok(())
}
