<script lang="ts">
  import { onMount } from 'svelte';
  import { CaretRight, LockKey } from 'phosphor-svelte';
  import { listAlbums, type AlbumSummary } from '../api';

  let albums: AlbumSummary[] = [];
  let loading = true;
  let error = '';

  /** 'N photos · M videos' with zero parts omitted ('' for an empty album). */
  function countLine(album: AlbumSummary): string {
    const parts: string[] = [];
    if (album.photos > 0)
      parts.push(`${album.photos.toLocaleString()} photo${album.photos === 1 ? '' : 's'}`);
    if (album.videos > 0)
      parts.push(`${album.videos.toLocaleString()} video${album.videos === 1 ? '' : 's'}`);
    return parts.join(' · ');
  }

  onMount(async () => {
    document.title = 'Interactive Immich Public Proxy';
    try {
      albums = await listAlbums();
    } catch {
      error = 'Could not load albums.';
    } finally {
      loading = false;
    }
  });
</script>

<div class="index">
  <header>
    <!-- Same mark as the favicon, drawn with the live accent token. -->
    <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
      <rect width="32" height="32" rx="7" fill="var(--accent)" />
      <circle cx="21.5" cy="10.5" r="3.2" fill="#fff" opacity="0.85" />
      <path d="M4 25l8.5-11.5 5.5 6.5 3.5-4.5 6.5 9.5z" fill="#fff" />
    </svg>
    <div class="head-text">
      <h1>Interactive Immich Public Proxy</h1>
      <p class="tagline">
        {#if !loading && !error && albums.length > 0}
          {albums.length} shared album{albums.length === 1 ? '' : 's'}
        {:else}
          Shared albums
        {/if}
      </p>
    </div>
  </header>

  {#if loading}
    <div class="list">
      {#each Array(3) as _, i (i)}
        <div class="card skeleton-card">
          <div class="line skeleton"></div>
        </div>
      {/each}
    </div>
  {:else if error}
    <div class="state error">
      <h2>Something went wrong</h2>
      <p>{error}</p>
    </div>
  {:else if albums.length === 0}
    <div class="state empty">
      <p>No albums here yet.</p>
    </div>
  {:else}
    <div class="list">
      {#each albums as album (album.key)}
        <a class="card" href={'/share/' + album.key}>
          <div class="info">
            <div class="title-row">
              <span class="title">{album.title || 'Untitled album'}</span>
              {#if album.needsPassword}
                <span class="lock" title="Password protected">
                  <LockKey size={13} weight="fill" />
                  Protected
                </span>
              {/if}
            </div>
            {#if countLine(album)}
              <div class="counts">{countLine(album)}</div>
            {/if}
          </div>
          <CaretRight size={16} class="go" />
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .index {
    max-width: 640px;
    margin: 0 auto;
    padding: 32px 16px 48px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 0 22px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 20px;
  }
  .mark {
    width: 38px;
    height: 38px;
    flex: none;
  }
  h1 {
    margin: 0;
    font-size: clamp(17px, 3.5vw, 22px);
    letter-spacing: -0.01em;
    color: var(--text);
  }
  .tagline {
    margin: 2px 0 0;
    color: var(--text-dim);
    font-size: 13.5px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    text-decoration: none;
    color: var(--text);
    transition:
      background 0.15s ease,
      box-shadow 0.15s ease;
  }
  .card:hover {
    background: var(--bg-elev-2);
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.07);
  }
  .card :global(.go) {
    flex: none;
    color: var(--text-dim);
  }
  .info {
    min-width: 0;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lock {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11.5px;
    color: var(--text-dim);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 2px 7px;
    line-height: 1.5;
  }
  .counts {
    margin-top: 2px;
    color: var(--text-dim);
    font-size: 13px;
  }
  /* Loading skeletons — same shimmer as the gallery's. */
  .skeleton-card .line {
    height: 18px;
    width: 55%;
  }
  .skeleton {
    background: linear-gradient(
      100deg,
      var(--bg-elev) 30%,
      var(--bg-elev-2) 50%,
      var(--bg-elev) 70%
    );
    background-size: 200% 100%;
    animation: shimmer 1.3s infinite linear;
    border-radius: var(--radius);
  }
  @keyframes shimmer {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }
  .state {
    text-align: center;
    color: var(--text-dim);
    padding: 80px 20px;
  }
  .state h2 {
    color: var(--text);
    margin: 0 0 8px;
  }
  .state.error h2 {
    color: var(--danger);
  }
</style>
