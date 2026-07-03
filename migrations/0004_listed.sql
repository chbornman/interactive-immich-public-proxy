-- Public album index at /.
-- listed: include this album on the public index page (password-protected
--   albums are excluded there regardless of this flag).
ALTER TABLE tenant ADD COLUMN listed INTEGER NOT NULL DEFAULT 1;
