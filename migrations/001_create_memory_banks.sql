-- Migration: Create memory_banks table
-- Description: Multi-tenant, hierarchical memory bank architecture for agent isolation.

CREATE TABLE IF NOT EXISTS memory_banks (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    owner_id UUID,
    bank_type TEXT NOT NULL CHECK (bank_type IN ('session', 'dictionary', 'shared')),
    mission TEXT,
    directives JSONB DEFAULT '[]'::jsonb,
    disposition JSONB DEFAULT '{"skepticism": 2.0, "literalism": 2.0, "empathy": 3.0, "verbosity": 2.0}'::jsonb,
    parent_bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_memory_banks_type ON memory_banks(bank_type);
CREATE INDEX IF NOT EXISTS idx_memory_banks_owner ON memory_banks(owner_id);
CREATE INDEX IF NOT EXISTS idx_memory_banks_parent ON memory_banks(parent_bank_id);

-- Insert default shared bank for backwards compatibility
INSERT INTO memory_banks (id, name, bank_type, mission, directives, disposition, created_at, updated_at)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'default-shared',
    'shared',
    'General purpose memory bank for all agents',
    '[]'::jsonb,
    '{"skepticism": 2.0, "literalism": 2.0, "empathy": 3.0, "verbosity": 2.0}'::jsonb,
    NOW(),
    NOW()
)
ON CONFLICT (id) DO NOTHING;