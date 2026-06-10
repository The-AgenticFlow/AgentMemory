import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

const emptyMemory = { episodes: [], patterns: [], engrams: [], schemas: [] };

function App() {
  const [overview, setOverview] = useState(null);
  const [graph, setGraph] = useState({ nodes: [], edges: [] });
  const [memory, setMemory] = useState(emptyMemory);
  const [config, setConfig] = useState(null);
  const [selectedKind, setSelectedKind] = useState("all");
  const [status, setStatus] = useState("Loading memory system");
  const [sim, setSim] = useState(null);

  async function refresh() {
    const [overviewData, graphData, episodes, patterns, engrams, schemas, configData] = await Promise.all([
      api("/control/overview"),
      api("/control/graph"),
      api("/memory/episodes"),
      api("/memory/patterns"),
      api("/memory/engrams"),
      api("/memory/schemas"),
      api("/control/config")
    ]);
    setOverview(overviewData);
    setGraph(graphData);
    setMemory({ episodes, patterns, engrams, schemas });
    setConfig(configData);
    setStatus("Live");
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
    await api("/consolidate", { method: "POST", body: { debug: true } });
    await refresh();
  }

  const counts = overview?.counts || {};

  return (
    <main className="app-shell">
      <aside className="rail">
        <div className="brand">
          <span className="brand-mark">AM</span>
          <div>
            <h1>Agent Memory</h1>
            <p>Engram control panel</p>
          </div>
        </div>
        <nav>
          {["overview", "graph", "thalamus", "buffers", "engrams", "schemas", "tuning"].map((item) => (
            <a key={item} href={`#${item}`}>{item}</a>
          ))}
        </nav>
        <div className="system-card">
          <span className="pulse" />
          <strong>{status}</strong>
          <small>{overview?.neo4j?.message || "Waiting for server"}</small>
        </div>
      </aside>

      <section className="workspace">
        <header className="hero" id="overview">
          <div>
            <p className="eyebrow">Deployable persistent cognition</p>
            <h2>Memory cockpit for episodes, buffers, engrams, schemas, and behavior tuning.</h2>
          </div>
          <button onClick={consolidate} className="primary">Run Deep Sleep</button>
        </header>

        <section className="metrics">
          <Metric label="Sessions" value={counts.sessions} />
          <Metric label="Episodes" value={counts.episodes} />
          <Metric label="Buffers" value={counts.patterns} />
          <Metric label="Engrams" value={counts.engrams} />
          <Metric label="Schemas" value={counts.schemas} />
          <Metric label="MCP" value={overview?.mcp?.http_enabled ? "HTTP on" : "off"} />
        </section>

        <section className="panel graph-panel" id="graph">
          <PanelTitle title="Memory Graph" subtitle="Neo4j-shaped projection of the current Agent Memory state." />
          <div className="filter-row">
            {["all", "session", "episode", "pattern", "engram", "schema"].map((kind) => (
              <button key={kind} className={selectedKind === kind ? "chip active" : "chip"} onClick={() => setSelectedKind(kind)}>
                {kind}
              </button>
            ))}
          </div>
          <MemoryGraph graph={graph} selectedKind={selectedKind} />
        </section>

        <section className="grid two">
          <section className="panel" id="thalamus">
            <PanelTitle title="Thalamus Lab" subtitle="Preview intake scoring without storing an episode." />
            <form className="lab-form" onSubmit={runThalamusPreview}>
              <input name="action" placeholder="Action, e.g. fixed failing cargo test" required />
              <input name="context" placeholder="Context" required />
              <input name="outcome" placeholder="Outcome" required />
              <input name="expectation" placeholder="Expectation" defaultValue="the task should progress" />
              <input name="task_context" placeholder="Task context" />
              <select name="mode" defaultValue="Exploration">
                <option>Exploration</option>
                <option>Routine</option>
                <option>Critical</option>
              </select>
              <button className="primary" type="submit">Simulate</button>
            </form>
            {sim && <ScoreCard sim={sim} />}
          </section>

          <section className="panel">
            <PanelTitle title="Latest Thalamus Scores" subtitle="Recent accepted and rejected memory intake." />
            <div className="score-list">
              {(overview?.latest_scores || []).map((record) => (
                <div className="score-row" key={record.id}>
                  <span className={record.accepted ? "badge ok" : "badge reject"}>{record.accepted ? "accepted" : "rejected"}</span>
                  <strong>{record.score.toFixed(3)}</strong>
                  <small>{record.pattern_hash || "no pattern"}</small>
                </div>
              ))}
            </div>
          </section>
        </section>

        <section className="grid three">
          <MemoryTable id="buffers" title="Buffers" rows={memory.patterns} columns={["pattern_hash", "strength", "occurrences", "threshold", "decay_rate"]} />
          <MemoryTable id="engrams" title="Engrams" rows={memory.engrams} columns={["id", "strength", "status", "access_count", "tags"]} />
          <MemoryTable id="schemas" title="Schemas" rows={memory.schemas} columns={["id", "strength", "prediction_fields", "source_engram_ids"]} />
        </section>

        <section className="panel" id="tuning">
          <PanelTitle title="Behavior Tuning" subtitle="Hot-update runtime memory behavior without editing code." />
          {config && <ConfigEditor config={config} onSave={saveConfig} onReset={resetConfig} />}
        </section>
      </section>
    </main>
  );
}

function Metric({ label, value }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value ?? "..."}</strong>
    </div>
  );
}

function PanelTitle({ title, subtitle }) {
  return (
    <div className="panel-title">
      <h3>{title}</h3>
      <p>{subtitle}</p>
    </div>
  );
}

function MemoryGraph({ graph, selectedKind }) {
  const layout = useMemo(() => {
    const filtered = selectedKind === "all" ? graph.nodes : graph.nodes.filter((node) => node.kind === selectedKind);
    const ids = new Set(filtered.map((node) => node.id));
    const edges = graph.edges.filter((edge) => ids.has(edge.source) && ids.has(edge.target));
    const radius = 250;
    return {
      nodes: filtered.map((node, index) => {
        const angle = (index / Math.max(filtered.length, 1)) * Math.PI * 2;
        return { ...node, x: 360 + Math.cos(angle) * radius, y: 300 + Math.sin(angle) * radius };
      }),
      edges
    };
  }, [graph, selectedKind]);
  const byId = new Map(layout.nodes.map((node) => [node.id, node]));

  return (
    <svg className="graph" viewBox="0 0 720 600" role="img" aria-label="Memory graph">
      <defs>
        <filter id="glow"><feGaussianBlur stdDeviation="3" result="blur" /><feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge></filter>
      </defs>
      {layout.edges.map((edge) => {
        const source = byId.get(edge.source);
        const target = byId.get(edge.target);
        if (!source || !target) return null;
        return <line key={edge.id} x1={source.x} y1={source.y} x2={target.x} y2={target.y} className="graph-edge" />;
      })}
      {layout.nodes.map((node) => (
        <g key={node.id} transform={`translate(${node.x} ${node.y})`} className={`node ${node.kind}`}>
          <circle r="28" filter="url(#glow)" />
          <text y="4">{node.label.slice(0, 2)}</text>
          <title>{node.title || node.id}</title>
        </g>
      ))}
    </svg>
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
        <strong>{sim.score.toFixed(3)} / threshold {sim.threshold.toFixed(3)}</strong>
      </div>
      {bars.map(([name, value]) => (
        <div className="bar" key={name}><span>{name}</span><i style={{ width: `${value * 100}%` }} /></div>
      ))}
    </div>
  );
}

function MemoryTable({ id, title, rows, columns }) {
  return (
    <section className="panel table-panel" id={id}>
      <PanelTitle title={title} subtitle={`${rows.length} records`} />
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

  const fields = [
    ["thalamus", "novelty_weight"], ["thalamus", "surprise_weight"], ["thalamus", "task_relevance_weight"], ["thalamus", "valence_weight"],
    ["thalamus", "exploration_threshold"], ["thalamus", "routine_threshold"], ["thalamus", "critical_threshold"],
    ["buffer", "similarity_threshold"], ["buffer", "promotion_threshold"], ["buffer", "decay_rate"],
    ["pattern", "completion_threshold"], ["retrieval", "top_k"],
    ["consolidation", "active_threshold"], ["consolidation", "archive_threshold"], ["consolidation", "schema_threshold"], ["consolidation", "base_decay_rate"]
  ];

  return (
    <div>
      <div className="config-grid">
        {fields.map((path) => (
          <label key={path.join(".")}>
            <span>{path.join(".")}</span>
            <input type="number" step={path.at(-1) === "top_k" ? "1" : "0.01"} value={path.reduce((obj, key) => obj[key], draft)} onChange={(event) => setNumber(path, event.target.value)} />
          </label>
        ))}
      </div>
      <div className="actions">
        <button className="primary" onClick={() => onSave(draft)}>Save behavior profile</button>
        <button className="secondary" onClick={onReset}>Reset defaults</button>
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

function formatCell(value) {
  if (Array.isArray(value)) return value.slice(0, 4).join(", ");
  if (typeof value === "number") return value.toFixed ? value.toFixed(3) : String(value);
  if (value && typeof value === "object") return JSON.stringify(value).slice(0, 80);
  return String(value ?? "");
}

createRoot(document.getElementById("root")).render(<App />);
