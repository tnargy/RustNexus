use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    db::{self, AgentSummary, MetricSnapshot, Threshold},
    AppState,
};

// ---------------------------------------------------------------------------
// Status computation
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Online,
    Warning,
    Critical,
    Offline,
}

fn compute_status(
    summary: &AgentSummary,
    thresholds: &[Threshold],
    offline_threshold_secs: u64,
) -> AgentStatus {
    let elapsed = (Utc::now() - summary.last_seen_at).num_seconds();
    if elapsed < 0 || elapsed as u64 > offline_threshold_secs {
        return AgentStatus::Offline;
    }

    let snapshot = match &summary.latest_metric {
        None => return AgentStatus::Online,
        Some(s) => s,
    };

    // Thresholds that apply to this agent (agent-specific or global).
    let applicable = thresholds.iter().filter(|t| {
        t.agent_id.is_none() || t.agent_id.as_deref() == Some(summary.agent_id.as_str())
    });

    let mut worst = AgentStatus::Online;
    for threshold in applicable {
        let metric_value: f64 = match threshold.metric_name.as_str() {
            "cpu" => snapshot.cpu_percent,
            "memory" => snapshot.memory_percent,
            "disk" => snapshot
                .disks
                .iter()
                .map(|d| d.percent)
                .fold(f64::NEG_INFINITY, f64::max),
            _ => continue,
        };

        // A threshold of 0.0 means "unconfigured — no alerting".
        if threshold.critical_value > 0.0 && metric_value >= threshold.critical_value {
            return AgentStatus::Critical;
        }
        if threshold.warning_value > 0.0 && metric_value >= threshold.warning_value {
            worst = AgentStatus::Warning;
        }
    }
    worst
}

// ---------------------------------------------------------------------------
// Response type
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AgentResponse {
    #[serde(flatten)]
    pub summary: AgentSummary,
    pub status: AgentStatus,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/v1/agents`
///
/// Returns all known agents with a computed status field. Returns an empty
/// array (not 404) when no agents have been seen yet.
pub async fn list_agents(
    State(state): State<AppState>,
) -> Result<Json<Vec<AgentResponse>>, StatusCode> {
    let (summaries, thresholds) = tokio::try_join!(
        db::queries::get_agents_summary(&state.pool),
        db::queries::get_thresholds(&state.pool),
    )
    .map_err(|e| {
        tracing::error!(error = %e, "database error in list_agents");
        StatusCode::SERVICE_UNAVAILABLE
    })?;

    let responses = summaries
        .into_iter()
        .map(|summary| {
            let status = compute_status(&summary, &thresholds, state.offline_threshold_secs);
            AgentResponse { summary, status }
        })
        .collect();

    Ok(Json(responses))
}

/// `GET /api/v1/agents/:agent_id/snapshot`
///
/// Returns the most-recent metric snapshot for the agent. 404 if unknown.
pub async fn get_snapshot(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<MetricSnapshot>, StatusCode> {
    let snapshot = db::queries::get_snapshot(&state.pool, &agent_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "database error in get_snapshot");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    match snapshot {
        Some(s) => Ok(Json(s)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    range: Option<String>,
}

/// `GET /api/v1/agents/:agent_id/history?range=1h|6h|24h|7d`
///
/// Returns snapshots ordered by `timestamp ASC`, subsampled to ≤ 300 points.
/// Defaults to `1h` if `range` is absent or unrecognised.
/// Returns an empty array (not 404) if no data exists for the requested window.
pub async fn get_history(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<Vec<MetricSnapshot>>, StatusCode> {
    let duration_secs: i64 = match params.range.as_deref().unwrap_or("1h") {
        "6h" => 6 * 3600,
        "24h" => 24 * 3600,
        "7d" => 7 * 24 * 3600,
        _ => 3600, // "1h" and any unrecognised value
    };

    let since = Utc::now() - chrono::Duration::seconds(duration_secs);
    let mut snapshots = db::queries::get_history(&state.pool, &agent_id, since)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "database error in get_history");
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    // Subsample to at most 300 data points by taking every Nth row.
    const MAX_POINTS: usize = 300;
    if snapshots.len() > MAX_POINTS {
        let step = (snapshots.len() + MAX_POINTS - 1) / MAX_POINTS;
        snapshots = snapshots
            .into_iter()
            .enumerate()
            .filter(|(i, _)| i % step == 0)
            .map(|(_, s)| s)
            .collect();
    }

    Ok(Json(snapshots))
}
