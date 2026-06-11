# Artificial Engram

A neuroscience-inspired memory architecture for AI agents, built in Rust.

## Overview

<<<<<<< HEAD
Artificial Engram implements an **Engram-Based Memory System** — a biologically grounded architecture that enables AI agents to learn from experience, recognize patterns, abstract knowledge, and actively forget what is irrelevant. Instead of treating every interaction as new, the agent builds persistent memory structures that evolve over time.

<img width="1331" height="1181" alt="image" src="https://github.com/user-attachments/assets/c99c4e5c-a664-4930-a8b4-4d5e9116026f" />
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
| `web/` | React/Vite control panel |

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

# 3. Open the control panel
http://127.0.0.1:3001
```

## Containers

The project uses a multi-stage Docker build orchestrated by `docker-compose.yml`. The following containers are created:

| Container | Base Image | Purpose |
|---|---|---|
| **web-builder** (stage) | `node:22-bookworm` | Builds the React control panel into `web/dist`. |
| **builder** (stage) | `rust:bookworm` | Compiles the Rust `engram-server` binary in release mode. Copies `Cargo.toml`, `Cargo.lock`, `crates/`, and the built dashboard into the image. |
| **engram** (runtime) | `debian:bookworm-slim` | Runs the compiled `engram-server` binary and serves the built dashboard. Exposes port `3001` for the REST, dashboard, and MCP endpoints. Runs as non-root user `engram` (UID 10001). |
| **neo4j** | `neo4j:5.26-community` | Primary graph backend for sessions, episodes, patterns, engrams, schemas, and runtime config. |

### Volumes

| Volume | Mount Path | Purpose |
|---|---|---|
| `engram-data` | `/data` | Persistent storage for memory stores (engram dictionary, sessions, schemas). Survives container restarts and rebuilds. |
| `neo4j-data` | `/data` | Persistent Neo4j graph storage. |
| `neo4j-logs` | `/logs` | Neo4j logs. |

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `ENGRAM_SERVER_ADDR` | `0.0.0.0:3001` | Address and port the server binds to |
| `ENGRAM_DATA_DIR` | `/data` | Directory for persistent memory data |
| `ENGRAM_NEO4J_URI` | `http://neo4j:7474` | Neo4j HTTP transactional endpoint |
| `ENGRAM_NEO4J_USER` | `neo4j` | Neo4j username |
| `ENGRAM_NEO4J_PASSWORD` | `engram-memory` | Neo4j password |
| `ENGRAM_NEO4J_DATABASE` | `neo4j` | Neo4j database name |
| `ENGRAM_API_TOKEN` | *(none)* | Optional bearer token for API access |
| `ENGRAM_ALLOWED_ORIGINS` | `*` | CORS allowlist for the control panel and remote clients |
| `ENGRAM_MCP_HTTP_ENABLED` | `true` | Enables the MCP HTTP endpoint at `/mcp` |
| `ENGRAM_MCP_STDIO_ENABLED` | `true` | Enables `engram-server mcp-stdio` |
| `ENGRAM_DASHSCOPE_API_KEY` | *(none)* | API key for Qwen LLM integration (set in `.env`) |

## API

The server exposes a REST + WebSocket API on port `3001`. Key endpoints:

- `POST /sessions` — Open a new session
- `POST /sessions/{id}/chat` — Send a chat message (with optional memory retrieval)
- `POST /sessions/{id}/retrieve` — Manual memory retrieval
- `POST /sessions/{id}/episodes` — Process an episode manually
- `POST /consolidate` — Trigger the nightly consolidation pipeline
- `GET /sessions/{id}/ws` — WebSocket chat stream
- `GET /control/overview` — Dashboard health and runtime overview
- `GET /control/graph` — Graph projection for the memory map
- `GET /control/config` — Active runtime tuning profile
- `PUT /control/config` — Update tunable behavior values
- `POST /control/simulate/thalamus` — Preview thalamus scoring
- `GET /memory/episodes`, `GET /memory/patterns`, `GET /memory/engrams`, `GET /memory/schemas` — Memory inspection APIs
- `POST /mcp` — MCP HTTP endpoint

Run `engram-server mcp-stdio` for stdio-based MCP clients.

See `WEB_UI_API_CONTRACT.md` for the full API specification.

## Documentation

- [Agent Memory Architecture](docs/agent-memory.md) — Full 13-chapter design document
- [Memory Structures](docs/memory-structures.md) — Core data structures and their interactions
- [Episode to Schema Flow](docs/episode-to-schema.md) — How experiences become abstract knowledge

## License

MIT
