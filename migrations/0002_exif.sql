-- Store the full Immich exifInfo blob per asset (JSON), for the metadata panel.
-- Populated on the next asset-list sync (TTL refresh or admin resync).
ALTER TABLE asset ADD COLUMN exif_json TEXT NOT NULL DEFAULT '';
