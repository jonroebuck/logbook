use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Router, http::Method, http::header::CONTENT_TYPE};
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
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST])
                .allow_headers([CONTENT_TYPE]),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { log })
}
