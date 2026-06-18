//! Neo4j graph adapter used as the primary deployment backend.
//!
//! The adapter talks to Neo4j's transactional HTTP endpoint so the project
//! does not need a native driver process. If Neo4j is unavailable, callers can
//! continue using the local JSON stores as a development fallback.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use engram_core::{EngramEntry, Episode, MetaEngram, PatternEntry, Session, WorkingContext};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

/// Runtime Neo4j connection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neo4jConfig {
    pub uri: String,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl Neo4jConfig {
    pub fn from_env() -> Option<Self> {
        let uri = std::env::var("ENGRAM_NEO4J_URI").ok()?;
        Some(Self {
            uri,
            user: std::env::var("ENGRAM_NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string()),
            password: std::env::var("ENGRAM_NEO4J_PASSWORD")
                .unwrap_or_else(|_| "engram-memory".to_string()),
            database: std::env::var("ENGRAM_NEO4J_DATABASE")
                .unwrap_or_else(|_| "neo4j".to_string()),
        })
    }

    fn tx_url(&self) -> String {
        format!(
            "{}/db/{}/tx/commit",
            self.uri.trim_end_matches('/'),
            self.database
        )
    }
}

/// Thin Neo4j HTTP client.
#[derive(Debug, Clone)]
pub struct Neo4jMemoryStore {
    config: Option<Neo4jConfig>,
    client: Client,
}

impl Default for Neo4jMemoryStore {
    fn default() -> Self {
        Self::new(Neo4jConfig::from_env())
    }
}

impl Neo4jMemoryStore {
    pub fn new(config: Option<Neo4jConfig>) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub fn configured(&self) -> bool {
        self.config.is_some()
    }

    pub async fn health(&self) -> Neo4jHealth {
        let Some(config) = &self.config else {
            return Neo4jHealth {
                configured: false,
                connected: false,
                message: "ENGRAM_NEO4J_URI is not set; using local fallback stores".to_string(),
            };
        };

        match self.execute("RETURN 1 AS ok", json!({})).await {
            Ok(_) => Neo4jHealth {
                configured: true,
                connected: true,
                message: format!("connected to {}", config.uri),
            },
            Err(error) => Neo4jHealth {
                configured: true,
                connected: false,
                message: error.to_string(),
            },
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        if !self.configured() {
            return Ok(());
        }

        for statement in [
            "CREATE CONSTRAINT engram_session_id IF NOT EXISTS FOR (n:Session) REQUIRE n.id IS UNIQUE",
            "CREATE CONSTRAINT engram_episode_id IF NOT EXISTS FOR (n:Episode) REQUIRE n.id IS UNIQUE",
            "CREATE CONSTRAINT engram_pattern_hash IF NOT EXISTS FOR (n:Pattern) REQUIRE n.pattern_hash IS UNIQUE",
            "CREATE CONSTRAINT engram_engram_id IF NOT EXISTS FOR (n:Engram) REQUIRE n.id IS UNIQUE",
            "CREATE CONSTRAINT engram_schema_id IF NOT EXISTS FOR (n:Schema) REQUIRE n.id IS UNIQUE",
            "CREATE CONSTRAINT engram_config_id IF NOT EXISTS FOR (n:ConfigProfile) REQUIRE n.id IS UNIQUE",
        ] {
            self.execute(statement, json!({})).await?;
        }
        Ok(())
    }

    pub async fn upsert_session(&self, session: &Session) -> Result<()> {
        self.execute(
            "MERGE (s:Session {id: $id})
             SET s.user_id = $user_id, s.current_expectation = $current_expectation,
                 s.current_mode = $current_mode, s.task_context = $task_context,
                 s.created_at = $created_at, s.updated_at = $updated_at, s.closed_at = $closed_at",
            json!({
                "id": session.id,
                "user_id": session.user_id,
                "current_expectation": session.current_expectation,
                "current_mode": format!("{:?}", session.current_mode),
                "task_context": session.task_context,
                "created_at": session.created_at,
                "updated_at": session.updated_at,
                "closed_at": session.closed_at,
            }),
        )
        .await
    }

    pub async fn upsert_working_context(&self, context: &WorkingContext) -> Result<()> {
        self.execute(
            "MATCH (s:Session {id: $session_id})
             MERGE (w:WorkingContext {id: $id})
             SET w.task_id = $task_id, w.goal_stack = $goal_stack, w.active_engrams = $active_engrams,
                 w.episodic_buffer = $episodic_buffer, w.inference_layer = $inference_layer,
                 w.context_metadata = $context_metadata, w.opened_at = $opened_at,
                 w.updated_at = $updated_at, w.closed_at = $closed_at, w.bank_id = $bank_id
             MERGE (s)-[:ACTIVATES]->(w)",
            json!({
                "id": context.id,
                "session_id": context.session_id,
                "task_id": context.task_id,
                "goal_stack": serde_json::to_string(&context.goal_stack)?,
                "active_engrams": context.active_engrams,
                "episodic_buffer": context.episodic_buffer,
                "inference_layer": context.inference_layer,
                "context_metadata": context.context_metadata.to_string(),
                "opened_at": context.opened_at,
                "updated_at": context.updated_at,
                "closed_at": context.closed_at,
                "bank_id": context.bank_id.map(|id| id.to_string()),
            }),
        )
        .await
    }

    pub async fn upsert_episode(&self, episode: &Episode) -> Result<()> {
        self.execute(
            "MATCH (s:Session {id: $session_id})
             MERGE (e:Episode {id: $id})
             SET e.action = $action, e.context = $context, e.outcome = $outcome,
                 e.created_at = $created_at, e.session_id = $session_id
             MERGE (s)-[:CAPTURED]->(e)",
            json!({
                "id": episode.id,
                "action": episode.action,
                "context": episode.context,
                "outcome": episode.outcome,
                "session_id": episode.session_id,
                "created_at": episode.created_at,
            }),
        )
        .await
    }

    pub async fn upsert_pattern(&self, pattern: &PatternEntry, session_id: Uuid) -> Result<()> {
        self.execute(
            "MATCH (s:Session {id: $session_id})
             MERGE (p:Pattern {pattern_hash: $pattern_hash})
             SET p.embedding = $embedding, p.occurrences = $occurrences, p.strength = $strength,
                 p.context_tags = $context_tags, p.content = $content, p.threshold = $threshold,
                 p.decay_rate = $decay_rate, p.source = $source, p.episode_refs = $episode_refs,
                 p.first_seen = $first_seen, p.last_seen = $last_seen
             MERGE (s)-[:BUFFERED_AS]->(p)",
            json!({
                "session_id": session_id,
                "pattern_hash": pattern.pattern_hash,
                "embedding": pattern.embedding,
                "occurrences": pattern.occurrences,
                "strength": pattern.strength,
                "context_tags": pattern.context_tags,
                "content": pattern.content,
                "threshold": pattern.threshold,
                "decay_rate": pattern.decay_rate,
                "source": format!("{:?}", pattern.source),
                "episode_refs": pattern.episode_refs,
                "first_seen": pattern.first_seen,
                "last_seen": pattern.last_seen,
            }),
        )
        .await
    }

    pub async fn upsert_engram(
        &self,
        engram: &EngramEntry,
        pattern_hash: Option<&str>,
    ) -> Result<()> {
        self.execute(
            "MATCH (s:Session {id: $session_ref})
             MERGE (g:Engram {id: $id})
             SET g.embedding = $embedding, g.tags = $tags, g.strength = $strength,
                 g.thalamus_scores = $thalamus_scores, g.created_at = $created_at,
                 g.last_accessed = $last_accessed, g.access_count = $access_count,
                 g.session_ref = $session_ref, g.kinship_ref = $kinship_ref,
                 g.source = $source, g.status = $status,
                 g.episodic_content_ref = $episodic_content_ref, g.schema_refs = $schema_refs
             MERGE (s)-[:PROMOTED_TO]->(g)
             WITH g
             OPTIONAL MATCH (k:Engram {id: $kinship_ref})
             FOREACH (_ IN CASE WHEN k IS NULL THEN [] ELSE [1] END | MERGE (g)-[:KINSHIP]->(k))
             WITH g
             OPTIONAL MATCH (p:Pattern {pattern_hash: $pattern_hash})
             FOREACH (_ IN CASE WHEN p IS NULL THEN [] ELSE [1] END | MERGE (p)-[:PROMOTED_TO]->(g))",
            json!({
                "id": engram.id,
                "embedding": engram.embedding,
                "tags": engram.tags,
                "strength": engram.strength,
                "thalamus_scores": serde_json::to_string(&engram.thalamus_scores)?,
                "created_at": engram.created_at,
                "last_accessed": engram.last_accessed,
                "access_count": engram.access_count,
                "session_ref": engram.session_ref,
                "kinship_ref": engram.kinship_ref,
                "source": format!("{:?}", engram.source),
                "status": format!("{:?}", engram.status),
                "episodic_content_ref": engram.episodic_content_ref,
                "schema_refs": engram.schema_refs,
                "pattern_hash": pattern_hash,
            }),
        )
        .await
    }

    pub async fn upsert_schema(&self, schema: &MetaEngram) -> Result<()> {
        self.execute(
            "MERGE (m:Schema {id: $id})
             SET m.embedding = $embedding, m.tags = $tags, m.strength = $strength,
                 m.source_engram_ids = $source_engram_ids, m.prediction_fields = $prediction_fields,
                 m.created_at = $created_at
             WITH m
             UNWIND $source_engram_ids AS source_id
             MATCH (g:Engram {id: source_id})
             MERGE (g)-[:SOURCE_OF]->(m)",
            json!({
                "id": schema.id,
                "embedding": schema.embedding,
                "tags": schema.tags,
                "strength": schema.strength,
                "source_engram_ids": schema.source_engram_ids,
                "prediction_fields": schema.prediction_fields,
                "created_at": schema.created_at,
            }),
        )
        .await
    }

    pub async fn upsert_config(&self, version: u64, profile: &Value, actor: &str) -> Result<()> {
        self.execute(
            "MERGE (c:ConfigProfile {id: 'active'})
             SET c.version = $version, c.profile = $profile, c.updated_by = $actor, c.updated_at = $updated_at
             CREATE (a:ConfigAudit {id: $audit_id, version: $version, actor: $actor, change: $profile, created_at: $updated_at})
             MERGE (c)-[:UPDATED_BY]->(a)",
            json!({
                "version": version,
                "profile": profile.to_string(),
                "actor": actor,
                "updated_at": Utc::now(),
                "audit_id": Uuid::new_v4(),
            }),
        )
        .await
    }

    async fn execute(&self, statement: &str, parameters: Value) -> Result<()> {
        let Some(config) = &self.config else {
            return Ok(());
        };
        let body = json!({
            "statements": [{
                "statement": statement,
                "parameters": parameters
            }]
        });

        let mut backoff = std::time::Duration::from_millis(200);
        let max_backoff = std::time::Duration::from_secs(10);
        let max_retries: u32 = 5;

        for attempt in 0..=max_retries {
            match self
                .client
                .post(config.tx_url())
                .basic_auth(&config.user, Some(&config.password))
                .json(&body)
                .send()
                .await
            {
                Ok(response) => {
                    let payload: Neo4jResponse =
                        response.json().await.context("parsing Neo4j response")?;
                    if payload.errors.is_empty() {
                        return Ok(());
                    }
                    // Neo4j returned application-level errors — these are not transient.
                    anyhow::bail!("Neo4j errors: {:?}", payload.errors);
                }
                Err(err) => {
                    // Only retry on connection-level errors (connection refused, timeout, etc.)
                    if attempt < max_retries && Self::is_transient(&err) {
                        tracing::warn!(
                            "Neo4j request failed (attempt {}/{max_retries}): {err}; retrying in {:?}",
                            attempt + 1,
                            backoff,
                        );
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(max_backoff);
                        continue;
                    }
                    return Err(err)
                        .with_context(|| format!("connecting to Neo4j at {}", config.uri));
                }
            }
        }

        unreachable!()
    }

    /// Returns true for network-level errors that are likely transient
    /// (connection refused, timeouts, DNS failures, etc.)
    fn is_transient(err: &reqwest::Error) -> bool {
        err.is_connect() || err.is_timeout()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neo4jHealth {
    pub configured: bool,
    pub connected: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct Neo4jResponse {
    #[serde(default)]
    errors: Vec<Value>,
}

#[allow(dead_code)]
fn _date(_value: DateTime<Utc>) {}
