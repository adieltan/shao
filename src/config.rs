use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub power: PowerConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub apps: Vec<AppCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_interval")]
    pub polling_interval_ms: u64,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_retention_days")]
    pub history_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerConfig {
    #[serde(default = "default_kwh_cost")]
    pub kwh_cost: f64,
    #[serde(default = "default_currency")]
    pub currency_symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    #[serde(default = "default_lan_interfaces")]
    pub lan_interfaces: Vec<String>,
    #[serde(default = "default_vpn_interfaces")]
    pub vpn_interfaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppCard {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default)]
    pub container: Option<String>,
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_interval() -> u64 { 1000 }
fn default_db_path() -> String { "shao.db".to_string() }
fn default_retention_days() -> u32 { 7 }
fn default_kwh_cost() -> f64 { 0.15 }
fn default_currency() -> String { "$".to_string() }
fn default_icon() -> String { "globe".to_string() }

fn default_lan_interfaces() -> Vec<String> {
    vec![
        "enp2s0".into(), "eth0".into(), "wlp3s0".into(), "wlan0".into(), "en0".into(),
    ]
}

fn default_vpn_interfaces() -> Vec<String> {
    vec![
        "tailscale0".into(), "wg0".into(), "tun0".into(), "utun".into(),
    ]
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            polling_interval_ms: default_interval(),
            db_path: default_db_path(),
            history_retention_days: default_retention_days(),
        }
    }
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            kwh_cost: default_kwh_cost(),
            currency_symbol: default_currency(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            lan_interfaces: default_lan_interfaces(),
            vpn_interfaces: default_vpn_interfaces(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            power: PowerConfig::default(),
            network: NetworkConfig::default(),
            apps: vec![
                AppCard {
                    name: "Immich".into(),
                    url: "http://192.168.1.1:2283".into(),
                    description: "Photo & Video Backup Server".into(),
                    icon: "image".into(),
                    container: Some("immich_server".into()),
                },
                AppCard {
                    name: "Dockge".into(),
                    url: "http://192.168.1.1:5001".into(),
                    description: "Docker Compose Stack Manager".into(),
                    icon: "layers".into(),
                    container: Some("dockge".into()),
                },
                AppCard {
                    name: "Tailscale".into(),
                    url: "https://login.tailscale.com/admin/machines".into(),
                    description: "Encrypted Mesh VPN".into(),
                    icon: "shield".into(),
                    container: None,
                },
            ],
        }
    }
}

impl Config {
    pub fn load_or_default<P: AsRef<Path>>(path: Option<P>) -> Self {
        if let Some(p) = path {
            let path_ref = p.as_ref();
            if path_ref.exists() {
                if let Ok(content) = fs::read_to_string(path_ref) {
                    if let Ok(cfg) = toml::from_str::<Config>(&content) {
                        return cfg;
                    }
                }
            }
        }
        Config::default()
    }
}
