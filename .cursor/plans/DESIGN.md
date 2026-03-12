# RustNexus — Dashboard UI Design

## Design Principles

- **Clarity first.** Non-technical stakeholders must instantly read system health without training.
- **Status is never ambiguous.** Every agent card communicates one unambiguous state: online, warning, critical, or offline.
- **Stale data must not look current.** If the connection drops, metric values are hidden — not frozen in place.
- **Density with breathing room.** Show 21–100 agents in a scannable grid without feeling cramped.
- **Inline action, minimal chrome.** Threshold editing lives directly on the detail view; no modal dialogs.

---

## Visual Language

### Status Colors

| State      | Color        | Hex       | Border / Accent use       |
|------------|--------------|-----------|---------------------------|
| Online     | Green        | `#22c55e` | Card left border          |
| Warning    | Amber        | `#f59e0b` | Card left border + badges |
| Critical   | Red          | `#ef4444` | Card left border + badges |
| Offline    | Neutral gray | `#6b7280` | Card left border          |
| No data    | Slate        | `#94a3b8` | Placeholder text          |
| Disconnect | Red/banner   | `#ef4444` | Top-of-page banner        |

Status colors apply consistently to: card borders, status badges, chart threshold lines, and inline metric indicators.

### Typography

| Role                | Size       | Weight   |
|---------------------|------------|----------|
| App title           | `text-xl`  | Semibold |
| Section heading     | `text-sm`  | Uppercase + tracking |
| Card agent name     | `text-sm`  | Semibold |
| Metric value        | `text-lg`  | Bold     |
| Metric label        | `text-xs`  | Normal, muted |
| Threshold input     | `text-sm`  | Monospace |
| Timestamp / uptime  | `text-xs`  | Muted    |

Use a single sans-serif system font stack (`ui-sans-serif, system-ui`). Threshold inputs use a monospace face for alignment.

### Iconography

Keep icons minimal and consistent. Use a single icon library (e.g. Lucide).

| Meaning              | Icon         |
|----------------------|--------------|
| Online               | Filled circle (green) |
| Warning              | Triangle exclamation  |
| Critical             | X circle              |
| Offline              | Dashed circle / dash  |
| Disconnected (WS)    | Wifi-off              |
| Duplicate agent ID   | Triangle exclamation (red, on card) |
| Back navigation      | Arrow left            |

---

## Layout System

The dashboard is a single-page React SPA with two top-level views: **Agent Grid** and **Agent Detail**. No routing library is required — a single piece of state controls which view is active.

```
┌─────────────────── App Shell ───────────────────┐
│  Header: App name + connection status badge      │
├──────────────────────────────────────────────────┤
│  Content area (one of two views at a time)       │
│    • Agent Grid View                             │
│    • Agent Detail View                           │
└──────────────────────────────────────────────────┘
```

The header is always visible. It is the only persistent UI element.

---

## Views

### Agent Grid View

```
┌──────────────────────────────────────────────────────────────────────┐
│  RustNexus                                        ● Connected        │
├──────────────────────────────────────────────────────────────────────┤
│  Agents (21)    Filter: [All ▼]    [Search agents...]               │
├─────────────────┬─────────────────┬─────────────────┬───────────────┤
│  ● server-01    │  ● server-02    │  ⚠ server-03   │  — server-04  │
│  CPU:  42%      │  CPU:  12%      │  CPU:  78% ⚠   │               │
│  MEM:  61%      │  MEM:  34%      │  MEM:  90% ✖   │   OFFLINE     │
│  DISK: 44%      │  DISK: 21%      │  DISK: 55%      │  Last: 5m ago │
│  Up: 1d 5h      │  Up: 3d 2h      │  Up: 12h        │               │
│  12s ago        │  8s ago         │  3s ago         │               │
├─────────────────┼─────────────────┼─────────────────┼───────────────┤
│  ● server-05    │  ...            │                 │               │
└─────────────────┴─────────────────┴─────────────────┴───────────────┘

Legend:  ●  online    ⚠  warning    ✖  critical    —  offline
```

**Grid layout:** Responsive CSS grid. Target card width ~200px; grid fills the viewport width (`grid-cols-2` → `grid-cols-4` → `grid-cols-5` at common breakpoints).

**Agent card anatomy:**
- Left border colored by status (4px, full card height)
- Top row: status icon + agent name (bold)
- Metric rows: label, value, inline status icon if threshold breached
- Bottom row: uptime + last-seen timestamp (muted, smallest text)
- Entire card is clickable → opens Detail View

**Filter / search bar:** Sits above the grid. Filter dropdown options: All, Online, Warning, Critical, Offline. Search filters by agent name (client-side, instant).

**Offline cards** display "OFFLINE" in large muted text with the last-seen timestamp. No metric values shown.

---

### Agent Detail View

```
┌──────────────────────────────────────────────────────────────────────┐
│  ← All Agents    server-03    ⚠ Warning    Last seen: 3s ago        │
├──────────────────────────────────────────────────────────────────────┤
│  Time Range:  [1h]  [6h]  [24h]  [7d]                              │
├──────────────────────────┬───────────────────────────────────────────┤
│  CPU Usage               │  Memory Usage                            │
│  Current: 78%  ⚠         │  Current: 90%  ✖                        │
│  Warn: [70]%  Crit: [90]%│  Warn: [80]%  Crit: [90]%              │
│  ┌────────────────────┐  │  ┌────────────────────────────────────┐  │
│  │  (line chart)      │  │  │  (line chart)                      │  │
│  └────────────────────┘  │  └────────────────────────────────────┘  │
├──────────────────────────┼───────────────────────────────────────────┤
│  Disk Usage              │  Network Throughput                      │
│  /        55%  ██████░   │  In:  100 KB/s                          │
│  /data    80%  ████████⚠ │  Out:  50 KB/s                          │
│  Warn: [75]%  Crit:[90]% │  ┌────────────────────────────────────┐  │
│  ┌────────────────────┐  │  │  (dual-line chart: in + out)       │  │
│  │  (bar per mount)   │  │  └────────────────────────────────────┘  │
│  └────────────────────┘  │                                          │
├──────────────────────────┴───────────────────────────────────────────┤
│  Uptime: 12h 31m                                                    │
└──────────────────────────────────────────────────────────────────────┘
```

**Layout:** Two-column panel grid (stacks to one column on narrow viewports). Each panel holds one metric group.

**Time range selector:** Pill-style toggle buttons (`1h · 6h · 24h · 7d`). Active selection is filled; others are outlined. Selecting a range refetches history from the collector and re-renders all charts.

**Chart library:** Recharts. All four charts share the same time axis range so they are visually aligned.

| Metric   | Chart type       | Notes                                       |
|----------|------------------|---------------------------------------------|
| CPU      | Line chart       | 0–100% Y axis; horizontal dashed lines at warn/critical thresholds |
| Memory   | Line chart       | Same as CPU                                 |
| Disk     | Horizontal bar per mount point + line chart for trend | Bars show current percent; chart shows history for the primary mount |
| Network  | Dual-line chart  | Two series: bytes_in and bytes_out; Y axis auto-scaled with KB/MB/GB formatting |

**Threshold inputs:** Inline `<input type="number">` fields next to each metric. On blur (or Enter), the value is immediately PUT/POSTed to the collector. Visual feedback: input briefly flashes green on success, red on failure. No separate "Save" button.

**Threshold lines on charts:** Horizontal dashed lines drawn at the current warn (amber) and critical (red) values. Lines reposition immediately when threshold inputs change.

---

### Disconnected State

Shown when the WebSocket connection is lost. Overlays the Agent Grid view content.

```
┌──────────────────────────────────────────────────────────────────────┐
│  RustNexus                         ✖  DISCONNECTED — reconnecting... │
├──────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  ⚠  Data unavailable. Connection to collector lost.          │   │
│  │     Metric values are hidden until connection is restored.   │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  server-01   │  │  server-02   │  │  server-03   │              │
│  │  (no data)   │  │  (no data)   │  │  (no data)   │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└──────────────────────────────────────────────────────────────────────┘
```

- Header badge changes to `✖ DISCONNECTED — reconnecting...` (red background).
- A full-width amber/red banner appears below the header with plain-language message.
- Agent cards render with their names intact but all metric values replaced with `—` or `(no data)`.
- Cards are not clickable while disconnected.
- When the connection restores, the banner disappears, the header badge returns to `● Connected`, and metric values repopulate from the next WebSocket event.

---

### Duplicate Agent ID State

No modal or banner. The conflict is surfaced on the affected agent card only.

```
│  ⚠ server-01     ← agent name in red, warning icon prefix
│  CPU:  42%
│  ...
```

- Agent name text color: `text-red-500`
- Warning icon prefix on the agent name
- Card border color: amber (warning), regardless of actual metric status, to keep the duplicate visible
- Tooltip on hover (or accessible `title` attribute): `"Duplicate agent ID detected"`

---

## Component Inventory

| Component         | Responsibility                                                  |
|-------------------|-----------------------------------------------------------------|
| `App.tsx`         | Top-level view state (grid vs. detail), WebSocket lifecycle     |
| `Header.tsx`      | App title + connection status badge                             |
| `AgentGrid.tsx`   | Search/filter bar + responsive card grid                        |
| `AgentCard.tsx`   | Single agent card; all status/duplicate/offline visual states   |
| `AgentDetail.tsx` | Detail view shell; time range selector; panel layout            |
| `MetricPanel.tsx` | One metric group (chart + current value + threshold inputs)     |
| `MetricChart.tsx` | Recharts wrapper; accepts data + thresholds; renders lines      |
| `StatusBadge.tsx` | Reusable pill badge for online/warning/critical/offline states  |
| `ThresholdInput.tsx` | Controlled number input; optimistic save on blur/Enter       |
| `DisconnectedBanner.tsx` | Full-width banner; only renders when WS is down        |

---

## Interaction Patterns

**Live update:** When a `metric_update` WebSocket event arrives, the matching agent card in the grid updates in place (no full re-render of the grid). If the Detail View is open for that agent, all panels update immediately.

**Navigation:** Clicking an agent card sets `selectedAgentId` in top-level state. The "← All Agents" back link clears it. Browser history is not involved (no URL changes needed for this tool).

**Threshold save feedback:**
1. User changes input value.
2. On blur or Enter: input enters a `saving` state (slightly dimmed).
3. On API success: brief green flash, input returns to normal.
4. On API failure: red flash, value reverts to the last confirmed value, a small inline error message appears.

**Stale data policy:** The dashboard never displays metric values with a "live-looking" timestamp unless those values arrived over the active WebSocket connection during the current session. On reconnect, the initial HTTP load re-fetches fresh snapshots before showing values.

---

## Styling Notes

- **Framework:** Tailwind CSS utility classes throughout. No custom CSS files except for any chart-specific overrides.
- **Dark mode:** Not in scope for v1. The palette above assumes a light background (`bg-white` / `bg-gray-50`).
- **Card shadow:** Subtle (`shadow-sm`) to create depth without visual noise.
- **Spacing:** Use Tailwind spacing scale consistently (`p-4` for card padding, `gap-4` for grid gaps).
- **Transitions:** Status color changes animate with `transition-colors duration-300`. Chart data transitions are handled by Recharts' built-in animation.
