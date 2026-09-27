use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use eventsdb::sqlite::SqliteEventLog;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::handlers::{AppState, append_event, health, list_events};

pub fn router(log: Arc<SqliteEventLog>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/streams/{stream_id}/events",
            post(append_event).get(list_events),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { log })
}
