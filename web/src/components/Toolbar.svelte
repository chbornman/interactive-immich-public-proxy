<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import {
    SquaresFour,
    Star,
    ChatCircle,
    MagnifyingGlass,
    X,
    CheckSquare,
    DownloadSimple,
    User,
    Square,
    Image,
    VideoCamera,
    Stack,
    Export,
    GithubLogo,
    Play,
  } from 'phosphor-svelte';
  import type { FilterName, KindFilter } from '../types';
  import { supportsShareFiles } from '../api';

  export let title = '';
  export let total = 0;
  export let photos = 0;
  export let videos = 0;
  export let filter: FilterName = 'all';
  export let kind: KindFilter = 'all';
  export let query = '';
  export let selectMode = false;
  export let selectedCount = 0;
  export let downloading = false;
  export let marking = false;
  export let visitorName = '';
  export let tileSize = 220;

  const dispatch = createEventDispatcher<{
    filter: { filter: FilterName };
    search: { q: string };
    toggleSelect: void;
    download: void;
    markSelected: void;
    unmarkSelected: void;
    editName: void;
    slideshow: void;
    size: { value: number };
    kind: { kind: KindFilter };
  }>();

  const nf = new Intl.NumberFormat();
  $: countLabel =
    videos > 0
      ? `${nf.format(photos)} photos · ${nf.format(videos)} videos`
      : `${nf.format(total)} ${total === 1 ? 'photo' : 'photos'}`;

  let localQuery = query;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  function onInput() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => dispatch('search', { q: localQuery.trim() }), 300);
  }
  function clearSearch() {
    localQuery = '';
    clearTimeout(debounceTimer);
    dispatch('search', { q: '' });
  }
  function setFilter(f: FilterName) {
    dispatch('filter', { filter: f });
  }
  function setKind(k: KindFilter) {
    dispatch('kind', { kind: k });
  }

  onDestroy(() => clearTimeout(debounceTimer));
</script>

<header class="toolbar">
  <div class="title-block">
    <h1 title={title}>{title || 'Shared Album'}</h1>
    <span class="count">{countLabel}</span>
  </div>

  <div class="filters" role="group" aria-label="Filter">
    <button class:active={filter === 'all'} on:click={() => setFilter('all')} title="All">
      <SquaresFour size={16} weight={filter === 'all' ? 'fill' : 'regular'} />
      <span class="label">All</span>
    </button>
    <button class:active={filter === 'marked'} on:click={() => setFilter('marked')} title="Marked">
      <Star size={16} weight={filter === 'marked' ? 'fill' : 'regular'} />
      <span class="label">Marked</span>
    </button>
    <button class:active={filter === 'noted'} on:click={() => setFilter('noted')} title="Has notes">
      <ChatCircle size={16} weight={filter === 'noted' ? 'fill' : 'regular'} />
      <span class="label">Notes</span>
    </button>
  </div>

  <div class="filters types" role="group" aria-label="Type">
    <button class:active={kind === 'all'} on:click={() => setKind('all')} title="All types" aria-label="All types">
      <Stack size={16} weight={kind === 'all' ? 'fill' : 'regular'} />
    </button>
    <button class:active={kind === 'image'} on:click={() => setKind('image')} title="Photos only" aria-label="Photos only">
      <Image size={16} weight={kind === 'image' ? 'fill' : 'regular'} />
    </button>
    <button class:active={kind === 'video'} on:click={() => setKind('video')} title="Videos only" aria-label="Videos only">
      <VideoCamera size={16} weight={kind === 'video' ? 'fill' : 'regular'} />
    </button>
  </div>

  <div class="search">
    <MagnifyingGlass size={15} class="s-icon" />
    <input
      type="search"
      placeholder="Search…"
      bind:value={localQuery}
      on:input={onInput}
      aria-label="Search photos"
    />
    {#if localQuery}
      <button class="clear" on:click={clearSearch} aria-label="Clear search"><X size={14} /></button>
    {/if}
  </div>

  <div class="sizer" title="Thumbnail size">
    <Square size={11} weight="bold" />
    <input
      type="range"
      min="110"
      max="380"
      step="10"
      value={tileSize}
      on:input={(e) => dispatch('size', { value: Number(e.currentTarget.value) })}
      aria-label="Thumbnail size"
    />
    <Square size={16} weight="bold" />
  </div>

  <div class="actions">
    {#if selectMode && selectedCount > 0}
      <button class="mark-btn" on:click={() => dispatch('markSelected')} disabled={marking} title="Mark selected">
        <Star size={16} weight="fill" />
        <span class="label">Mark ({selectedCount})</span>
      </button>
      <button class="unmark-btn" on:click={() => dispatch('unmarkSelected')} disabled={marking} title="Unmark selected">
        <Star size={16} weight="regular" />
        <span class="label">Unmark ({selectedCount})</span>
      </button>
      <button
        class="primary"
        on:click={() => dispatch('download')}
        disabled={downloading}
        title={supportsShareFiles ? 'Share selected' : 'Download selected'}
      >
        {#if supportsShareFiles}<Export size={16} />{:else}<DownloadSimple size={16} />{/if}
        <span class="label">
          {downloading ? 'Preparing…' : `${supportsShareFiles ? 'Share' : 'Download'} (${selectedCount})`}
        </span>
      </button>
    {/if}
    <button on:click={() => dispatch('slideshow')} title="Slideshow" aria-label="Slideshow">
      <Play size={16} weight="fill" />
      <span class="label">Slideshow</span>
    </button>
    <button class:active={selectMode} on:click={() => dispatch('toggleSelect')} title="Select">
      <CheckSquare size={16} weight={selectMode ? 'fill' : 'regular'} />
      <span class="label">{selectMode ? 'Done' : 'Select'}</span>
    </button>
    <button class="name-btn" on:click={() => dispatch('editName')} title="Set or change your display name">
      <User size={16} />
      <span class="label">{visitorName || 'Set name'}</span>
    </button>
    <a
      class="src-link"
      href="https://github.com/chbornman/interactive-immich-public-proxy"
      target="_blank"
      rel="noopener noreferrer"
      title="Source code (AGPL-3.0)"
      aria-label="Source code"
    >
      <GithubLogo size={16} />
    </a>
  </div>
</header>

<style>
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 10px;
    padding: 8px max(10px, env(safe-area-inset-left, 0)) 8px max(10px, env(safe-area-inset-left, 0));
    padding-top: calc(8px + env(safe-area-inset-top, 0));
    background: #fff;
    border-bottom: 1px solid var(--border);
  }
  .title-block {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    margin-right: auto;
  }
  h1 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 40vw;
  }
  .count {
    color: var(--text-dim);
    font-size: 12px;
    white-space: nowrap;
  }
  .filters {
    display: flex;
    gap: 3px;
    background: var(--bg-elev);
    padding: 3px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .filters button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    padding: 6px 10px;
    border-radius: calc(var(--radius) - 2px);
    font-size: 13px;
  }
  .filters button.active {
    background: var(--accent);
    color: #fff;
  }
  .search {
    position: relative;
    display: flex;
    align-items: center;
  }
  .search :global(.s-icon) {
    position: absolute;
    left: 9px;
    color: var(--text-dim);
    pointer-events: none;
  }
  .search input {
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 7px 28px 7px 30px;
    width: 200px;
    max-width: 46vw;
  }
  .search input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .search .clear {
    position: absolute;
    right: 5px;
    display: inline-flex;
    border: none;
    background: transparent;
    color: var(--text-dim);
    padding: 3px;
  }
  .sizer {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
  }
  .sizer input[type='range'] {
    width: 110px;
    accent-color: var(--accent);
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .actions button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text);
    padding: 7px 12px;
    border-radius: var(--radius);
    font-size: 13px;
  }
  .actions button.active {
    border-color: var(--accent);
    color: var(--accent);
  }
  .actions button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .actions button.mark-btn {
    border-color: var(--accent);
    color: var(--accent);
  }
  .actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .src-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text-dim);
    padding: 8px;
    border-radius: var(--radius);
  }
  .src-link:hover {
    color: var(--text);
  }

  /* Compact: collapse labels to icon-only on small screens */
  @media (max-width: 680px) {
    .label {
      display: none;
    }
    .actions button,
    .filters button {
      padding: 8px;
    }
    h1 {
      max-width: 52vw;
    }
    .count {
      display: none;
    }
    .search input {
      width: 130px;
    }
    .sizer {
      display: none;
    }
  }
</style>
