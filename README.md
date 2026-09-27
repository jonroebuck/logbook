# logbook

Logbook is a small standalone HTTP service that wraps the [`eventsdb`](https://crates.io/crates/eventsdb) embedded event store and exposes it over a REST API. Each deployment owns its own SQLite file, so instances are fully independent and can be run side-by-side without any shared state.

## Configuration

Logbook is configured with environment variables:

- `LOGBOOK_DB_PATH` - path to the SQLite database file (default: `/data/logbook.db`)
- `LOGBOOK_PORT` - port to listen on (default: `8080`)

## Running locally

```bash
LOGBOOK_DB_PATH=./logbook.db LOGBOOK_PORT=8080 cargo run
```

## Running with Docker

Build the image:

```bash
docker build -t logbook .
```

Run the container with a persistent volume for the database file:

```bash
docker run --rm -p 8080:8080 \
  -e LOGBOOK_DB_PATH=/data/logbook.db \
  -e LOGBOOK_PORT=8080 \
  -v "$PWD/data:/data" \
  logbook
```

## API

### `POST /streams/{stream_id}/events`

Append an event to a stream.

Request body:

```json
{
  "kind": "placed",
  "meta": { "tenant": "acme", "priority": 3 },
  "data": { "total": 42 }
}
```

Response body:

```json
{
  "kind": "placed",
  "meta": { "tenant": "acme", "priority": 3 },
  "data": { "total": 42 },
  "seq": 1,
  "epoch_ms": 1720000000000,
  "schema_version": 1
}
```

`meta` values must be flat scalars (strings, numbers, booleans), matching `eventsdb`'s envelope rules.

### `GET /streams/{stream_id}/events`

Read events for a stream in order.

Optional query parameters:

- `kind` - only return events of a single kind
- `meta_key` and `meta_value` - filter by an event `meta` key/value pair
- `limit` - maximum number of events to return (default `100`, max `1000`)

`meta_value` is parsed as a JSON scalar when possible, so `true` matches a boolean, `3` matches a number, and `%22text%22` (URL-encoded `"text"`) matches the literal string `"text"`.

Examples:

```text
GET /streams/orders-1/events
GET /streams/orders-1/events?kind=placed
GET /streams/orders-1/events?meta_key=tenant&meta_value=acme
GET /streams/orders-1/events?kind=placed&meta_key=priority&meta_value=3
GET /streams/orders-1/events?meta_key=literal&meta_value=%22true%22&limit=10
```

### `GET /health`

Basic liveness check that verifies the event store is reachable.

## Multiple independent instances

Logbook has no shared state beyond the SQLite file configured for a single process. You can run multiple independent instances side-by-side (for example, one per consuming project), each pointing at its own database file.

## Container image

The GitHub Actions workflow builds and publishes a container image to:

```text
ghcr.io/<owner>/logbook
```

On pushes to `main`, the image is published with `latest` and the git SHA. Version tag pushes also publish the tag name.
