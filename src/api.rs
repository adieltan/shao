use crate::config::Config;
use crate::db::Database;
use crate::sensors::FullTelemetry;
use axum::{
    extract::{Query, State},
    http::{header, HeaderValue, StatusCode, Uri},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::get,
    Json, Router,
};
use rust_embed::RustEmbed;
use serde::Deserialize;
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

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/telemetry", get(get_telemetry))
        .route("/api/history", get(get_history))
        .route("/api/config", get(get_config))
        .route("/api/stream", get(stream_sse))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .with_state(state)
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
                [(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())],
                content.data,
            )
                .into_response()
        }
        None => {
            // Fallback to index.html for SPA routing
            if let Some(index) = Assets::get("index.html") {
                (
                    [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
                    index.data,
                )
                    .into_response()
            } else {
                (StatusCode::NOT_FOUND, "404 Not Found").into_response()
            }
        }
    }
}
