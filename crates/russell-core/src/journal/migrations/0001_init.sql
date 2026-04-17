-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Initial schema. See cybernetic-health-harness.md §8 and ADR-0004.

CREATE TABLE IF NOT EXISTS samples (
  ts         INTEGER NOT NULL,   -- unix seconds
  scope      TEXT    NOT NULL DEFAULT 'host' CHECK(scope IN ('host','self')),
  probe      TEXT    NOT NULL,
  value_num  REAL,
  value_text TEXT,
  unit       TEXT,
  PRIMARY KEY (ts, scope, probe)
);

CREATE INDEX IF NOT EXISTS samples_scope_probe_ts
  ON samples(scope, probe, ts);

CREATE TABLE IF NOT EXISTS events (
  id           TEXT PRIMARY KEY,      -- ULID
  ts_unix      INTEGER NOT NULL,
  ts           TEXT    NOT NULL,      -- RFC3339
  schema       TEXT    NOT NULL,
  scope        TEXT    NOT NULL DEFAULT 'host' CHECK(scope IN ('host','self')),
  tier         TEXT,
  module       TEXT,
  run_id       TEXT,
  severity     TEXT    NOT NULL CHECK(severity IN ('info','warn','alert','crit')),
  action       TEXT    NOT NULL,
  dry_run      INTEGER NOT NULL DEFAULT 0,
  summary      TEXT,
  evidence_ref TEXT,
  duration_ms  INTEGER,
  payload      TEXT    NOT NULL       -- full JSON Event, for replay
);

CREATE INDEX IF NOT EXISTS events_scope_ts
  ON events(scope, ts_unix);

CREATE INDEX IF NOT EXISTS events_severity_ts
  ON events(severity, ts_unix);

CREATE TABLE IF NOT EXISTS baselines (
  probe      TEXT NOT NULL,
  scope      TEXT NOT NULL DEFAULT 'host' CHECK(scope IN ('host','self')),
  ewma_mean  REAL,
  ewma_var   REAL,
  p50        REAL,
  p95        REAL,
  p99        REAL,
  updated_ts INTEGER,
  PRIMARY KEY (probe, scope)
);

CREATE TABLE IF NOT EXISTS confirmations (
  evidence_id TEXT PRIMARY KEY,
  confirmed_ts INTEGER NOT NULL,
  actor        TEXT    NOT NULL,
  note         TEXT
);
