// SPDX-License-Identifier: AGPL-3.0-or-later
// terroir-core build script — compiles proto/core.proto via tonic-build.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["../proto/core.proto"], &["../proto"])?;
    // Re-run if the proto changes.
    println!("cargo:rerun-if-changed=../proto/core.proto");
    Ok(())
}
