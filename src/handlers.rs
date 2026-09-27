use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use eventsdb::{
    Error as EventsError, EventLog, EventStore, Filter, Position, event::stamp,
    sqlite::SqliteEventLog,
};
use serde_json::Value;
use thiserror::Error;

use crate::models::{
    AppendEventRequest, ErrorResponse, EventResponse, HealthResponse, ListEventsQuery,
};

const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 1_000;

#[derive(Clone)]
pub struct AppState {
    pub log: Arc<SqliteEventLog>,
}

pub async fn append_event(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
    Json(payload): Json<AppendEventRequest>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
    let event = payload.into_event_map();
    let mut stream = state.log.stream_handle(&stream_id);
    let committed = stream.append(event.clone()).await?;
    let stamped = stamp(event, committed.seq, committed.epoch_ms)?;
    let response = EventResponse::from_event_map(stamped).map_err(AppError::response_shape)?;

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn list_events(
    State(state): State<AppState>,
    Path(stream_id): Path<String>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<Vec<EventResponse>>, AppError> {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .min(MAX_LIST_LIMIT);
    let filter = build_filter(stream_id, query)?;
    let events = state
        .log
        .read_all(Position::BEGINNING, &filter, limit)
        .await?;
    let response = events
        .into_iter()
        .map(|recorded| EventResponse::from_event_map(recorded.into_inner()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::response_shape)?;

    Ok(Json(response))
}

pub async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, AppError> {
    state.log.head_position().await?;
    Ok(Json(HealthResponse { status: "ok" }))
}

fn build_filter(stream_id: String, query: ListEventsQuery) -> Result<Filter, AppError> {
    let mut filter = Filter::all().stream(stream_id);

    if let Some(kind) = query.kind {
        filter.kinds = Some(vec![kind]);
    }

    match (query.meta_key, query.meta_value) {
        (Some(key), Some(value)) => {
            filter = filter.meta(key, parse_meta_value(&value));
        }
        (None, None) => {}
        _ => {
            return Err(AppError::InvalidQuery(
                "`meta_key` and `meta_value` must be provided together".to_string(),
            ));
        }
    }

    filter.validate()?;
    Ok(filter)
}

fn parse_meta_value(raw: &str) -> Value {
    match serde_json::from_str::<Value>(raw) {
        Ok(scalar @ Value::Bool(_))
        | Ok(scalar @ Value::Number(_))
        | Ok(scalar @ Value::String(_)) => scalar,
        _ => Value::String(raw.to_string()),
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Events(#[from] EventsError),
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("internal response conversion failed: {0}")]
    ResponseShape(String),
}

impl AppError {
    fn response_shape(message: String) -> Self {
        Self::ResponseShape(message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::Events(EventsError::Validation(_)) | AppError::InvalidQuery(_) => {
                StatusCode::BAD_REQUEST
            }
            AppError::Events(EventsError::Busy(_)) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Events(EventsError::Timeout(_)) => StatusCode::GATEWAY_TIMEOUT,
            AppError::Events(EventsError::HeadMismatch { .. }) => StatusCode::CONFLICT,
            AppError::Events(
                EventsError::Storage(_)
                | EventsError::Corruption(_)
                | EventsError::Unsupported(_)
                | EventsError::Truncated { .. }
                | EventsError::ConsumerBehind { .. }
                | EventsError::NotExported { .. },
            )
            | AppError::ResponseShape(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Events(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorResponse {
            error: self.to_string(),
        });

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use crate::routes::router;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use eventsdb::sqlite::SqliteEventLog;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    async fn app() -> axum::Router {
        let log = SqliteEventLog::open_in_memory().await.unwrap();
        router(std::sync::Arc::new(log))
    }

    #[tokio::test]
    async fn appends_and_lists_events() {
        let app = app().await;

        let response = app
            .clone()
            .oneshot(
                Request::post("/streams/orders-1/events")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "kind": "placed",
                            "meta": { "tenant": "acme", "priority": 3 },
                            "data": { "total": 42 }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let response = app
            .oneshot(
                Request::get("/streams/orders-1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["kind"], json!("placed"));
        assert_eq!(json[0]["meta"]["tenant"], json!("acme"));
        assert_eq!(json[0]["data"]["total"], json!(42));
        assert_eq!(json[0]["seq"], json!(1));
        assert!(json[0]["epoch_ms"].as_u64().is_some());
        assert_eq!(json[0]["schema_version"], json!(1));
    }

    #[tokio::test]
    async fn filters_by_kind_and_meta() {
        let app = app().await;

        for payload in [
            json!({ "kind": "placed", "meta": { "tenant": "acme" }, "data": {} }),
            json!({ "kind": "paid", "meta": { "tenant": "acme" }, "data": {} }),
            json!({ "kind": "placed", "meta": { "tenant": "other" }, "data": {} }),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/streams/orders-2/events")
                        .header("content-type", "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let response = app
            .oneshot(
                Request::get(
                    "/streams/orders-2/events?kind=placed&meta_key=tenant&meta_value=acme",
                )
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["kind"], json!("placed"));
        assert_eq!(json[0]["meta"]["tenant"], json!("acme"));
    }

    #[tokio::test]
    async fn parses_boolean_and_numeric_meta_filters() {
        let app = app().await;

        for payload in [
            json!({ "kind": "noted", "meta": { "closed": true, "priority": 3 }, "data": {} }),
            json!({ "kind": "noted", "meta": { "closed": false, "priority": 4 }, "data": {} }),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/streams/orders-4/events")
                        .header("content-type", "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let response = app
            .clone()
            .oneshot(
                Request::get("/streams/orders-4/events?meta_key=closed&meta_value=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["meta"]["closed"], json!(true));

        let response = app
            .oneshot(
                Request::get("/streams/orders-4/events?meta_key=priority&meta_value=3")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["meta"]["priority"], json!(3));
    }

    #[tokio::test]
    async fn supports_quoted_string_meta_filters_and_limits() {
        let app = app().await;

        for payload in [
            json!({ "kind": "noted", "meta": { "literal": "true" }, "data": {} }),
            json!({ "kind": "noted", "meta": { "literal": "true" }, "data": {} }),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/streams/orders-5/events")
                        .header("content-type", "application/json")
                        .body(Body::from(payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let response = app
            .oneshot(
                Request::get(
                    "/streams/orders-5/events?meta_key=literal&meta_value=%22true%22&limit=1",
                )
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["meta"]["literal"], json!("true"));
    }

    #[tokio::test]
    async fn health_checks_store_access() {
        let app = app().await;

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, json!({ "status": "ok" }));
    }

    #[tokio::test]
    async fn rejects_partial_meta_filter() {
        let app = app().await;

        let response = app
            .oneshot(
                Request::get("/streams/orders-3/events?meta_key=tenant")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
