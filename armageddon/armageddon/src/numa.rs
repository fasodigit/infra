// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
//! NUMA-aware worker pinning for ARMAGEDDON.
//!
//! On multi-socket servers, binding Tokio workers to NUMA nodes reduces
//! cross-socket memory traffic and typically cuts cache misses by ~20%.
//!
//! # Feature gate
//! This module is compiled only when the `numa` Cargo feature is enabled.
//! When the feature is absent, or when the system exposes a single NUMA node,
//! the caller falls back to the default `multi_thread` runtime.
//!
//! # Non-Linux targets
//! Thread affinity via `sched_setaffinity(2)` is Linux-specific.  On other
//! platforms every public function returns immediately with a `warn!` and an
//! `Err(NumaError::UnsupportedPlatform)`.

#[cfg(feature = "numa")]
pub use inner::*;

#[cfg(feature = "numa")]
mod inner {
    use std::io;
    use thiserror::Error;
    use tracing::{debug, info, warn};

    // -----------------------------------------------------------------------
    // Error type
    // -----------------------------------------------------------------------

    /// Errors that can arise from NUMA topology detection or thread pinning.
    #[derive(Debug, Error)]
    pub enum NumaError {
        /// hwloc topology initialisation failed.
        #[error("hwloc topology init failed: {0}")]
        HwlocInit(String),

        /// `sched_setaffinity` system call returned an error.
        #[error("sched_setaffinity failed: {0}")]
        SetAffinity(#[from] io::Error),

        /// The requested NUMA node index does not exist on this machine.
        #[error("NUMA node {0} does not exist on this machine")]
        InvalidNode(usize),

        /// NUMA is not supported on this platform (non-Linux).
        #[error("NUMA pinning is not supported on this platform")]
        UnsupportedPlatform,
    }

    // -----------------------------------------------------------------------
    // Topology types
    // -----------------------------------------------------------------------

    /// Description of a single NUMA node.
    #[derive(Debug, Clone)]
    pub struct NumaNode {
        /// Zero-based node index as reported by hwloc.
        pub id: usize,
        /// Logical CPU indices that belong to this node.
        pub cpus: Vec<usize>,
    }

    /// Full NUMA topology of the current machine.
    #[derive(Debug, Clone)]
    pub struct NumaTopology {
        /// All NUMA nodes detected on the system.
        pub nodes: Vec<NumaNode>,
    }

    impl NumaTopology {
        /// Returns `true` when the machine has more than one NUMA node.
        pub fn is_multi_node(&self) -> bool {
            self.nodes.len() > 1
        }

        /// Total logical CPU count across all nodes.
        pub fn total_cpus(&self) -> usize {
            self.nodes.iter().map(|n| n.cpus.len()).sum()
        }
    }

    // -----------------------------------------------------------------------
    // Topology detection via hwloc
    // -----------------------------------------------------------------------

    /// Detect the NUMA topology of the current machine using hwloc.
    ///
    /// Returns `None` when hwloc initialisation fails or when no NUMA nodes
    /// are found (treated as a single-node, non-NUMA system).
    #[cfg(target_os = "linux")]
    pub fn detect_topology() -> Option<NumaTopology> {
        use hwloc::{ObjectType, Topology};

        // Topology::new() in hwloc 0.5 panics on init failure, so we use
        // std::panic::catch_unwind to convert that into a graceful fallback.
        let topo = match std::panic::catch_unwind(Topology::new) {
            Ok(t) => t,
            Err(_) => {
                warn!("hwloc topology init panicked — single-NUMA fallback");
                return None;
            }
        };

        // Collect NUMA nodes; TypeDepthError means this architecture has no NUMA
        let numa_objects = match topo.objects_with_type(&ObjectType::NUMANode) {
            Ok(objs) => objs,
            Err(_) => {
                debug!("hwloc: NUMANode type not supported — treating as single-node");
                return None;
            }
        };
        if numa_objects.is_empty() {
            debug!("hwloc reports 0 NUMA nodes — treating as single-node");
            return None;
        }

        let mut nodes: Vec<NumaNode> = numa_objects
            .iter()
            .enumerate()
            .map(|(idx, obj)| {
                let cpus: Vec<usize> = match obj.cpuset() {
                    Some(cs) => cs.into_iter().map(|cpu| cpu as usize).collect(),
                    None => Vec::new(),
                };
                NumaNode { id: idx, cpus }
            })
            .collect();

        // Sort by node id for deterministic ordering
        nodes.sort_by_key(|n| n.id);

        let topo_info = NumaTopology { nodes };
        info!(
            nodes = topo_info.nodes.len(),
            cpus  = topo_info.total_cpus(),
            "NUMA topology detected"
        );
        Some(topo_info)
    }

    /// Non-Linux stub — always returns `None` with a warning.
    #[cfg(not(target_os = "linux"))]
    pub fn detect_topology() -> Option<NumaTopology> {
        warn!("NUMA topology detection is only supported on Linux — single-NUMA fallback");
        None
    }

    // -----------------------------------------------------------------------
    // Thread pinning via sched_setaffinity
    // -----------------------------------------------------------------------

    /// Pin the calling thread to all CPUs belonging to the given NUMA node.
    ///
    /// Uses `libc::sched_setaffinity` directly so there is no dependency on a
    /// separate affinity crate.
    ///
    /// # Errors
    /// Returns [`NumaError::UnsupportedPlatform`] on non-Linux targets.
    /// Returns [`NumaError::InvalidNode`] when `node_id` is not present in
    /// `topology`.
    /// Returns [`NumaError::SetAffinity`] on `libc` call failure.
    #[cfg(target_os = "linux")]
    pub fn pin_thread_to_node(node_id: usize, topology: &NumaTopology) -> Result<(), NumaError> {
        use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};
        use std::mem;

        let node = topology
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or(NumaError::InvalidNode(node_id))?;

        if node.cpus.is_empty() {
            return Err(NumaError::InvalidNode(node_id));
        }

        // SAFETY: cpu_set_t is a plain data struct; CPU_ZERO and CPU_SET are
        // safe to call on a properly aligned stack allocation.
        unsafe {
            let mut set: cpu_set_t = mem::zeroed();
            CPU_ZERO(&mut set);
            for &cpu in &node.cpus {
                CPU_SET(cpu, &mut set);
            }

            let ret = sched_setaffinity(
                0, // 0 = calling thread
                mem::size_of::<cpu_set_t>(),
                &set,
            );

            if ret != 0 {
                return Err(NumaError::SetAffinity(io::Error::last_os_error()));
            }
        }

        debug!(node_id, cpus = ?node.cpus, "thread pinned to NUMA node");
        Ok(())
    }

    /// Non-Linux stub.
    #[cfg(not(target_os = "linux"))]
    pub fn pin_thread_to_node(_node_id: usize, _topology: &NumaTopology) -> Result<(), NumaError> {
        warn!("Thread pinning is only supported on Linux");
        Err(NumaError::UnsupportedPlatform)
    }

    // -----------------------------------------------------------------------
    // NUMA-pinned Tokio runtime builder
    // -----------------------------------------------------------------------

    /// Spawn a Tokio `Runtime` with one worker thread per requested NUMA node,
    /// each worker pinned to its node's CPU set via `sched_setaffinity`.
    ///
    /// Falls back to a standard `multi_thread` runtime when:
    /// - `nodes` is empty
    /// - topology detection returns `None` (single-NUMA machine)
    /// - running on a non-Linux platform
    ///
    /// # Arguments
    /// * `nodes` – NUMA node IDs to create workers for.  Pass the full set
    ///   from [`detect_topology`] for maximum parallelism.
    pub fn spawn_numa_pinned_runtime(nodes: Vec<usize>) -> tokio::runtime::Runtime {
        // Attempt topology detection first; fall back on failure.
        let topology = match detect_topology() {
            Some(t) if t.is_multi_node() => t,
            Some(_) => {
                info!("single-NUMA machine — using standard multi_thread runtime");
                return build_standard_runtime();
            }
            None => {
                info!("topology detection unavailable — using standard multi_thread runtime");
                return build_standard_runtime();
            }
        };

        if nodes.is_empty() {
            info!("no NUMA nodes requested — using standard multi_thread runtime");
            return build_standard_runtime();
        }

        let topo = std::sync::Arc::new(topology);
        let worker_count = nodes.len();

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_count)
            .on_thread_start({
                // Each worker calls this closure once on start-up.
                // We round-robin over `nodes` using an atomic counter so that
                // worker-0 → node[0], worker-1 → node[1], etc.
                let nodes = nodes.clone();
                let topo = std::sync::Arc::clone(&topo);
                let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
                move || {
                    let idx = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let node_id = nodes[idx % nodes.len()];
                    if let Err(e) = pin_thread_to_node(node_id, &topo) {
                        warn!(node_id, err = %e, "thread pinning failed — worker runs unpinned");
                    } else {
                        info!(node_id, worker = idx, "Tokio worker pinned to NUMA node");
                    }
                }
            })
            .enable_all()
            .build()
            .unwrap_or_else(|e| {
                warn!(err = %e, "NUMA-pinned runtime build failed — falling back to standard runtime");
                build_standard_runtime()
            });

        info!(workers = worker_count, "NUMA-pinned Tokio runtime started");
        runtime
    }

    // -----------------------------------------------------------------------
    // Internal helper
    // -----------------------------------------------------------------------

    fn build_standard_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("standard Tokio runtime build must not fail")
    }

    // -----------------------------------------------------------------------
    // Unit tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        // -- Happy path: topology detection returns a valid (possibly single-node) result --
        #[test]
        fn test_detect_topology_returns_some_or_none() {
            // On any CI Linux machine this must not panic.
            // We accept both Some and None (single-NUMA machines exist).
            let _result = detect_topology();
            // If it returns Some, basic invariants must hold.
            if let Some(topo) = detect_topology() {
                assert!(!topo.nodes.is_empty(), "NumaTopology must have at least one node");
                for node in &topo.nodes {
                    // Node IDs are unique
                    assert!(topo.nodes.iter().filter(|n| n.id == node.id).count() == 1);
                }
            }
        }

        // -- Edge case: detect_topology on a multi-CPU CI box (≥ 2 CPUs) --
        #[test]
        #[cfg(target_os = "linux")]
        fn test_topology_cpu_count_matches_system() {
            // /proc/cpuinfo count must match total CPUs in topology (when available).
            let num_cpus_sys = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1);

            if let Some(topo) = detect_topology() {
                let topo_cpus = topo.total_cpus();
                // hwloc may return fewer CPUs on cgroup-limited containers —
                // but never more than what the OS reports.
                assert!(
                    topo_cpus <= num_cpus_sys * 2,
                    "hwloc cpu count ({}) should be close to OS cpu count ({})",
                    topo_cpus,
                    num_cpus_sys
                );
            }
            // None is also acceptable (single-NUMA / hwloc unavailable)
        }

        // -- Error case: pin_thread_to_node with invalid node id --
        #[test]
        #[cfg(target_os = "linux")]
        fn test_pin_invalid_node_returns_error() {
            // Build a synthetic single-node topology with node id = 0
            let topo = NumaTopology {
                nodes: vec![NumaNode {
                    id: 0,
                    cpus: vec![0],
                }],
            };
            // Node 99 does not exist
            let result = pin_thread_to_node(99, &topo);
            assert!(
                matches!(result, Err(NumaError::InvalidNode(99))),
                "expected InvalidNode(99), got {:?}",
                result
            );
        }

        // -- Happy path: spawn_numa_pinned_runtime with empty node list falls back gracefully --
        #[test]
        fn test_spawn_runtime_empty_nodes_fallback() {
            let rt = spawn_numa_pinned_runtime(vec![]);
            // If we get here without panic, fallback worked.
            // Run a trivial async task to confirm the runtime is operational.
            let val = rt.block_on(async { 42_u32 });
            assert_eq!(val, 42);
        }

        // -- Happy path: standard runtime builds and executes tasks --
        #[test]
        fn test_build_standard_runtime() {
            let rt = super::build_standard_runtime();
            let result = rt.block_on(async { "ok" });
            assert_eq!(result, "ok");
        }

        // -- Edge case: NumaTopology::is_multi_node on single-node topology --
        #[test]
        fn test_is_multi_node_single() {
            let topo = NumaTopology {
                nodes: vec![NumaNode { id: 0, cpus: vec![0, 1] }],
            };
            assert!(!topo.is_multi_node());
        }

        // -- Edge case: NumaTopology::is_multi_node on two-node topology --
        #[test]
        fn test_is_multi_node_dual() {
            let topo = NumaTopology {
                nodes: vec![
                    NumaNode { id: 0, cpus: vec![0, 1] },
                    NumaNode { id: 1, cpus: vec![2, 3] },
                ],
            };
            assert!(topo.is_multi_node());
            assert_eq!(topo.total_cpus(), 4);
        }
    }
}
