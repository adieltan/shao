use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::time::Instant;
use sysinfo::Networks;

#[derive(Debug, Clone, Serialize)]
pub struct NetworkMetrics {
    pub lan_rx_speed_bps: f64,
    pub lan_tx_speed_bps: f64,
    pub lan_rx_speed_human: String,
    pub lan_tx_speed_human: String,
    pub lan_rx_total_human: String,
    pub lan_tx_total_human: String,
    pub lan_combined_total_human: String,

    pub wlan_rx_speed_bps: f64,
    pub wlan_tx_speed_bps: f64,
    pub wlan_rx_speed_human: String,
    pub wlan_tx_speed_human: String,
    pub wlan_rx_total_human: String,
    pub wlan_tx_total_human: String,
    pub wlan_combined_total_human: String,

    pub vpn_rx_speed_bps: f64,
    pub vpn_tx_speed_bps: f64,
    pub vpn_rx_speed_human: String,
    pub vpn_tx_speed_human: String,
    pub vpn_rx_total_human: String,
    pub vpn_tx_total_human: String,
    pub vpn_combined_total_human: String,

    pub interfaces: Vec<InterfaceDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceDetail {
    pub name: String,
    pub category: String,
    pub rx_speed_human: String,
    pub tx_speed_human: String,
    pub rx_total_human: String,
    pub tx_total_human: String,
}

pub struct NetworkCollector {
    last_samples: HashMap<String, (u64, u64)>,
    last_time: Instant,
    lan_interfaces: Vec<String>,
    wlan_interfaces: Vec<String>,
    vpn_interfaces: Vec<String>,
    fallback_networks: Networks,
}

impl NetworkCollector {
    pub fn new(
        lan_interfaces: Vec<String>,
        wlan_interfaces: Vec<String>,
        vpn_interfaces: Vec<String>,
    ) -> Self {
        Self {
            last_samples: HashMap::new(),
            last_time: Instant::now(),
            lan_interfaces,
            wlan_interfaces,
            vpn_interfaces,
            fallback_networks: Networks::new_with_refreshed_list(),
        }
    }

    pub fn collect(&mut self) -> NetworkMetrics {
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f64().max(0.1);
        self.last_time = now;

        let current_samples = self.read_proc_net_dev();

        let mut lan_rx_spd = 0.0;
        let mut lan_tx_spd = 0.0;
        let mut lan_rx_tot = 0u64;
        let mut lan_tx_tot = 0u64;

        let mut wlan_rx_spd = 0.0;
        let mut wlan_tx_spd = 0.0;
        let mut wlan_rx_tot = 0u64;
        let mut wlan_tx_tot = 0u64;

        let mut vpn_rx_spd = 0.0;
        let mut vpn_tx_spd = 0.0;
        let mut vpn_rx_tot = 0u64;
        let mut vpn_tx_tot = 0u64;

        let mut interface_details = Vec::new();

        for (iface, (rx, tx)) in &current_samples {
            let is_wlan = self.wlan_interfaces.iter().any(|p| iface.starts_with(p));
            let is_lan = self.lan_interfaces.iter().any(|p| iface.starts_with(p));
            let is_vpn = self.vpn_interfaces.iter().any(|p| iface.starts_with(p));

            let (old_rx, old_tx) = self.last_samples.get(iface).copied().unwrap_or((*rx, *tx));
            let rx_spd = (rx.saturating_sub(old_rx) as f64) / dt;
            let tx_spd = (tx.saturating_sub(old_tx) as f64) / dt;

            let category = if is_wlan {
                wlan_rx_spd += rx_spd;
                wlan_tx_spd += tx_spd;
                wlan_rx_tot += rx;
                wlan_tx_tot += tx;
                "WLAN".to_string()
            } else if is_lan {
                lan_rx_spd += rx_spd;
                lan_tx_spd += tx_spd;
                lan_rx_tot += rx;
                lan_tx_tot += tx;
                "LAN".to_string()
            } else if is_vpn {
                vpn_rx_spd += rx_spd;
                vpn_tx_spd += tx_spd;
                vpn_rx_tot += rx;
                vpn_tx_tot += tx;
                "VPN".to_string()
            } else {
                "Other".to_string()
            };

            interface_details.push(InterfaceDetail {
                name: iface.clone(),
                category,
                rx_speed_human: format_speed(rx_spd),
                tx_speed_human: format_speed(tx_spd),
                rx_total_human: format_bytes(*rx),
                tx_total_human: format_bytes(*tx),
            });
        }

        self.last_samples = current_samples;

        NetworkMetrics {
            lan_rx_speed_bps: lan_rx_spd,
            lan_tx_speed_bps: lan_tx_spd,
            lan_rx_speed_human: format_speed(lan_rx_spd),
            lan_tx_speed_human: format_speed(lan_tx_spd),
            lan_rx_total_human: format_bytes(lan_rx_tot),
            lan_tx_total_human: format_bytes(lan_tx_tot),
            lan_combined_total_human: format_bytes(lan_rx_tot + lan_tx_tot),

            wlan_rx_speed_bps: wlan_rx_spd,
            wlan_tx_speed_bps: wlan_tx_spd,
            wlan_rx_speed_human: format_speed(wlan_rx_spd),
            wlan_tx_speed_human: format_speed(wlan_tx_spd),
            wlan_rx_total_human: format_bytes(wlan_rx_tot),
            wlan_tx_total_human: format_bytes(wlan_tx_tot),
            wlan_combined_total_human: format_bytes(wlan_rx_tot + wlan_tx_tot),

            vpn_rx_speed_bps: vpn_rx_spd,
            vpn_tx_speed_bps: vpn_tx_spd,
            vpn_rx_speed_human: format_speed(vpn_rx_spd),
            vpn_tx_speed_human: format_speed(vpn_tx_spd),
            vpn_rx_total_human: format_bytes(vpn_rx_tot),
            vpn_tx_total_human: format_bytes(vpn_tx_tot),
            vpn_combined_total_human: format_bytes(vpn_rx_tot + vpn_tx_tot),

            interfaces: interface_details,
        }
    }

    fn read_proc_net_dev(&mut self) -> HashMap<String, (u64, u64)> {
        let mut map = HashMap::new();
        if let Ok(content) = fs::read_to_string("/proc/net/dev") {
            for line in content.lines().skip(2) {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() == 2 {
                    let iface = parts[0].trim().to_string();
                    let fields: Vec<u64> = parts[1]
                        .split_whitespace()
                        .filter_map(|s| s.parse::<u64>().ok())
                        .collect();
                    if fields.len() >= 9 {
                        let rx = fields[0];
                        let tx = fields[8];
                        map.insert(iface, (rx, tx));
                    }
                }
            }
        } else {
            self.fallback_networks.refresh();
            for (iface, data) in &self.fallback_networks {
                map.insert(iface.clone(), (data.total_received(), data.total_transmitted()));
            }
        }
        map
    }
}

pub fn format_bytes(bytes: u64) -> String {
    let mut b = bytes as f64;
    for u in ["B", "KB", "MB", "GB", "TB"] {
        if b < 1024.0 {
            return format!("{:.1} {}", b, u);
        }
        b /= 1024.0;
    }
    format!("{:.1} PB", b)
}

pub fn format_speed(bps: f64) -> String {
    if bps < 1024.0 {
        format!("{:.0} B/s", bps)
    } else if bps < 1024.0 * 1024.0 {
        format!("{:.1} KB/s", bps / 1024.0)
    } else if bps < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB/s", bps / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bps / (1024.0 * 1024.0 * 1024.0))
    }
}
