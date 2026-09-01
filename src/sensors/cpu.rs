use serde::Serialize;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[derive(Debug, Clone, Serialize)]
pub struct CpuMetrics {
    pub total_usage_percent: f32,
    pub core_count: usize,
    pub thread_count: usize,
    pub avg_frequency_mhz: u64,
    pub per_core_usage: Vec<f32>,
    pub per_core_frequency: Vec<u64>,
}

pub struct CpuCollector {
    sys: System,
}

impl CpuCollector {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        );
        Self { sys }
    }

    pub fn collect(&mut self) -> CpuMetrics {
        self.sys.refresh_cpu_all();
        let cpus = self.sys.cpus();
        
        let mut total_usage = 0.0;
        let mut per_core_usage = Vec::with_capacity(cpus.len());
        let mut per_core_freq = Vec::with_capacity(cpus.len());
        let mut total_freq = 0;

        for cpu in cpus {
            let u = cpu.cpu_usage();
            let f = cpu.frequency();
            per_core_usage.push(u);
            per_core_freq.push(f);
            total_usage += u;
            total_freq += f;
        }

        let thread_count = cpus.len();
        let core_count = self.sys.physical_core_count().unwrap_or(thread_count);
        let avg_usage = if thread_count > 0 { total_usage / thread_count as f32 } else { 0.0 };
        let avg_freq = if thread_count > 0 { total_freq / thread_count as u64 } else { 0 };

        CpuMetrics {
            total_usage_percent: (avg_usage * 10.0).round() / 10.0,
            core_count,
            thread_count,
            avg_frequency_mhz: avg_freq,
            per_core_usage,
            per_core_frequency: per_core_freq,
        }
    }
}
