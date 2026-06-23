<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';
  import { X, CaretLeft, CaretRight, Pause, Play } from 'phosphor-svelte';
  import type { Asset } from '../types';
  import { assetUrl } from '../api';

  export let items: Asset[] = [];
  /** Index to start at; set to a number to start, null to keep inactive. */
  export let startIndex: number | null = null;

  const dispatch = createEventDispatcher<{ close: void }>();

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

  $: if (startIndex !== null && items.length > 0 && active === null) {
    begin(startIndex);
  }

  function begin(i: number) {
    index = Math.max(0, Math.min(i, items.length - 1));
    running = true;
    enterFullscreen();
    mountSlide();
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
    active = items[index] ?? null;
    if (!active) return;
    isVideo = active.kind === 'VIDEO';
    // Wait for Svelte to mount the video element (or image) before wiring events.
    queueMicrotask(attachMediaHandlers);
  }

  function attachMediaHandlers() {
    if (isVideo && videoEl) {
      const onEnded = () => {
        videoEl.removeEventListener('ended', onEnded);
        advance();
      };
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
    clearTimeout(timer);
    if (videoEl) videoEl.pause();
    exitFullscreen();
    active = null;
    startIndex = null;
    dispatch('close');
  }

  function onKey(e: KeyboardEvent) {
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
    }
  }

  onMount(() => {
    fsChangeHandler = () => {
      if (!document.fullscreenElement) close();
    };
    document.addEventListener('fullscreenchange', fsChangeHandler);
    window.addEventListener('keydown', onKey);
  });

  onDestroy(() => {
    clearTimeout(timer);
    document.removeEventListener('fullscreenchange', fsChangeHandler);
    window.removeEventListener('keydown', onKey);
  });
</script>

{#if active}
  <div class="slide-host" bind:this={host}>
    {#if isVideo}
      <video
        bind:this={videoEl}
        class="media"
        src={assetUrl(active.id, 'original')}
        poster={assetUrl(active.id, 'preview')}
        controls
        playsinline
        preload="metadata"
      >
        <track kind="captions" />
      </video>
    {:else}
      <img class="media" src={assetUrl(active.id, 'preview')} alt={active.filename} />
    {/if}

    <div class="bar">
      <button class="bbtn" on:click={prev} title="Previous" aria-label="Previous"><CaretLeft size={20} weight="bold" /></button>
      <button class="bbtn" on:click={togglePlay} title={running ? 'Pause' : 'Play'} aria-label={running ? 'Pause' : 'Play'}>
        {#if running}<Pause size={20} weight="fill" />{:else}<Play size={20} weight="fill" />{/if}
      </button>
      <button class="bbtn" on:click={next} title="Next" aria-label="Next"><CaretRight size={20} weight="bold" /></button>
      <span class="counter">{index + 1} / {items.length}</span>
      <button class="bbtn end" on:click={close} title="Exit slideshow" aria-label="Exit slideshow"><X size={20} /></button>
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
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px max(16px, env(safe-area-inset-left, 0)) calc(12px + env(safe-area-inset-bottom, 0)) max(16px, env(safe-area-inset-left, 0));
    background: linear-gradient(to top, rgba(0, 0, 0, 0.7), transparent);
  }
  .bbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
    border: none;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
    backdrop-filter: blur(4px);
  }
  .bbtn:hover {
    background: rgba(255, 255, 255, 0.28);
  }
  .bbtn.end {
    margin-left: auto;
  }
  .counter {
    color: #fff;
    font-size: 14px;
    font-variant-numeric: tabular-nums;
    margin-left: 8px;
  }
</style>
