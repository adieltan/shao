use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub timestamp: i64,
    pub cpu_usage: f32,
    pub cpu_temp: f32,
    pub cpu_freq: u64,
    pub fan_rpm: u32,
    pub power_watts: f32,
    pub lan_rx_speed: f64,
    pub lan_tx_speed: f64,
    pub vpn_rx_speed: f64,
    pub vpn_tx_speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CumulativeEnergy {
    pub today_wh: f64,
    pub month_wh: f64,
    pub year_wh: f64,
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS metrics_history (
                timestamp INTEGER PRIMARY KEY,
                cpu_usage REAL,
                cpu_temp REAL,
                cpu_freq INTEGER DEFAULT 800,
                fan_rpm INTEGER,
                power_watts REAL,
                lan_rx_speed REAL,
                lan_tx_speed REAL,
                vpn_rx_speed REAL,
                vpn_tx_speed REAL
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics_history(timestamp);

            CREATE TABLE IF NOT EXISTS daily_energy (
                date TEXT PRIMARY KEY,
                energy_uj INTEGER NOT NULL DEFAULT 0,
                energy_wh REAL NOT NULL DEFAULT 0.0
            );
            ",
        )?;

        // Ensure cpu_freq column exists
        let _ = conn.execute("ALTER TABLE metrics_history ADD COLUMN cpu_freq INTEGER DEFAULT 800", []);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_point(&self, p: &HistoryPoint) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO metrics_history 
            (timestamp, cpu_usage, cpu_temp, cpu_freq, fan_rpm, power_watts, lan_rx_speed, lan_tx_speed, vpn_rx_speed, vpn_tx_speed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                p.timestamp,
                p.cpu_usage,
                p.cpu_temp,
                p.cpu_freq as i64,
                p.fan_rpm,
                p.power_watts,
                p.lan_rx_speed,
                p.lan_tx_speed,
                p.vpn_rx_speed,
                p.vpn_tx_speed,
            ],
        )?;
        Ok(())
    }

    pub fn add_energy_delta(&self, delta_uj: u64, delta_wh: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO daily_energy (date, energy_uj, energy_wh)
             VALUES (strftime('%Y-%m-%d', 'now', 'localtime'), ?1, ?2)
             ON CONFLICT(date) DO UPDATE SET
                energy_uj = energy_uj + ?1,
                energy_wh = energy_wh + ?2",
            params![delta_uj as i64, delta_wh],
        )?;
        Ok(())
    }

    pub fn get_cumulative_energy(&self) -> Result<CumulativeEnergy> {
        let conn = self.conn.lock().unwrap();

        let today_wh: f64 = conn.query_row(
            "SELECT COALESCE(SUM(energy_wh), 0.0) FROM daily_energy WHERE date = strftime('%Y-%m-%d', 'now', 'localtime')",
            [],
            |r| r.get(0),
        ).unwrap_or(0.0);

        let month_wh: f64 = conn.query_row(
            "SELECT COALESCE(SUM(energy_wh), 0.0) FROM daily_energy WHERE date LIKE (strftime('%Y-%m', 'now', 'localtime') || '%')",
            [],
            |r| r.get(0),
        ).unwrap_or(today_wh);

        let year_wh: f64 = conn.query_row(
            "SELECT COALESCE(SUM(energy_wh), 0.0) FROM daily_energy WHERE date LIKE (strftime('%Y', 'now', 'localtime') || '%')",
            [],
            |r| r.get(0),
        ).unwrap_or(month_wh);

        Ok(CumulativeEnergy {
            today_wh,
            month_wh,
            year_wh,
        })
    }

    /// Query history with automatic Smart Time-Bucket Aggregation + EMA Smoothing
    pub fn query_history(&self, seconds: i64) -> Result<Vec<HistoryPoint>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let start_time = now - seconds;

        // Calculate dynamic bucket interval in seconds
        let bucket_seconds: i64 = match seconds {
            s if s <= 90 => 1,       // Live 60s -> raw resolution
            s if s <= 900 => 10,     // 15m -> 10s buckets (~90 points)
            s if s <= 3600 => 30,    // 1h -> 30s buckets (~120 points)
            s if s <= 21600 => 180,  // 6h -> 3m buckets (~120 points)
            s if s <= 86400 => 600,  // 24h -> 10m buckets (~144 points)
            _ => 3600,               // 7d -> 1h buckets (~168 points)
        };

        let conn = self.conn.lock().unwrap();

        let mut points = if bucket_seconds <= 1 {
            // Raw resolution query
            let mut stmt = conn.prepare(
                "SELECT timestamp, cpu_usage, cpu_temp, COALESCE(cpu_freq, 800), fan_rpm, power_watts, lan_rx_speed, lan_tx_speed, vpn_rx_speed, vpn_tx_speed
                 FROM metrics_history
                 WHERE timestamp >= ?1
                 ORDER BY timestamp ASC",
            )?;

            let rows = stmt.query_map(params![start_time], |row| {
                Ok(HistoryPoint {
                    timestamp: row.get(0)?,
                    cpu_usage: row.get(1)?,
                    cpu_temp: row.get(2)?,
                    cpu_freq: row.get::<_, i64>(3)? as u64,
                    fan_rpm: row.get(4)?,
                    power_watts: row.get(5)?,
                    lan_rx_speed: row.get(6)?,
                    lan_tx_speed: row.get(7)?,
                    vpn_rx_speed: row.get(8)?,
                    vpn_tx_speed: row.get(9)?,
                })
            })?;

            let mut pts = Vec::new();
            for r in rows {
                if let Ok(p) = r {
                    pts.push(p);
                }
            }
            pts
        } else {
            // Time-bucket aggregated query
            let mut stmt = conn.prepare(
                "SELECT 
                    (timestamp / ?2) * ?2 AS bucket_ts,
                    AVG(cpu_usage) AS avg_cpu,
                    AVG(cpu_temp) AS avg_temp,
                    AVG(COALESCE(cpu_freq, 800)) AS avg_freq,
                    CAST(AVG(fan_rpm) AS INTEGER) AS avg_fan,
                    AVG(power_watts) AS avg_power,
                    AVG(lan_rx_speed) AS avg_lan_rx,
                    AVG(lan_tx_speed) AS avg_lan_tx,
                    AVG(vpn_rx_speed) AS avg_vpn_rx,
                    AVG(vpn_tx_speed) AS avg_vpn_tx
                 FROM metrics_history
                 WHERE timestamp >= ?1
                 GROUP BY bucket_ts
                 ORDER BY bucket_ts ASC",
            )?;

            let rows = stmt.query_map(params![start_time, bucket_seconds], |row| {
                Ok(HistoryPoint {
                    timestamp: row.get(0)?,
                    cpu_usage: (row.get::<_, f64>(1)? as f32 * 10.0).round() / 10.0,
                    cpu_temp: (row.get::<_, f64>(2)? as f32 * 10.0).round() / 10.0,
                    cpu_freq: row.get::<_, f64>(3)?.round() as u64,
                    fan_rpm: row.get::<_, u32>(4)?,
                    power_watts: (row.get::<_, f64>(5)? as f32 * 100.0).round() / 100.0,
                    lan_rx_speed: (row.get::<_, f64>(6)? * 10.0).round() / 10.0,
                    lan_tx_speed: (row.get::<_, f64>(7)? * 10.0).round() / 10.0,
                    vpn_rx_speed: (row.get::<_, f64>(8)? * 10.0).round() / 10.0,
                    vpn_tx_speed: (row.get::<_, f64>(9)? * 10.0).round() / 10.0,
                })
            })?;

            let mut pts = Vec::new();
            for r in rows {
                if let Ok(p) = r {
                    pts.push(p);
                }
            }
            pts
        };

        // Apply EMA (Exponential Moving Average) smoothing pass if > 15 points
        if points.len() > 15 {
            apply_ema_smoothing(&mut points, 0.35);
        }

        Ok(points)
    }

    pub fn prune(&self, retention_days: u32) -> Result<usize> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let cutoff = now - (retention_days as i64 * 86400);

        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM metrics_history WHERE timestamp < ?1", params![cutoff])
    }
}

/// Applies Exponential Moving Average (EMA) smoothing over a vector of HistoryPoints
fn apply_ema_smoothing(points: &mut [HistoryPoint], alpha: f32) {
    if points.is_empty() {
        return;
    }

    let mut last_cpu = points[0].cpu_usage;
    let mut last_power = points[0].power_watts;
    let mut last_lan_rx = points[0].lan_rx_speed;
    let mut last_lan_tx = points[0].lan_tx_speed;
    let mut last_vpn_rx = points[0].vpn_rx_speed;
    let mut last_vpn_tx = points[0].vpn_tx_speed;

    for p in points.iter_mut() {
        last_cpu = alpha * p.cpu_usage + (1.0 - alpha) * last_cpu;
        p.cpu_usage = (last_cpu * 10.0).round() / 10.0;

        last_power = alpha * p.power_watts + (1.0 - alpha) * last_power;
        p.power_watts = (last_power * 100.0).round() / 100.0;

        last_lan_rx = (alpha as f64) * p.lan_rx_speed + (1.0 - (alpha as f64)) * last_lan_rx;
        p.lan_rx_speed = (last_lan_rx * 10.0).round() / 10.0;

        last_lan_tx = (alpha as f64) * p.lan_tx_speed + (1.0 - (alpha as f64)) * last_lan_tx;
        p.lan_tx_speed = (last_lan_tx * 10.0).round() / 10.0;

        last_vpn_rx = (alpha as f64) * p.vpn_rx_speed + (1.0 - (alpha as f64)) * last_vpn_rx;
        p.vpn_rx_speed = (last_vpn_rx * 10.0).round() / 10.0;

        last_vpn_tx = (alpha as f64) * p.vpn_tx_speed + (1.0 - (alpha as f64)) * last_vpn_tx;
        p.vpn_tx_speed = (last_vpn_tx * 10.0).round() / 10.0;
    }
}
