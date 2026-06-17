-- Support password-protected Immich shared links.
-- needs_password: this album's Immich share requires a password.
-- share_token: the immich_shared_link_token captured after a successful password
--   login, replayed as a Cookie on all sync/asset requests (stable per password).
ALTER TABLE tenant ADD COLUMN needs_password INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tenant ADD COLUMN share_token TEXT NOT NULL DEFAULT '';
