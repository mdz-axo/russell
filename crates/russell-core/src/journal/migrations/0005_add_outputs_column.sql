-- SPDX-License-Identifier: MIT OR Apache-2.0
-- Add the `outputs` column to events for storing structured
-- key-value pairs (reflex arc metadata, rule results, etc.).
-- Used by `list_reflex_events` in the reader and `reflex_proposed`
-- and `reflex_fired` event writers in the sentinel.

ALTER TABLE events ADD COLUMN outputs TEXT;

