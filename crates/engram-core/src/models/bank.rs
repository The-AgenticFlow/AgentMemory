/// Multi-tenant, hierarchical memory bank architecture.
///
/// Memory banks provide isolation between agents/users while enabling
/// optional cross-agent schema sharing through a hierarchical parent
/// relationship.
///
/// Hierarchy:
/// - Global Shared Bank → Dictionary Banks → Session Banks

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The type of memory bank, determining its scope and sharing behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BankType {
    /// Private episodic memory, short-lived (per session).
    #[default]
    Session,
    /// Private consolidated knowledge base (per agent).
    Dictionary,
    /// Cross-agent shared schemas and patterns.
    Shared,
}

impl std::fmt::Display for BankType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BankType::Session => write!(f, "session"),
            BankType::Dictionary => write!(f, "dictionary"),
            BankType::Shared => write!(f, "shared"),
        }
    }
}

/// Soft personality traits for an agent's memory bank.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DispositionConfig {
    /// Skepticism level (0-5 scale). Higher = questions assumptions more.
    pub skepticism: f32,
    /// Literalism level (0-5 scale). Higher = takes things at face value.
    pub literalism: f32,
    /// Empathy level (0-5 scale). Higher = prioritizes understanding user intent.
    pub empathy: f32,
    /// Verbosity level (0-5 scale). Higher = provides more detail.
    pub verbosity: f32,
}

impl Default for DispositionConfig {
    fn default() -> Self {
        Self {
            skepticism: 2.0,
            literalism: 2.0,
            empathy: 3.0,
            verbosity: 2.0,
        }
    }
}

/// A memory bank that isolates and organizes agent memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryBank {
    /// Unique bank identity.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Owner (agent or user). None for shared banks.
    pub owner_id: Option<Uuid>,
    /// Scope and sharing behavior.
    pub bank_type: BankType,
    /// Natural language mission statement defining what knowledge to prioritize.
    pub mission: Option<String>,
    /// Hard rules/guardrails (one directive per entry).
    pub directives: Vec<String>,
    /// Soft personality traits.
    pub disposition: DispositionConfig,
    /// Parent bank for hierarchical schema propagation.
    pub parent_bank_id: Option<Uuid>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl MemoryBank {
    /// Creates a new memory bank with the specified parameters.
    pub fn new(
        name: impl Into<String>,
        owner_id: Option<Uuid>,
        bank_type: BankType,
        mission: Option<String>,
        directives: Vec<String>,
        disposition: DispositionConfig,
        parent_bank_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            owner_id,
            bank_type,
            mission,
            directives,
            disposition,
            parent_bank_id,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a default shared bank for single-agent scenarios.
    pub fn default_shared() -> Self {
        Self::new(
            "default-shared",
            None,
            BankType::Shared,
            Some("General purpose memory bank for all agents".to_string()),
            vec![],
            DispositionConfig::default(),
            None,
        )
    }

    /// Creates a dictionary bank for an agent.
    pub fn new_dictionary(
        owner_id: Uuid,
        name: impl Into<String>,
        mission: Option<String>,
        directives: Vec<String>,
        parent_bank_id: Option<Uuid>,
    ) -> Self {
        Self::new(
            name,
            Some(owner_id),
            BankType::Dictionary,
            mission,
            directives,
            DispositionConfig::default(),
            parent_bank_id,
        )
    }

    /// Creates a session bank under a dictionary bank.
    pub fn new_session(
        owner_id: Uuid,
        name: impl Into<String>,
        parent_bank_id: Option<Uuid>,
    ) -> Self {
        Self::new(
            name,
            Some(owner_id),
            BankType::Session,
            None,
            vec![],
            DispositionConfig::default(),
            parent_bank_id,
        )
    }

    /// Updates the bank's mutable fields.
    pub fn update(
        &mut self,
        mission: Option<String>,
        directives: Vec<String>,
        disposition: DispositionConfig,
    ) {
        self.mission = mission;
        self.directives = directives;
        self.disposition = disposition;
        self.updated_at = Utc::now();
    }
}

/// Summary information for bank listing in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankSummary {
    pub id: Uuid,
    pub name: String,
    pub bank_type: BankType,
    pub mission: Option<String>,
    pub directive_count: usize,
    pub memory_count: usize,
    pub schema_count: usize,
    pub owner_id: Option<Uuid>,
    pub parent_bank_id: Option<Uuid>,
}
