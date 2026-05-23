-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Persist nonce replay state for macaroon authentication.
-- See ADR-0026 (macaroon OCAP).

CREATE TABLE IF NOT EXISTS used_nonces (
  token_id   TEXT    NOT NULL,
  nonce      TEXT    NOT NULL,
  expires_at INTEGER NOT NULL,  -- unix seconds
  PRIMARY KEY (token_id, nonce)
);

CREATE INDEX IF NOT EXISTS used_nonces_expires
  ON used_nonces(expires_at);
