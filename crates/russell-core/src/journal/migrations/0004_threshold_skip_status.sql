-- ADR-0020: threshold-gated LLM escalation
-- Adds 'threshold_skip' as a valid help_sessions status value.
-- Migration 0002 has a CHECK constraint that only allows
-- 'ok','error','fallback'. This ALTER TABLE drops and recreates
-- the constraint with the new valid value added.

ALTER TABLE help_sessions DROP CONSTRAINT IF EXISTS help_sessions_status_check;
ALTER TABLE help_sessions ADD CONSTRAINT help_sessions_status_check
  CHECK (status IN ('ok','error','fallback','threshold_skip'));
