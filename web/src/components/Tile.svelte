<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { Play, Star, ChatCircle, Check } from 'phosphor-svelte';
  import type { Asset } from '../types';
  import { assetUrl } from '../api';

  export let asset: Asset;
  export let width: number;
  export let height: number;
  export let selectMode = false;
  export let selected = false;

  const dispatch = createEventDispatcher<{ activate: { asset: Asset } }>();

  function onClick() {
    dispatch('activate', { asset });
  }
</script>

<button
  class="tile"
  style="width:{width}px;height:{height}px;"
  on:click={onClick}
  aria-label={asset.filename}
>
  <img
    src={assetUrl(asset.id, 'thumbnail')}
    alt={asset.filename}
    width={Math.round(width)}
    height={Math.round(height)}
    loading="lazy"
    decoding="async"
    draggable="false"
  />

  {#if asset.kind === 'VIDEO'}
    <span class="badge play" aria-hidden="true">
      <Play size={16} weight="fill" />
    </span>
  {/if}

  {#if asset.markCount > 0 || asset.hasNote}
    <span class="indicators">
      {#if asset.markCount > 0}
        <span class="chip" title="{asset.markCount} mark(s)">
          <Star size={11} weight="fill" />{asset.markCount}
        </span>
      {/if}
      {#if asset.hasNote}
        <span class="chip" title="Has notes"><ChatCircle size={12} weight="fill" /></span>
      {/if}
    </span>
  {/if}

  {#if selectMode}
    <span class="checkbox" class:checked={selected} aria-hidden="true">
      {#if selected}<Check size={14} weight="bold" />{/if}
    </span>
  {/if}
</button>

<style>
  .tile {
    position: relative;
    padding: 0;
    border: none;
    background: var(--bg-elev);
    border-radius: var(--radius);
    overflow: hidden;
    display: block;
    flex: none;
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.25s ease;
  }
  .tile:hover img {
    transform: scale(1.04);
  }
  .badge.play {
    position: absolute;
    top: 6px;
    left: 6px;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    border-radius: 50%;
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    pointer-events: none;
  }
  .indicators {
    position: absolute;
    bottom: 6px;
    left: 6px;
    display: flex;
    gap: 4px;
    pointer-events: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    font-size: 11px;
    padding: 2px 6px;
    border-radius: var(--radius);
    line-height: 1.4;
  }
  .checkbox {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid #fff;
    background: rgba(0, 0, 0, 0.35);
    color: #fff;
    display: grid;
    place-items: center;
  }
  .checkbox.checked {
    background: var(--accent);
    border-color: var(--accent);
  }
</style>
