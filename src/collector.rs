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

            // 1. Sample Power Microjoules & Accumulate Real Cumulative Energy into SQLite
            let (watts, delta_uj, delta_wh, is_supported) = self.sensor_mgr.power.sample_raw();
            if delta_uj > 0 {
                let _ = self.db.add_energy_delta(delta_uj, delta_wh);
            }

            let energy_totals = self.db.get_cumulative_energy().unwrap_or(crate::db::CumulativeEnergy {
                today_wh: 0.0,
                month_wh: 0.0,
                year_wh: 0.0,
            });

            let power_metrics = self.sensor_mgr.power.build_metrics(
                watts,
                energy_totals.today_wh,
                energy_totals.month_wh,
                energy_totals.year_wh,
                is_supported,
            );

            // 2. Fetch Immich stats periodically
            if tick_count % 10 == 1 {
                if let Some(ref client) = self.immich_client {
                    if let Some(stats) = client.fetch_stats().await {
                        cached_immich = Some(stats);
                    }
                }
            }

            let containers = self.docker_client.list_containers().await;
            let dockge = Some(DockgeCollector::collect(&containers));

            // Fetch Glacier AI server activity status (fast, non-blocking, 1s timeout)
            let glacier_status = {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(1))
                    .build()
                    .ok();
                if let Some(client) = client {
                    match client.get("http://127.0.0.1:8899/api/status").send().await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                json["status"].as_str().unwrap_or("offline").to_string()
                            } else {
                                "offline".to_string()
                            }
                        }
                        _ => "offline".to_string(),
                    }
                } else {
                    "offline".to_string()
                }
            };

            let telemetry = self.sensor_mgr.collect_all(power_metrics, containers, cached_immich.clone(), dockge, glacier_status);

            // 3. Record history point in SQLite
            let point = HistoryPoint {
                timestamp: telemetry.timestamp,
                cpu_usage: telemetry.cpu.total_usage_percent,
                cpu_temp: telemetry.thermals.cpu_temp_celsius,
                cpu_freq: telemetry.cpu.avg_frequency_mhz,
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

            // 4. Update shared latest telemetry snapshot
            {
                let mut lock = self.latest_telemetry.write().await;
                *lock = Some(telemetry.clone());
            }

            // 5. Broadcast to active SSE subscribers
            let _ = self.tx.send(telemetry);

            // 6. Daily history pruning
            if tick_count % 7200 == 0 {
                let _ = self.db.prune(self.config.server.history_retention_days);
            }
        }
    }
}
