import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

const emptyMemory = { episodes: [], patterns: [], engrams: [], schemas: [], workingMemory: [] };
const tabs = ["overview", "graph", "sessions", "episodes", "engrams", "schemas", "working-memory", "thalamus", "tuning", "performance"];
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
  const [banks, setBanks] = useState([]);
  const [selectedBank, setSelectedBank] = useState("");
  const selectedBankRef = useRef("");

  function apiQuery(path, bankId) {
    return bankId ? `${path}?bank_id=${bankId}` : path;
  }

  async function refresh(bankId) {
    const effectiveBankId = bankId ?? selectedBankRef.current;
    const health = await safeApi("/health");
    const banksData = await safeApi("/banks", []);
    setBanks(banksData);

    let effectiveBank = effectiveBankId;
    if (!effectiveBank && banksData.length > 0) {
      effectiveBank = banksData[0].id;
    }
    if (effectiveBank && effectiveBank !== selectedBankRef.current) {
      setSelectedBank(effectiveBank);
      selectedBankRef.current = effectiveBank;
    }

    const q = apiQuery;
    const [overviewData, graphData, episodes, patterns, engrams, schemas, sessionsData, wmData, configData] = await Promise.all([
      safeApi(q("/control/overview", effectiveBank)),
      safeApi(q("/control/graph", effectiveBank)),
      safeApi(q("/memory/episodes", effectiveBank), []),
      safeApi(q("/memory/patterns", effectiveBank), []),
      safeApi(q("/memory/engrams", effectiveBank), []),
      safeApi(q("/memory/schemas", effectiveBank), []),
      safeApi(q("/sessions", effectiveBank), []),
      safeApi(q("/working-memory", effectiveBank), []),
      safeApi("/control/config"),
    ]);

    // Flatten SessionSummary -> SessionView -> Session for table display
    const flatSessions = (sessionsData || []).map((s) => s.session?.session || {});

    setOverview(overviewData);
    setGraph(graphData || { nodes: [], edges: [] });
    setMemory({ episodes, patterns, engrams, schemas, sessions: flatSessions, workingMemory: wmData || [] });
    setConfig(configData);
    setStatus(health ? `Live · ${health.sessions} sessions` : "Backend unavailable");
  }

  useEffect(() => {
    refresh("").catch((error) => setStatus(error.message));
    const timer = window.setInterval(() => refresh().catch((error) => setStatus(error.message)), 15000);
    const handleRefresh = () => refresh().catch((error) => setStatus(error.message));
    window.addEventListener("engram-refresh", handleRefresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("engram-refresh", handleRefresh);
    };
  }, []);

  useEffect(() => {
    if (selectedBank) {
      refresh(selectedBank).catch((error) => setStatus(error.message));
    }
  }, [selectedBank]);

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
if (consolidating) return;
    setConsolidating(true);
    setConsolidateFlash("running");
    setStatus("Deep sleep running…");
    try {
      const result = await safeApi("/consolidate", null, { method: "POST", body: { debug: true } });
      await refresh(selectedBank);
      const count = Array.isArray(result) ? result.length : 0;
      setStatus(count > 0 ? `Deep sleep complete · ${count} schema(s) formed` : "Deep sleep complete");
      setConsolidateFlash("success");
      window.setTimeout(() => setConsolidateFlash(null), 2500);
    } catch (error) {
      setStatus(`Deep sleep failed: ${error.message}`);
      setConsolidateFlash("error");
      window.setTimeout(() => setConsolidateFlash(null), 4000);
    } finally {
      setConsolidating(false);
    }
  }

  const [showCreateBank, setShowCreateBank] = useState(false);
  const [bankSearch, setBankSearch] = useState("");
  const [consolidating, setConsolidating] = useState(false);
  const [consolidateFlash, setConsolidateFlash] = useState(null);

  async function createBank(event) {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const directives = (form.get("directives") || "").toString().split("\n").filter((s) => s.trim());
    await api("/banks", {
      method: "POST",
      body: {
        name: form.get("name"),
        bank_type: form.get("bank_type"),
        mission: form.get("mission") || null,
        directives,
        disposition: { skepticism: 2, literalism: 2, empathy: 3, verbosity: 2 },
        parent_bank_id: form.get("parent_bank_id") || null,
      }
    });
    setShowCreateBank(false);
    window.dispatchEvent(new Event("engram-refresh"));
  }

  async function deleteBank(bankId) {
    if (!window.confirm("Delete this bank and all its data?")) return;
    await safeApi(`/banks/${bankId}`, null, { method: "DELETE" });
    if (selectedBankRef.current === bankId) {
      setSelectedBank("");
      selectedBankRef.current = "";
    }
    window.dispatchEvent(new Event("engram-refresh"));
  }

  const counts = overview?.counts || {};
  const currentBank = banks.find((b) => b.id === selectedBank);

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

        <section className="bank-portal">
          <div className="bank-portal-header">
            <h2>Memory Banks</h2>
            <span className="bank-count">{banks.length}</span>
          </div>
          <input
            className="bank-search"
            type="text"
            placeholder="Search banks..."
            value={bankSearch}
            onChange={(e) => setBankSearch(e.target.value)}
          />
          <div className="bank-list">
            {(() => {
              const bankMap = new Map(banks.map((b) => [b.id, b]));
              const getDepth = (bank) => {
                let depth = 0;
                let current = bank;
                while (current?.parent_bank_id) {
                  const parent = bankMap.get(current.parent_bank_id);
                  if (!parent) break;
                  depth++;
                  current = parent;
                }
                return depth;
              };
              const query = bankSearch.trim().toLowerCase();
              const filtered = query
                ? banks.filter((b) => b.name.toLowerCase().includes(query) || b.bank_type.toLowerCase().includes(query))
                : banks;
              return filtered.length === 0 ? (
                <div className="bank-list-empty">No banks match</div>
              ) : (
                filtered.map((bank) => {
                  const depth = getDepth(bank);
                  const isActive = selectedBank === bank.id;
                  return (
                    <button
                      key={bank.id}
                      type="button"
                      className={isActive ? "bank-list-item active" : "bank-list-item"}
                      style={{ paddingLeft: `${10 + depth * 14}px` }}
                      onClick={() => setSelectedBank(bank.id)}
                      title={`${bank.name} · ${bank.bank_type}${bank.parent_bank_id ? " (child)" : ""}`}
                    >
                      <span className={`bank-type-indicator ${bank.bank_type}`} />
                      <div className="bank-list-meta">
                        <strong>{bank.name}</strong>
                        <small>{bank.memory_count} mem · {bank.schema_count} schema · {bank.directive_count} dir</small>
                      </div>
                      {isActive && <span className="bank-active-mark" />}
                    </button>
                  );
                })
              );
            })()}
          </div>

          {currentBank && (
            <div className="bank-info">
              <div className="bank-info-header">
                <strong>{currentBank.name}</strong>
                <span className="badge">{currentBank.bank_type}</span>
              </div>
              {currentBank.mission && <p>{currentBank.mission}</p>}
            </div>
          )}
          <div className="bank-actions">
            <button className="primary slim" type="button" onClick={() => setShowCreateBank(true)}>
              + New Bank
            </button>
          </div>
        </section>

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
            <h2>{currentBank ? currentBank.name : "All Banks"}</h2>
            <p className="hero-note">
              {currentBank?.mission || "Select a memory bank from the left panel to isolate its memory."}
            </p>
          </div>
          <div className="hero-summary">
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
            <OverviewTab overview={overview} counts={counts} banks={banks} currentBank={currentBank} onOpenCreateModal={() => setShowCreateBank(true)} onDeleteBank={deleteBank} onSelectBank={setSelectedBank} onDeepSleep={consolidate} consolidating={consolidating} consolidateFlash={consolidateFlash} />
          )}
          {activeTab === "graph" && (
            <GraphTab graph={graph} selectedKind={selectedKind} setSelectedKind={setSelectedKind} />
          )}
          {activeTab === "sessions" && (
            <MemoryTab
              title="Sessions"
              subtitle="Sessions scoped to the selected bank."
              rows={memory.sessions || []}
              columns={["id", "current_mode", "task_context", "current_expectation", "created_at"]}
            />
          )}
          {activeTab === "episodes" && (
            <MemoryTab
              title="Episodes"
              subtitle="Completed experiences in the selected bank."
              rows={memory.episodes || []}
              columns={["id", "action", "context", "outcome", "created_at"]}
            />
          )}
          {activeTab === "engrams" && (
            <MemoryTab
              title="Engrams"
              subtitle="Long-term memory indices in the selected bank."
              rows={memory.engrams || []}
              columns={["id", "strength", "status", "access_count", "tags"]}
            />
          )}
          {activeTab === "schemas" && (
            <MemoryTab
              title="Schemas"
              subtitle="Compressed patterns and predictions in the selected bank."
              rows={memory.schemas || []}
              columns={["id", "strength", "prediction_fields", "source_engram_ids"]}
            />
          )}
          {activeTab === "working-memory" && (
            <MemoryTab
              title="Working Memory"
              subtitle="Short-lived memory entries in the selected bank."
              rows={memory.workingMemory || []}
              columns={["id", "content", "strength", "tags", "created_at"]}
            />
          )}
          {activeTab === "thalamus" && (
            <ThalamusTab sim={sim} onSubmit={runThalamusPreview} />
          )}
          {activeTab === "tuning" && config && (
            <TuningTab config={config} onSave={saveConfig} onReset={resetConfig} />
          )}
          {activeTab === "performance" && (
            <PerformanceTab overview={overview} />
          )}
        </section>
      </section>

      {showCreateBank && (
        <BankCreateModal banks={banks} onSubmit={createBank} onClose={() => setShowCreateBank(false)} />
      )}
    </main>
  );
}

function OverviewTab({ overview, counts, banks, currentBank, onOpenCreateModal, onDeleteBank, onSelectBank, onDeepSleep, consolidating, consolidateFlash }) {
  const activeConfig = overview?.active_config || {};
  const thalamus = activeConfig.thalamus || {};
  const buffer = activeConfig.buffer || {};
  const retrieval = activeConfig.retrieval || {};
  const bankMap = useMemo(() => new Map(banks.map((b) => [b.id, b])), [banks]);

  return (
    <div className="dashboard-grid overview-grid">
      <section className="panel subtle span-2">
        <PanelTitle title="Snapshot" subtitle={currentBank ? `Memory load for bank: ${currentBank.name}` : "Current memory load and operational posture."} />
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

      <section className="panel subtle action-panel">
        <PanelTitle title="Memory Maintenance" subtitle="Run overnight consolidation to decay weak memories, clean buffers, and form schemas." />
        <div className="action-row">
          <button
            className={
              consolidateFlash === "success"
                ? "primary slim flash-success"
                : consolidateFlash === "error"
                  ? "primary slim flash-error"
                  : consolidating
                    ? "primary slim consolidating"
                    : "primary slim"
            }
            type="button"
            onClick={onDeepSleep}
            disabled={consolidating}
            aria-busy={consolidating}
          >
            {consolidating ? (
              <span className="btn-inner">
                <span className="spinner" /> Consolidating…
              </span>
            ) : consolidateFlash === "success" ? (
              <span className="btn-inner">Done</span>
            ) : consolidateFlash === "error" ? (
              <span className="btn-inner">Failed</span>
            ) : (
              <span className="btn-inner">Deep sleep</span>
            )}
          </button>
          <span className="action-hint">
            {consolidating
              ? "Decaying engrams, expiring working memory, compressing schemas…"
              : "Decay weak patterns, expire working memory, and compress schemas."}
          </span>
        </div>
      </section>

      <section className="panel subtle span-2">
        <div className="bank-section-header">
          <PanelTitle title="Memory Banks" subtitle="Active memory banks in the system." />
          <button className="primary slim" type="button" onClick={onOpenCreateModal}>
            + New Bank
          </button>
        </div>
        <div className="bank-grid">
          {banks.length === 0 ? (
            <div className="empty-state">No banks yet.</div>
          ) : (
            banks.map((bank) => {
              const parent = bankMap.get(bank.parent_bank_id);
              return (
                <div key={bank.id} className="bank-card">
                  <div className="bank-card-body">
                    <div className="bank-card-title">
                      <span className={`bank-type-indicator ${bank.bank_type}`} />
                      <strong>{bank.name}</strong>
                      <span className="badge">{bank.bank_type}</span>
                    </div>
                    {parent && <small className="bank-card-parent">Parent: {parent.name}</small>}
                    <small>{bank.memory_count} memories · {bank.schema_count} schemas · {bank.directive_count} directives</small>
                    {bank.mission && <p className="bank-card-mission">{bank.mission}</p>}
                  </div>
                  <div className="bank-card-actions">
                    <button className="secondary slim" type="button" onClick={() => onSelectBank(bank.id)}>Open</button>
                    <button className="secondary slim danger" type="button" onClick={() => onDeleteBank(bank.id)}>Delete</button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </section>
    </div>
  );
}

function BankCreateModal({ banks, onSubmit, onClose }) {
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-panel" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3>Create Memory Bank</h3>
          <button className="secondary slim" type="button" onClick={onClose}>Close</button>
        </div>
        <form className="lab-form modal-form" onSubmit={onSubmit}>
          <input name="name" placeholder="Bank name" required />
          <select name="bank_type" defaultValue="dictionary">
            <option value="session">Session</option>
            <option value="dictionary">Dictionary</option>
            <option value="shared">Shared</option>
          </select>
          <textarea name="mission" placeholder="Mission statement" rows={3} />
          <textarea name="directives" placeholder="Directives (one per line)" rows={3} />
          <select name="parent_bank_id">
            <option value="">No parent</option>
            {banks.map((b) => (
              <option key={b.id} value={b.id}>{b.name} ({b.bank_type})</option>
            ))}
          </select>
          <button className="primary" type="submit">Create Bank</button>
        </form>
      </div>
    </div>
  );
}

function GraphTab({ graph, selectedKind, setSelectedKind }) {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Memory graph" subtitle="Filtered graph projection scoped to the selected bank." />
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

function ThalamusTab({ sim, onSubmit }) {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Intake Simulator" subtitle="Preview how an episode would score before entering memory. Nothing is stored." />
        <p className="panel-help">
          The thalamus filter scores every episode on four dimensions — novelty, surprise, task relevance, and emotional valence.
          Use this simulator to test whether a given action/context/outcome would pass the current thresholds before running real ingestion.
        </p>
        <form className="lab-form thalamus-lab" onSubmit={onSubmit}>
          <input name="action" placeholder="Action" required />
          <input name="context" placeholder="Context" required />
          <input name="outcome" placeholder="Outcome" required />
          <input name="expectation" placeholder="Expectation" defaultValue="the task should progress" />
          <select name="mode" defaultValue="Exploration">
            <option>Exploration</option>
            <option>Routine</option>
            <option>Critical</option>
            <option>Analogy</option>
            <option>Validation</option>
          </select>
          <button className="primary" type="submit">Simulate intake</button>
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
  const [panning, setPanning] = useState(null);
  const [view, setView] = useState({ scale: 1, x: 0, y: 0 });
  const [viewportSize, setViewportSize] = useState({ width: 0, height: 0 });
  const [inspectorCollapsed, setInspectorCollapsed] = useState(false);
  const boardRef = useRef(null);
  const viewTouchedRef = useRef(false);
  const selectedKindRef = useRef(selectedKind);

  const layout = useMemo(() => {
    const filtered = selectedKind === "all" ? graph.nodes : graph.nodes.filter((node) => node.kind === selectedKind);
    const ids = new Set(filtered.map((node) => node.id));
    const edges = graph.edges.filter((edge) => ids.has(edge.source) && ids.has(edge.target));
    const nodeSize = clamp(112 - filtered.length * 1.15, 56, 108);
    const radius = filtered.length <= 1 ? 0 : Math.max(160, filtered.length * (nodeSize * 0.85 / Math.PI));
    const stageSize = Math.ceil(Math.max(graphCanvas.width, graphCanvas.height, radius * 2 + nodeSize * 4));
    const center = { x: stageSize / 2, y: stageSize / 2 };

    const nodes = filtered.map((node, index) => {
      const angle = filtered.length === 1 ? -Math.PI / 2 : (index / filtered.length) * Math.PI * 2 - Math.PI / 2;
      return {
        ...node,
        x: positions[node.id]?.x ?? center.x + Math.cos(angle) * radius,
        y: positions[node.id]?.y ?? center.y + Math.sin(angle) * radius
      };
    });

    return { nodes, edges, filtered, stageSize, nodeSize };
  }, [graph, positions, selectedKind]);

  const fitView = useMemo(() => {
    const width = viewportSize.width || graphCanvas.width;
    const height = viewportSize.height || graphCanvas.height;
    const scale = clamp(Math.min(width / layout.stageSize, height / layout.stageSize) * 0.92, 0.05, 1);
    return {
      scale,
      x: (width - layout.stageSize * scale) / 2,
      y: (height - layout.stageSize * scale) / 2
    };
  }, [layout.stageSize, viewportSize.height, viewportSize.width]);

  useEffect(() => {
    const element = boardRef.current;
    if (!element) return;
    const update = () => {
      const rect = element.getBoundingClientRect();
      setViewportSize({ width: rect.width, height: rect.height });
    };
    update();
    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(update);
      observer.observe(element);
      return () => observer.disconnect();
    }
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);

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
    if (selectedKindRef.current !== selectedKind) {
      selectedKindRef.current = selectedKind;
      viewTouchedRef.current = false;
    }
  }, [selectedKind]);

  useEffect(() => {
    if (!viewTouchedRef.current) {
      setView(fitView);
    }
  }, [fitView]);

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
      const point = pointFromEvent(event, rect, view);
      setPositions((current) => ({
        ...current,
        [dragging.id]: {
          x: clamp(point.x - dragging.offsetX, 72, layout.stageSize - 72),
          y: clamp(point.y - dragging.offsetY, 72, layout.stageSize - 72)
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
  }, [dragging, layout.stageSize, view]);

  useEffect(() => {
    if (!panning) return;
    function move(event) {
      viewTouchedRef.current = true;
      setView((current) => ({
        ...current,
        x: panning.startX + (event.clientX - panning.clientX),
        y: panning.startY + (event.clientY - panning.clientY)
      }));
    }
    function end() {
      setPanning(null);
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
    };
  }, [panning]);

  function zoomTo(nextScale, anchor = { x: viewportSize.width / 2, y: viewportSize.height / 2 }) {
    if (anchor.x == null || anchor.y == null) return;
    viewTouchedRef.current = true;
    setView((current) => {
      const clampedScale = clamp(nextScale, 0.05, 2.75);
      const contentX = (anchor.x - current.x) / current.scale;
      const contentY = (anchor.y - current.y) / current.scale;
      return {
        scale: clampedScale,
        x: anchor.x - contentX * clampedScale,
        y: anchor.y - contentY * clampedScale
      };
    });
  }

  function fitGraph() {
    viewTouchedRef.current = false;
    setView(fitView);
  }

  function resetLayout() {
    setPositions({});
    fitGraph();
  }

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
          <span className="graph-zoom-readout">{Math.round(view.scale * 100)}%</span>
          <button className="secondary slim" type="button" onClick={() => zoomTo(view.scale / 1.2)}>-</button>
          <input
            className="graph-zoom-slider"
            type="range"
            min="0.05"
            max="2.75"
            step="0.01"
            value={view.scale}
            onChange={(event) => zoomTo(Number.parseFloat(event.target.value))}
            aria-label="Zoom graph"
          />
          <button className="secondary slim" type="button" onClick={fitGraph}>Fit</button>
          <button className="secondary slim" type="button" onClick={() => zoomTo(view.scale * 1.2)}>+</button>
          <button className="secondary slim" type="button" onClick={resetLayout}>Reset layout</button>
          <button className="secondary slim" type="button" onClick={() => setExpanded((value) => !value)}>
            {expanded ? "Collapse" : "Expand"}
          </button>
        </div>
      </div>

      <div className={inspectorCollapsed ? "graph-body inspector-collapsed" : "graph-body"}>
        <div
          className={panning ? "graph-board panning" : "graph-board"}
          ref={boardRef}
          onPointerDown={(event) => {
            if (event.target instanceof Element && event.target.closest(".graph-node")) return;
            if (event.button !== 0) return;
            viewTouchedRef.current = true;
            setPanning({
              clientX: event.clientX,
              clientY: event.clientY,
              startX: view.x,
              startY: view.y
            });
          }}
          onWheel={(event) => {
            event.preventDefault();
            const rect = boardRef.current?.getBoundingClientRect();
            if (!rect) return;
            const anchor = {
              x: event.clientX - rect.left,
              y: event.clientY - rect.top
            };
            const nextScale = event.deltaY > 0 ? view.scale / 1.08 : view.scale * 1.08;
            zoomTo(nextScale, anchor);
          }}
        >
          <div
            className="graph-stage"
            style={{
              width: `${layout.stageSize}px`,
              height: `${layout.stageSize}px`,
              transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`
            }}
          >
            <svg className="graph-links" viewBox={`0 0 ${layout.stageSize} ${layout.stageSize}`} aria-hidden="true">
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
              const compact = layout.nodeSize < 84;
              return (
                <button
                  key={node.id}
                  type="button"
                  className={isActive ? `graph-node ${node.kind} active${compact ? " compact" : ""}` : `graph-node ${node.kind}${compact ? " compact" : ""}`}
                  style={{
                    "--node-size": `${layout.nodeSize}px`,
                    left: `${node.x}px`,
                    top: `${node.y}px`
                  }}
                  onPointerDown={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    const rect = boardRef.current?.getBoundingClientRect();
                    if (!rect) return;
                    const point = pointFromEvent(event, rect, view);
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
                  <strong>{truncate(node.title || node.label, compact ? 18 : 24)}</strong>
                  <small>{truncate(nodeDetailLine(node), compact ? 22 : 34)}</small>
                  <em>{shortId(node.id)}</em>
                </button>
              );
            })}
          </div>
        </div>

        <aside className={inspectorCollapsed ? "graph-inspector collapsed" : "graph-inspector"}>
          <div className="inspector-header">
            <span>{inspectorCollapsed ? "Inspect" : "Inspector"}</span>
            {!inspectorCollapsed && (
              <strong>{selectedNode ? labelForKind(selectedNode.kind) : "No selection"}</strong>
            )}
            <button
              className="secondary slim inspector-toggle"
              type="button"
              onClick={() => setInspectorCollapsed((value) => !value)}
              aria-label={inspectorCollapsed ? "Expand inspector" : "Collapse inspector"}
            >
              {inspectorCollapsed ? "‹" : "›"}
            </button>
          </div>
          {!inspectorCollapsed && selectedNode ? (
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
          ) : !inspectorCollapsed ? (
            <div className="empty-state">Select a node to inspect its properties and connections.</div>
          ) : null}
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
  const [activeProfile, setActiveProfile] = useState(config?.tuning_profile || "Balanced");
  useEffect(() => setDraft(config), [config]);

  function setNumber(path, value) {
    const next = structuredClone(draft);
    let cursor = next;
    for (const key of path.slice(0, -1)) cursor = cursor[key];
    cursor[path.at(-1)] = path.at(-1) === "top_k" || path.at(-1) === "separation_search_candidates" || path.at(-1) === "max_content_length" ? Number.parseInt(value, 10) : Number.parseFloat(value);
    setDraft(next);
  }

  async function applyProfile(profile) {
    setActiveProfile(profile);
    const body = { ...draft, tuning_profile: profile };
    const saved = await api("/control/config", { method: "PUT", body });
    setDraft(saved);
    await onSave(saved);
  }

  const groups = [
    {
      title: "Thalamus Intake",
      fields: [
        ["thalamus", "novelty_weight", "Novelty Weight", "How much weight to give new/unfamiliar patterns", 0, 1],
        ["thalamus", "surprise_weight", "Surprise Weight", "How much weight to give unexpected outcomes", 0, 1],
        ["thalamus", "task_relevance_weight", "Relevance Weight", "How much weight to give task-context match", 0, 1],
        ["thalamus", "valence_weight", "Valence Weight", "How much weight to give emotional tone", 0, 1],
      ]
    },
    {
      title: "Mode Thresholds",
      fields: [
        ["thalamus", "exploration_threshold", "Exploration", "Minimum score to accept in exploration mode", 0, 1],
        ["thalamus", "routine_threshold", "Routine", "Minimum score to accept in routine mode", 0, 1],
        ["thalamus", "critical_threshold", "Critical", "Minimum score to accept in critical mode", 0, 1],
        ["thalamus", "analogy_threshold", "Analogy", "Minimum score in cross-domain analogy mode", 0, 1],
        ["thalamus", "validation_threshold", "Validation", "Minimum score in evidence-based validation mode", 0, 1],
      ]
    },
    {
      title: "Buffer",
      fields: [
        ["buffer", "similarity_threshold", "Similarity Threshold", "Similarity needed to merge buffered patterns", 0, 1],
        ["buffer", "promotion_threshold", "Promotion Threshold", "Threshold for promoting a pattern to engram", 0, 1],
        ["buffer", "decay_rate", "Decay Rate", "Base decay rate for buffered patterns", 0, 1],
        ["buffer", "strength_base_coefficient", "Strength Base", "Base coefficient for strength calculation", 0, 1],
        ["buffer", "surprise_contribution", "Surprise Contrib", "How much surprise contributes to strength", 0, 1],
        ["buffer", "valence_contribution", "Valence Contrib", "How much valence contributes to strength", 0, 1],
      ]
    },
    {
      title: "Pattern Resolution",
      fields: [
        ["pattern", "completion_threshold", "Completion Threshold", "Similarity needed to merge with existing engram", 0.5, 1.0],
        ["pattern", "separation_search_candidates", "Search Candidates", "Number of candidates to check during separation", 1, 10],
        ["pattern", "strength_merge_ratio", "Merge Ratio", "How much of pattern strength merges into engram", 0, 1],
      ]
    },
    {
      title: "Retrieval",
      fields: [
        ["retrieval", "top_k", "Top K", "Number of results to return", 1, 20],
        ["retrieval", "keyword_tag_weight", "Tag Weight", "Weight for keyword-tag overlap boost", 0, 0.2],
        ["retrieval", "keyword_content_weight", "Content Weight", "Weight for keyword-content overlap boost", 0, 0.2],
        ["retrieval", "schema_bonus_weight", "Schema Bonus", "Weight for schema prediction match bonus", 0, 0.2],
        ["retrieval", "max_content_length", "Max Content", "Maximum content length before truncation", 50, 1000],
      ]
    },
    {
      title: "Consolidation",
      fields: [
        ["consolidation", "active_threshold", "Active Threshold", "Strength above which engram stays active", 0, 1],
        ["consolidation", "archive_threshold", "Archive Threshold", "Strength below which engram gets archived", 0, 1],
        ["consolidation", "schema_threshold", "Schema Threshold", "Similarity threshold for schema formation", 0, 1],
        ["consolidation", "base_decay_rate", "Decay Rate", "Base decay rate per day of inactivity", 0, 1],
      ]
    }
  ];

  return (
    <div className="config-shell">
      <div className="config-presets">
        <span>Profiles:</span>
        {["Conservative", "Balanced", "Exploratory", "Adaptive"].map((profile) => (
          <button
            key={profile}
            className={`preset-btn ${activeProfile === profile ? "active" : ""}`}
            type="button"
            onClick={() => applyProfile(profile)}
          >
            {profile}
          </button>
        ))}
      </div>
      {groups.map((group) => (
        <section className="config-group" key={group.title}>
          <h4>{group.title}</h4>
          <div className="config-grid">
            {group.fields.map((field) => {
              const [section, key, label, description, min, max] = field;
              const value = draft[section][key];
              const step = key === "top_k" || key === "separation_search_candidates" || key === "max_content_length" ? "1" : "0.01";
              return (
                <div className="config-field" key={section + "." + key}>
                  <label>
                    <span className="config-label">{label}</span>
                    <div className="slider-row">
                      <input
                        type="range"
                        min={min}
                        max={max}
                        step={step}
                        value={value}
                        onChange={(event) => setNumber([section, key], event.target.value)}
                        title={description}
                      />
                      <input
                        type="number"
                        step={step}
                        min={min}
                        max={max}
                        value={value}
                        onChange={(event) => setNumber([section, key], event.target.value)}
                        className="config-input"
                        title={description}
                      />
                    </div>
                  </label>
                  <small className="config-desc">{description}</small>
                </div>
              );
            })}
          </div>
        </section>
      ))}
      <div className="actions">
        <button className="primary slim" onClick={() => onSave(draft)}>Save</button>
        <button className="secondary slim" onClick={onReset}>Reset to Defaults</button>
      </div>
    </div>
  );
}

function PerformanceTab({ overview }) {
  const counts = overview?.counts || {};
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Performance" subtitle="High-level operation metrics." />
        <div className="metric-grid">
          {Object.entries(counts).map(([key, value]) => (
            <Metric key={key} label={key.replace(/_/g, " ")} value={value} />
          ))}
        </div>
      </section>
    </div>
  );
}

function ExperimentsTab() {
  return (
    <div className="tab-stack">
      <section className="panel subtle">
        <PanelTitle title="Experiments" subtitle="Coming soon." />
        <div className="empty-state">No experiments running.</div>
      </section>
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
      : tab === "sessions"
        ? "Sessions"
      : tab === "episodes"
        ? "Episodes"
      : tab === "engrams"
        ? "Engrams"
      : tab === "schemas"
        ? "Schemas"
      : tab === "working-memory"
        ? "Working Memory"
      : tab === "thalamus"
        ? "Simulator"
      : tab === "tuning"
        ? "Tuning"
      : tab === "performance"
        ? "Performance"
      : tab === "experiments"
        ? "Experiments"
      : tab;
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
      ["Thalamus valence", formatNumber(thalamus.valence)]
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

function pointFromEvent(event, rect, view = { scale: 1, x: 0, y: 0 }) {
  return {
    x: (event.clientX - rect.left - view.x) / view.scale,
    y: (event.clientY - rect.top - view.y) / view.scale
  };
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

const root = createRoot(document.getElementById("root"));
root.render(<App />);
