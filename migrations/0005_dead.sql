-- Automatic dead-share detection.
-- dead_since: epoch seconds when the share key was first confirmed revoked by
--   Immich (NULL while alive; cleared again when a resync succeeds).
ALTER TABLE tenant ADD COLUMN dead_since INTEGER;
