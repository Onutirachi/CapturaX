//! Keeping the GPU at full clocks while the preview presents video (FR-012, research R7).

use crate::os;

/// Keeps the GPU's performance request while it lives; the previous setting comes back when it
/// drops. The request is this process's own: other processes' settings are neither changed nor
/// overridden when it drops. A guard that could not make the request does nothing (the reason is
/// logged once per run). Hold one guard at a time: a second guard taken while the first lives
/// finds the request already in place and leaves it to the first.
#[must_use = "the GPU keeps its clocks up only while the guard lives"]
pub struct PerformanceGuard {
    _os: os::PerformanceGuard,
}

/// Asks the GPU at `pci_bus_id` (as wgpu reports it, e.g. `0000:01:00.0`; the first GPU when
/// None or not found) to prefer maximum performance until the guard drops: NVIDIA's PowerMizer
/// mode through NVML on Linux. A no-op on Windows, which keeps its clocks up under the
/// preview's load, and on GPUs without such a control.
pub fn gpu_performance(pci_bus_id: Option<&str>) -> PerformanceGuard {
    PerformanceGuard {
        _os: os::PerformanceGuard::new(pci_bus_id),
    }
}
