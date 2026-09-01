use crate::config::Config;
use crate::db::{Database, HistoryPoint};
use crate::docker::DockerClient;
use crate::sensors::{FullTelemetry, SensorManager};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info};

pub struct CollectorService {
    config: Config,
    db: Database,
    sensor_mgr: SensorManager,
    docker_client: DockerClient,
    latest_telemetry: Arc<RwLock<Option<FullTelemetry>>>,
    tx: broadcast::Sender<FullTelemetry>,
}

impl CollectorService {
    pub fn new(
        config: Config,
        db: Database,
        latest_telemetry: Arc<RwLock<Option<FullTelemetry>>>,
        tx: broadcast::Sender<FullTelemetry>,
    ) -> Self {
        let sensor_mgr = SensorManager::new(&config);
        let docker_client = DockerClient::new();

        Self {
            config,
            db,
            sensor_mgr,
            docker_client,
            latest_telemetry,
            tx,
        }
    }

    pub async fn run(mut self) {
        let interval_ms = self.config.server.polling_interval_ms.max(250);
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        let mut prune_ticker = 0u64;

        info!("Collector service started (Polling interval: {}ms)", interval_ms);

        loop {
            interval.tick().await;

            let containers = self.docker_client.list_containers().await;
            let telemetry = self.sensor_mgr.collect_all(containers);

            // 1. Record history point in SQLite
            let point = HistoryPoint {
                timestamp: telemetry.timestamp,
                cpu_usage: telemetry.cpu.total_usage_percent,
                cpu_temp: telemetry.thermals.cpu_temp_celsius,
                fan_rpm: telemetry.thermals.fan_rpm,
                power_watts: telemetry.power.current_watts,
                lan_rx_speed: telemetry.network.lan_rx_speed_bps,
                lan_tx_speed: telemetry.network.lan_tx_speed_bps,
                vpn_rx_speed: telemetry.network.vpn_rx_speed_bps,
                vpn_tx_speed: telemetry.network.vpn_tx_speed_bps,
            };

            if let Err(e) = self.db.insert_point(&point) {
                error!("Failed to record history point: {}", e);
            }

            // 2. Update shared latest telemetry snapshot
            {
                let mut lock = self.latest_telemetry.write().await;
                *lock = Some(telemetry.clone());
            }

            // 3. Broadcast to active SSE subscribers
            let _ = self.tx.send(telemetry);

            // 4. Daily history pruning
            prune_ticker += 1;
            if prune_ticker % 3600 == 0 {
                let _ = self.db.prune(self.config.server.history_retention_days);
            }
        }
    }
}
