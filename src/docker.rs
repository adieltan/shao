use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(unix)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(unix)]
use tokio::net::UnixStream;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub is_running: bool,
    pub created_at: i64,
    #[serde(default)]
    pub ports: Vec<u16>,
}

#[derive(Deserialize)]
struct RawPort {
    #[serde(rename = "IP")]
    _ip: Option<String>,
    #[serde(rename = "PrivatePort")]
    private_port: Option<u16>,
    #[serde(rename = "PublicPort")]
    public_port: Option<u16>,
    #[serde(rename = "Type")]
    _port_type: Option<String>,
}

#[derive(Deserialize)]
struct RawContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Names")]
    names: Vec<String>,
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "State")]
    state: String,
    #[serde(rename = "Created")]
    created: i64,
    #[serde(rename = "Ports", default)]
    ports: Vec<RawPort>,
}

pub struct DockerClient {
    socket_path: String,
}

impl DockerClient {
    pub fn new() -> Self {
        Self {
            socket_path: "/var/run/docker.sock".to_string(),
        }
    }

    pub async fn list_containers(&self) -> Vec<DockerContainer> {
        if !Path::new(&self.socket_path).exists() {
            return Vec::new();
        }

        #[cfg(unix)]
        {
            match UnixStream::connect(&self.socket_path).await {
                Ok(mut stream) => {
                    let request = "GET /containers/json?all=1 HTTP/1.0\r\nHost: localhost\r\nAccept: application/json\r\n\r\n";
                    if stream.write_all(request.as_bytes()).await.is_err() {
                        return Vec::new();
                    }

                    let mut response = Vec::new();
                    if stream.read_to_end(&mut response).await.is_err() {
                        return Vec::new();
                    }

                    let response_str = String::from_utf8_lossy(&response);
                    if let Some(body_start) = response_str.find("\r\n\r\n") {
                        let json_body = &response_str[body_start + 4..];
                        if let Ok(raw_list) = serde_json::from_str::<Vec<RawContainer>>(json_body) {
                            return raw_list
                                .into_iter()
                                .map(|c| {
                                    let clean_name = c
                                        .names
                                        .first()
                                        .cloned()
                                        .unwrap_or_else(|| "container".into())
                                        .trim_start_matches('/')
                                        .to_string();
                                    let is_running = c.state.to_lowercase() == "running";
                                    let mut ports: Vec<u16> = c
                                        .ports
                                        .iter()
                                        .filter_map(|p| p.public_port.or(p.private_port))
                                        .collect();
                                    ports.sort_unstable();
                                    ports.dedup();

                                    DockerContainer {
                                        id: c.id[..12.min(c.id.len())].to_string(),
                                        name: clean_name,
                                        image: c.image,
                                        status: c.status,
                                        state: c.state,
                                        is_running,
                                        created_at: c.created,
                                        ports,
                                    }
                                })
                                .collect();
                        }
                    }
                }
                Err(_) => return Vec::new(),
            }
        }

        Vec::new()
    }
}
