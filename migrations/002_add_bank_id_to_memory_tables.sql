-- Migration: Add bank_id foreign keys to memory tables
-- Description: Links sessions, episodes, patterns, engrams, and schemas to memory banks.

ALTER TABLE sessions ADD COLUMN IF NOT EXISTS bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL;
ALTER TABLE episodes ADD COLUMN IF NOT EXISTS bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL;
ALTER TABLE patterns ADD COLUMN IF NOT EXISTS bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL;
ALTER TABLE engrams ADD COLUMN IF NOT EXISTS bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL;
ALTER TABLE schemas ADD COLUMN IF NOT EXISTS bank_id UUID REFERENCES memory_banks(id) ON DELETE SET NULL;

-- Index new columns for fast bank-scoped queries
CREATE INDEX IF NOT EXISTS idx_sessions_bank ON sessions(bank_id);
CREATE INDEX IF NOT EXISTS idx_episodes_bank ON episodes(bank_id);
CREATE INDEX IF NOT EXISTS idx_patterns_bank ON patterns(bank_id);
CREATE INDEX IF NOT EXISTS idx_engrams_bank ON engrams(bank_id);
CREATE INDEX IF NOT EXISTS idx_schemas_bank ON schemas(bank_id);

-- Backfill existing rows to the default shared bank
UPDATE sessions SET bank_id = '00000000-0000-0000-0000-000000000001' WHERE bank_id IS NULL;
UPDATE episodes SET bank_id = '00000000-0000-0000-0000-000000000001' WHERE bank_id IS NULL;
UPDATE patterns SET bank_id = '00000000-0000-0000-0000-000000000001' WHERE bank_id IS NULL;
UPDATE engrams SET bank_id = '00000000-0000-0000-0000-000000000001' WHERE bank_id IS NULL;
UPDATE schemas SET bank_id = '00000000-0000-0000-0000-000000000001' WHERE bank_id IS NULL;