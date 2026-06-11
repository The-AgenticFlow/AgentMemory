# Adaptive Configuration & Enhanced Control Plane

## Overview

Refactor hardcoded values into a configurable, adaptive system and redesign the web control plane inspired by Hindsight, while incorporating Cogniti's architectural advantages.

---

## Part 1: Adaptive Configuration System

### Problem

Currently 50+ hardcoded values are scattered across:
- `thalamus.rs`: Word lists for valence, magic coefficients
- `pattern.rs`: Similarity thresholds, strength formulas
- `retrieval.rs`: Spread factors, boost weights, truncation limits
- `plasticity.rs`: Multipliers, thresholds, window sizes
- `buffer.rs`: Threshold adjustments, strength formulas

These values are:
- Not documented or justified
- Not adaptive (same for all use cases)
- Not measurable (no performance feedback loop)
- Language-specific (English word lists)

### Solution: Layered Configuration

```
RuntimeConfig (current)→ Expanded to include all magic numbers
├── ThalamusConfig (expand)
├── PatternConfig (expand)
├── RetrievalConfig (expand)├── PlasticityConfig (new)
├── AdaptiveConfig (enhance)
└── TuningProfile (new)
```

### 1.1 Expand `ThalamusConfig`

```rust
pub struct ThalamusConfig {
    // Existing weights
    pub novelty_weight: f32,
    pub surprise_weight: f32,
    pub task_relevance_weight: f32,
    pub valence_weight: f32,
    
    // Thresholds (move from hardcoded)
    pub exploration_threshold: f32,
    pub routine_threshold: f32,
    pub critical_threshold: f32,
    // NEW: Analogy mode threshold
    pub analogy_threshold: f32,
    // NEW: Validation mode threshold
    pub validation_threshold: f32,
    
    // NEW: Semantic scoring (replaces word lists)
    pub use_semantic_valence: bool,
    pub valence_positive_anchors: Vec<String>,  // Embedding anchors
    pub valence_negative_anchors: Vec<String>,
    
    // NEW: Task relevance mode
    pub task_relevance_mode: TaskRelevanceMode,  // TokenOverlap | Semantic
}
```

### 1.2 Expand `PatternConfig`

```rust
pub struct PatternConfig {
    pub completion_threshold: f32,
    // NEW
    pub separation_search_candidates: usize,  // was hardcoded 3
    pub strength_merge_ratio: f32,             // was hardcoded 0.2
    pub kinship_link_enabled: bool,
    pub min_strength_for_kinship: f32,
}
```

### 1.3 Expand `RetrievalConfig`

```rust
pub struct RetrievalConfig {
    pub top_k: usize,
    
    // NEW: Mode-specific parameters
    pub spread_factors: ModeFloats,          // per-mode spread factors
    pub mode_bonuses: ModeFloats,            // per-mode similarity bonuses
    pub keyword_tag_weight: f32,             // was 0.08
    pub keyword_content_weight: f32,         // was 0.05
    pub schema_bonus_weight: f32,            // was 0.04
    pub max_content_length: usize,           // was 300
    
    // NEW: Multi-strategy retrieval (TEMPR from Cogniti)
    pub use_temporal_search: bool,
    pub use_bm25_search: bool,
    pub use_graph_traversal: bool,
    pub fusion_strategy: FusionStrategy,      // ReciprocalRank| WeightedSum
}
```

### 1.4 New `PlasticityConfig`

```rust
pub struct PlasticityConfig {
    pub surprise_multiplier: f32,            // was 0.45
    pub outcome_multiplier: f32,             // was 0.20
    pub stress_penalty: f32,                 // was 0.20
    pub reconsolidation_window_hours: i64,   // was 12
    pub max_strength_delta: f32,            // was 0.35
    
    // NEW: Make thresholds configurable
    pub high_plasticity_surprise_threshold: f32,  // was 0.7
    pub high_plasticity_valence_threshold: f32,    // was 0.75
    pub mode_multipliers: ModeFloats,              // was [1.0, 0.9, 1.1]decay_clamp_min: f32,            // was 0.01
    pub strength_clamp_min: f32,            // was 0.05
}
```

### 1.5 New `BufferConfig` Enhancements

```rust
pub struct BufferConfig {
    pub similarity_threshold: f32,
    pub promotion_threshold: f32,
    pub decay_rate: f32,
    
    // NEW: Strength formula coefficients
    pub strength_base_coefficient: f32,     // was 0.25
    pub strength_min_base: f32,              // was 0.05
    pub surprise_contribution: f32,         // was 0.08
    pub valence_contribution: f32,           // was 0.03
    pub threshold_sensitivity: f32,          // was 0.1
}
```

### 1.6 New `TuningProfile` System

```rust
pub enum TuningProfile {
    Conservative,  // High thresholds, low recall, high precision
    Balanced,      // Current defaults
    Exploratory,   // Low thresholds, high recall, lower precision
    Adaptive,      // Uses performance metrics to adjust
    Custom(RuntimeConfig),
}
```

---

## Part 2: Control Plane Redesign

### Current Issues

1. **Tuning tab is minimal** — Raw number inputs without context
2. **No documentation** — Users don't know what values mean
3. **No presets** — No way to save/load configurations
4. **No performance feedback** — Can't see if changes improved things
5. **No search/filter** — Can't find specific engrams/patterns easily

### New Control Plane Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│Agent Memory Control Plane                    │
│├─────────────────────────────────────────────────────────────────┤
││[Overview] [Graph] [Episodes] [Buffers] [Engrams] [Schemas]   │
││[Sessions] [Performance] [Tuning] [Experiments]               │
└─────────────────────────────────────────────────────────────────┘
```

### 2.1 Enhanced Overview Tab

Add:
- Memory health score (based on distribution of strengths)
- Retrieval success rate (tracked over sessions)
- Consolidation queue depth
- Schema coverage percentage

### 2.2 New Sessions Tab

Show active/closed sessions with:
- Session ID, mode, task context
- Episode count, engrams created
- Retrieval success rate for that session

### 2.3 New Performance Tab

```jsx
<PerformanceTab>
  <MetricCard title="Retrieval Precision" value={precision} trend="up" />
  <MetricCard title="Recall Coverage" value={recall} trend="stable" />
  <MetricCard title="Schema Utilization" value={coverage} trend="down" />
  <MetricCard title="Consolidation Rate" value={rate} trend="up" />
  
  <PerformanceGraph metric="retrieval_success" timeRange="7d" />
  
  <AdaptiveThresholds>
    <ThresholdAdjustment 
      metric="completion_threshold"
      currentValue={config.pattern.completion_threshold}
      suggestedValue={adaptive.suggested.completion_threshold}
      confidence={adaptive.confidence}
    />
  </AdaptiveThresholds>
</PerformanceTab>
```

### 2.4 Redesigned Tuning Tab

Replace raw numbers with contextual controls:

```jsx
<TuningTab>
  <TuningSection title="Intake Filtering">
    <SliderWithLabel
      label="Novelty Weight"
      value={config.thalamus.novelty_weight}
      range={[0, 1]}
      step={0.01}
      description="How much weight to give new/unfamiliar patterns"
      impact="Higher = more new experiences stored"
    />
    <SliderWithLabel
      label="Surprise Weight"
      value={config.thalamus.surprise_weight}
      range={[0, 1]}
      step={0.01}
      description="How much weight to give unexpected outcomes"
      impact="Higher = more surprising events stored"
    />
    <ModeThresholdEditor
      modes={[Exploration, Routine, Critical, Analogy, Validation]}
      thresholds={config.thalamus.mode_thresholds}
      description="Minimum score to accept an episode"
    />
  </TuningSection>
  
  <TuningSection title="Pattern Resolution">
    <SliderWithLabel
      label="Completion Threshold"
      value={config.pattern.completion_threshold}
      range={[0.5, 1.0]}
      step={0.01}
      description="Similarity needed to merge with existing engram"
      impact="Higher = more new engrams, fewer updates"
      visualization={<SimilarityHistogram data={patternSimilarities} />}
    />
  </TuningSection>
  
  <PresetsBar>
    <PresetButton profile="Conservative" onClick={() => applyPreset("conservative")} />
    <PresetButton profile="Balanced" onClick={() => applyPreset("balanced")} />
    <PresetButton profile="Exploratory" onClick={() => applyPreset("exploratory")} />
    <SavePresetButton />
    <LoadPresetButton />
  </PresetsBar>
  
  <DocumentationLink href="/docs/configuration">
    What do these values mean?
  </DocumentationLink>
</TuningTab>
```

### 2.5 New Experiments Tab

For A/B testing configurations:

```jsx
<ExperimentsTab>
  <ExperimentRunner>
    <Select options={savedConfigs} label="Baseline config" />
    <Select options={savedConfigs} label="Experimental config" />
    <Select options={sessions} label="Test sessions" />
    <Button>Run comparison</Button>
  </ExperimentRunner>
  
  <ResultsTable>
    <MetricRow name="Retrieval Precision" baseline={0.82} experimental={0.85} delta={+3.7%} />
    <MetricRow name="Storage Rate" baseline={0.45} experimental={0.38} delta={-15.6%} />
  </ResultsTable>
</ExperimentsTab>
```

---

## Part 3: Cogniti Advantages to Implement

### 3.0 Memory Bank Architecture ⭐(Key Feature)

The most significant Cogniti advantage: **Multi-tenant, hierarchical memory banks**.

**Current AgentMemory:**
```
Single global memory
├── All sessions share the same memory space
├── No isolation between agents/users
└── No shared schema layer
```

**Cogniti's Approach:**
```
Memory Bank (per agent/session)
├── Agent Session → Private episodic memory
├── Agent Dictionary → Private consolidated knowledge
└── Shared Memory → Cross-agent schemas (optional)
```

**Isolation Model: Per-Agent**
- Each agent instance gets its own MemoryBank
- Sessions created by the same agent share the same bank
- Different agents cannot access each other's episodic memories

**Schema Sharing: Automatic Hierarchical**
- When a bank creates a schema, it automatically propagates to its parent bank
- Hierarchy: Agent Bank → Shared Dictionary Bank → Global Shared Bank
- Enables cross-agent learning without manual sharing

**Implementation:**

```rust
// New model: MemoryBank
pub struct MemoryBank {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Option<Uuid>,         // User or agent ID
    pub bank_type: BankType,
    pub mission: Option<String>,         // Natural language identity
    pub directives: Vec<String>,        // Hard rules (guardrails)
    pub disposition: DispositionConfig, // Soft traits (skepticism, empathy, etc.)
    pub parent_bank_id: Option<Uuid>,   // For hierarchical sharing
    pub created_at: DateTime<Utc>,
}

pub enum BankType {
    Session,    // Private episodic, short-lived
    Dictionary, // Private consolidated knowledge
    Shared,     // Cross-agent schemas
}

pub struct DispositionConfig {
    pub skepticism: f32,    // 0-5 scale
    pub literalism: f32,    // 0-5 scale
    pub empathy: f32,       // 0-5 scale
    pub verbosity: f32,     // 0-5 scale
}
```

**Database Schema:**

```sql
CREATE TABLE memory_banks (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    owner_id UUID,-- NULL for shared banks
    bank_type TEXT NOT NULL,-- 'session', 'dictionary', 'shared'
    mission TEXT,
    directives JSONB,           -- Array of strings
    disposition JSONB,           -- {skepticism, literalism, empathy, verbosity}
    parent_bank_id UUID REFERENCES memory_banks(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Add bank_id to all memory tables
ALTER TABLE sessions ADD COLUMN bank_id UUID REFERENCES memory_banks(id);
ALTER TABLE episodes ADD COLUMN bank_id UUID REFERENCES memory_banks(id);
ALTER TABLE patterns ADD COLUMN bank_id UUID REFERENCES memory_banks(id);
ALTER TABLE engrams ADD COLUMN bank_id UUID REFERENCES memory_banks(id);
ALTER TABLE schemas ADD COLUMN bank_id UUID REFERENCES memory_banks(id);
```

**API Changes:**

```rust
// Bank management
POST /banks                     // Create a new memory bank
GET  /banks/{id}                // Get bank details
PUT  /banks/{id}                // Update bank (mission, directives, disposition)
DELETE /banks/{id}              // Delete bank cascade

// Sessions now require a bank
POST /sessions{bank_id: ..., ...}  // Open session in a bank

// Retrieval can span banks
POST /retrieve { query: ..., banks: [...], include_shared: true }
```

**Control Plane UI:**

```jsx
<BanksTab>
  <BankList>
    <BankCard 
      name="Research Agent"
      type="dictionary"
      mission="I am a research assistant specializing in ML"
      directives={["Always cite sources", "Never recommend specific stocks"]}
      disposition={{ skepticism: 4, empathy: 2 }}
      memoryCount={1247}
      schemaCount={23}
    />
    <BankCard 
      name="Code Assistant"
      type="dictionary"
      mission="I help with software development"
      directives={["Test before deploying"]}
      disposition={{ skepticism: 2, literalism: 4 }}
      memoryCount={3892}
      schemaCount={87}
    />
  </BankList>
  
  <BankCreation>
    <input placeholder="Bank name" />
    <select options={["session", "dictionary", "shared"]} />
    <textarea placeholder="Mission statement (what knowledge to prioritize)" />
    <textarea placeholder="Directives (one per line - hard rules to follow)" />
    <DispositionSliders />
  </BankCreation>
</BanksTab>
```

**MCP Integration:**

```json
// MCP tools for bank-aware memory
{
  "name": "memory_open_session",
  "arguments": {
    "bank_id": "uuid-or-name",
    "task_context": "current task",
    "mode": "Exploration",
    "expectation": "what to expect"
  }
}
```

**Files to modify:**
- `engram-core/src/models/bank.rs` — New file
- `engram-core/src/models/mod.rs` — Export bank model
- `engram-core/src/models/session.rs` — Add bank_id
- `engram-store/src/postgres.rs` — Bank CRUD and filtering
- `engram-runtime/src/engine.rs` — Bank-aware memory operations
- `engram-server/src/routes.rs` — Bank endpoints
- `web/src/main.jsx` — Banks tab

**Schema Propagation Flow:**
```
1. Agent A creates engrams in Bank A
2. Consolidation creates schema in Bank A
3. Schema automatically propagates to Bank A's parent (Shared Dictionary)
4. Agent B can retrieve schemas from parent bank during recall
5. Agent B's episodic memories remain private to Bank B
```

**Priority: HIGH** — This is Cogniti's most significant architectural advantage for multi-agent scenarios.

---

### 3.1 Working Memory Tier

Add a third storage layer between episodes and buffer:

```
Current:
Episode → PatternEntry (Buffer) → EngramEntry → MetaEngram

With Working Memory:
Episode → WorkingMemory (pre-C1, fragile) → Buffer (post-C1) → Engram → Schema
```

**Implementation:**
- Add `WorkingMemoryEntry` struct in `engram-core/src/models/`
- Add PostgreSQL table for working memory entries
- Modify ingestion flow to write to working memory first
- Add consolidation trigger to promote to buffer

**Files to modify:**
- `engram-core/src/models/mod.rs` — add WorkingMemoryEntry
- `engram-store/src/postgres.rs` — add working memory table
- `engram-runtime/src/flows.rs` — add promotion logic
- `engram-runtime/src/engine.rs` — add working memory lifecycle

### 3.2 Add "Analogy" Session Mode

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SessionMode {
    #[default]
    Exploration,
    Routine,
    Critical,
    Analogy,    // NEW: Cross-domain structural similarity
    Validation, // NEW: Evidence-based retrieval
}
```

**Analogy mode behavior:**
- Lower strength pre-filter (≥0.3)
- Prefer weak links (kinship references)
- Medium traversal depth
- Cross-domain construction

**Files to modify:**
- `engram-core/src/models/session.rs` — add mode variants
- `engram-runtime/src/adaptive.rs` — add mode mappings
- `engram-runtime/src/nodes/retrieval.rs` — add spread factor and mode bonus
- `engram-runtime/src/nodes/thalamus.rs` — add threshold

### 3.3 Schema-Only Neo4j

Move engrams out of Neo4j, keep only schemas:

```
Current:
Neo4j: sessions, episodes, patterns, engrams, schemas
Qdrant: patterns, engrams

After:
PostgreSQL: sessions, episodes, working_memory
Qdrant: patterns, engrams
Neo4j: schemas only (neocortex abstraction)
```

**This is a major migration—consider for later.**

### 3.4 Multi-Strategy Retrieval (TEMPR)

Add parallel retrieval strategies:

```rust
pub struct MultiStrategyRetrieval {
    pub semantic: bool,      // Current Qdrant similarity search
    pub bm25: bool,          // NEW: Keyword/exact matching
    pub temporal: bool,      // NEW: Time-based queries
    pub graph: bool,        // NEW: Neo4j traversal
    pub fusion: FusionStrategy,
}

pub enum FusionStrategy {
    ReciprocalRank,          // Cogniti's approach
    WeightedSum { weights: Vec<f32> },
}
```

**Files to add:**
- `engram-runtime/src/retrieval/bm25.rs`
- `engram-runtime/src/retrieval/temporal.rs`
- `engram-runtime/src/retrieval/fusion.rs`

### 3.5 Named Operations API

Add clearer operation names (inspired by Cogniti):

```rust
// Current: POST /sessions/{id}/episodes
// Add alias: POST /sessions/{id}/retain

// Current: POST /sessions/{id}/retrieve
// Add alias: POST /sessions/{id}/recall

// Current: POST /consolidate
// Add alias: POST /reflect
```

---

## Part 4: Semantic Value Functions

Replace hardcoded word lists with semantic scoring:

### 4.1 Semantic Valence

```rust
pub struct ValenceScorer {
    positive_anchors: Vec<Vec<f32>>,// Embeddings of positive concepts
    negative_anchors: Vec<Vec<f32>>,
    embedder: Embedder,
}

impl ValenceScorer {
    pub fn score(&self, text: &str) -> f32 {
        let embedding = self.embedder.embed(text);
        let pos_sim = self.positive_anchors.iter()
            .map(|a| cosine_similarity(&embedding, a))
            .max_by(f32::total_cmp)
            .unwrap_or(0.0);
        let neg_sim = self.negative_anchors.iter()
            .map(|a| cosine_similarity(&embedding, a))
            .max_by(f32::total_cmp)
            .unwrap_or(0.0);
        ((pos_sim - neg_sim) + 1.0) / 2.0// Normalize to [0, 1]
    }
}
```

### 4.2 Semantic Task Relevance

```rust
pub struct TaskRelevanceScorer {
    embedder: Embedder,
}

impl TaskRelevanceScorer {
    pub fn score(&self, context: &str, task_context: &str) -> f32 {
        let context_emb = self.embedder.embed(context);
        let task_emb = self.embedder.embed(task_context);
        cosine_similarity(&context_emb, &task_emb)
    }
}
```

### 4.3 Embedding-Based Novelty

```rust
pub fn novelty_score_semantic(
    action_embedding: &[f32],
    recent_engrams: &[EngramEntry],
) -> f32 {
    if recent_engrams.is_empty() {
        return 1.0;
    }
    
    let max_similarity = recent_engrams.iter()
        .map(|e| cosine_similarity(action_embedding, &e.embedding))
        .fold(0.0, f32::max);
    
    1.0 - max_similarity// Higher similarity = lower novelty
}
```

---

## Part 5: Implementation Order

### Phase 1: Configuration Expansion (Est. 2-3 days)

1. Expand `config.rs` with all missing parameters
2. Add validation for new fields
3. Update all nodes to read from config instead of hardcoded
4. Add migration for existing configs

**Priority: High** — Addresses immediate "hardcoded" concern

### Phase 1.5: Memory Bank Architecture (Est. 4-5 days)

1. Add `MemoryBank` model and `BankType` enum
2. Add PostgreSQL migration for banks table
3. Add `bank_id` foreign key to all memory tables
4. Update sessions to require a bank context
5. Implement bank CRUD endpoints
6. Add bank-aware filtering to retrieval operations
7. Add Banks tab to control plane

**Priority: High** — Enables multi-tenant, multi-agent scenarios

### Phase 2: Semantic Scoring (Est. 2 days)

1. Create `ValenceScorer` struct
2. Create `TaskRelevanceScorer` struct
3. Update thalamus to use semantic scoring when configured
4. Add toggle for keyword vs semantic mode

**Priority: High** — Fixes language-specific word lists

### Phase 3: Control Plane Redesign (Est. 3-4 days)

1. Restructure `main.jsx` into component files
2. Add Sessions tab
3. Add Performance tab with metrics
4. Redesign Tuning tab with contextual controls
5. Add preset/profile system

**Priority: Medium** — Improves usability significantly

### Phase 4: Mode Enhancements (Est. 1-2 days)

1. Add Analogy mode to SessionMode
2. Add Validation mode to SessionMode
3. Update adaptive thresholds for new modes
4. Add mode-specific spread factors and bonuses

**Priority: Medium** — Brings parity with Cogniti

### Phase 5: Working Memory Tier (Est. 3-4 days)

1. Add WorkingMemoryEntry model
2. Add PostgreSQL migration
3. Modify ingestion flow
4. Add promotion trigger

**Priority: Medium** — Major architectural improvement

### Phase 6: Multi-Strategy Retrieval (Est. 3 days)

1. Add BM25 retrieval
2. Add temporal query support
3. Add fusion strategy
4. Update retrieval architecture

**Priority: Lower** — Nice to have, not urgent

### Phase 7: Schema-Only Neo4j (Est. 5+ days)

1. Design migration strategy
2. Create engram migration to Postgres
3. Update all engram queries
4. Test thoroughly

**Priority: Lower** — Major change, defer until needed

---

## Files to Modify

### Core Models
- `engram-core/src/models/bank.rs` — **NEW** MemoryBank, BankType, DispositionConfig
- `engram-core/src/models/session.rs` — Add Analogy/Validation modes, add bank_id
- `engram-core/src/models/episode.rs` — Add bank_id
- `engram-core/src/models/pattern.rs` — Add bank_id
- `engram-core/src/models/engram.rs` — Add bank_id
- `engram-core/src/models/schema.rs` — Add bank_id
- `engram-core/src/models/mod.rs` — Export new models
- `engram-core/src/state.rs` — Add RetrievalState variants

### Runtime
- `engram-runtime/src/config.rs` — Expand all configs
- `engram-runtime/src/nodes/thalamus.rs` — Use config, add semantic scoring
- `engram-runtime/src/nodes/pattern.rs` — Use config values
- `engram-runtime/src/nodes/retrieval.rs` — Use config, add multi-strategy, add bank filtering
- `engram-runtime/src/plasticity.rs` — Use config
- `engram-runtime/src/adaptive.rs` — Add new modes
- `engram-runtime/src/nodes/buffer.rs` — Use config coefficients
- `engram-runtime/src/engine.rs` — Add bank context to all operations

### Storage
- `engram-store/src/postgres.rs` — Add banks table, working memory table, bank_id to all tables
- `engram-store/src/qdrant.rs` — Add BM25 support, bank-aware filtering

### Server
- `engram-server/src/routes.rs` — Add retain/recall/reflect aliases, add bank CRUD endpoints

### Web UI
- `web/src/components/BankCard.jsx` — **NEW** Bank display component
- `web/src/components/BanksTab.jsx` — **NEW** Banks management tab
- `web/src/components/TuningTab.jsx` — **NEW** Redesigned tuning
- `web/src/components/PerformanceTab.jsx` — **NEW** Metrics dashboard
- `web/src/main.jsx` — Refactor to use components, add banks tab

### New Files
- `engram-runtime/src/scoring/valence.rs` — Semantic valence
- `engram-runtime/src/scoring/relevance.rs` — Semantic task relevance
- `engram-runtime/src/scoring/novelty.rs` — Embedding-based novelty
- `engram-runtime/src/retrieval/bm25.rs` — BM25 retrieval
- `engram-runtime/src/retrieval/temporal.rs` — Time-based queries
- `engram-runtime/src/retrieval/fusion.rs` — Result fusion
- `migrations/` — Database migrations for banks and working memory

---

## Decisions Made

1. **Bank isolation level: Per-agent** — Each agent instance has its own memory bank. Sessions from the same agent share the bank.

2. **Schema sharing: Automatic hierarchical** — Schemas automatically propagate to parent banks. No manual opt-in required.

---

## Remaining Questions

1. **Which phase should we start with?** Recommended order:
   - Phase 1 (Configuration Expansion) — addresses immediate hardcoded concern
   - Phase 1.5 (Memory Banks) — key architectural feature for multi-agent

2. **Semantic scoring toggle or replace?** Should we keep keyword-based scoring as a fallback, or fully replace with semantic scoring? Semantic requires an embedding call per episode.

3. **Working Memory tier now or later?** This is a significant architectural change. Should we implement it in Phase 5, or defer?

4. **Control Plane refactor?** The current `main.jsx` is a single 1000+ line file. Should we:
   - Split into React components?
   - Keep as-is and just add new tabs?
   - Use a framework (React Router, etc.)?

5. **Default bank structure?** Should we create a default bank hierarchy on first run?
   ```
   Global Shared Bank (id: "default-shared")
   └── Default Dictionary Bank (id: "default-dictionary")
       └── Agent banks created automatically for each new agent
   ```