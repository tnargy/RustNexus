---
name: System Monitor Spec
overview: "A distributed monitoring platform: a lightweight Rust agent per server ships metrics to a Rust collector API, which persists them to a database; a modern web dashboard displays real-time health, configurable thresholds, and offline alerts across 21–100 machines for both technical operators and non-technical stakeholders."
todos:
  - id: resolve-ambiguities
    content: Resolve the 8 Ambiguity Warnings with the user before handing spec to an implementing agent
    status: completed
  - id: save-spec
    content: Save the final spec as a .md file in the project directory once ambiguities are resolved
    status: pending
  - id: implement-agent
    content: Implement the Rust agent component
    status: pending
  - id: implement-collector
    content: Implement the Rust collector API and database layer
    status: pending
  - id: implement-dashboard
    content: Implement the web dashboard frontend
    status: pending
isProject: false
---

# System Monitoring Platform — Specification

## System Overview

A distributed monitoring platform consisting of three independent applications: a lightweight Rust agent deployed on each monitored server, a Rust-based collector service that receives and persists metrics over a REST API, and a modern web dashboard that displays real-time system health with configurable thresholds and offline detection. The platform serves both technical operators who configure and triage issues and non-technical stakeholders who need at-a-glance health visibility across 21–100 machines.

---

## Behavioral Contract

### Agent Behaviors

- When an agent starts, it reads its configuration file, validates it, and begins collecting metrics at the configured interval. If the config file is missing or malformed, the agent exits immediately with a descriptive error and a non-zero exit code.
- When the collection interval elapses, the agent gathers: CPU usage (percent), memory (used/total/percent), disk usage per mounted volume (used/total/percent/mount point), network throughput (bytes in/out since last sample), and system uptime (seconds).
- When the agent has a collected batch, it sends it via HTTP POST to the configured collector URL, including the agent's unique identifier and a UTC timestamp.
- When the collector is unreachable or returns a non-2xx response, the agent buffers the payload in memory and retries at the configured interval until the collector responds or the buffer duration limit is reached.
- When the buffer duration limit is reached, the agent drops the oldest buffered payloads and records a warning in its local log. It does not crash.

### Collector Behaviors

- When the collector receives a valid metric payload, it persists the data to the database and returns HTTP 200.
- When the collector receives a payload from an agent identifier not previously seen, it creates a new agent record automatically and persists the data (no pre-registration required).
- When the collector receives a malformed payload or one missing required fields, it returns HTTP 400 and does not persist anything.
- When simultaneous payloads arrive from multiple agents, the collector handles them concurrently without data loss or corruption.
- When metric data in the database is older than the configured retention period, the collector purges it (via a scheduled internal process).
- When the database is unavailable, the collector returns HTTP 503 to the calling agent.

### Dashboard Behaviors

- When a user opens the dashboard, they see all known agents with a status indicator: online, offline, warning, or critical.
- When an agent's most recent report timestamp exceeds the configured offline threshold, the dashboard marks that agent as **offline** with a prominent, unambiguous visual indicator.
- When a metric value exceeds its configured warning threshold, the dashboard shows a **warning** indicator on that metric.
- When a metric value exceeds its configured critical threshold, the dashboard shows a **critical** indicator on that metric.
- When a user selects an agent, they see current metric values and a historical trend for each metric over a selectable time range.
- When the dashboard is open, it maintains a persistent WebSocket connection to the collector. When the collector receives new metric data from any agent, it immediately pushes the update to all connected dashboard clients. The dashboard updates the affected agent card and metric displays without any manual page reload.
- When the WebSocket connection drops, the dashboard displays a visible "disconnected" indicator and attempts to reconnect automatically. While disconnected it does not update metric displays.
- When a user modifies a threshold value through the dashboard UI, the new value is persisted immediately and applied to the display from that point forward — including re-evaluating currently visible data.
- When the dashboard cannot reach its data source, it displays an explicit "data unavailable" state. It must not display previously fetched data with a current-looking timestamp.
- When two agents are detected sharing the same identifier, the dashboard renders that agent's card with a warning icon and the agent name displayed in red to indicate a potential identifier conflict.

### Error Flows

- When the collector's database is unavailable, the collector returns HTTP 503 to agents; agents buffer and retry.
- When the dashboard loses its connection to the collector API, it shows a clear disconnected state to the viewer.
- When an agent configuration file is missing or malformed on startup, the agent exits with a descriptive message; it does not silently start with defaults.

### Boundary Conditions

- When two agents report using the same identifier, the system records both payloads under that identifier — data is not silently dropped. The dashboard renders the affected agent card with a warning icon and the agent name in red.
- When a threshold is configured as zero or left unconfigured for a metric, no alerting occurs for that metric.
- When a metric value is exactly equal to a threshold (not just exceeding it), the behavior (flag or no flag) must be defined consistently (flag at ≥ threshold).

---

## Explicit Non-Behaviors

- The system must not take automated remediation actions (restart services, kill processes, execute scripts) because this is a read-only observability tool and automated actions introduce serious operational risk.
- The system must not expose controls to manage or restart services on monitored machines because remote execution is explicitly out of scope.
- The system must not aggregate or display log file contents because this is a metrics-only platform.
- The system must not send email, SMS, webhook, or push notifications because alerting is delivered exclusively through dashboard visual indicators.
- The system must not require pre-registration of an agent in the dashboard before it can report data, because the intended setup flow is: copy binary + config → run.

---

## Integration Boundaries

### Agent → Collector (HTTP REST, write)

- **Direction**: Agent POSTs metric payloads to Collector
- **Payload (JSON)**:
  - `agent_id` (string): unique agent identifier
  - `timestamp` (ISO 8601 UTC): time of collection
  - `cpu_percent` (float)
  - `memory`: `{ used_bytes, total_bytes, percent }`
  - `disks`: array of `{ mount_point, used_bytes, total_bytes, percent }`
  - `network`: `{ bytes_in, bytes_out }` (delta since last sample)
  - `uptime_seconds` (integer)
- **Expected responses**: 200 (success), 400 (invalid payload), 503 (unavailable)
- **Agent behavior on failure**: buffer in memory up to configured duration (default: 5 minutes), retry each interval, drop oldest on overflow
- **During development**: use a local stub HTTP server that logs payloads to stdout

### Dashboard → Collector (HTTP REST + WebSocket, read)

- **Direction**: Dashboard is a React SPA. It calls the collector's read API directly — there is no separate dashboard backend server.
- **Initial load (HTTP REST)**: On page load, the dashboard fetches the full agent list, their latest metric snapshots, threshold configurations, and any duplicate-ID flags via REST endpoints.
- **Live updates (WebSocket)**: After initial load, the dashboard opens a persistent WebSocket connection to the collector. The collector pushes metric update events to all connected clients whenever a new payload is persisted. The dashboard updates in place on receipt.
- **Expected REST endpoints** (behavior-level):
  - List all agents with latest status and last-seen timestamp
  - Current metric snapshot for a given agent
  - Historical metric data for a given agent with time range filter (1h / 6h / 24h / 7d)
  - CRUD operations for threshold configurations
- **Unavailability behavior**: Dashboard renders an explicit disconnected/error state on WebSocket drop or REST failure; never shows stale data as current. Auto-reconnects the WebSocket.
- **During development**: use fixture JSON representing realistic agent populations (10+ agents, mixed statuses)

### Collector → Database

- **Direction**: Collector reads and writes
- **Data stored**: agent records, time-series metric readings, threshold configurations per agent or global, agent metadata (first seen, last seen)
- **Unavailability behavior**: Collector returns HTTP 503 to agents; it does not silently discard data
- **During development**: use the same database technology as production (no in-memory substitutes for integration work)

---

## Behavioral Scenarios

*These scenarios are for external evaluation only. They must not be provided to the implementing agent during development.*

---

**Scenario 1 — Happy Path: Agent self-registers and appears in dashboard**

Setup: Collector and dashboard are running. No agents yet registered.

Actions:

1. Install agent on a machine with a valid config file pointing at the collector.
2. Start the agent.
3. Wait for two reporting intervals.
4. Open the dashboard.

Expected: The agent appears in the dashboard agent list with status "online." CPU, memory, disk, and network values are present and plausible for the machine's actual state at the time of the most recent report. Uptime is non-zero. No warning or critical indicators appear (assuming normal system load).

---

**Scenario 2 — Happy Path: Threshold breach is detected and displayed**

Setup: One agent running and reporting. Dashboard open. CPU warning threshold set to 70%, critical to 90%.

Actions:

1. Apply sustained high CPU load on the monitored machine.
2. Wait for one full reporting interval plus one dashboard refresh interval.
3. Observe the dashboard.

Expected: The CPU metric for that agent displays a warning or critical indicator. The indicator persists while load continues. When load is removed and the next report arrives below threshold, the indicator clears automatically.

---

**Scenario 3 — Happy Path: Ten agents report simultaneously without data loss**

Setup: 10 agents configured and running on 10 separate machines, all pointing at the same collector.

Actions:

1. Let all agents run for 5 minutes.
2. Open the dashboard and inspect each agent's metrics.

Expected: All 10 agents appear in the dashboard. Each agent's disk mount points are consistent with their respective machine. No agent's data contains values that clearly belong to a different machine. Every agent has a last-seen timestamp within one reporting interval of the current time.

---

**Scenario 4 — Error: Offline detection fires after threshold elapses**

Setup: One agent reporting every 30 seconds. Offline threshold configured to 2 minutes. Dashboard open.

Actions:

1. Kill the agent process abruptly.
2. Wait 2 minutes and 30 seconds.
3. Observe the dashboard.

Expected: The agent transitions to "offline" status with a prominent visual indicator after the threshold elapses. Last known metric values may remain visible but must be clearly labeled as stale/historical. When the agent is restarted, it transitions back to "online" within one reporting interval plus one dashboard refresh interval.

---

**Scenario 5 — Error: Collector outage does not crash agent; gap appears correctly**

Setup: One agent reporting every 15 seconds. Collector running normally.

Actions:

1. Stop the collector.
2. Wait 4 minutes.
3. Restart the collector.
4. Wait for two agent reporting intervals.

Expected: The agent does not crash during the outage. When the collector restarts, the agent resumes reporting. The dashboard shows a historical gap covering the outage window. Buffered data within the configured buffer window may appear; data beyond the buffer limit is legitimately absent. No fabricated or out-of-order data appears.

---

**Scenario 6 — Edge Case: Dashboard shows explicit error when data is unavailable — not stale data**

Setup: Dashboard running and displaying live data from agents.

Actions:

1. Sever the network connection between dashboard server and collector server.
2. Wait for two dashboard refresh intervals.
3. Observe the dashboard.

Expected: The dashboard displays an explicit "data unavailable" or "disconnected" state. It does not show the previously fetched metric values with a current-looking timestamp. A non-technical viewer would not mistake this state for healthy data.

---

**Scenario 7 — Edge Case: Threshold configuration survives full service restart**

Setup: Dashboard and collector running. CPU critical threshold configured via the dashboard UI to 85%.

Actions:

1. Restart the collector service.
2. Restart the dashboard service.
3. Trigger a CPU spike above 85% on a monitored machine.
4. Wait for one reporting interval plus one dashboard refresh interval.

Expected: The dashboard flags the CPU spike as critical, using the 85% threshold that was set before the restart. The threshold was not silently reset to a default or zero value on restart.

---

## Ambiguity Log

All ambiguities from initial spec review have been resolved. Decisions are documented here for traceability.

1. **Dashboard backend architecture** — *Was ambiguous: SPA vs. full-stack framework.* **Decision: React SPA, no separate dashboard backend. The SPA calls the collector's REST and WebSocket endpoints directly.**
2. **Database technology** — *Was ambiguous: PostgreSQL vs. SQLite.* **Decision: SQLite. Embedded, file-based, no separate database server required.**
3. **Dashboard authentication** — *Was ambiguous: open access vs. any form of auth.* **Decision: No authentication. The dashboard is open to anyone who can reach the server; assumed to be on a trusted internal network.**
4. **Agent unique identifier source** — *Was ambiguous: hostname, config file, or UUID.* **Decision: Configurable in TOML (`agent_id` field). If absent, the agent falls back to the machine's hostname at startup. Operators on cloned VMs must set `agent_id` explicitly in the config to avoid collisions.**
5. **Default values for configurable settings** — *Was ambiguous: no defaults stated.* **Decision: Suggested defaults accepted.**
  - Reporting interval: **30 seconds**
  - Offline detection threshold: **2 minutes**
  - Data retention period: **30 days**
  - Agent-side buffer duration (collector unavailable): **5 minutes**
  - Dashboard WebSocket reconnect: **automatic with exponential backoff**
6. **Real-time update mechanism** — *Was ambiguous: polling, SSE, or WebSocket.* **Decision: WebSocket push. The collector pushes metric update events to all connected dashboard clients immediately on persistence. The dashboard does not poll.**
7. **Historical data time ranges** — *Was ambiguous: no time range options defined.* **Decision: Suggested ranges accepted: 1h, 6h, 24h, 7d.**
8. **Duplicate agent ID dashboard behavior** — *Was ambiguous: banner, card indicator, or conflict view.* **Decision: The affected agent card displays a warning icon and the agent name is rendered in red. No separate conflict view or banner.**

---

## Implementation Constraints

- **Agent**: Rust. Deployed by copying a single binary and a TOML config file. No runtime dependencies. Must be runnable as a background service (systemd-compatible on Linux; service-compatible on Windows). Default configurable values: reporting interval 30s, buffer duration 5 minutes.
- **Collector**: Rust. Exposes an HTTP REST API and a WebSocket endpoint for live push to dashboard clients. Database: SQLite (embedded, file-based, no separate server required). Configuration via external TOML file. Runnable as a background service. Default configurable values: offline detection threshold 2 minutes, data retention 30 days.
- **Dashboard**: React SPA (no separate backend server). Communicates with the collector via REST for initial load and WebSocket for live updates. No authentication required. Historical time range options: 1h, 6h, 24h, 7d. Default dashboard WebSocket reconnect behavior: automatic, with exponential backoff.
- **Agent identifier**: Configured in TOML (`agent_id`). If the field is absent from the config, the agent defaults to the machine's hostname at startup.
- **Config files**: External to binaries, human-readable TOML, never overwritten by a binary update.
- **All three components**: Must be independently deployable on separate servers. No shared filesystem assumptions.
- Implementation details beyond the above (data structures, algorithms, internal architecture, HTTP library choices) are intentionally left to the implementing agent.

