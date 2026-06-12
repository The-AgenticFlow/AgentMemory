-- Migration: Create working_memory table
-- Description: Pre-consolidation (pre-C1) fragile memory tier between episodes and the buffer.

CREATE TABLE IF NOT EXISTS working_memory (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL,
    bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    embedding JSONB,
    strength REAL NOT NULL DEFAULT 0.5 CHECK (strength >= 0.0 AND strength <= 1.0),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_accessed TIMESTAMPTZ,
    decay_rate REAL NOT NULL DEFAULT 0.1 CHECK (decay_rate >= 0.0 AND decay_rate <= 1.0),
    tags JSONB DEFAULT '[]'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_working_memory_session ON working_memory(session_id);
CREATE INDEX IF NOT EXISTS idx_working_memory_bank ON working_memory(bank_id);
CREATE INDEX IF NOT EXISTS idx_working_memory_strength ON working_memory(strength);