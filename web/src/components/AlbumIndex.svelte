<script lang="ts">
  import { onMount } from 'svelte';
  import { listAlbums, type AlbumSummary } from '../api';

  let albums: AlbumSummary[] = [];
  let loading = true;
  let error = '';

  /** Thumbnail URL for an album's cover asset. */
  function coverUrl(album: AlbumSummary): string {
    return `/api/s/${encodeURIComponent(album.key)}/media/${encodeURIComponent(
      album.cover ?? '',
    )}/thumbnail`;
  }

  /** 'N photos · M videos' with zero parts omitted ('' for an empty album). */
  function countLine(album: AlbumSummary): string {
    const parts: string[] = [];
    if (album.photos > 0) parts.push(`${album.photos} photo${album.photos === 1 ? '' : 's'}`);
    if (album.videos > 0) parts.push(`${album.videos} video${album.videos === 1 ? '' : 's'}`);
    return parts.join(' · ');
  }

  onMount(async () => {
    document.title = 'Albums';
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
    <h1>Albums</h1>
  </header>

  {#if loading}
    <div class="grid">
      {#each Array(6) as _, i (i)}
        <div class="card skeleton-card">
          <div class="cover skeleton"></div>
          <div class="info">
            <div class="line skeleton"></div>
          </div>
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
    <div class="grid">
      {#each albums as album (album.key)}
        <a class="card" href={'/share/' + album.key}>
          {#if album.cover}
            <img class="cover" src={coverUrl(album)} alt="" loading="lazy" />
          {:else}
            <div class="cover placeholder"></div>
          {/if}
          <div class="info">
            <div class="title">{album.title || 'Untitled album'}</div>
            {#if countLine(album)}
              <div class="counts">{countLine(album)}</div>
            {/if}
          </div>
        </a>
      {/each}
    </div>
  {/if}
</div>

<style>
  .index {
    max-width: 1100px;
    margin: 0 auto;
    padding: 24px 16px 48px;
  }
  header {
    padding: 8px 0 20px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 20px;
  }
  h1 {
    margin: 0;
    font-size: 22px;
    color: var(--text);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 16px;
  }
  .card {
    display: block;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    text-decoration: none;
    color: var(--text);
    transition: background 0.15s ease;
  }
  .card:hover {
    background: var(--bg-elev-2);
  }
  .cover {
    display: block;
    width: 100%;
    aspect-ratio: 3 / 2;
    object-fit: cover;
  }
  .cover.placeholder {
    background: var(--bg-elev-2);
  }
  .info {
    padding: 10px 12px;
  }
  .title {
    font-weight: 600;
  }
  .counts {
    margin-top: 2px;
    color: var(--text-dim);
    font-size: 13px;
  }
  /* Loading skeletons — same shimmer as the gallery's. */
  .skeleton-card .line {
    height: 16px;
    width: 70%;
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
