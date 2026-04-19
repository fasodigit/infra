// SPDX-License-Identifier: AGPL-3.0-or-later
//! Build script for armageddon-ebpf.
//!
//! When the `ebpf` feature is active, uses `aya-build` to cross-compile
//! `armageddon-ebpf-programs` for the `bpfel-unknown-none` target, producing
//! an ELF object embedded via `include_bytes!` at runtime.
//!
//! Without the `ebpf` feature this script is a no-op, so `cargo check -p
//! armageddon-ebpf` succeeds on any host without `bpf-linker`.

fn main() {
    #[cfg(feature = "ebpf")]
    compile_ebpf_programs();
}

#[cfg(feature = "ebpf")]
fn compile_ebpf_programs() {
    use std::path::PathBuf;

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set by Cargo");
    let programs_dir = PathBuf::from(&manifest_dir)
        .parent()
        .expect("parent workspace dir")
        .join("armageddon-ebpf-programs");

    aya_build::build_ebpf([programs_dir])
        .expect("aya-build failed — ensure bpf-linker is installed: cargo install bpf-linker");
}
