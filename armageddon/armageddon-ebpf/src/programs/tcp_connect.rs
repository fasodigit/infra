// SPDX-License-Identifier: AGPL-3.0-or-later
//! Userspace attachment logic for the `tcp_connect` kprobe/kretprobe pair.
//!
//! This module is compiled only on Linux. The actual eBPF bytecode is embedded
//! from the ELF object produced by `armageddon-ebpf-programs` via `aya-build`.

#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub(crate) mod linux {
    use aya::{
        maps::RingBuf,
        programs::{KProbe, KRetProbe},
        Ebpf,
    };
    use tracing::{debug, instrument, warn};

    use crate::error::EbpfError;

    /// Attach kprobe + kretprobe for `tcp_connect`.
    ///
    /// Returns the ring buffer map handle so the caller can poll it for events.
    #[instrument(skip(bpf), err)]
    pub fn attach(bpf: &mut Ebpf) -> Result<RingBuf<&mut aya::maps::MapData>, EbpfError> {
        // -- kprobe entry --
        let prog_entry: &mut KProbe = bpf
            .program_mut("tcp_connect_enter")
            .ok_or_else(|| EbpfError::ProgramNotFound("tcp_connect_enter".into()))?
            .try_into()
            .map_err(|e| EbpfError::Load(format!("{e}")))?;
        prog_entry
            .load()
            .map_err(|e| EbpfError::Load(format!("{e}")))?;
        prog_entry
            .attach("tcp_connect", 0)
            .map_err(|e| EbpfError::Attach(format!("{e}")))?;
        debug!("kprobe tcp_connect_enter attached");

        // -- kretprobe exit --
        let prog_exit: &mut KRetProbe = bpf
            .program_mut("tcp_connect_exit")
            .ok_or_else(|| EbpfError::ProgramNotFound("tcp_connect_exit".into()))?
            .try_into()
            .map_err(|e| EbpfError::Load(format!("{e}")))?;
        prog_exit
            .load()
            .map_err(|e| EbpfError::Load(format!("{e}")))?;
        prog_exit
            .attach("tcp_connect", 0)
            .map_err(|e| EbpfError::Attach(format!("{e}")))?;
        debug!("kretprobe tcp_connect_exit attached");

        // -- ring buffer map --
        let ring_buf = RingBuf::try_from(
            bpf.map_mut("TCP_CONNECT_EVENTS")
                .ok_or_else(|| EbpfError::MapNotFound("TCP_CONNECT_EVENTS".into()))?,
        )
        .map_err(|e| EbpfError::Map(format!("{e}")))?;

        Ok(ring_buf)
    }
}
