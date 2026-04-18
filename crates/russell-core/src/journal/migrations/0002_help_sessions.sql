-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Phase 1: help_sessions for `russell help`.
-- See ADR-0016, docs/specifications/PERSISTENCE_CATALOG.md §2.1.

CREATE TABLE IF NOT EXISTS help_sessions (
  id            TEXT PRIMARY KEY,            -- ULID matching evidence dir name
  ts_unix       INTEGER NOT NULL,
  ts            TEXT    NOT NULL,            -- RFC3339
  backend       TEXT    NOT NULL,            -- 'openrouter' | 'ollama' | 'mock' | 'offline'
  model         TEXT,                        -- NULL for 'offline'
  note          TEXT,                        -- operator --note text
  prompt_chars  INTEGER NOT NULL,
  response_chars INTEGER NOT NULL,
  latency_ms    INTEGER,                     -- NULL for 'offline'
  status        TEXT    NOT NULL CHECK(status IN ('ok','error','fallback')),
  error_kind    TEXT,                        -- present when status='error'
  evidence_ref  TEXT NOT NULL                -- path under evidence/help/
);

CREATE INDEX IF NOT EXISTS help_sessions_ts
  ON help_sessions(ts_unix);
