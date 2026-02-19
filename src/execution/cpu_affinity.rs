use core_affinity::CoreId;
use tracing::{info, warn};

/// CPU affinity manager for pinning threads to specific cores
pub struct CpuPinner {
    core_ids: Vec<CoreId>,
}

impl CpuPinner {
    /// Initialize CPU pinner with available cores
    pub fn new() -> Option<Self> {
        match core_affinity::get_core_ids() {
            Some(core_ids) if !core_ids.is_empty() => {
                info!("Available CPU cores: {}", core_ids.len());
                Some(Self { core_ids })
            }
            _ => {
                warn!("Could not get CPU core IDs");
                None
            }
        }
    }

    /// Pin current thread to a specific core
    /// Core index wraps around if larger than available cores
    pub fn pin_to_core(&self, core_index: usize) -> bool {
        let actual_index = core_index % self.core_ids.len();
        let core_id = self.core_ids[actual_index];

        if core_affinity::set_for_current(core_id) {
            info!("Pinned thread to core {}", actual_index);
            true
        } else {
            warn!("Failed to pin thread to core {}", actual_index);
            false
        }
    }

    /// Pin strategy thread to dedicated core (core 0)
    pub fn pin_strategy_thread(&self) -> bool {
        self.pin_to_core(0)
    }

    /// Pin WebSocket thread to separate core (core 1)
    pub fn pin_websocket_thread(&self) -> bool {
        self.pin_to_core(1)
    }

    /// Get number of available cores
    pub fn core_count(&self) -> usize {
        self.core_ids.len()
    }
}

impl Default for CpuPinner {
    fn default() -> Self {
        Self::new().unwrap_or_else(|| {
            // Fallback: create empty pinner
            Self { core_ids: vec![] }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_pinner_creation() {
        let pinner = CpuPinner::new();
        if let Some(p) = pinner {
            assert!(p.core_count() > 0);
        }
    }
}
