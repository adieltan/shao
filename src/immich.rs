use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmichStats {
    pub photos: u64,
    pub videos: u64,
    pub total_assets: u64,
    pub usage_bytes: u64,
    pub usage_human: String,
    pub user_name: String,
}

#[derive(Deserialize)]
struct RawUserUsage {
    #[serde(rename = "userName")]
    user_name: String,
}

#[derive(Deserialize)]
struct RawImmichStats {
    photos: u64,
    videos: u64,
    usage: u64,
    #[serde(rename = "usageByUser")]
    usage_by_user: Option<Vec<RawUserUsage>>,
}

pub struct ImmichClient {
    url: String,
    api_key: String,
}

impl ImmichClient {
    pub fn new(url: String, api_key: String) -> Self {
        Self { url, api_key }
    }

    pub async fn fetch_stats(&self) -> Option<ImmichStats> {
        let endpoint = format!("{}/api/server/statistics", self.url.trim_end_matches('/'));
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .ok()?;

        let resp = client
            .get(&endpoint)
            .header("x-api-key", &self.api_key)
            .send()
            .await
            .ok()?;

        if resp.status().is_success() {
            if let Ok(raw) = resp.json::<RawImmichStats>().await {
                let user_name = raw
                    .usage_by_user
                    .as_ref()
                    .and_then(|u| u.first())
                    .map(|u| u.user_name.clone())
                    .unwrap_or_else(|| "User".into());

                return Some(ImmichStats {
                    photos: raw.photos,
                    videos: raw.videos,
                    total_assets: raw.photos + raw.videos,
                    usage_bytes: raw.usage,
                    usage_human: format_bytes(raw.usage),
                    user_name,
                });
            }
        }

        None
    }
}

fn format_bytes(bytes: u64) -> String {
    let mut b = bytes as f64;
    for u in ["B", "KB", "MB", "GB", "TB"] {
        if b < 1024.0 {
            return format!("{:.1} {}", b, u);
        }
        b /= 1024.0;
    }
    format!("{:.1} PB", b)
}
