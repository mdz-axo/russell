-- SPDX-License-Identifier: MIT OR Apache-2.0
-- T6: Event integrity chain — adds prev_hash and hash columns.
--
-- Each event links to its predecessor via SHA-256 hash, providing
-- tamper-evident persistence (JR-7). The genesis hash is computed
-- from /etc/machine-id at first-write time.
--
-- Existing events get NULL hashes — only new events are chained.
-- The `russell verify-journal` CLI verb skips NULL-hash events
-- during verification (a chain starts at the first non-NULL hash).

ALTER TABLE events ADD COLUMN prev_hash TEXT;
ALTER TABLE events ADD COLUMN hash TEXT;
