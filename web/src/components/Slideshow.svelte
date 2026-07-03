<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount, tick } from 'svelte';
  import { X, CaretLeft, CaretRight, Pause, Play } from 'phosphor-svelte';
  import type { Asset } from '../types';
  import { assetUrl } from '../api';

  export let items: Asset[] = [];
  /** Index to start at; set to a number to start, null to keep inactive. */
  export let startIndex: number | null = null;
  /** True while more assets are being fetched in the background. */
  export let loading = false;

  const dispatch = createEventDispatcher<{ close: { index: number } }>();

  /** Seconds each image is shown. Videos play to their natural end then advance. */
  const IMAGE_DURATION = 5;

  let host: HTMLDivElement;
  let videoEl: HTMLVideoElement;
  let index = 0;
  let running = false; // paused state
  let active: Asset | null = null;
  let isVideo = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let fsChangeHandler: () => void;
  /** Current video 'ended' listener, tracked so it can be torn down before reuse. */
  let onEnded: (() => void) | null = null;
  /** True once close() has begun, to make the exit path re-entry-safe. */
  let closing = false;
  /** Controls fade out after a few idle seconds; any pointer/key brings them back. */
  let barVisible = true;
  let barTimer: ReturnType<typeof setTimeout> | undefined;

  function showBar() {
    barVisible = true;
    clearTimeout(barTimer);
    barTimer = setTimeout(() => {
      barVisible = false;
    }, 3000);
  }

  $: if (startIndex !== null && items.length > 0 && active === null) {
    begin(startIndex);
  }

  function begin(i: number) {
    index = Math.max(0, Math.min(i, items.length - 1));
    running = true;
    closing = false;
    // Only listen for keys while the slideshow is actually active, so arrow keys
    // in the lightbox never leak in and pop the slideshow open.
    window.addEventListener('keydown', onKey);
    showBar();
    mountSlide();
    // .slide-host only mounts once `active` is set, so the fullscreen request
    // must wait for the bind (still within the launching click's activation).
    void tick().then(enterFullscreen);
  }

  async function enterFullscreen() {
    try {
      await host.requestFullscreen?.();
    } catch {
      /* fullscreen blocked — still run inline, fullscreen is best-effort */
    }
  }

  async function exitFullscreen() {
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
    } catch {
      /* ignore */
    }
  }

  function mountSlide() {
    clearTimeout(timer);
    // Tear down the outgoing slide's media before switching so listeners can't
    // stack on the reused <video> element and no audio bleeds across slides.
    teardownMedia();
    active = items[index] ?? null;
    if (!active) {
      // Index beyond currently-loaded assets (background load in flight) — retry shortly.
      timer = setTimeout(mountSlide, 400);
      return;
    }
    isVideo = active.kind === 'VIDEO';
    // Wait for Svelte to mount the video element (or image) before wiring events.
    queueMicrotask(attachMediaHandlers);
    // Preload the next slide's image (or video poster) so advancing never flashes.
    const next = items[(index + 1) % items.length];
    if (next) new Image().src = assetUrl(next.id, 'preview');
  }

  /** Remove any tracked video listener and pause the current video element. */
  function teardownMedia() {
    if (onEnded && videoEl) videoEl.removeEventListener('ended', onEnded);
    onEnded = null;
    if (videoEl) videoEl.pause();
  }

  function attachMediaHandlers() {
    if (isVideo && videoEl) {
      onEnded = () => advance();
      videoEl.addEventListener('ended', onEnded);
      videoEl.muted = false;
      videoEl.play().catch((err) => {
        if (err instanceof DOMException && err.name === 'NotAllowedError') {
          videoEl.muted = true;
          videoEl.play().catch(() => {});
        }
      });
    } else if (!isVideo) {
      timer = setTimeout(advance, IMAGE_DURATION * 1000);
    }
  }

  function advance() {
    if (!running) return;
    if (index >= items.length - 1) {
      // Loop back to the start for a continuous show.
      index = 0;
    } else {
      index += 1;
    }
    mountSlide();
  }

  function prev() {
    clearTimeout(timer);
    if (index <= 0) index = items.length - 1;
    else index -= 1;
    mountSlide();
  }

  function next() {
    clearTimeout(timer);
    if (index >= items.length - 1) index = 0;
    else index += 1;
    mountSlide();
  }

  function togglePlay() {
    running = !running;
    if (running) {
      if (isVideo && videoEl) {
        videoEl.play().catch(() => {});
      } else {
        advance();
      }
    } else {
      clearTimeout(timer);
      if (isVideo && videoEl) videoEl.pause();
    }
  }

  function close() {
    // Guard re-entry: exitFullscreen() fires fullscreenchange, which calls close()
    // again — bail if we've already started closing so we don't dispatch twice.
    if (closing) return;
    closing = true;
    clearTimeout(timer);
    clearTimeout(barTimer);
    teardownMedia();
    window.removeEventListener('keydown', onKey);
    exitFullscreen();
    const finalIndex = index;
    active = null;
    startIndex = null;
    dispatch('close', { index: finalIndex });
  }

  /** Click/tap the media toggles playback (video-player convention) and
      recalls the faded control bar — the only recall touch users have. */
  function onHostClick(e: MouseEvent) {
    if ((e.target as HTMLElement | null)?.closest('.bar')) return;
    showBar();
    togglePlay();
  }

  function onKey(e: KeyboardEvent) {
    showBar();
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      next();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      prev();
    } else if (e.key === ' ') {
      e.preventDefault();
      togglePlay();
    } else if (e.key === 'f' || e.key === 'F') {
      e.preventDefault();
      if (document.fullscreenElement) exitFullscreen();
      else enterFullscreen();
    }
  }

  onMount(() => {
    fsChangeHandler = () => {
      if (!document.fullscreenElement) close();
    };
    document.addEventListener('fullscreenchange', fsChangeHandler);
  });

  onDestroy(() => {
    clearTimeout(timer);
    clearTimeout(barTimer);
    document.removeEventListener('fullscreenchange', fsChangeHandler);
    window.removeEventListener('keydown', onKey);
  });
</script>

{#if active}
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
  <div class="slide-host" bind:this={host} on:pointermove={showBar} on:click={onHostClick}>
    {#if isVideo}
      <video
        bind:this={videoEl}
        class="media"
        src={assetUrl(active.id, 'original')}
        poster={assetUrl(active.id, 'preview')}
        playsinline
        preload="metadata"
      >
        <track kind="captions" />
      </video>
    {:else}
      <img class="media" src={assetUrl(active.id, 'preview')} alt={active.filename} />
    {/if}

    <div class="bar" class:faded={!barVisible}>
      <button class="bbtn" on:click={prev} title="Previous (←)" aria-label="Previous"><CaretLeft size={18} weight="bold" /></button>
      <button class="bbtn" on:click={togglePlay} title={running ? 'Pause (Space)' : 'Play (Space)'} aria-label={running ? 'Pause' : 'Play'}>
        {#if running}<Pause size={18} weight="fill" />{:else}<Play size={18} weight="fill" />{/if}
      </button>
      <button class="bbtn" on:click={next} title="Next (→)" aria-label="Next"><CaretRight size={18} weight="bold" /></button>
      <span class="counter">
        {index + 1} / {items.length}{#if loading} <span class="loading">·</span>{/if}
      </span>
      <button class="bbtn end" on:click={close} title="Exit slideshow (Esc)" aria-label="Exit slideshow"><X size={18} /></button>
    </div>
  </div>
{/if}

<style>
  .slide-host {
    position: fixed;
    inset: 0;
    z-index: 4000000;
    background: #000;
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .slide-host:fullscreen {
    background: #000;
  }
  .media {
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: auto;
    object-fit: contain;
    background: #000;
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    transition: opacity 0.25s ease;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px max(16px, env(safe-area-inset-left, 0)) calc(12px + env(safe-area-inset-bottom, 0)) max(16px, env(safe-area-inset-left, 0));
    background: var(--bg-elev);
    border-top: 1px solid var(--border);
  }
  /* Idle: controls fade out; any pointer move or key brings them back. */
  .bar.faded {
    opacity: 0;
    pointer-events: none;
  }
  .bbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 34px;
    min-width: 34px;
    padding: 0 9px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text);
    border-radius: var(--radius);
    transition: background 0.15s ease, border-color 0.15s ease;
  }
  .bbtn:hover:not(:disabled) {
    background: var(--bg-elev-2);
  }
  .bbtn.end {
    margin-left: auto;
  }
  .counter {
    color: var(--text-dim);
    font-size: 14px;
    font-variant-numeric: tabular-nums;
    margin-left: 8px;
  }
  .loading {
    opacity: 0.7;
    animation: pulse 1s infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 0.3; }
    50% { opacity: 1; }
  }
</style>
