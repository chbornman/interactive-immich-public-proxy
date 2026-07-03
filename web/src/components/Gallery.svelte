<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import type { Asset } from '../types';
  import { layoutJustified, type LaidOutRow } from '../layout';
  import Tile from './Tile.svelte';

  export let assets: Asset[] = [];
  export let loading = false; // initial / page load in flight
  export let hasMore = false;
  export let error = '';
  export let emptyMessage = 'No photos match.';
  export let selectMode = false;
  export let selectedIds: Set<string> = new Set();
  export let targetRowHeight = 220;
  /** Index to scroll to after render (used when returning from lightbox/slideshow). */
  export let galleryScrollTarget: number | null = null;

  const dispatch = createEventDispatcher<{
    loadMore: void;
    activate: { asset: Asset; index: number };
    toggleSelect: { asset: Asset };
  }>();

  const GAP = 4;

  let containerEl: HTMLDivElement;
  let containerWidth = 0;
  let rows: LaidOutRow[] = [];

  let sentinel: HTMLDivElement;
  let io: IntersectionObserver | null = null;
  let ro: ResizeObserver | null = null;

  $: rows = layoutJustified(assets, {
    containerWidth,
    targetRowHeight,
    gap: GAP,
  });

  /** Scroll to the tile at galleryScrollTarget after the gallery re-renders. */
  $: if (galleryScrollTarget !== null && assets.length > 0 && containerEl) {
    const targetIndex = galleryScrollTarget;
    // Wait a frame so the justified layout is settled before measuring.
    requestAnimationFrame(() => {
      // containerEl is nulled by Svelte if we unmount before the frame fires.
      if (!containerEl) return;
      const tiles = containerEl.querySelectorAll('.tile');
      const target = tiles[targetIndex];
      // No-op if the target tile isn't present (e.g. filtered out).
      target?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
  }

  function onActivate(asset: Asset) {
    if (selectMode) {
      dispatch('toggleSelect', { asset });
      return;
    }
    const index = assets.findIndex((a) => a.id === asset.id);
    dispatch('activate', { asset, index });
  }

  onMount(() => {
    ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        containerWidth = entry.contentRect.width;
      }
    });
    ro.observe(containerEl);
    containerWidth = containerEl.clientWidth;

    io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting && hasMore && !loading) {
            dispatch('loadMore');
          }
        }
      },
      { rootMargin: '600px 0px' },
    );
    if (sentinel) io.observe(sentinel);
  });

  onDestroy(() => {
    ro?.disconnect();
    io?.disconnect();
  });
</script>

<div class="gallery" bind:this={containerEl}>
  {#if error}
    <div class="state error">
      <h2>Something went wrong</h2>
      <p>{error}</p>
    </div>
  {:else if !loading && assets.length === 0}
    <div class="state empty">
      <p>{emptyMessage}</p>
    </div>
  {:else}
    {#each rows as row, ri (ri)}
      <div class="row" style="height:{row.height}px;gap:{GAP}px;">
        {#each row.tiles as t (t.asset.id)}
          <Tile
            asset={t.asset}
            width={t.width}
            height={t.height}
            {selectMode}
            selected={selectedIds.has(t.asset.id)}
            on:activate={(e) => onActivate(e.detail.asset)}
          />
        {/each}
      </div>
    {/each}

    {#if loading}
      <div class="loading-row" style="height:{targetRowHeight}px;">
        {#each Array(6) as _, i (i)}
          <div class="skeleton" style="flex:{1 + (i % 3)}"></div>
        {/each}
      </div>
    {/if}
  {/if}

  <div class="sentinel" bind:this={sentinel}></div>
</div>

<style>
  .gallery {
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
  }
  .row {
    display: flex;
    flex-direction: row;
  }
  .loading-row {
    display: flex;
    gap: 4px;
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
  .sentinel {
    height: 1px;
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
