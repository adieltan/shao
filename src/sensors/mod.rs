pub mod cpu;
pub mod disk;
pub mod memory;
pub mod network;
pub mod power;
pub mod thermals;

use cpu::{CpuCollector, CpuMetrics};
use disk::{DiskCollector, DiskMetrics};
use memory::{MemoryCollector, MemoryMetrics};
use network::{NetworkCollector, NetworkMetrics};
use power::{PowerCollector, PowerMetrics};
use thermals::{ThermalCollector, ThermalMetrics};

use crate::config::Config;
use crate::dockge::DockgeStats;
use crate::docker::DockerContainer;
use crate::immich::ImmichStats;
use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
pub struct SystemSummary {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub uptime_seconds: u64,
    pub uptime_human: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FullTelemetry {
    pub timestamp: i64,
    pub system: SystemSummary,
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub thermals: ThermalMetrics,
    pub power: PowerMetrics,
    pub network: NetworkMetrics,
    pub containers: Vec<DockerContainer>,
    pub immich: Option<ImmichStats>,
    pub dockge: Option<DockgeStats>,
}

pub struct SensorManager {
    cpu: CpuCollector,
    memory: MemoryCollector,
    disk: DiskCollector,
    thermals: ThermalCollector,
    power: PowerCollector,
    network: NetworkCollector,
}

impl SensorManager {
    pub fn new(config: &Config) -> Self {
        Self {
            cpu: CpuCollector::new(),
            memory: MemoryCollector::new(),
            disk: DiskCollector::new(),
            thermals: ThermalCollector::new(),
            power: PowerCollector::new(config.power.kwh_cost, config.power.currency_symbol.clone()),
            network: NetworkCollector::new(
                config.network.lan_interfaces.clone(),
                config.network.vpn_interfaces.clone(),
            ),
        }
    }

    pub fn collect_all(
        &mut self,
        containers: Vec<DockerContainer>,
        immich: Option<ImmichStats>,
        dockge: Option<DockgeStats>,
    ) -> FullTelemetry {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let uptime = System::uptime();
        let days = uptime / 86400;
        let hours = (uptime % 86400) / 3600;
        let mins = (uptime % 3600) / 60;
        let uptime_human = if days > 0 {
            format!("{}d {}h {}m", days, hours, mins)
        } else if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        };

        let system = SystemSummary {
            hostname: System::host_name().unwrap_or_else(|| "linux-server".into()),
            os_name: System::name().unwrap_or_else(|| "Linux".into()),
            os_version: System::os_version().unwrap_or_default(),
            uptime_seconds: uptime,
            uptime_human,
        };

        FullTelemetry {
            timestamp,
            system,
            cpu: self.cpu.collect(),
            memory: self.memory.collect(),
            disk: self.disk.collect(),
            thermals: self.thermals.collect(),
            power: self.power.collect(),
            network: self.network.collect(),
            containers,
            immich,
            dockge,
        }
    }
}
