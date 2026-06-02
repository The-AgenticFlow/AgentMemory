# Artificial Engram

A neuroscience-inspired memory architecture for AI agents, built in Rust.

## Overview

Artifical Engram implements an **Engram-Based Memory System** — a biologically grounded architecture that enables AI agents to learn from experience, recognize patterns, abstract knowledge, and actively forget what is irrelevant. Instead of treating every interaction as new, the agent builds persistent memory structures that evolve over time.

<img width="1331" height="1181" alt="image" src="https://github.com/user-attachments/assets/c99c4e8c-a664-4930-a8b4-4d5e9116026f" />

## Core Concepts

The system follows one guiding principle: **What would the brain do?**

| Concept | Biological Inspiration | Role in the System |
|---|---|---|
| **Thalamus Filter** | Sensory gating | Scores incoming episodes for relevance, surprise, novelty, and emotional valence |
| **Pre-Engram Buffer** | Hippocampal short-term storage | Accumulates weak signals and holds episodes before consolidation |
| **Engram Dictionary** | Long-term memory engrams | Stable memory indices with embeddings, tags, and strength tracking |
| **Meta-Engrams / Schemas** | Neocortical abstraction | Compressed patterns extracted from repeated engrams |
| **Nightly Consolidation** | Sleep-based memory processing | Strengthens relevant memories, decays weak ones, creates schemas |
| **Working Context** | Prefrontal cortex state | Live task workspace with active goals and loaded memories |

## Architecture Flow

```
Episode → Thalamus Filter → Pre-Engram Buffer → Pattern Separation → Engram Dictionary
                                                                    ↓
Nightly Consolidation → Schema Creation → Retrieval → Working Context
```

## Project Structure

| Crate | Purpose |
|---|---|
| `engram-core` | Data models: Episode, Session, PatternEntry, EngramEntry, MetaEngram |
| `engram-runtime` | Memory engine: flows, consolidation, embeddings, plasticity, STC |
| `engram-store` | Persistent storage layer |
| `engram-qwen` | LLM integration for consolidation and reasoning |
| `engram-server` | HTTP/WebSocket API server (Axum) |
| `engram-cli` | Command-line interface |
| `web/` | Frontend application |

## Key Features

- **Complementary Learning Systems** — Fast hippocampal buffer + slow neocortical schema storage prevents catastrophic interference
- **Pattern Separation & Completion** — Recognizes new vs. known patterns, creates or updates engrams accordingly
- **Synaptic Tagging & Capture** — Temporal association links related episodes for later consolidation
- **Constructive Retrieval** — Reassembles context-aware answers from facts, inferences, and identified gaps
- **Kinship Links** — Associative recall between related engrams for richer context
- **Session-Based Memory** — Each conversation session maintains its own expectation state and mode (Exploration, Routine, Critical)

## Quick Start

```bash
# 1. Configure environment
cp .env.example .env
# Add ENGRAM_DASHSCOPE_API_KEY for Qwen LLM integration

# 2. Run with Docker
docker compose up --build

# 3. Open the web UI
http://127.0.0.1:3000
```

The compose setup runs the Rust server and frontend in one container with a persistent `/data` volume for memory stores.

## API

The server exposes a REST + WebSocket API on port `3001`. Key endpoints:

- `POST /sessions` — Open a new session
- `POST /sessions/{id}/chat` — Send a chat message (with optional memory retrieval)
- `POST /sessions/{id}/retrieve` — Manual memory retrieval
- `POST /sessions/{id}/episodes` — Process an episode manually
- `POST /consolidate` — Trigger the nightly consolidation pipeline
- `GET /sessions/{id}/ws` — WebSocket chat stream

See `WEB_UI_API_CONTRACT.md` for the full API specification.

## Documentation

- [Agent Memory Architecture](docs/agent-memory.md) — Full 13-chapter design document
- [Memory Structures](docs/memory-structures.md) — Core data structures and their interactions
- [Episode to Schema Flow](docs/episode-to-schema.md) — How experiences become abstract knowledge

## License

MIT
