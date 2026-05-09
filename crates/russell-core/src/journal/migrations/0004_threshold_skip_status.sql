-- ADR-0020: threshold-gated LLM escalation
-- Adds 'threshold_skip' as a valid help_sessions status value.
-- SQLite doesn't support DROP CONSTRAINT, so we recreate the table.
-- Step 1: create new table with updated CHECK
CREATE TABLE help_sessions_new (
  id            TEXT PRIMARY KEY,
  ts_unix       INTEGER NOT NULL,
  ts            TEXT    NOT NULL,
  backend       TEXT    NOT NULL,
  model         TEXT,
  note          TEXT,
  prompt_chars  INTEGER NOT NULL,
  response_chars INTEGER NOT NULL,
  latency_ms    INTEGER,
  status        TEXT    NOT NULL CHECK(status IN ('ok','error','fallback','threshold_skip')),
  error_kind    TEXT,
  evidence_ref  TEXT NOT NULL
);
-- Step 2: copy all existing data
INSERT INTO help_sessions_new SELECT * FROM help_sessions;
-- Step 3: drop old table
DROP TABLE help_sessions;
-- Step 4: rename to original name
ALTER TABLE help_sessions_new RENAME TO help_sessions;
-- Step 5: recreate the index
CREATE INDEX IF NOT EXISTS help_sessions_ts ON help_sessions(ts_unix);
