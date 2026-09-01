use crate::config::Config;
use crate::db::Database;
use crate::sensors::FullTelemetry;
use crate::system::{execute_power_action, PowerAction};
use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode, Uri},
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

async fn ping_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store, no-cache, must-revalidate")),
            (header::CONTENT_TYPE, HeaderValue::from_static("text/plain")),
        ],
        "pong",
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
