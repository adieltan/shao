use crate::config::Config;
use crate::db::{Database, HistoryPoint};
use crate::dockge::DockgeCollector;
use crate::docker::DockerClient;
use crate::immich::{ImmichClient, ImmichStats};
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
    immich_client: Option<ImmichClient>,
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
        let immich_client = config
            .immich
            .as_ref()
            .map(|c| ImmichClient::new(c.url.clone(), c.api_key.clone()));

        Self {
            config,
            db,
            sensor_mgr,
            docker_client,
            immich_client,
            latest_telemetry,
            tx,
        }
    }

    pub async fn run(mut self) {
        let interval_ms = self.config.server.polling_interval_ms.max(250);
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        let mut tick_count = 0u64;
        let mut cached_immich: Option<ImmichStats> = None;

        info!("Collector service started (Polling interval: {}ms)", interval_ms);

        loop {
            interval.tick().await;
            tick_count += 1;

            // Fetch Immich stats every 10 ticks (e.g. every 5 seconds)
            if tick_count % 10 == 1 {
                if let Some(ref client) = self.immich_client {
                    if let Some(stats) = client.fetch_stats().await {
                        cached_immich = Some(stats);
                    }
                }
            }

            let containers = self.docker_client.list_containers().await;
            let dockge = Some(DockgeCollector::collect(&containers));

            let telemetry = self.sensor_mgr.collect_all(containers, cached_immich.clone(), dockge);

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
            if tick_count % 7200 == 0 {
                let _ = self.db.prune(self.config.server.history_retention_days);
            }
        }
    }
}
