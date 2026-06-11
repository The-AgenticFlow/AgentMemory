import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

const emptyMemory = { episodes: [], patterns: [], engrams: [], schemas: [] };
const tabs = ["overview", "graph", "buffers", "engrams", "schemas", "thalamus", "tuning"];
const graphKindOrder = ["session", "working_context", "episode", "pattern", "engram", "schema"];
const graphCanvas = { width: 1000, height: 700 };

function App() {
  const [overview, setOverview] = useState(null);
  const [graph, setGraph] = useState({ nodes: [], edges: [] });
  const [memory, setMemory] = useState(emptyMemory);
  const [config, setConfig] = useState(null);
  const [selectedKind, setSelectedKind] = useState("all");
  const [activeTab, setActiveTab] = useState("overview");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [status, setStatus] = useState("Loading");
  const [sim, setSim] = useState(null);

  async function refresh() {
    const health = await safeApi("/health");
    const [overviewData, graphData, episodes, patterns, engrams, schemas, configData] = await Promise.all([
      safeApi("/control/overview"),
      safeApi("/control/graph"),
      safeApi("/memory/episodes", []),
      safeApi("/memory/patterns", []),
      safeApi("/memory/engrams", []),
      safeApi("/memory/schemas", []),
      safeApi("/control/config")
    ]);
    setOverview(overviewData);
    setGraph(graphData || { nodes: [], edges: [] });
    setMemory({ episodes, patterns, engrams, schemas });
    setConfig(configData);
    setStatus(health ? `Live · ${health.sessions} sessions` : "Backend unavailable");
  }

  useEffect(() => {
    refresh().catch((error) => setStatus(error.message));
    const timer = window.setInterval(() => refresh().catch((error) => setStatus(error.message)), 15000);
    return () => window.clearInterval(timer);
  }, []);

  async function saveConfig(nextConfig) {
    const saved = await api("/control/config", { method: "PUT", body: nextConfig });
    setConfig(saved);
    await refresh();
  }

  async function resetConfig() {
    const saved = await api("/control/config/reset", { method: "POST" });
    setConfig(saved);
    await refresh();
  }

  async function runThalamusPreview(event) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const preview = await api("/control/simulate/thalamus", {
      method: "POST",
      body: {
        action: form.get("action"),
        context: form.get("context"),
        outcome: form.get("outcome"),
        expectation: form.get("expectation"),
        mode: form.get("mode"),
        task_context: form.get("task_context") || form.get("context")
      }
    });
    setSim(preview);
  }

  async function consolidate() {
    setStatus("Consolidating");
    await safeApi("/consolidate", null, { method: "POST", body: { debug: true } });
    await refresh();
  }

  const counts = overview?.counts || {};

  return (
    <main className={sidebarCollapsed ? "app-shell sidebar-collapsed" : "app-shell"}>
      <aside className="rail">
        <div className="brand">
          <span className="brand-mark">AM</span>
          <div>
            <h1>Agent Memory</h1>
            <p>Control panel</p>
          </div>
          <button
            className="rail-toggle"
            type="button"
            onClick={() => setSidebarCollapsed((value) => !value)}
            aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {sidebarCollapsed ? "›" : "‹"}
          </button>
        </div>

        <nav className="rail-nav" aria-label="Sections">
          {tabs.map((tab) => (
            <button
              key={tab}
              className={activeTab === tab ? "rail-tab active" : "rail-tab"}
              onClick={() => setActiveTab(tab)}
              type="button"
              title={labelForTab(tab)}
            >
              <span className="rail-tab-label">{labelForTab(tab)}</span>
            </button>
          ))}
        </nav>

        <section className="rail-status">
          <div>
            <span>Sessions</span>
            <strong>{counts.sessions ?? "—"}</strong>
          </div>
          <div>
            <span>Graph</span>
            <strong>{graph.nodes.length}</strong>
          </div>
          <div>
            <span>Neo4j</span>
            <strong>{overview?.neo4j?.configured ? "connected" : "local"}</strong>
          </div>
        </section>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div className="hero-copy">
            <p className="eyebrow">Memory operations deck</p>
            <h2>Structured memory control panel</h2>
            <p className="hero-note">
              A control-room surface for memory state, live graph inspection, and tuning the intake pipeline.
            </p>
          </div>
          <div className="hero-summary">
            <div className="summary-card">
              <span>Live pulse</span>
              <strong>{overview?.neo4j?.configured ? "Neo4j connected" : "Local fallback"}</strong>
              <small>{status}</small>
            </div>
            <div className="summary-card">
              <span>Memory counts</span>
              <strong>{counts.sessions ?? "—"} / {counts.episodes ?? "—"}</strong>
              <small>sessions and episodes tracked</small>
            </div>
            <div className="summary-card accent">
              <span>Graph nodes</span>
              <strong>{graph.nodes.length}</strong>
              <small>{graph.edges.length} linked edges</small>
            </div>
          </div>
        </header>

        <section className="workspace-body">
          {activeTab === "overview" && (
            <OverviewTab overview={overview} counts={counts} />
          )}
          {activeTab === "graph" && (
            <GraphTab graph={graph} selectedKind={selectedKind} setSelectedKind={setSelectedKind} />
          )}
          {activeTab === "buffers" && (
            <MemoryTab
              title="Buffers"
              subtitle="Pre-engram patterns with short, precise fields."
              rows={memory.patterns}
              columns={["pattern_hash", "strength", "occurrences", "threshold", "decay_rate"]}
            />
          )}
          {activeTab === "engrams" && (
            <MemoryTab
              title="Engrams"
              subtitle="Long-term memory indices."
              rows={memory.engrams}
              columns={["id", "strength", "status", "access_count", "tags"]}
            />
          )}
          {activeTab === "schemas" && (
            <MemoryTab
              title="Schemas"
              subtitle="Compressed patterns and predictions."
              rows={memory.schemas}
              columns={["id", "strength", "prediction_fields", "source_engram_ids"]}
            />
          )}
          {activeTab === "thalamus" && (
            <ThalamusTab sim={sim} onSubmit={runThalamusPreview} onDeepSleep={consolidate} />
          )}
          {activeTab === "tuning" && config && (
            <TuningTab config={config} onSave={saveConfig} onReset={resetConfig} />
          )}
        </section>
      </section>
    </main>
  );
}

function OverviewTab({ overview, counts }) {
  const activeConfig = overview?.active_config || {};
  const thalamus = activeConfig.thalamus || {};
  const buffer = activeConfig.buffer || {};
  const retrieval = activeConfig.retrieval || {};
  return (
    <div className="dashboard-grid overview-grid">
      <section className="panel subtle span-2">
        <PanelTitle title="Snapshot" subtitle="Current memory load and operational posture." />
        <div className="metric-grid">
          <Metric label="Sessions" value={counts.sessions} />
          <Metric label="Episodes" value={counts.episodes} />
          <Metric label="Buffers" value={counts.patterns} />
          <Metric label="Engrams" value={counts.engrams} />
          <Metric label="Schemas" value={counts.schemas} />
          <Metric label="MCP" value={overview?.mcp?.http_enabled ? "HTTP on" : "off"} />
        </div>
      </section>

      <section className="panel subtle">
        <PanelTitle title="Connection state" subtitle="Backend and graph backplane." />
        <div className="status-stack">
          <StatusCard
            title="Neo4j"
            body={overview?.neo4j?.configured ? "Connected and serving graph data." : "Not configured, using local store."}
            accent={overview?.neo4j?.configured}
          />
          <StatusCard
            title="MCP"
            body={overview?.mcp?.http_enabled ? `HTTP ${overview?.mcp?.endpoint || "/mcp"}` : "HTTP disabled"}
            accent={overview?.mcp?.http_enabled}
          />
        </div>
      </section>

      <section className="panel subtle">
        <PanelTitle title="Tuning snapshot" subtitle="The active intake profile at a glance." />
        <div className="detail-grid">
          <DetailItem label="Novelty" value={formatNumber(thalamus.novelty_weight)} />
          <DetailItem label="Surprise" value={formatNumber(thalamus.surprise_weight)} />
          <DetailItem label="Relevance" value={formatNumber(thalamus.task_relevance_weight)} />
          <DetailItem label="Valence" value={formatNumber(thalamus.valence_weight)} />
          <DetailItem label="Buffer threshold" value={formatNumber(buffer.similarity_threshold)} />
          <DetailItem label="Top K" value={retrieval.top_k ?? "—"} />
        </div>
      </section>

      <section className="panel subtle">
        <PanelTitle title="Latest scores" subtitle="Recent thalamus results." />
        {(overview?.latest_scores || []).length === 0 ? (
          <div className="empty-state">No score history yet.</div>
        ) : (
          <div className="score-list compact">
            {(overview?.latest_scores || []).slice(0, 6).map((record) => (
              <div className="score-row compact" key={record.id}>
                <span className={record.accepted ? "badge ok" : "badge reject"}>{record.accepted ? "accepted" : "rejected"}</span>
                <strong>{record.score.toFixed(3)}</strong>
                <small>{record.pattern_hash || "no pattern"}</small>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function GraphTab({ graph, selectedKind, setSelectedKind }) {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Memory graph" subtitle="Filtered graph projection." />
        <div className="filter-row">
          {[
            ["all", "all"],
            ["session", "session"],
            ["working_context", "context"],
            ["episode", "episode"],
            ["pattern", "pattern"],
            ["engram", "engram"],
            ["schema", "schema"]
          ].map(([kind, label]) => (
            <button
              key={kind}
              className={selectedKind === kind ? "chip active" : "chip"}
              onClick={() => setSelectedKind(kind)}
            >
              {label}
            </button>
          ))}
        </div>
        <MemoryGraph graph={graph} selectedKind={selectedKind} />
      </section>
    </div>
  );
}

function TuningTab({ config, onSave, onReset }) {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Behavior tuning" subtitle="Editable core knobs, grouped and kept short." />
        <ConfigEditor config={config} onSave={onSave} onReset={onReset} />
      </section>
    </div>
  );
}

function ThalamusTab({ sim, onSubmit, onDeepSleep }) {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Thalamus Lab" subtitle="Preview intake scoring without storing an episode." />
        <div className="thalamus-actions">
          <button className="secondary slim" type="button" onClick={onDeepSleep}>Deep Sleep</button>
        </div>
        <form className="lab-form thalamus-lab" onSubmit={onSubmit}>
          <input name="action" placeholder="Action" required />
          <input name="context" placeholder="Context" required />
          <input name="outcome" placeholder="Outcome" required />
          <input name="expectation" placeholder="Expectation" defaultValue="the task should progress" />
          <select name="mode" defaultValue="Exploration">
            <option>Exploration</option>
            <option>Routine</option>
            <option>Critical</option>
          </select>
          <button className="primary" type="submit">Simulate</button>
        </form>
        {sim && <ScoreCard sim={sim} />}
      </section>
    </div>
  );
}

function MemoryTab({ title, subtitle, rows, columns }) {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title={title} subtitle={subtitle} />
        <div className="table-scroll">
          <table>
            <thead><tr>{columns.map((column) => <th key={column}>{column}</th>)}</tr></thead>
            <tbody>
              {rows.slice(0, 20).map((row, index) => (
                <tr key={row.id || row.pattern_hash || index}>
                  {columns.map((column) => <td key={column}>{formatCell(row[column])}</td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

function Metric({ label, value }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value ?? "—"}</strong>
    </div>
  );
}

function DetailItem({ label, value }) {
  return (
    <div className="detail-item">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function StatusCard({ title, body, accent = false }) {
  return (
    <div className={accent ? "status-card accent" : "status-card"}>
      <span className="pulse" />
      <strong>{title}</strong>
      <small>{body}</small>
    </div>
  );
}

function QuickMetrics({ counts, mcp }) {
  return (
    <div className="quick-metrics">
      <div><span>Sessions</span><strong>{counts.sessions ?? "—"}</strong></div>
      <div><span>Episodes</span><strong>{counts.episodes ?? "—"}</strong></div>
      <div><span>Neo4j</span><strong>{mcp?.http_enabled ? "on" : "off"}</strong></div>
    </div>
  );
}

function PanelTitle({ title, subtitle }) {
  return (
    <div className="panel-title">
      <div>
        <h3>{title}</h3>
        <p>{subtitle}</p>
      </div>
    </div>
  );
}

function MemoryGraph({ graph, selectedKind }) {
  const [selectedNodeId, setSelectedNodeId] = useState(null);
  const [expanded, setExpanded] = useState(false);
  const [positions, setPositions] = useState({});
  const [dragging, setDragging] = useState(null);
  const boardRef = useRef(null);

  const layout = useMemo(() => {
    const filtered = selectedKind === "all" ? graph.nodes : graph.nodes.filter((node) => node.kind === selectedKind);
    const ids = new Set(filtered.map((node) => node.id));
    const edges = graph.edges.filter((edge) => ids.has(edge.source) && ids.has(edge.target));
    const radius = Math.min(280, Math.max(180, 110 + filtered.length * 4));
    const center = { x: graphCanvas.width / 2, y: graphCanvas.height / 2 };

    const nodes = filtered.map((node, index) => {
      const angle = filtered.length === 1 ? -Math.PI / 2 : (index / filtered.length) * Math.PI * 2 - Math.PI / 2;
      return {
        ...node,
        x: positions[node.id]?.x ?? center.x + Math.cos(angle) * radius,
        y: positions[node.id]?.y ?? center.y + Math.sin(angle) * radius
      };
    });

    return { nodes, edges, filtered };
  }, [graph, positions, selectedKind]);

  useEffect(() => {
    setPositions((current) => {
      let changed = false;
      const next = { ...current };
      layout.filtered.forEach((node) => {
        if (!next[node.id]) {
          next[node.id] = { x: node.x, y: node.y };
          changed = true;
        }
      });
      return changed ? next : current;
    });
  }, [layout.filtered]);

  useEffect(() => {
    if (selectedNodeId && !layout.nodes.some((node) => node.id === selectedNodeId)) {
      setSelectedNodeId(layout.nodes[0]?.id || null);
    }
    if (!selectedNodeId && layout.nodes[0]) {
      setSelectedNodeId(layout.nodes[0].id);
    }
  }, [layout.nodes, selectedNodeId]);

  useEffect(() => {
    if (!dragging) return;
    function move(event) {
      const rect = boardRef.current?.getBoundingClientRect();
      if (!rect) return;
      const point = pointFromEvent(event, rect);
      setPositions((current) => ({
        ...current,
        [dragging.id]: {
          x: clamp(point.x - dragging.offsetX, 72, graphCanvas.width - 72),
          y: clamp(point.y - dragging.offsetY, 44, graphCanvas.height - 44)
        }
      }));
    }
    function end() {
      setDragging(null);
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
    };
  }, [dragging]);

  const byId = useMemo(() => new Map(layout.nodes.map((node) => [node.id, node])), [layout.nodes]);
  const selectedNode = selectedNodeId ? byId.get(selectedNodeId) || null : null;
  const selectedConnections = useMemo(() => {
    if (!selectedNode) return [];
    return layout.edges
      .filter((edge) => edge.source === selectedNode.id || edge.target === selectedNode.id)
      .map((edge) => {
        const outgoing = edge.source === selectedNode.id;
        const other = outgoing ? byId.get(edge.target) : byId.get(edge.source);
        return { ...edge, outgoing, other };
      });
  }, [byId, layout.edges, selectedNode]);

  const graphBody = (
    <div className="graph-shell">
      <div className="graph-toolbar">
        <div>
          <strong>Graph console</strong>
          <p>{layout.nodes.length} visible nodes, {layout.edges.length} visible edges.</p>
        </div>
        <div className="graph-actions">
          <button className="secondary slim" type="button" onClick={() => setPositions({})}>Reset layout</button>
          <button className="secondary slim" type="button" onClick={() => setExpanded((value) => !value)}>
            {expanded ? "Collapse" : "Expand"}
          </button>
        </div>
      </div>

      <div className="graph-body">
        <div className="graph-board" ref={boardRef}>
          <svg className="graph-links" viewBox={`0 0 ${graphCanvas.width} ${graphCanvas.height}`} aria-hidden="true">
            {layout.edges.map((edge) => {
              const source = byId.get(edge.source);
              const target = byId.get(edge.target);
              if (!source || !target) return null;
              return (
                <g key={edge.id} className="graph-edge-group">
                  <line x1={source.x} y1={source.y} x2={target.x} y2={target.y} className="graph-edge" />
                  <text x={(source.x + target.x) / 2} y={(source.y + target.y) / 2} className="graph-edge-label">
                    {edge.label}
                  </text>
                </g>
              );
            })}
          </svg>

          {layout.nodes.map((node) => {
            const isActive = node.id === selectedNodeId;
            return (
              <button
                key={node.id}
                type="button"
                className={isActive ? `graph-node ${node.kind} active` : `graph-node ${node.kind}`}
                style={{
                  left: `${(node.x / graphCanvas.width) * 100}%`,
                  top: `${(node.y / graphCanvas.height) * 100}%`
                }}
                onPointerDown={(event) => {
                  event.preventDefault();
                  const rect = boardRef.current?.getBoundingClientRect();
                  if (!rect) return;
                  const point = pointFromEvent(event, rect);
                  setSelectedNodeId(node.id);
                  setDragging({
                    id: node.id,
                    offsetX: point.x - (positions[node.id]?.x ?? node.x),
                    offsetY: point.y - (positions[node.id]?.y ?? node.y)
                  });
                }}
                onClick={() => setSelectedNodeId(node.id)}
                aria-label={`${node.kind} ${node.title || node.id}`}
              >
                <span className="graph-node-kind">{labelForKind(node.kind)}</span>
                <strong>{truncate(node.title || node.label, 40)}</strong>
                <small>{nodeDetailLine(node)}</small>
                <em>{shortId(node.id)}</em>
              </button>
            );
          })}
        </div>

        <aside className="graph-inspector">
          <div className="inspector-header">
            <span>Inspector</span>
            <strong>{selectedNode ? labelForKind(selectedNode.kind) : "No selection"}</strong>
          </div>
          {selectedNode ? (
            <div className="inspector-stack">
              <section className="inspector-card">
                <h4>{selectedNode.title || selectedNode.label}</h4>
                <p>{selectedNode.properties?.content || nodeSummary(selectedNode)}</p>
                <div className="mini-grid">
                  <DetailItem label="Node ID" value={shortId(selectedNode.id, 10)} />
                  <DetailItem label="Kind" value={labelForKind(selectedNode.kind)} />
                  <DetailItem label="Edges" value={selectedConnections.length} />
                  <DetailItem label="Label" value={selectedNode.label} />
                </div>
              </section>

              <section className="inspector-card">
                <h4>Key fields</h4>
                <div className="field-list">
                  {graphFields(selectedNode).map((field) => (
                    <div key={field.label} className="field-row">
                      <span>{field.label}</span>
                      <strong>{field.value}</strong>
                    </div>
                  ))}
                </div>
              </section>

              <section className="inspector-card">
                <h4>Connections</h4>
                {selectedConnections.length === 0 ? (
                  <div className="empty-state compact">No visible edges for this node.</div>
                ) : (
                  <div className="connection-list">
                    {selectedConnections.map((edge) => (
                      <button
                        key={edge.id}
                        className="connection-row"
                        type="button"
                        onClick={() => edge.other && setSelectedNodeId(edge.other.id)}
                      >
                        <span>{edge.outgoing ? "out" : "in"}</span>
                        <strong>{edge.label}</strong>
                        <small>{edge.other ? edge.other.title || edge.other.label : shortId(edge.outgoing ? edge.target : edge.source)}</small>
                      </button>
                    ))}
                  </div>
                )}
              </section>
            </div>
          ) : (
            <div className="empty-state">Select a node to inspect its properties and connections.</div>
          )}
        </aside>
      </div>
    </div>
  );

  if (!expanded) return graphBody;

  return (
    <div className="graph-overlay">
      <div className="graph-overlay-backdrop" onClick={() => setExpanded(false)} />
      <section className="panel subtle graph-overlay-panel">{graphBody}</section>
    </div>
  );
}

function ScoreCard({ sim }) {
  const bars = [
    ["novelty", sim.scores.novelty],
    ["surprise", sim.scores.surprise],
    ["relevance", sim.scores.task_relevance],
    ["valence", sim.scores.emotional_valence]
  ];
  return (
    <div className="score-card">
      <div className="score-head">
        <span className={sim.accepted ? "badge ok" : "badge reject"}>{sim.accepted ? "accepted" : "rejected"}</span>
        <strong>{sim.score.toFixed(3)} / {sim.threshold.toFixed(3)}</strong>
      </div>
      {bars.map(([name, value]) => (
        <div className="bar" key={name}><span>{name}</span><i style={{ width: `${value * 100}%` }} /></div>
      ))}
    </div>
  );
}

function ConfigEditor({ config, onSave, onReset }) {
  const [draft, setDraft] = useState(config);
  useEffect(() => setDraft(config), [config]);

  function setNumber(path, value) {
    const next = structuredClone(draft);
    let cursor = next;
    for (const key of path.slice(0, -1)) cursor = cursor[key];
    cursor[path.at(-1)] = path.at(-1) === "top_k" ? Number.parseInt(value, 10) : Number.parseFloat(value);
    setDraft(next);
  }

  const groups = [
    {
      title: "Thalamus",
      fields: [
        ["thalamus", "novelty_weight"], ["thalamus", "surprise_weight"], ["thalamus", "task_relevance_weight"], ["thalamus", "valence_weight"]
      ]
    },
    {
      title: "Buffer",
      fields: [
        ["buffer", "similarity_threshold"], ["buffer", "promotion_threshold"], ["buffer", "decay_rate"]
      ]
    },
    {
      title: "Retrieval",
      fields: [
        ["pattern", "completion_threshold"], ["retrieval", "top_k"]
      ]
    },
    {
      title: "Consolidation",
      fields: [
        ["consolidation", "active_threshold"], ["consolidation", "archive_threshold"], ["consolidation", "schema_threshold"], ["consolidation", "base_decay_rate"]
      ]
    }
  ];

  return (
    <div className="config-shell">
      {groups.map((group) => (
        <section className="config-group" key={group.title}>
          <h4>{group.title}</h4>
          <div className="config-grid">
            {group.fields.map((path) => (
              <label key={path.join(".")}>
                <span>{path.join(".")}</span>
                <input
                  type="number"
                  step={path.at(-1) === "top_k" ? "1" : "0.01"}
                  value={path.reduce((obj, key) => obj[key], draft)}
                  onChange={(event) => setNumber(path, event.target.value)}
                />
              </label>
            ))}
          </div>
        </section>
      ))}
      <div className="actions">
        <button className="primary slim" onClick={() => onSave(draft)}>Save</button>
        <button className="secondary slim" onClick={onReset}>Reset</button>
      </div>
    </div>
  );
}

async function api(path, options = {}) {
  const response = await fetch(path, {
    method: options.method || "GET",
    headers: options.body ? { "Content-Type": "application/json" } : undefined,
    body: options.body ? JSON.stringify(options.body) : undefined
  });
  if (!response.ok) throw new Error(`${path}: ${response.status}`);
  return response.json();
}

async function safeApi(path, fallback = null, options = {}) {
  try {
    return await api(path, options);
  } catch {
    return fallback;
  }
}

function labelForTab(tab) {
  return tab === "overview"
    ? "Overview"
    : tab === "graph"
      ? "Graph"
      : tab === "buffers"
        ? "Buffers"
      : tab === "engrams"
          ? "Engrams"
          : tab === "schemas"
            ? "Schemas"
            : tab === "thalamus"
              ? "Thalamus"
              : "Tuning";
}

function formatCell(value) {
  if (Array.isArray(value)) return value.slice(0, 4).join(", ");
  if (typeof value === "number") return value.toFixed ? value.toFixed(3) : String(value);
  if (value && typeof value === "object") return JSON.stringify(value).slice(0, 80);
  return String(value ?? "");
}

function graphFields(node) {
  const props = node.properties || {};
  const thalamus = props.thalamus_scores || {};
  const fieldsByKind = {
    session: [
      ["Expectation", props.current_expectation],
      ["Mode", props.current_mode],
      ["Task", props.task_context],
      ["User", shortId(props.user_id)],
      ["Created", formatDate(props.created_at)],
      ["Updated", formatDate(props.updated_at)],
      ["Closed", formatDate(props.closed_at)]
    ],
    working_context: [
      ["Task", props.task_id],
      ["Goals", Array.isArray(props.goal_stack) ? props.goal_stack.length : 0],
      ["Active engrams", Array.isArray(props.active_engrams) ? props.active_engrams.length : 0],
      ["Buffer", Array.isArray(props.episodic_buffer) ? props.episodic_buffer.length : 0],
      ["Inference", Array.isArray(props.inference_layer) ? props.inference_layer.length : 0],
      ["Opened", formatDate(props.opened_at)],
      ["Updated", formatDate(props.updated_at)]
    ],
    episode: [
      ["Action", props.action],
      ["Context", props.context],
      ["Outcome", props.outcome],
      ["Session", shortId(props.session_id)],
      ["Created", formatDate(props.created_at)]
    ],
    pattern: [
      ["Hash", shortId(props.pattern_hash, 12)],
      ["Occurrences", props.occurrences],
      ["Strength", formatNumber(props.strength)],
      ["Threshold", formatNumber(props.threshold)],
      ["Decay", formatNumber(props.decay_rate)],
      ["Source", props.source],
      ["Tags", Array.isArray(props.context_tags) ? props.context_tags.join(", ") : ""],
      ["Episodes", Array.isArray(props.episode_refs) ? props.episode_refs.length : 0],
      ["First seen", formatDate(props.first_seen)],
      ["Last seen", formatDate(props.last_seen)]
    ],
    engram: [
      ["Strength", formatNumber(props.strength)],
      ["Access count", props.access_count],
      ["Status", props.status],
      ["Source", props.source],
      ["Tags", Array.isArray(props.tags) ? props.tags.join(", ") : ""],
      ["Session", shortId(props.session_ref)],
      ["Created", formatDate(props.created_at)],
      ["Last accessed", formatDate(props.last_accessed)],
      ["Kinship", shortId(props.kinship_ref)],
      ["Schema refs", Array.isArray(props.schema_refs) ? props.schema_refs.length : 0],
      ["Thalamus novelty", formatNumber(thalamus.novelty)],
      ["Thalamus surprise", formatNumber(thalamus.surprise)],
      ["Thalamus relevance", formatNumber(thalamus.task_relevance)],
      ["Thalamus valence", formatNumber(thalamus.emotional_valence)]
    ],
    schema: [
      ["Strength", formatNumber(props.strength)],
      ["Tags", Array.isArray(props.tags) ? props.tags.join(", ") : ""],
      ["Predictions", Array.isArray(props.prediction_fields) ? props.prediction_fields.length : 0],
      ["Sources", Array.isArray(props.source_engram_ids) ? props.source_engram_ids.length : 0],
      ["Created", formatDate(props.created_at)]
    ]
  };
  const entries = fieldsByKind[node.kind] || Object.entries(props).slice(0, 8);
  return entries.map(([label, value]) => ({
    label,
    value: formatValue(value)
  }));
}

function nodeSummary(node) {
  const props = node.properties || {};
  if (node.kind === "session") return `${props.current_mode || "session"} · ${truncate(props.task_context || "", 42)}`;
  if (node.kind === "working_context") return `${Array.isArray(props.goal_stack) ? props.goal_stack.length : 0} goals · ${Array.isArray(props.active_engrams) ? props.active_engrams.length : 0} active`;
  if (node.kind === "episode") return truncate(props.outcome || props.context || "Episode", 58);
  if (node.kind === "pattern") return `${props.occurrences ?? 0} activations · ${formatNumber(props.strength)}`;
  if (node.kind === "engram") return `${props.status || "active"} · ${Array.isArray(props.tags) ? props.tags.length : 0} tags`;
  if (node.kind === "schema") return `${Array.isArray(props.source_engram_ids) ? props.source_engram_ids.length : 0} source engrams`;
  return truncate(node.title || node.label || node.id, 54);
}

function nodeDetailLine(node) {
  const props = node.properties || {};
  if (node.kind === "session") return truncate(props.task_context || props.current_expectation || "", 36);
  if (node.kind === "working_context") return truncate(props.task_id || "", 36);
  if (node.kind === "episode") return truncate(props.action || "", 36);
  if (node.kind === "pattern") return truncate(Array.isArray(props.context_tags) ? props.context_tags.join(", ") : "", 36);
  if (node.kind === "engram") return truncate(Array.isArray(props.tags) ? props.tags.join(", ") : "", 36);
  if (node.kind === "schema") return truncate(Array.isArray(props.tags) ? props.tags.join(", ") : "", 36);
  return shortId(node.id);
}

function labelForKind(kind) {
  return kind === "working_context"
    ? "Context"
    : kind === "session"
      ? "Session"
      : kind === "episode"
        ? "Episode"
        : kind === "pattern"
          ? "Pattern"
          : kind === "engram"
            ? "Engram"
            : kind === "schema"
              ? "Schema"
              : kind;
}

function shortId(value, length = 8) {
  if (!value) return "—";
  const text = String(value);
  return text.length > length ? `${text.slice(0, length)}…` : text;
}

function truncate(value, length) {
  const text = String(value ?? "");
  return text.length > length ? `${text.slice(0, length - 1)}…` : text;
}

function formatNumber(value) {
  if (value === null || value === undefined || Number.isNaN(Number(value))) return "—";
  return Number(value).toFixed(3);
}

function formatDate(value) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return String(value);
  return date.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

function formatValue(value) {
  if (value === null || value === undefined || value === "") return "—";
  if (Array.isArray(value)) return value.length ? value.map((item) => formatValue(item)).join(", ") : "—";
  if (typeof value === "number") return Number.isInteger(value) ? String(value) : formatNumber(value);
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function pointFromEvent(event, rect) {
  return {
    x: ((event.clientX - rect.left) / rect.width) * graphCanvas.width,
    y: ((event.clientY - rect.top) / rect.height) * graphCanvas.height
  };
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

createRoot(document.getElementById("root")).render(<App />);
