use eventsdb::event::{
    FIELD_DATA, FIELD_EPOCH_MS, FIELD_KIND, FIELD_META, FIELD_SCHEMA_VERSION, FIELD_SEQ,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct AppendEventRequest {
    pub kind: String,
    #[serde(default)]
    pub meta: Option<Map<String, Value>>,
    #[serde(default)]
    pub data: Option<Map<String, Value>>,
}

impl AppendEventRequest {
    pub fn into_event_map(self) -> Map<String, Value> {
        let mut event = Map::new();
        event.insert(FIELD_KIND.to_string(), Value::String(self.kind));
        if let Some(meta) = self.meta {
            event.insert(FIELD_META.to_string(), Value::Object(meta));
        }
        if let Some(data) = self.data {
            event.insert(FIELD_DATA.to_string(), Value::Object(data));
        }
        event
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListEventsQuery {
    pub kind: Option<String>,
    pub meta_key: Option<String>,
    pub meta_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EventResponse {
    pub kind: String,
    pub meta: Map<String, Value>,
    pub data: Map<String, Value>,
    pub seq: u64,
    pub epoch_ms: u64,
    pub schema_version: u64,
}

impl EventResponse {
    pub fn from_event_map(mut event: Map<String, Value>) -> Result<Self, String> {
        Ok(Self {
            kind: take_string(&mut event, FIELD_KIND)?,
            meta: take_object(&mut event, FIELD_META)?,
            data: take_object(&mut event, FIELD_DATA)?,
            seq: take_u64(&mut event, FIELD_SEQ)?,
            epoch_ms: take_u64(&mut event, FIELD_EPOCH_MS)?,
            schema_version: take_u64(&mut event, FIELD_SCHEMA_VERSION)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

fn take_string(event: &mut Map<String, Value>, field: &str) -> Result<String, String> {
    event
        .remove(field)
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .ok_or_else(|| format!("missing or invalid `{field}` field"))
}

fn take_object(event: &mut Map<String, Value>, field: &str) -> Result<Map<String, Value>, String> {
    event
        .remove(field)
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| format!("missing or invalid `{field}` field"))
}

fn take_u64(event: &mut Map<String, Value>, field: &str) -> Result<u64, String> {
    event
        .remove(field)
        .and_then(|value| value.as_u64())
        .ok_or_else(|| format!("missing or invalid `{field}` field"))
}
