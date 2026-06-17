-- Interactive Immich Public Proxy — initial schema
-- All state is keyed by tenant (= one Immich shared album). Immich is never written to.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- One row per Immich shared album we've seen (auto-provisioned on first access).
CREATE TABLE IF NOT EXISTS tenant (
    id             TEXT PRIMARY KEY,            -- sha256(share_key) hex (does not expose the key)
    share_key      TEXT NOT NULL,               -- secret; needed to talk to Immich (e.g. admin resync)
    immich_album   TEXT,                        -- Immich album UUID
    title          TEXT,
    allow_marks    INTEGER NOT NULL DEFAULT 1,
    allow_notes    INTEGER NOT NULL DEFAULT 1,
    allow_download INTEGER NOT NULL DEFAULT 1,
    public_write   INTEGER NOT NULL DEFAULT 1,
    synced_at      INTEGER,                      -- last asset-list refresh (epoch seconds)
    created_at     INTEGER NOT NULL
);

-- Cached asset list per tenant, so pagination/search never wait on Immich.
CREATE TABLE IF NOT EXISTS asset (
    tenant_id    TEXT NOT NULL,
    asset_id     TEXT NOT NULL,                 -- Immich asset UUID
    kind         TEXT NOT NULL,                 -- IMAGE | VIDEO
    width        INTEGER NOT NULL DEFAULT 3,
    height       INTEGER NOT NULL DEFAULT 2,
    taken_at     INTEGER NOT NULL DEFAULT 0,    -- ordering / cursor (epoch seconds)
    filename     TEXT NOT NULL DEFAULT '',
    immich_tags  TEXT NOT NULL DEFAULT '',      -- denormalized text for search
    PRIMARY KEY (tenant_id, asset_id)
);
-- DESC browse order + cursor pagination
CREATE INDEX IF NOT EXISTS ix_asset_order ON asset(tenant_id, taken_at DESC, asset_id DESC);

-- Lightweight visitor identity (no accounts; signed cookie).
CREATE TABLE IF NOT EXISTS visitor (
    id           TEXT PRIMARY KEY,              -- random uuid, carried in signed cookie
    display_name TEXT NOT NULL DEFAULT '',
    ip_hash      TEXT NOT NULL DEFAULT '',
    banned       INTEGER NOT NULL DEFAULT 0,
    first_seen   INTEGER NOT NULL
);

-- Collaborative marks: an asset is "marked" if it has >= 1 row.
CREATE TABLE IF NOT EXISTS mark (
    tenant_id  TEXT NOT NULL,
    asset_id   TEXT NOT NULL,
    visitor_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, asset_id, visitor_id)
);
CREATE INDEX IF NOT EXISTS ix_mark ON mark(tenant_id, asset_id);

-- Attributed notes (a comment thread per asset).
CREATE TABLE IF NOT EXISTS note (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tenant_id   TEXT NOT NULL,
    asset_id    TEXT NOT NULL,
    visitor_id  TEXT NOT NULL,
    display_name TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL,                  -- plain text; escaped on render
    created_at  INTEGER NOT NULL,
    hidden      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS ix_note ON note(tenant_id, asset_id, hidden);
