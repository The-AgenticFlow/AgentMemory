# Memory Structures

This guide explains the core structures in `engram-core`, why they exist, and how they interact with the full memory system.

## Big Picture

The architecture has six layers:

1. `Session` defines the current task frame and expectation state.
2. `Episode` captures one completed interaction or tool outcome.
3. `PatternEntry` stores weak repeated signals before they become stable memories.
4. `EngramEntry` stores long-term memory indices and links to content.
5. `MetaEngram` stores compressed schema-level knowledge.
6. `WorkingContext` holds the live task workspace while the agent is reasoning.

## `Episode`

### Why it exists

An `Episode` is the atomic unit of experience. It exists so the system can remember what happened without immediately turning every event into long-term memory.

### Needs

- Capture one completed action.
- Preserve the surrounding context.
- Store the actual outcome before memory decisions are made.

### Use cases

- Input to the Thalamus Filter.
- Replay source for consolidation.
- Audit trail for what the agent actually experienced.

### Interactions

- Comes from the agent loop after a response, tool call, or task step.
- Uses `session_id` to stay attached to one task frame.
- Feeds the pre-engram buffer and later consolidation flows.

## `Session`

### Why it exists

A `Session` is the control frame for memory policy. It tells the system what the agent expects, how risky the environment is, and how selective memory should be.

### Needs

- Carry the current expectation.
- Track the active mode.
- Bind episodes to a single task context.

### Use cases

- Open a new user interaction.
- Update mode when the task shifts.
- Close the session when the task ends.

### Interactions

- Drives Thalamus scoring.
- Guides retrieval mode selection.
- Prioritizes replay during consolidation.

## `PatternEntry`

### Why it exists

`PatternEntry` is the bridge between raw episodes and stable engrams. It lets weak signals accumulate before the system commits to long-term storage.

### Needs

- Track repetition.
- Decay when the pattern goes stale.
- Hold an embedding for ANN matching.

### Use cases

- Pre-engram buffer storage.
- Borderline replay during consolidation.
- Threshold-based promotion to engrams.

### Interactions

- Created after a filtered episode enters the buffer.
- Compared with other buffered patterns in Qdrant.
- Promoted into the engram layer when strong enough.

## `EngramEntry`

### Why it exists

`EngramEntry` is the searchable long-term index for a memory cluster. It does not try to hold everything itself; it points to the other pieces.

### Needs

- Provide stable similarity search.
- Record strength, age, and access frequency.
- Link to source content and schema abstractions.

### Use cases

- Direct recall.
- Memory updating through reconsolidation.
- Compression and archiving decisions.

### Interactions

- Written by pattern separation/completion.
- Read by retrieval and consolidation.
- Linked to `Episode` content and `MetaEngram` abstractions.

## `MetaEngram`

### Why it exists

`MetaEngram` is a schema-level compression of repeated engrams. It represents the abstract pattern that many source memories share.

### Needs

- Store the shared embedding.
- Track source engrams.
- Describe expected fields for retrieval.

### Use cases

- Schema activation.
- Retrieval narrowing.
- Compression of recurring knowledge.

### Interactions

- Built during nightly consolidation.
- Used to pre-weight retrieval.
- Can be dissolved if it stops helping.

## `WorkingContext`

### Why it exists

`WorkingContext` is the live workspace where the agent keeps its current goals, loaded memories, and provisional inferences.

### Needs

- Limit active context size.
- Keep goal priorities visible.
- Make task state serializable and resumable.

### Use cases

- Running a multi-step task.
- Loading context before generation.
- Capturing task-end state for consolidation.

### Interactions

- Pulls from retrieval.
- Pushes completed task traces into episode capture.
- Hands state back to session-level consolidation.

## How They Work Together

The normal flow is:

1. `Session` starts.
2. `WorkingContext` opens for the current task.
3. The agent produces or observes an `Episode`.
4. The episode is filtered by relevance.
5. Weak repetitions form `PatternEntry` records.
6. Strong patterns become `EngramEntry` records.
7. Repeated engrams are compressed into `MetaEngram` records.
8. The working context closes and the session can be updated or ended.

This creates the full loop from experience to memory to schema to future reasoning.
