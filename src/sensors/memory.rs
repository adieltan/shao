use serde::Serialize;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub usage_percent: f32,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_usage_percent: f32,
}

pub struct MemoryCollector {
    sys: System,
}

impl MemoryCollector {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::new().with_memory(MemoryRefreshKind::everything()),
        );
        Self { sys }
    }

    pub fn collect(&mut self) -> MemoryMetrics {
        self.sys.refresh_memory();
        
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        let free = self.sys.free_memory();
        let usage_pct = if total > 0 { (used as f32 / total as f32) * 100.0 } else { 0.0 };

        let swap_total = self.sys.total_swap();
        let swap_used = self.sys.used_swap();
        let swap_pct = if swap_total > 0 { (swap_used as f32 / swap_total as f32) * 100.0 } else { 0.0 };

        MemoryMetrics {
            total_bytes: total,
            used_bytes: used,
            free_bytes: free,
            usage_percent: (usage_pct * 10.0).round() / 10.0,
            swap_total_bytes: swap_total,
            swap_used_bytes: swap_used,
            swap_usage_percent: (swap_pct * 10.0).round() / 10.0,
        }
    }
}
