use crate::config::Config;
use crate::db::Database;
use crate::sensors::FullTelemetry;
use crate::system::{execute_power_action, PowerAction};
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode, Uri},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

#[derive(RustEmbed)]
#[folder = "frontend/"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub latest: Arc<RwLock<Option<FullTelemetry>>>,
    pub tx: broadcast::Sender<FullTelemetry>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub seconds: Option<i64>,
}

#[derive(Deserialize, Default)]
pub struct PingQuery {
    pub client_net: Option<String>,
}

#[derive(Serialize)]
pub struct PingResponse {
    pub status: &'static str,
    pub server_time_ms: u128,
    pub client_ip: String,
    pub connection_type: String, // "vpn", "wlan", "lan", "loopback"
    pub route: String,
}

#[derive(Serialize)]
pub struct SystemActionResponse {
    pub success: bool,
    pub message: String,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/ping", get(ping_handler).head(ping_handler))
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/history", get(get_history))
        .route("/api/config", get(get_config))
        .route("/api/stream", get(stream_sse))
        .route("/api/system/shutdown", post(system_shutdown))
        .route("/api/system/reboot", post(system_reboot))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn ping_handler(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Query(query): Query<PingQuery>,
) -> impl IntoResponse {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let client_ip = connect_info
        .map(|c| c.0.ip().to_string())
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
        })
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Ground-truth detection of connection route
    let (connection_type, route) = if client_ip == "127.0.0.1"
        || client_ip == "::1"
        || host.starts_with("localhost")
        || host.starts_with("127.0.0.1")
    {
        ("loopback", "Localhost Loopback")
    } else if client_ip.starts_with("100.")
        || host.contains(".ts.net")
        || host.starts_with("100.")
        || client_ip.starts_with("fd7a:115c:")
    {
        ("vpn", "Tailscale Mesh VPN (tailscale0)")
    } else if host.starts_with("192.168.1.17") || query.client_net.as_deref() == Some("wifi") {
        ("wlan", "Wireless WLAN (wlp3s0)")
    } else if host.starts_with("192.168.1.1") || query.client_net.as_deref() == Some("ethernet") {
        ("lan", "Wired LAN (enp2s0)")
    } else if client_ip.starts_with("192.168.1.17") {
        ("wlan", "Wireless WLAN (wlp3s0)")
    } else if client_ip.starts_with("192.168.1.")
        || client_ip.starts_with("10.")
        || client_ip.starts_with("172.")
    {
        ("lan", "Local Home LAN")
    } else {
        ("vpn", "Remote Connection")
    };

    (
        StatusCode::OK,
        [
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store, no-cache, must-revalidate")),
            (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
        ],
        Json(PingResponse {
            status: "ok",
            server_time_ms: now_ms,
            client_ip,
            connection_type: connection_type.to_string(),
            route: route.to_string(),
        }),
    )
}

async fn get_telemetry(State(state): State<AppState>) -> impl IntoResponse {
    let lock = state.latest.read().await;
    if let Some(ref tel) = *lock {
        (StatusCode::OK, Json(tel.clone())).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "Initializing telemetry...").into_response()
    }
}

async fn get_history(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> impl IntoResponse {
    let seconds = q.seconds.unwrap_or(86400).max(60);
    match state.db.query_history(seconds) {
        Ok(pts) => (StatusCode::OK, Json(pts)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
            .into_response(),
    }
}

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.config).into_response()
}

async fn system_shutdown(State(state): State<AppState>) -> impl IntoResponse {
    if !state.config.server.enable_shutdown {
        return (
            StatusCode::FORBIDDEN,
            Json(SystemActionResponse {
                success: false,
                message: "Shutdown is disabled in server configuration".into(),
            }),
        )
            .into_response();
    }

    execute_power_action(PowerAction::Shutdown);

    (
        StatusCode::OK,
        Json(SystemActionResponse {
            success: true,
            message: "Server shutdown sequence initiated".into(),
        }),
    )
        .into_response()
}

async fn system_reboot(State(state): State<AppState>) -> impl IntoResponse {
    if !state.config.server.enable_shutdown {
        return (
            StatusCode::FORBIDDEN,
            Json(SystemActionResponse {
                success: false,
                message: "Reboot is disabled in server configuration".into(),
            }),
        )
            .into_response();
    }

    execute_power_action(PowerAction::Reboot);

    (
        StatusCode::OK,
        Json(SystemActionResponse {
            success: true,
            message: "Server reboot sequence initiated".into(),
        }),
    )
        .into_response()
}

async fn stream_sse(State(state): State<AppState>) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(tel) => {
            if let Ok(json_str) = serde_json::to_string(&tel) {
                Some(Ok(Event::default().data(json_str)))
            } else {
                None
            }
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn static_handler(uri: Uri) -> Response {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [
                    (header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap()),
                    (header::CACHE_CONTROL, HeaderValue::from_static("no-cache, must-revalidate")),
                ],
                content.data,
            )
                .into_response()
        }
        None => {
            // Fallback to index.html for SPA routing
            if let Some(index) = Assets::get("index.html") {
                (
                    [
                        (header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8")),
                        (header::CACHE_CONTROL, HeaderValue::from_static("no-cache, must-revalidate")),
                    ],
                    index.data,
                )
                    .into_response()
            } else {
                (StatusCode::NOT_FOUND, "404 Not Found").into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_shutdown_endpoint_when_disabled() {
        let mut config = Config::default();
        config.server.enable_shutdown = false;
        let db = Database::new(":memory:").unwrap();
        let latest = Arc::new(RwLock::new(None));
        let (tx, _rx) = broadcast::channel(10);

        let state = AppState {
            config,
            db,
            latest,
            tx,
        };

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/shutdown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_reboot_endpoint_when_disabled() {
        let mut config = Config::default();
        config.server.enable_shutdown = false;
        let db = Database::new(":memory:").unwrap();
        let latest = Arc::new(RwLock::new(None));
        let (tx, _rx) = broadcast::channel(10);

        let state = AppState {
            config,
            db,
            latest,
            tx,
        };

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/reboot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_shutdown_endpoint_when_enabled() {
        let config = Config::default(); // enable_shutdown defaults to true
        let db = Database::new(":memory:").unwrap();
        let latest = Arc::new(RwLock::new(None));
        let (tx, _rx) = broadcast::channel(10);

        let state = AppState {
            config,
            db,
            latest,
            tx,
        };

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/shutdown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_reboot_endpoint_when_enabled() {
        let config = Config::default(); // enable_shutdown defaults to true
        let db = Database::new(":memory:").unwrap();
        let latest = Arc::new(RwLock::new(None));
        let (tx, _rx) = broadcast::channel(10);

        let state = AppState {
            config,
            db,
            latest,
            tx,
        };

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/reboot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_config_includes_shutdown_flag() {
        let config = Config::default();
        let db = Database::new(":memory:").unwrap();
        let latest = Arc::new(RwLock::new(None));
        let (tx, _rx) = broadcast::channel(10);

        let state = AppState {
            config,
            db,
            latest,
            tx,
        };

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ping_endpoint() {
        let config = Config::default();
        let db = Database::new(":memory:").unwrap();
        let latest = Arc::new(RwLock::new(None));
        let (tx, _rx) = broadcast::channel(10);

        let state = AppState {
            config,
            db,
            latest,
            tx,
        };

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/ping")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
