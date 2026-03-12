use axum::Json;
use chrono::Utc;
use serde::Serialize;

/// The package version from `Cargo.toml`, embedded at compile time.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Response body for `GET /health`.
#[derive(Serialize)]
pub struct HealthResponse {
    /// Service health status — always `"ok"` when this handler is reachable.
    pub status: &'static str,
    /// Semantic version of the running collector binary.
    pub version: &'static str,
    /// Current UTC timestamp in ISO 8601 / RFC 3339 format.
    pub timestamp: String,
}

/// `GET /health`
///
/// Lightweight liveness check.  Returns HTTP 200 with a small JSON body as
/// long as the process is alive and able to handle requests.  Suitable for
/// load-balancer health checks and Kubernetes liveness / readiness probes.
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: VERSION,
        timestamp: Utc::now().to_rfc3339(),
    })
}
