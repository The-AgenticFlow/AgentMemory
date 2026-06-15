name: agent-memory
description: Use when interacting with the Agent Memory (Engram) system via MCP tools, sessions, episodes, retrieval, or consolidation. Covers all memory_create_bank, memory_open_session, memory_retain, memory_recall, memory_reflect, memory_get_working_context, memory_update_working_context operations, and the thalamus intake scoring model.
---

# Agent Memory (Engram) — MCP Usage Guide

This skill covers the hierarchical memory system available via MCP at `/mcp` (Streamable HTTP) or stdio. Use these tools whenever you need persistent episodic memory, retrieval-augmented reasoning, schema consolidation, or behavioral tuning.

## Quick Start Flow

1. **Create a bank** (once per project/agent)
2. **Open a session** (per task)
3. **Retain episodes** as you work
4. **Recall memories** when you need context
5. **Reflect/consolidate** when done with a task phase
6. **Close the session** when finished

---

## 1. Memory Banks

Banks are top-level memory containers with hierarchical relationships.

### `memory_create_bank`

Creates an isolated memory container. Banks define what knowledge to prioritize.

**Parameters:**
| Param | Type | Required | Notes |
|-------|------|----------|-------|
| `name` | string | Yes | Short identifier, e.g. `refactor-auth` |
| `type` | string | Yes | `"session"`, `"dictionary"`, or `"shared"` |
| `mission` | string | No | Natural-language directive for what to prioritize |
| `directives` | string[] | No | Hard rules, one per entry |
| `parent_bank_id` | uuid | No | Links to an existing bank for schema propagation |

```
memory_create_bank: {
  "name": "my-project-dictionary",
  "type": "dictionary",
  "mission": "Remember architectural decisions and patterns for this project",
  "directives": ["Prefer composition over inheritance", "Always log errors with context"]
}
```

### `memory_get_bank`

Look up an existing bank by name (case-insensitive) or UUID.

**Parameters:**
| Param | Type | Required | Notes |
|-------|------|----------|-------|
| `bank_id` | uuid | No* | Exact UUID lookup |
| `name` | string | No* | Case-insensitive name match |

*At least one of `bank_id` or `name` is required.

```
memory_get_bank: {
  "name": "my-project-dictionary"
}
```

**Bank types:**
- **`session`** — Short-lived, per-task episodic memory. Transient.
- **`dictionary`** — Per-agent persistent knowledge base. The default for most use cases.
- **`shared`** — Cross-agent shared schemas and patterns.

### `memory_get_config` / `memory_update_config`

Read and modify the active tuning profile (thalamus weights, buffer thresholds, retrieval settings).

Config fields are grouped into: **Thalamus Intake**, **Mode Thresholds**, **Buffer**, **Pattern Resolution**, **Retrieval**, and **Consolidation**.

---

## 2. Sessions

Sessions are active task frames. Every episode, retrieval, and working context operation happens inside a session.

### `memory_open_session`

Opens a new session bound to a bank (optional). Returns a `session_id` you must reuse for all subsequent operations.

**Parameters:**
| Param | Type | Required | Notes |
|-------|------|----------|-------|
| `expectation` | string | No | What you expect to happen. Default: `"remember useful context"` |
| `mode` | string | No | `"Exploration"`, `"Routine"`, `"Critical"`, `"Analogy"`, `"Validation"` |
| `task_context` | string | No | Human-readable task description |
| `bank_id` | uuid | No | Target a specific memory bank |

```
memory_open_session: {
  "expectation": "Refactor the auth module without breaking JWT flow",
  "mode": "Critical",
  "task_context": "Refactor authentication middleware to use new token format",
  "bank_id": "uuid-from-create-bank"
}
```

**Session modes and their thalamus thresholds:**

| Mode | Threshold | When to Use | Intake Behavior |
|------|-----------|-------------|----------------|
| `Exploration` | 0.35 | Open-ended research, discovery | Accepts broadly, filters lightly |
| `Routine` | 0.55 | Repetitive tasks, standard patterns | Only keeps meaningful deviations |
| `Critical` | 0.0 | High-stakes, nothing should be lost | Accepts everything — no filtering |
| `Analogy` | 0.30 | Cross-domain structural mapping | Looks for structural similarity |
| `Validation` | 0.60 | Evidence-based reasoning | Only accepts strongly supported episodes |

### `memory_close_session`

Closes a session and persists its final state. Call this when the task is done.

```
memory_close_session: {
  "session_id": "uuid-from-open-session"
}
```

---

## 3. Working Context

Working context is short-term scratchpad memory scoped to the active session — think of it as a transient blackboard.

### `memory_get_working_context`

Returns the current working state: goal stack, active engrams, episodic buffer, and inference layer.

```
memory_get_working_context: {
  "session_id": "uuid"
}
```

### `memory_update_working_context`

Sets or replaces the working context with a new task frame.

```
memory_update_working_context: {
  "session_id": "uuid",
  "task_id": "implement-user-preferences"
}
```

---

## 4. Episodes — `memory_retain`

An episode is one action/context/outcome triplet. It is the atomic unit of memory ingestion.

### `memory_retain`

Scores and stores an episode. The thalamus filter evaluates it against the session's expectation, mode, and recent engrams before deciding whether to retain it.

**Thalamus scoring dimensions:**
- **Novelty** (0-1): How different this is from recent engrams
- **Surprise** (0-1): How much the outcome diverged from expectation
- **Relevance** (0-1): How well the context overlaps with the task
- **Valence** (0-1): Emotional/qualitative tone of the outcome

**Composite score** = novelty×w₁ + surprise×w₂ + relevance×w₃ + valence×w₄

If the composite score ≥ the mode's threshold, the episode is accepted into the episodic buffer. Accepted episodes may later promote to engrams, and recurring patterns become schemas.

```
memory_retain: {
  "session_id": "uuid",
  "action": "Replaced bcrypt with Argon2id in auth middleware",
  "context": "Upgrading password hashing to meet OWASP 2024 guidelines",
  "outcome": "All auth tests pass, migration script created for existing hashes"
}
```

### Writing good episodes:

1. **`action`** — What you did. Be specific: function names, file paths, decisions made.
2. **`context`** — Why you did it. Include the surrounding situation and constraints.
3. **`outcome`** — What happened. Results, errors, surprises, or confirmations.

Good episode:
```
action: "Added memoization to calculateTotal in src/services/cart.ts"
context: "Cart was re-rendering total on every keystroke, causing 200ms lag"
outcome: "Render time dropped to <16ms, all unit tests pass"
```

Bad episode:
```
action: "Fixed cart"
context: "It was slow"
outcome: "It works now"
```

---

## 5. Retrieval — `memory_recall`

Queries the memory system for relevant past episodes, engrams, and schemas.

```
memory_recall: {
  "session_id": "uuid",
  "query": "How did we handle password hashing migration?"
}
```

Returns a `RetrievalOutcome` with:
- **`knowledge.facts`** — Retrieved factual statements
- **`knowledge.inferences`** — Derived conclusions from past patterns
- **`knowledge.gaps`** — Missing pieces the system identified

Use retrieved facts as numbered evidence when answering questions. If memory is empty or gaps dominate, state that clearly rather than inventing answers.

---

## 6. Consolidation — `memory_reflect`

Runs pattern recognition and schema generation across all retained episodes. This compresses repeated patterns into reusable schemas and archives weak engrams.

```
memory_reflect: {}
```

Returns created schemas. Call this at natural breakpoints — after completing a feature, fixing a class of bugs, or before closing a session.

---

## Resources (read-only data endpoints)

| URI | Description |
|-----|-------------|
| `engram://overview` | Full system snapshot: counts, config, latest scores |
| `engram://graph` | Memory graph projection with nodes and edges |
| `engram://sessions/{id}` | Session detail with working context |
| `engram://engrams/{id}` | Individual engram data |
| `engram://schemas/{id}` | Generated schema details |

## Prompts (behavioral templates)

| Prompt | Purpose |
|--------|---------|
| `memory-grounded-answer` | Answer using only retrieved Agent Memory evidence |
| `session-summary` | Summarize active session state and gaps |
| `consolidation-review` | Review new schemas and archival decisions |

---

## Typical Workflow

```
1. memory_create_bank → get bank_id
2. memory_open_session(bank_id, mode, expectation) → get session_id
3. memory_update_working_context(session_id, task_id)
4. ... work loop ...
   - memory_retain(session_id, action, context, outcome)
   - memory_recall(session_id, query)  ← when you need past context
5. memory_reflect()  ← when a phase completes
6. memory_close_session(session_id)
```

## Important Notes

- **Always pass `session_id`** to `memory_retain`, `memory_recall`, and `memory_update_working_context`. Without it, these tools cannot route to the correct memory frame.
- **`memory_open_session` returns the session object** — extract the `id` field from the response and store it for later calls.
- **Use `Critical` mode** when you need to ensure nothing is silently dropped by the thalamus filter.
- **Bank hierarchy matters**: child banks inherit schema knowledge from parents. Create session banks under dictionary banks for proper propagation.
- **The thalamus filter is active by default**: episodes below the mode threshold are silently discarded. If you're losing memories you care about, either switch to `Critical` mode or adjust thresholds via `memory_update_config`.
