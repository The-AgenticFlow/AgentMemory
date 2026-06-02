const storageKeys = {
  selectedSession: "engram.selectedSessionId",
  historyPrefix: "engram.session.history.",
};

const state = {
  apiBase: window.location.origin,
  health: null,
  sessions: [],
  selectedSessionId: localStorage.getItem(storageKeys.selectedSession) || null,
  filter: "",
  retrievalEnabled: localStorage.getItem("engram.retrievalEnabled") === "true",
  debugMode: localStorage.getItem("engram.debugMode") === "true",
  demoRunning: false,
};

const els = {
  serverStatus: document.getElementById("server-status"),
  serverDot: document.getElementById("server-dot"),
  qwenStatus: document.getElementById("qwen-status"),
  qwenDot: document.getElementById("qwen-dot"),
  sessionFilter: document.getElementById("session-filter"),
  sessionList: document.getElementById("session-list"),
  newSessionButton: document.getElementById("new-session-button"),
  newChatEmpty: document.getElementById("new-chat-empty"),
  runDemoEmpty: document.getElementById("run-demo-empty"),
  sessionTitle: document.getElementById("session-title"),
  sessionSubtitle: document.getElementById("session-subtitle"),
  headerRight: document.getElementById("header-right"),
  emptyState: document.getElementById("empty-state"),
  messages: document.getElementById("messages"),
  chatForm: document.getElementById("chat-form"),
  chatInput: document.getElementById("chat-input"),
  sendButton: document.getElementById("send-button"),
  sidebar: document.getElementById("sidebar"),
  menuToggle: document.getElementById("menu-toggle"),
  sidebarOverlay: document.getElementById("sidebar-overlay"),
  toolRetrieve: document.getElementById("tool-retrieve"),
  retrieveLabel: document.getElementById("retrieve-label"),
  deepSleepButton: document.getElementById("deep-sleep-button"),
  deepSleepLabel: document.getElementById("deep-sleep-label"),
  deepSleepInline: document.getElementById("deep-sleep-inline"),
  deepSleepInlineText: document.getElementById("deep-sleep-inline-text"),
  debugToggle: document.getElementById("debug-toggle"),
};

bootstrap();

async function bootstrap() {
  wireEvents();
  els.chatInput.style.height = "auto";

  await Promise.all([refreshHealth(), refreshSessions({ preserveSelection: false })]);

  if (state.selectedSessionId) {
    try {
      await selectSession(state.selectedSessionId, { scroll: false });
    } catch {
      state.selectedSessionId = null;
      localStorage.removeItem(storageKeys.selectedSession);
      if (state.sessions.length > 0) {
        await selectSession(state.sessions[0].session.session.id, { scroll: false });
      } else {
        renderEmptyState();
      }
    }
  } else if (state.sessions.length > 0) {
    await selectSession(state.sessions[0].session.session.id, { scroll: false });
  } else {
    renderEmptyState();
  }

  window.setInterval(refreshHealth, 10000);
}

function wireEvents() {
  els.newSessionButton.addEventListener("click", () => createSession());
  if (els.newChatEmpty) els.newChatEmpty.addEventListener("click", () => createSession());
  if (els.runDemoEmpty) els.runDemoEmpty.addEventListener("click", runDemo);
  els.chatForm.addEventListener("submit", onSendMessage);
  els.chatInput.addEventListener("input", () => autoResize(els.chatInput));
  els.chatInput.addEventListener("keydown", async (event) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      await els.chatForm.requestSubmit();
    }
  });
  els.menuToggle.addEventListener("click", () => {
    els.sidebar.classList.add("open");
    els.sidebarOverlay.classList.add("show");
  });
  els.sidebarOverlay.addEventListener("click", () => {
    els.sidebar.classList.remove("open");
    els.sidebarOverlay.classList.remove("show");
  });
  els.toolRetrieve.addEventListener("click", toggleRetrieval);
  els.deepSleepButton.addEventListener("click", deepSleep);
  if (els.debugToggle) {
    els.debugToggle.addEventListener("click", toggleDebug);
  }
  renderRetrievalState();
  renderDebugState();
}

function toggleRetrieval() {
  state.retrievalEnabled = !state.retrievalEnabled;
  localStorage.setItem("engram.retrievalEnabled", String(state.retrievalEnabled));
  renderRetrievalState();
}

function renderRetrievalState() {
  const active = state.retrievalEnabled;
  els.toolRetrieve.classList.toggle("active", active);
  els.toolRetrieve.setAttribute("aria-pressed", String(active));
  if (els.retrieveLabel) {
    els.retrieveLabel.textContent = active ? "Retrieval on" : "Retrieval off";
  }
}

function toggleDebug() {
  state.debugMode = !state.debugMode;
  localStorage.setItem("engram.debugMode", String(state.debugMode));
  renderDebugState();
}

function renderDebugState() {
  const active = state.debugMode;
  if (els.debugToggle) {
    els.debugToggle.classList.toggle("active", active);
    els.debugToggle.setAttribute("aria-pressed", String(active));
    els.debugToggle.title = active ? "Debug mode ON" : "Debug mode OFF";
  }
}

async function refreshHealth() {
  try {
    const health = await fetchJson("/health");
    state.health = health;
    els.serverStatus.textContent = `Online · ${health.sessions} sessions`;
    els.serverDot.classList.remove("muted");
    els.qwenStatus.textContent = health.qwen_connected ? "Qwen connected" : "Qwen offline";
    els.qwenDot.classList.toggle("muted", !health.qwen_connected);
  } catch (error) {
    els.serverStatus.textContent = "Offline";
    els.serverDot.classList.add("muted");
    els.qwenStatus.textContent = "Qwen unavailable";
    els.qwenDot.classList.add("muted");
  }
}

async function refreshSessions({ preserveSelection = true } = {}) {
  const sessions = await fetchJson("/sessions");
  state.sessions = sessions;
  renderSessions();
  if (!preserveSelection) return;
  if (state.selectedSessionId && sessions.some((item) => item.session.session.id === state.selectedSessionId)) return;
  if (sessions.length > 0) {
    await selectSession(sessions[0].session.session.id, { scroll: false });
  } else {
    state.selectedSessionId = null;
    localStorage.removeItem(storageKeys.selectedSession);
    renderEmptyState();
  }
}

function renderSessions() {
  const filtered = state.sessions.filter((entry) => {
    if (!state.filter) return true;
    const haystack = [
      entry.title, entry.subtitle,
      entry.session.session.task_context,
      entry.session.session.current_expectation,
      entry.session.session.id,
    ].join(" ").toLowerCase();
    return haystack.includes(state.filter);
  });

  els.sessionList.replaceChildren();

  if (filtered.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = state.filter ? "No matches." : "No sessions yet.";
    els.sessionList.appendChild(empty);
    return;
  }

  for (const entry of filtered) {
    els.sessionList.appendChild(renderSessionItem(entry));
  }
}

function renderSessionItem(entry) {
  const session = entry.session.session;
  const item = document.createElement("div");
  item.className = "session-item";
  if (session.id === state.selectedSessionId) item.classList.add("active");

  const title = entry.title || session.task_context || "Untitled";
  const subtitle = entry.subtitle || session.current_expectation || "";
  const closed = Boolean(session.closed_at);

  const select = document.createElement("div");
  select.className = "session-select";
  select.tabIndex = 0;
  select.setAttribute("role", "button");
  select.setAttribute("aria-label", `Select ${title}`);

  const titleNode = document.createElement("div");
  titleNode.className = "session-title";
  titleNode.textContent = title;

  const metaNode = document.createElement("div");
  metaNode.className = "session-meta";
  metaNode.textContent = closed ? `Closed · ${subtitle}` : subtitle;

  select.append(titleNode, metaNode);

  const closeBtn = document.createElement("button");
  closeBtn.type = "button";
  closeBtn.className = "session-close";
  closeBtn.setAttribute("aria-label", "Close session");
  closeBtn.textContent = "×";

  select.addEventListener("click", () => selectSession(session.id));
  select.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectSession(session.id);
    }
  });
  closeBtn.addEventListener("click", (event) => {
    event.stopPropagation();
    showDeleteConfirmation(session.id, title);
  });

  item.append(select, closeBtn);
  return item;
}

async function createSession(options = {}) {
  const expectation = options.expectation || "remember what I say";
  const taskContext = options.taskContext || "chat";
  const mode = options.mode || "Exploration";

  const response = await fetchJson("/sessions", {
    method: "POST",
    body: { user_id: null, expectation, mode, task_context: taskContext },
  });

  await refreshSessions({ preserveSelection: false });
  await selectSession(response.session.session.id);
  if (options.scroll !== false) scrollMessagesToBottom();
  return response;
}

async function closeSession(sessionId) {
  await fetchJson(`/sessions/${sessionId}`, { method: "DELETE" });
  clearHistory(sessionId);
  if (state.selectedSessionId === sessionId) {
    state.selectedSessionId = null;
    localStorage.removeItem(storageKeys.selectedSession);
  }
  await refreshSessions({ preserveSelection: false });
  if (state.selectedSessionId) {
    await selectSession(state.selectedSessionId, { scroll: false });
  } else if (state.sessions.length > 0) {
    await selectSession(state.sessions[0].session.session.id, { scroll: false });
  } else {
    renderEmptyState();
  }
}

async function deleteSession(sessionId) {
  await fetchJson(`/sessions/${sessionId}/delete`, { method: "DELETE" });
  clearHistory(sessionId);
  if (state.selectedSessionId === sessionId) {
    state.selectedSessionId = null;
    localStorage.removeItem(storageKeys.selectedSession);
  }
  await refreshSessions({ preserveSelection: false });
  if (state.sessions.length > 0) {
    await selectSession(state.sessions[0].session.session.id, { scroll: false });
  } else {
    renderEmptyState();
  }
}

async function selectSession(sessionId, { scroll = true } = {}) {
  const response = await fetchJson(`/sessions/${sessionId}/view`);
  const session = response.session.session;
  const history = loadHistory(sessionId);

  state.selectedSessionId = sessionId;
  localStorage.setItem(storageKeys.selectedSession, sessionId);

  updateHeader(session, response.session.working_context);
  renderHistory(sessionId, history);
  renderSessions();

  if (history.length === 0) {
    appendSystemMessage(
      sessionId,
      `Session started: ${session.task_context}. Tell me something and I will remember it.`,
      { persist: false }
    );
  }

  if (window.innerWidth <= 768) {
    els.sidebar.classList.remove("open");
    els.sidebarOverlay.classList.remove("show");
  }
  if (scroll) scrollMessagesToBottom();
}

function updateHeader(session, workingContext) {
  els.sessionTitle.textContent = session.task_context || "Untitled";
  els.sessionSubtitle.textContent = session.current_expectation || "No expectation set";
  els.emptyState.classList.remove("show");
  els.emptyState.classList.add("hidden");

  els.headerRight.replaceChildren();
  if (workingContext) {
    const chip = document.createElement("span");
    chip.className = "badge accepted";
    chip.textContent = workingContext.task_id;
    els.headerRight.appendChild(chip);
  }
}

function renderEmptyState() {
  els.sessionTitle.textContent = "No session selected";
  els.sessionSubtitle.textContent = "Start a new chat to begin";
  els.headerRight.replaceChildren();
  els.messages.replaceChildren();
  els.emptyState.classList.remove("hidden");
  els.emptyState.classList.add("show");
}

function renderHistory(sessionId, history) {
  els.messages.replaceChildren();
  const isEmpty = history.length === 0;
  els.emptyState.classList.toggle("show", isEmpty);
  els.emptyState.classList.toggle("hidden", !isEmpty);
  for (const entry of history) {
    els.messages.appendChild(renderMessage(sessionId, entry));
  }
}

async function onSendMessage(event) {
  event.preventDefault();
  const message = els.chatInput.value.trim();
  if (!message) return;

  if (!state.selectedSessionId) {
    await createSession({ scroll: false });
  }
  const sessionId = state.selectedSessionId;

  els.chatInput.value = "";
  autoResize(els.chatInput);

  appendMessage(sessionId, { role: "user", content: message, label: "You" });

  const pendingId = `pending-${Date.now()}`;
  appendMessage(sessionId, {
    id: pendingId, role: "assistant", content: "Thinking…",
    label: "EngramAgent", pending: true,
  });
  scrollMessagesToBottom();

  try {
    const chatResponse = await fetchJson(`/sessions/${sessionId}/chat`, {
      method: "POST",
      body: { message, retrieval_enabled: state.retrievalEnabled, debug: state.debugMode },
    });

    state.selectedSessionId = chatResponse.session.session.id;
    localStorage.setItem(storageKeys.selectedSession, state.selectedSessionId);
    updateHeader(chatResponse.session.session, chatResponse.session.working_context);

    replacePendingMessage(sessionId, pendingId, {
      role: "assistant",
      content: chatResponse.reply,
      label: "EngramAgent",
      trace: chatResponse.retrieval_enabled ? buildTracePayload(chatResponse) : null,
      traceEnabled: chatResponse.retrieval_enabled,
      ingestion: chatResponse.ingestion,
      debug: chatResponse.debug || null,
    });

    await refreshSessions({ preserveSelection: true });
    renderSessions();
  } catch (error) {
    replacePendingMessage(sessionId, pendingId, {
      role: "assistant",
      content: `Request failed: ${error.message || String(error)}`,
      label: "System", error: true,
    });
  } finally {
    scrollMessagesToBottom();
  }
}

async function deepSleep() {
  if (els.deepSleepButton.disabled) return;
  els.deepSleepButton.disabled = true;
  els.deepSleepInline.style.display = "inline-flex";
  els.deepSleepInlineText.textContent = "Consolidating...";

  try {
    const result = await fetchJson("/consolidate", { method: "POST", body: { debug: state.debugMode } });
    els.deepSleepInlineText.textContent = `Done · ${result.created_schemas} schema(s)`;
    
    if (state.selectedSessionId) {
      appendSystemMessage(
        state.selectedSessionId,
        `Deep Sleep complete. ${result.created_schemas} new schema(s) extracted.`,
        { persist: false }
      );
    }
  } catch (error) {
    els.deepSleepInlineText.textContent = "Failed";
  } finally {
    els.deepSleepButton.disabled = false;
    setTimeout(() => {
      els.deepSleepInline.style.display = "none";
    }, 3000);
  }
}

const demoScript = [
  { message: "I work as a frontend developer. I prefer React with TypeScript and functional components.", retrieval: false },
  { message: "For state management I use Zustand. I like minimal tooling without boilerplate.", retrieval: false },
  { message: "I prefer dark themes with purple or green accent colors.", retrieval: false },
  { message: "What technologies and design preferences do I have?", retrieval: true },
  { message: "I also enjoy Rust for backend work because the type system prevents bugs at compile time.", retrieval: false },
  { message: "What kind of projects would I enjoy building based on everything I have told you?", retrieval: true },
];

async function runDemo() {
  if (state.demoRunning) return;
  state.demoRunning = true;
  if (els.runDemoEmpty) els.runDemoEmpty.disabled = true;

  try {
    const response = await createSession({
      expectation: "remember my technical and design preferences",
      taskContext: "developer preferences",
      mode: "Exploration",
      scroll: true,
    });
    const sessionId = response.session.session.id;

    if (!state.retrievalEnabled) toggleRetrieval();
    await sleep(400);

    for (const step of demoScript) {
      appendMessage(sessionId, { role: "user", content: step.message, label: "You" });
      const pendingId = `pending-${Date.now()}`;
      appendMessage(sessionId, {
        id: pendingId, role: "assistant", content: "Retrieving memory…",
        label: "EngramAgent", pending: true,
      });
      scrollMessagesToBottom();
      await sleep(800);

      const chatResponse = await fetchJson(`/sessions/${sessionId}/chat`, {
        method: "POST",
        body: { message: step.message, retrieval_enabled: step.retrieval, debug: state.debugMode },
      });

      replacePendingMessage(sessionId, pendingId, {
        role: "assistant",
        content: chatResponse.reply,
        label: "EngramAgent",
        trace: chatResponse.retrieval_enabled ? buildTracePayload(chatResponse) : null,
        traceEnabled: chatResponse.retrieval_enabled,
        ingestion: chatResponse.ingestion,
        debug: chatResponse.debug || null,
      });
      await refreshSessions({ preserveSelection: true });
      renderSessions();
      scrollMessagesToBottom();
      await sleep(1200);
    }

    appendSystemMessage(
      sessionId,
      "Demo complete. Try Deep Sleep to consolidate these memories into schemas.",
      { persist: false }
    );
  } catch (error) {
    console.error("Demo failed:", error);
  } finally {
    state.demoRunning = false;
    if (els.runDemoEmpty) els.runDemoEmpty.disabled = false;
  }
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

function showDeleteConfirmation(sessionId, sessionTitle) {
  let modal = document.getElementById("delete-confirm-modal");
  if (modal) modal.remove();
  
  modal = document.createElement("div");
  modal.id = "delete-confirm-modal";
  modal.className = "delete-modal show";
  modal.innerHTML = `
    <div class="delete-modal-content">
      <div class="delete-modal-title">Delete Session</div>
      <div class="delete-modal-body">
        Are you sure you want to permanently delete <strong>"${sessionTitle}"</strong>? This will remove all memories and conversations from this session permanently. This cannot be undone.
      </div>
      <div class="delete-modal-actions">
        <button class="delete-modal-btn delete-modal-btn-cancel" id="delete-cancel-btn">Cancel</button>
        <button class="delete-modal-btn delete-modal-btn-danger" id="delete-confirm-btn">Delete</button>
      </div>
    </div>
  `;
  document.body.appendChild(modal);
  
  document.getElementById("delete-cancel-btn").addEventListener("click", () => {
    modal.remove();
  });
  document.getElementById("delete-confirm-btn").addEventListener("click", () => {
    confirmDeleteSession(sessionId);
  });
}

async function confirmDeleteSession(sessionId) {
  try {
    const modal = document.getElementById("delete-confirm-modal");
    if (modal) modal.remove();
    await deleteSession(sessionId);
  } catch (error) {
    console.error("Failed to delete session:", error);
  }
}

function appendMessage(sessionId, entry) {
  const history = loadHistory(sessionId);
  const next = history.concat([{ ...entry, time: new Date().toISOString() }]);
  persistHistory(sessionId, next);
  els.messages.appendChild(renderMessage(sessionId, next[next.length - 1]));
  els.emptyState.classList.remove("show");
  els.emptyState.classList.add("hidden");
  scrollMessagesToBottom();
}

function appendSystemMessage(sessionId, content, options = {}) {
  if (options.persist === false) {
    els.messages.appendChild(renderMessage(sessionId, {
      role: "system", content, label: "System", time: new Date().toISOString(),
    }));
    els.emptyState.classList.add("hidden");
    return;
  }
  appendMessage(sessionId, { role: "system", content, label: "System" });
}

function replacePendingMessage(sessionId, pendingId, replacement) {
  const history = loadHistory(sessionId);
  const index = history.findIndex((item) => item.id === pendingId);
  if (index >= 0) {
    history[index] = { ...history[index], ...replacement, time: new Date().toISOString(), id: pendingId };
    persistHistory(sessionId, history);
  }
  const existing = els.messages.querySelector(`[data-message-id="${pendingId}"]`);
  if (!existing) return;
  const rendered = renderMessage(sessionId, { ...replacement, id: pendingId, time: new Date().toISOString() });
  existing.replaceWith(rendered);
}

function renderMessage(_sessionId, entry) {
  const article = document.createElement("article");
  const kind = entry.role === "assistant" ? "assistant" : entry.role === "user" ? "user" : "system";
  article.className = `message ${kind}${entry.pending ? " pending" : ""}`;
  article.dataset.messageId = entry.id || `${kind}-${crypto.randomUUID()}`;

  const avatar = document.createElement("div");
  avatar.className = `avatar ${kind}`;
  avatar.textContent = kind === "user" ? "YOU" : kind === "assistant" ? "EA" : "SYS";

  const bubble = document.createElement("div");
  bubble.className = "bubble";

  const head = document.createElement("div");
  head.className = "bubble-head";
  const label = document.createElement("div");
  label.className = "bubble-label";
  label.textContent = entry.label || (kind === "user" ? "You" : kind === "assistant" ? "EngramAgent" : "System");
  const time = document.createElement("div");
  time.className = "bubble-time";
  time.textContent = formatTime(entry.time);

  if (kind === "assistant" && entry.ingestion && !entry.pending) {
    const badge = document.createElement("div");
    badge.className = `badge ${entry.ingestion.accepted ? "accepted" : "rejected"}`;
    badge.textContent = entry.ingestion.accepted
      ? `Stored · ${entry.ingestion.score.toFixed(2)}`
      : `Rejected · ${entry.ingestion.score.toFixed(2)}`;
    badge.title = entry.ingestion.accepted
      ? "This reply was accepted into episodic memory"
      : "This reply did not score high enough to be stored";
    head.append(label, badge, time);
  } else {
    head.append(label, time);
  }

  const body = document.createElement("div");
  body.className = "bubble-body";
  body.textContent = entry.content;

  bubble.append(head, body);

  if (entry.trace && entry.traceEnabled) {
    bubble.appendChild(renderTrace(entry.trace));
  }

  if (entry.debug) {
    bubble.appendChild(renderDebug(entry.debug));
  }

  if (kind === "user") article.append(bubble, avatar);
  else article.append(avatar, bubble);

  return article;
}

function renderTrace(trace) {
  const wrapper = document.createElement("div");
  wrapper.className = "trace";

  const details = document.createElement("details");
  details.open = false;

  const summary = document.createElement("summary");
  const memoryCount = trace.retrieval?.knowledge?.facts?.length || 0;
  const hasSchema = trace.schema ? 1 : 0;
  const accepted = trace.ingestion?.accepted ? "stored" : "not stored";
  summary.innerHTML = `<span>Memories used: ${memoryCount}${hasSchema ? " · schema" : ""} · ${accepted}</span>`;

  const body = document.createElement("div");
  body.className = "trace-body";

  if (trace.ingestion) {
    const ingest = document.createElement("div");
    ingest.className = "trace-section";
    ingest.innerHTML = `<h4>Ingestion</h4><div class="ingestion-detail">
      <div class="ingestion-row"><span class="ingestion-key">Status:</span> <span class="ingestion-value ${trace.ingestion.accepted ? "good" : "warn"}">${trace.ingestion.accepted ? "Accepted" : "Rejected"}</span></div>
      <div class="ingestion-row"><span class="ingestion-key">Score:</span> <span class="ingestion-value">${trace.ingestion.score?.toFixed(4)}</span></div>
    </div>`;
    body.appendChild(ingest);
  }

  const facts = trace.retrieval?.knowledge?.facts || [];
  if (facts.length > 0) {
    const factsSection = document.createElement("div");
    factsSection.className = "trace-section";
    const title = document.createElement("h4");
    title.textContent = "Retrieved memories";
    factsSection.appendChild(title);
    facts.forEach((fact) => {
      const div = document.createElement("div");
      div.className = "fact";
      div.textContent = fact;
      factsSection.appendChild(div);
    });
    body.appendChild(factsSection);
  }

  const inferences = trace.retrieval?.knowledge?.inferences || [];
  if (inferences.length > 0) {
    const infSection = document.createElement("div");
    infSection.className = "trace-section";
    infSection.innerHTML = `<h4>Inferences</h4><ul>${inferences.map(i => `<li>${i}</li>`).join("")}</ul>`;
    body.appendChild(infSection);
  }

  const gaps = trace.retrieval?.knowledge?.gaps || [];
  if (gaps.length > 0) {
    const gapSection = document.createElement("div");
    gapSection.className = "trace-section";
    gapSection.innerHTML = `<h4>Gaps</h4><ul>${gaps.map(g => `<li>${g}</li>`).join("")}</ul>`;
    body.appendChild(gapSection);
  }

  details.append(summary, body);
  wrapper.appendChild(details);
  return wrapper;
}

function renderDebug(debug) {
  const wrapper = document.createElement("div");
  wrapper.className = "debug-panel";

  const details = document.createElement("details");
  details.open = false;

  const summary = document.createElement("summary");
  summary.innerHTML = `<span>Debug</span>`;

  const body = document.createElement("div");
  body.className = "debug-body";

  if (debug.episode) {
    const ep = document.createElement("div");
    ep.className = "debug-section";
    ep.innerHTML = `<h4>Episode</h4>
      <div class="debug-row"><span class="debug-key">Action:</span> <span class="debug-value">${escapeHtml(debug.episode.action)}</span></div>
      <div class="debug-row"><span class="debug-key">Context:</span> <span class="debug-value">${escapeHtml(debug.episode.context)}</span></div>
      <div class="debug-row"><span class="debug-key">Outcome:</span> <span class="debug-value">${escapeHtml(debug.episode.outcome)}</span></div>`;
    body.appendChild(ep);
  }

  if (debug.ingestion) {
    const ing = document.createElement("div");
    ing.className = "debug-section";
    ing.innerHTML = `<h4>Ingestion</h4>
      <div class="debug-row"><span class="debug-key">Accepted:</span> <span class="debug-value ${debug.ingestion.accepted ? "good" : "warn"}">${debug.ingestion.accepted}</span></div>
      <div class="debug-row"><span class="debug-key">Score:</span> <span class="debug-value mono">${debug.ingestion.score?.toFixed(4)}</span></div>
      <div class="debug-row"><span class="debug-key">Pattern:</span> <span class="debug-value mono">${escapeHtml(debug.ingestion.pattern_hash || "—")}</span></div>
      <div class="debug-row"><span class="debug-key">Engram:</span> <span class="debug-value mono">${escapeHtml(debug.ingestion.engram_id || "—")}</span></div>`;
    body.appendChild(ing);
  }

  if (debug.retrieval) {
    const ret = document.createElement("div");
    ret.className = "debug-section";
    ret.innerHTML = `<h4>Retrieval</h4>
      <div class="debug-row"><span class="debug-key">Mode:</span> <span class="debug-value">${escapeHtml(debug.retrieval.mode)}</span></div>
      <div class="debug-row"><span class="debug-key">Candidates:</span> <span class="debug-value mono">${debug.retrieval.candidate_count}</span></div>
      <div class="debug-row"><span class="debug-key">Schema matched:</span> <span class="debug-value">${debug.retrieval.schema_matched}</span></div>
      <div class="debug-row"><span class="debug-key">Facts:</span> <span class="debug-value mono">${debug.retrieval.facts_count}</span></div>
      <div class="debug-row"><span class="debug-key">Inferences:</span> <span class="debug-value mono">${debug.retrieval.inferences_count}</span></div>
      <div class="debug-row"><span class="debug-key">Gaps:</span> <span class="debug-value mono">${debug.retrieval.gaps_count}</span></div>`;
    body.appendChild(ret);
  }

  details.append(summary, body);
  wrapper.appendChild(details);
  return wrapper;
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

function buildTracePayload(response) {
  return {
    schema: response.retrieval?.schema ? {
      id: response.retrieval.schema.id,
      tags: response.retrieval.schema.tags,
      prediction_fields: response.retrieval.schema.prediction_fields,
    } : null,
    retrieval: {
      knowledge: response.retrieval?.knowledge,
    },
    ingestion: response.ingestion,
  };
}

function persistHistory(sessionId, history) {
  localStorage.setItem(storageKeys.historyPrefix + sessionId, JSON.stringify(history));
}

function loadHistory(sessionId) {
  try { return JSON.parse(localStorage.getItem(storageKeys.historyPrefix + sessionId) || "[]"); }
  catch { return []; }
}

function clearHistory(sessionId) {
  localStorage.removeItem(storageKeys.historyPrefix + sessionId);
}

function formatTime(iso) {
  if (!iso) return "";
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function autoResize(textarea) {
  textarea.style.height = "auto";
  textarea.style.height = `${Math.min(textarea.scrollHeight, 200)}px`;
}

function scrollMessagesToBottom() {
  els.messages.scrollTop = els.messages.scrollHeight;
}

async function fetchJson(path, options = {}) {
  const response = await fetch(new URL(path, state.apiBase), {
    method: options.method || "GET",
    headers: { "Content-Type": "application/json", ...(options.headers || {}) },
    body: options.body ? JSON.stringify(options.body) : undefined,
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || response.statusText);
  }
  return response.json();
}
