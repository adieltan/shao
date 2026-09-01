use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub timestamp: i64,
    pub cpu_usage: f32,
    pub cpu_temp: f32,
    pub fan_rpm: u32,
    pub power_watts: f32,
    pub lan_rx_speed: f64,
    pub lan_tx_speed: f64,
    pub vpn_rx_speed: f64,
    pub vpn_tx_speed: f64,
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
                fan_rpm INTEGER,
                power_watts REAL,
                lan_rx_speed REAL,
                lan_tx_speed REAL,
                vpn_rx_speed REAL,
                vpn_tx_speed REAL
            );
            CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics_history(timestamp);
            ",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_point(&self, p: &HistoryPoint) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO metrics_history 
            (timestamp, cpu_usage, cpu_temp, fan_rpm, power_watts, lan_rx_speed, lan_tx_speed, vpn_rx_speed, vpn_tx_speed)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                p.timestamp,
                p.cpu_usage,
                p.cpu_temp,
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

    pub fn query_history(&self, seconds: i64) -> Result<Vec<HistoryPoint>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let start_time = now - seconds;

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT timestamp, cpu_usage, cpu_temp, fan_rpm, power_watts, lan_rx_speed, lan_tx_speed, vpn_rx_speed, vpn_tx_speed
             FROM metrics_history
             WHERE timestamp >= ?1
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![start_time], |row| {
            Ok(HistoryPoint {
                timestamp: row.get(0)?,
                cpu_usage: row.get(1)?,
                cpu_temp: row.get(2)?,
                fan_rpm: row.get(3)?,
                power_watts: row.get(4)?,
                lan_rx_speed: row.get(5)?,
                lan_tx_speed: row.get(6)?,
                vpn_rx_speed: row.get(7)?,
                vpn_tx_speed: row.get(8)?,
            })
        })?;

        let mut points = Vec::new();
        for r in rows {
            if let Ok(p) = r {
                points.push(p);
            }
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
