# interactive-immich-public-proxy

A fast, public, read-only gallery for [Immich](https://immich.app) shared albums, with a
small stateful layer: visitors can mark favorites, leave notes, search, filter, and
download, all without an Immich account.

Built quickly for a family photo project. It runs in production at home, but it is not
hardened for hostile use; read the code before relying on it. Issues and PRs welcome.

## Why

[immich-public-proxy](https://github.com/alangrainger/immich-public-proxy) (IPP) by Alan
Grainger puts a small, isolated proxy in front of Immich so you can share public albums
without exposing Immich's API to the internet. It is stateless by design, which is its
security value. Use it if that is all you need.

This project adds a few interactive controls for sharing albums with family (mark the good
ones, leave a comment, search, download a few), which requires state. It keeps the same
isolation idea but adds its own database for the interactive features. Immich is still only
ever read.

It also paginates for large libraries. IPP renders a whole album into one page, which gets
slow once an album has thousands of items. This proxy caches the asset list in its own
database and serves it in pages as you scroll (cursor pagination), so a ~15,000-photo album
first paints in about a second. Filtering and search run as indexed database queries rather
than in the browser.

## Features

- Read-only to Immich: only GET requests, authenticated with the public share key. Immich's
  API is never exposed and originals are never modified.
- Server-side pagination, so it stays fast regardless of album size.
- Collaborative marks: anyone with the link can mark or unmark favorites (a shared flag).
- Notes: comments on individual photos and videos, attributed to an optional name.
- Search over filename, EXIF metadata (camera, lens, location, caption), and notes.
- Filters: Marked, Has-notes, and a Photos/Videos toggle (combinable).
- Download and share: single or multi-select; native share sheet on iOS/Android, zip on desktop.
- Metadata panel: full EXIF (date, camera, lens, exposure, dimensions, location).
- Password-protected shares: if the Immich share has a password, visitors are prompted and
  it is validated against Immich.
- Multi-tenant: each Immich shared album is isolated (its own marks, notes, and search).
  Albums auto-provision on first visit, with no per-album setup.
- Light theme, responsive layout, thumbnail size slider.

All state (marks, notes, visitor identity) lives in the proxy's own SQLite. Delete the
database and Immich is byte-for-byte untouched.

## Stack

Rust, [axum](https://github.com/tokio-rs/axum), and SQLite
([sqlx](https://github.com/launchbadge/sqlx)) on the backend.
[Svelte 5](https://svelte.dev), [Phosphor icons](https://phosphoricons.com), and
[PhotoSwipe](https://photoswipe.com) on the frontend. Single multi-stage Docker image.

## Quick start

```sh
docker build -t interactive-ipp .
docker run --rm -p 3000:3000 \
  -e IMMICH_URL=http://immich-server:2283 \
  -e PUBLIC_BASE_URL=http://localhost:3000 \
  -e COOKIE_SECRET="$(openssl rand -hex 32)" \
  -v ipp-data:/data \
  interactive-ipp
```

Then open `http://localhost:3000/share/<immich-share-key>` (the part after `/share/` in an
Immich public-album link). The album syncs on first visit.

### Point Immich's share links at this proxy

Immich builds its public share links from its **External Domain** setting. Set it to this
service's public URL so the links Immich generates use that domain:

Immich: Administration > Settings > Server Settings > External Domain = `https://photos.example.com`

Without this, a generated link looks like `https://your-immich-host/share/<key>` and hits
Immich directly. With it set, Immich produces `https://photos.example.com/share/<key>`, which
this proxy serves. The share key is identical either way, so you can also just swap the host
in an existing link.

### Configuration (env)

| Var | Default | Notes |
|---|---|---|
| `IMMICH_URL` | `http://immich_server:2283` | internal Immich API (read-only) |
| `PUBLIC_BASE_URL` | `http://localhost:3000` | public URL of this service |
| `COOKIE_SECRET` | *(ephemeral)* | set a stable secret (16+ chars) so cookies survive restarts |
| `DB_PATH` | `/data/ipp.db` | SQLite file (mount a volume) |
| `BIND` | `0.0.0.0:3000` | listen address |
| `SYNC_TTL_SECS` | `3600` | how often to refresh an album's cached asset list |
| `MAX_NOTE_LEN` | `2000` | note length cap |
| `MAX_DOWNLOAD_COUNT` / `MAX_DOWNLOAD_BYTES` | `300` / `2 GiB` | multi-download caps |

There is an example reverse-proxy and compose setup in [DEPLOY.md](./DEPLOY.md).

## Development

```sh
cd web
npm install
BACKEND=http://localhost:3000 npm run dev   # Vite on :5173, proxies /api to your backend
# open http://localhost:5173/share/<key>
```

Run a backend separately (the Docker image, or `cargo run`) and point `BACKEND` at it.

## Security

- Immich is only ever read (no admin API key, just the public share key).
- User content (notes and names) is stored and served as plain text. Render it as text,
  never HTML.
- Public visitors can write marks and notes by design (it is a family share). A simple
  in-memory rate limiter is included; for an internet-facing deployment, add IP-based
  limiting and a CSP header at the reverse proxy.
- The `/admin/*` moderation API has no built-in auth. Gate it at the reverse proxy (IP
  allowlist or SSO). See [DEPLOY.md](./DEPLOY.md).
- Not audited for adversarial use.

## Credits

- [Immich](https://immich.app), the photo server this runs against.
- [immich-public-proxy](https://github.com/alangrainger/immich-public-proxy) by
  [Alan Grainger](https://github.com/alangrainger), the inspiration and the reference for
  password-protected shares. Use IPP if you want stateless and simple.

## License

[AGPL-3.0](./LICENSE), matching Immich and immich-public-proxy. If you run a modified
version as a network service, AGPL section 13 requires offering your source to its users.
