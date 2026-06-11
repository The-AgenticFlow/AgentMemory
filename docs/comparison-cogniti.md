# AgentMemory vs Cogniti: Architectural Comparison

A comparison of two neuroscience-inspired agent memory systems.

---

## Overview

| Aspect | AgentMemory | Cogniti |
|--------|-------------|---------|
| **Language** | Rust | Python (FastAPI) |
| **Foundation** | Built from scratch | Extends Hindsight (Vectorize.io) |
| **License** | MIT | MIT |
| **Core Model** | Episode → Engram → Schema | Fact → Observation→ Mental Model |

---

## Shared Foundations

Both systems are built on **Complementary Learning Systems (CLS) theory**and share:

- **Engram-based memory** — Memories have strength, context, and activation history
- **Two-system architecture** — Fast hippocampal buffer + slow neocortical consolidation
- **Active forgetting** — Decay mechanisms, not just storage
- **Schema emergence** — Patterns compressed from repeated experiences
- **Session modes** — Different retrieval thresholds based on task context

---

## Key Architectural Differences

### 1. Memory Hierarchy

**AgentMemory:**
```
Episode → PatternEntry (Pre-Engram Buffer) → EngramEntry → MetaEngram(Buffer)→ Neo4j schemas
```

**Cogniti:**
```
Working Memory (pre-C1) → Buffer (post-C1) → Neocortex (Postgres + Qdrant)
```

Both systems separate fragile early memories from stable long-term ones.

### 2. Storage Architecture

**AgentMemory:**
| Component | Storage |
|-----------|----------|
| Everything | Neo4j + Qdrant |
| Sessions, Episodes, Patterns, Engrams, Schemas | Allin Neo4j |

**Cogniti:**
| Tier | What Lives Here | Storage |
|------|------------------|---------|
| Working Memory | Fresh engrams (pre-C1) | PostgreSQL |
| Buffer/Hippocampus | Important engrams (post-C1) | PostgreSQL + Qdrant |
| Neocortex | Schemas only | Neo4j + Qdrant |

**Advantage Cogniti:** Cleaner separation — Neo4j stores ONLY schemas (neocortex abstraction), not individual engrams.

### 3. Session Modes

**AgentMemory (3 modes):**
| Mode | Behavior |
|------|----------|
| Exploration | Low threshold, accept more input |
| Routine | High threshold, only strong signals |
| Critical | Zero threshold, store everything |

**Cogniti (4 modes):**
| Mode | Strength Filter | Boost | Traversal | Construction |
|------|------------------|-------|-----------|--------------|
| Precision | ≥0.5 | Task-relevance | Shallow | Conservative |
| Exploration | ≥0.1 | Novelty | Deep | Creative |
| Analogy | ≥0.3 | (none) | Medium | Cross-domain |
| Validation |≥0.3 | Surprise | Medium | Evidence-based |

**Advantage Cogniti:** Explicit "Analogy" mode for cross-domain knowledge transfer and per-mode behavior table.

### 4. Operations API

**AgentMemory:**
```
POST /sessions                → Open session
POST /sessions/{id}/episodes  → Process episode
POST /sessions/{id}/retrieve   → Manual retrieval
POST /consolidate             → Nightly run
```

**Cogniti:**
```
retain() → Store information
recall() → Retrieve memories
reflect() → Deep analysis + reconsolidation
```

**Advantage Cogniti:** Simpler, more memorable API names; "reflect" gives clear entry point for schema evolution.

### 5. Retrieval Strategy

**AgentMemory:**
- Semantic similarity via Qdrant
- Kinship links for associative recall
- Schema activation to narrow search
- Mode-aware thresholds

**Cogniti (TEMPR):**
- **T**emporal — Time-based queries ("last spring")
- **E**xact (BM25) — Keyword/term matching
- **M**ulti-strategy — Parallel execution
- **P**ath (Graph) — Entity relationships
- **R**erank — Cross-encoder re-ranking

**Advantage Cogniti:** Multi-strategy retrieval runs allfour in parallel and merges via reciprocal rank fusion.

### 6. Multi-Agent Support

**AgentMemory:** Single-agent focused

**Cogniti:**
```
Agent Session → Agent Dictionary → Shared Memory
```

**Advantage Cogniti:** Explicit multi-bank architecture for sharing learned schemas across agents.

---

## What Cogniti Inherits from Hindsight

Cognitiis built on Hindsight (Vectorize.io) and inherits:

| Feature | Description |
|---------|-------------|
| Core API | `retain()`, `recall()`, `reflect()` operations |
| Memory Types | Mental Models, Observations, World Facts, Experience Facts |
| Auto-consolidation | Facts → Observations synthesis |
| Mission/Directives | Bank-level reasoning configuration |
| LLM Wrapper | Drop-in wrapper for any LLM client that auto-manages memory |
| Client SDKs | Python, TypeScript, Rust |

---

## What AgentMemory Does Better

1. **Rust performance** — Native speed, memory safety, no GC pauses

2. **Deeper neuroscience alignment** —18-chapter conceptual document mapping every component to brain mechanisms

3. **Synaptic Tagging & Capture (STC)** — Temporal association mechanism for linking related episodes

4. **Constructive retrieval** — Output explicitly includes `{facts, inferences, gaps}`

5. **Binary artifacts** — Single compiled binary for deployment

6. **Pattern accumulation** — Pre-Engram Buffer tracks repeated weak signals before committing

---

## What Cogniti Does Better

1. **Working Memory tier** — Explicit pre-C1 stage for fragile fresh engrams

2. **Schema-only Neo4j** — Cleaner biological mapping (neocortex = schemas only)

3. **Multi-bank architecture** — Agent-isolated memory with shared schemas

4. **Analogy mode** — Dedicated cross-domain retrieval mode

5. **TEMPR retrieval** — Four-strategy parallel search

6. **Named operations** — Clearer API surface (`retain`, `recall`, `reflect`)

7. **LLM integration** — Drop-in wrapper for existing agents

8. **Project documentation** — Structured backlog/epics/milestones

---

## Adoption Opportunities for AgentMemory

### HighPriority

1. **Add "Analogy" session mode** — For structural similarity and cross-domain transfer

2. **Working Memory tier** — Postgres table for fresh engrams (pre-C1) before buffer promotion

3. **Schema-only Neo4j** — Move engrams to Postgres, keep only schemas in Neo4j

4. **Multi-strategy retrieval** — Add BM25 and temporal retrieval alongside semantic

### Medium Priority

5. **Rename operations** — Consider clearer API names (retain/recall/reflect)

6. **Per-mode behavior table** — Document thresholds, boosts, traversal depthexplicitly

7. **Multi-bank architecture** — Support shared schemas across agents

### Low Priority

8. **Mission/Directives configuration** — Bank-level reasoning hints

9. **LLM wrapper pattern** — Drop-in for existing agents

---

## Summary

| System | Strength | Best For |
|--------|----------|----------|
| **AgentMemory** | Neuroscience precision, Rust performance, deep conceptual grounding | Teams wanting full control, deep alignment with brain architecture |
| **Cogniti** | Faster start, established foundation, multi-agent friendly | Teams wanting production-ready memory with less build effort |

Both are valid approaches to the same problem: **enabling AI agents to learn from experience without catastrophic interference**.

---

## References

- **AgentMemory:** `docs/agent-memory.md` (18 chapters of neuroscience grounding)
- **Cogniti README:** Based on comparison analysis
- **Hindsight:** https://github.com/vectorize-io/hindsight

---

*Last updated: 2026-06-11*
