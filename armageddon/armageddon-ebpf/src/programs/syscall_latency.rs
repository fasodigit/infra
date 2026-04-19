// SPDX-License-Identifier: AGPL-3.0-or-later
//! Userspace attachment logic for `recvfrom` / `sendto` latency tracepoints.
//!
//! Attaches four tracepoints: `sys_enter_recvfrom`, `sys_exit_recvfrom`,
//! `sys_enter_sendto`, `sys_exit_sendto`.
//!
//! Returns the ring buffer map handle for the latency event stream.

#[cfg(all(target_os = "linux", feature = "ebpf"))]
pub(crate) mod linux {
    use aya::{
        maps::RingBuf,
        programs::TracePoint,
        Ebpf,
    };
    use tracing::{debug, instrument};

    use crate::error::EbpfError;

    /// Attach all four syscall latency tracepoints.
    #[instrument(skip(bpf), err)]
    pub fn attach(bpf: &mut Ebpf) -> Result<RingBuf<&mut aya::maps::MapData>, EbpfError> {
        let tracepoints: &[(&str, &str, &str)] = &[
            ("sys_enter_recvfrom", "syscalls", "sys_enter_recvfrom"),
            ("sys_exit_recvfrom",  "syscalls", "sys_exit_recvfrom"),
            ("sys_enter_sendto",   "syscalls", "sys_enter_sendto"),
            ("sys_exit_sendto",    "syscalls", "sys_exit_sendto"),
        ];

        for (prog_name, category, event) in tracepoints {
            let prog: &mut TracePoint = bpf
                .program_mut(prog_name)
                .ok_or_else(|| EbpfError::ProgramNotFound(prog_name.to_string()))?
                .try_into()
                .map_err(|e| EbpfError::Load(format!("{e}")))?;
            prog.load()
                .map_err(|e| EbpfError::Load(format!("{e}")))?;
            prog.attach(category, event)
                .map_err(|e| EbpfError::Attach(format!("{e}")))?;
            debug!(program = prog_name, "tracepoint attached");
        }

        let ring_buf = RingBuf::try_from(
            bpf.map_mut("SYSCALL_LATENCY_EVENTS")
                .ok_or_else(|| EbpfError::MapNotFound("SYSCALL_LATENCY_EVENTS".into()))?,
        )
        .map_err(|e| EbpfError::Map(format!("{e}")))?;

        Ok(ring_buf)
    }
}
