# Deploying

An example of running `interactive-ipp` next to Immich behind a reverse proxy. Adapt the
hostnames, networks, and IPs to your setup.

## 1. Build the image

```sh
docker build -t interactive-ipp:latest .
```

## 2. docker-compose service

Put it on the same Docker network as your Immich server so it can reach the internal API.

```yaml
  interactive-ipp:
    image: interactive-ipp:latest
    container_name: interactive-ipp
    restart: unless-stopped
    environment:
      - IMMICH_URL=http://immich-server:2283      # internal Immich API (read-only)
      - PUBLIC_BASE_URL=https://photos.example.com # this service's public URL
      - COOKIE_SECRET=${IPP_COOKIE_SECRET}         # openssl rand -hex 32, in your .env
      - DB_PATH=/data/ipp.db
    volumes:
      - ./ipp-data:/data                           # marks/notes/visitor SQLite
    networks: [immich]
```

Add `IPP_COOKIE_SECRET=<openssl rand -hex 32>` to your `.env` (a stable value so visitor
cookies survive restarts).

## 3. Reverse proxy (Caddy example)

```
photos.example.com {
    header {
        Strict-Transport-Security "max-age=31536000; includeSubDomains; preload"
        X-Content-Type-Options "nosniff"
        Referrer-Policy "strict-origin-when-cross-origin"
        # A CSP backstop is recommended (notes are user content rendered as text):
        # Content-Security-Policy "default-src 'self'; img-src 'self' data:; media-src 'self'; object-src 'none'; base-uri 'none'"
    }

    # The /admin/* moderation API has no built-in auth — gate it.
    # Example: allow only LAN/VPN ranges; everyone else gets 403.
    @admin_blocked {
        path /admin/*
        not remote_ip 10.0.0.0/8 172.16.0.0/12 192.168.0.0/16 127.0.0.1
    }
    respond @admin_blocked "Access Denied" 403

    reverse_proxy interactive-ipp:3000
}
```

> **Important:** the `/admin/*` endpoints (moderation: list/hide notes, ban visitors, resync,
> toggle per-album settings) have **no authentication in the app**. You must gate them at the
> proxy — by IP allowlist (as above) or behind SSO (e.g. Authentik `forward_auth`). If the
> proxy can be reached directly on the internal network, anyone on that network gets admin.

## 4. Point Immich at it

In Immich: **Administration → Settings → Server Settings → External Domain** =
`https://photos.example.com`, so the public-album links Immich generates point here.

## Notes

- First visit to an album triggers a one-time read-only sync of its asset list from Immich
  (seconds even for ~15k assets). After that it's cached and paginated; the list refreshes
  every `SYNC_TTL_SECS`.
- The SQLite DB in `/data` holds all marks/notes — back it up if you care about them.
- `interactive-ipp:3000` serves both the SPA and the API; no separate frontend host needed.

## Caddy single-file mount gotcha

If you bind-mount the `Caddyfile` as a single file, editing it changes the file's inode but
the running container keeps the old one — `caddy reload` will report "config unchanged." After
editing, **recreate** the container (`docker compose up -d --force-recreate caddy`) instead of
reloading.
