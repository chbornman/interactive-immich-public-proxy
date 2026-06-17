<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import PhotoSwipe from 'photoswipe';
  import 'photoswipe/style.css';
  import { Star, Info, DownloadSimple, Export, X, CaretDown, CaretUp } from 'phosphor-svelte';
  import type { Asset, AssetMeta } from '../types';
  import { assetUrl, getAssetMeta, toggleMark, addNote, downloadAssets, supportsShareFiles } from '../api';
  import NotesPanel from './NotesPanel.svelte';

  export let items: Asset[] = [];
  /** Index to open; set to a number to open, null to keep closed. */
  export let openIndex: number | null = null;

  const dispatch = createEventDispatcher<{
    assetchange: { id: string; markCount: number; hasNote: boolean };
  }>();

  const HEADER = 56;
  const SIDEBAR = 360;
  const HANDLE = 44;
  const MOBILE_MAX = 819;

  let pswp: PhotoSwipe | null = null;
  let currentAsset: Asset | null = null;
  let isMobile = false;
  let sheetOpen = false;
  let showInfo = false;

  let meta: AssetMeta | null = null;
  let metaLoading = false;
  let markBusy = false;
  let noteBusy = false;
  let metaToken = 0;

  function dims(a: Asset): { w: number; h: number } {
    const w = a.width > 0 ? a.width : 1600;
    const h = a.height > 0 ? a.height : 1200;
    return { w, h };
  }

  function buildDataSource(list: Asset[]) {
    return list.map((a) => {
      const { w, h } = dims(a);
      if (a.kind === 'VIDEO') {
        return {
          html: `<div class="pswp-video-wrap"><video class="pswp-video" controls playsinline preload="metadata" poster="${assetUrl(a.id, 'preview')}" src="${assetUrl(a.id, 'original')}"></video></div>`,
        };
      }
      return { src: assetUrl(a.id, 'preview'), width: w, height: h, alt: a.filename };
    });
  }

  function paddingFn() {
    if (isMobile) {
      return { top: HEADER, bottom: HANDLE, left: 6, right: 6 };
    }
    return { top: HEADER, bottom: 8, left: 8, right: SIDEBAR };
  }

  function updateMobile() {
    const m = window.innerWidth <= MOBILE_MAX;
    if (m !== isMobile) {
      isMobile = m;
      pswp?.updateSize(true);
    }
  }

  async function loadMeta(asset: Asset) {
    const token = ++metaToken;
    meta = null;
    metaLoading = true;
    try {
      const m = await getAssetMeta(asset.id);
      if (token === metaToken) meta = m;
    } catch {
      if (token === metaToken)
        meta = { markCount: asset.markCount, marked: asset.markCount > 0, notes: [], exif: null };
    } finally {
      if (token === metaToken) metaLoading = false;
    }
  }

  function open(index: number) {
    if (pswp) return;
    isMobile = window.innerWidth <= MOBILE_MAX;
    sheetOpen = false;

    pswp = new PhotoSwipe({
      dataSource: buildDataSource(items),
      index,
      bgOpacity: 1,
      showHideAnimationType: 'fade',
      trapFocus: false,
      wheelToZoom: true,
      paddingFn,
    });

    pswp.on('contentLoad', (e) => {
      const data = e.content.data as { html?: string };
      if (data.html) {
        e.preventDefault();
        const el = document.createElement('div');
        el.className = 'pswp__html-content';
        el.innerHTML = data.html;
        e.content.element = el;
        e.content.onLoaded();
      }
    });

    pswp.on('contentDeactivate', (e) => {
      const v = e.content.element?.querySelector('video');
      if (v) v.pause();
    });

    pswp.on('change', () => {
      const idx = pswp?.currIndex ?? 0;
      currentAsset = items[idx] ?? null;
      if (currentAsset) loadMeta(currentAsset);
    });

    pswp.on('destroy', () => {
      pswp = null;
      currentAsset = null;
      meta = null;
      sheetOpen = false;
      openIndex = null;
      window.removeEventListener('resize', updateMobile);
    });

    window.addEventListener('resize', updateMobile);
    pswp.init();
    currentAsset = items[index] ?? null;
    if (currentAsset) loadMeta(currentAsset);
  }

  $: if (openIndex !== null && !pswp) open(openIndex);

  function emitChange() {
    if (!currentAsset || !meta) return;
    dispatch('assetchange', {
      id: currentAsset.id,
      markCount: meta.markCount,
      hasNote: meta.notes.length > 0,
    });
  }

  async function onToggleMark() {
    if (!currentAsset || !meta) return;
    markBusy = true;
    const prev = { ...meta };
    meta = { ...meta, marked: !meta.marked, markCount: meta.marked ? 0 : meta.markCount + 1 };
    try {
      const res = await toggleMark(currentAsset.id);
      meta = { ...meta, marked: res.marked, markCount: res.markCount };
      emitChange();
    } catch {
      meta = prev;
    } finally {
      markBusy = false;
    }
  }

  async function onAddNote(e: CustomEvent<{ body: string }>) {
    if (!currentAsset || !meta) return;
    noteBusy = true;
    try {
      const note = await addNote(currentAsset.id, e.detail.body);
      meta = { ...meta, notes: [...meta.notes, note] };
      emitChange();
    } catch {
      /* surface nothing for now */
    } finally {
      noteBusy = false;
    }
  }

  async function onDownload() {
    if (!currentAsset) return;
    try {
      await downloadAssets([currentAsset.id]);
    } catch {
      /* ignore */
    }
  }

  function toggleInfo() {
    showInfo = !showInfo;
    if (showInfo && isMobile) sheetOpen = true;
  }

  onDestroy(() => {
    pswp?.destroy();
    window.removeEventListener('resize', updateMobile);
  });
</script>

{#if pswp}
  <div class="lb-chrome">
    <header class="lb-header">
      <span class="fname" title={currentAsset?.filename}>{currentAsset?.filename ?? ''}</span>
      <button
        class="hbtn mark"
        class:on={meta?.marked}
        on:click={onToggleMark}
        disabled={markBusy || metaLoading}
        title={meta?.marked ? 'Unmark' : 'Mark'}
        aria-label={meta?.marked ? 'Unmark' : 'Mark'}
      >
        <Star size={18} weight={meta?.marked ? 'fill' : 'regular'} />
        <span class="cnt">{meta?.markCount ?? 0}</span>
      </button>
      <button class="hbtn" class:on={showInfo} on:click={toggleInfo} title="Image info" aria-label="Image info">
        <Info size={18} weight={showInfo ? 'fill' : 'regular'} />
      </button>
      <button
        class="hbtn"
        on:click={onDownload}
        title={supportsShareFiles ? 'Share' : 'Download'}
        aria-label={supportsShareFiles ? 'Share' : 'Download'}
      >
        {#if supportsShareFiles}<Export size={18} />{:else}<DownloadSimple size={18} />{/if}
      </button>
      <button class="hbtn" on:click={() => pswp?.close()} title="Close" aria-label="Close">
        <X size={18} />
      </button>
    </header>

    {#if !isMobile}
      <aside class="lb-side">
        <NotesPanel {meta} loading={metaLoading} {noteBusy} asset={currentAsset} {showInfo} on:addNote={onAddNote} />
      </aside>
    {:else}
      <div class="lb-sheet" class:open={sheetOpen}>
        <button class="sheet-handle" on:click={() => (sheetOpen = !sheetOpen)}>
          {#if sheetOpen}<CaretDown size={16} />{:else}<CaretUp size={16} />{/if}
          <span>Notes{meta?.notes?.length ? ` (${meta.notes.length})` : ''}</span>
        </button>
        {#if sheetOpen}
          <div class="sheet-body">
            <NotesPanel {meta} loading={metaLoading} {noteBusy} asset={currentAsset} {showInfo} on:addNote={onAddNote} />
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  /* Solid light stage so the gallery doesn't bleed through; dark icons for arrows. */
  :global(.pswp) {
    --pswp-bg: #eceee8;
    --pswp-icon-color: #2b2e2a;
    --pswp-icon-color-secondary: #fff;
    --pswp-icon-stroke-color: #fff;
  }
  :global(.pswp__top-bar) {
    display: none !important;
  }

  .lb-chrome {
    position: fixed;
    inset: 0;
    z-index: 2000000;
    pointer-events: none;
  }

  .lb-header {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    min-height: 48px;
    padding-top: env(safe-area-inset-top, 0);
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: max(10px, env(safe-area-inset-left, 0));
    padding-right: max(10px, env(safe-area-inset-right, 0));
    background: #fff;
    border-bottom: 1px solid var(--border);
    pointer-events: auto;
  }
  .fname {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hbtn {
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text);
    border-radius: var(--radius);
    height: 34px;
    min-width: 34px;
    padding: 0 9px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
  }
  .hbtn.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .hbtn .cnt {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }
  .hbtn:disabled {
    opacity: 0.6;
  }

  .lb-side {
    position: absolute;
    top: 48px;
    right: 0;
    bottom: 0;
    width: 360px;
    border-left: 1px solid var(--border);
    background: var(--bg-elev);
    pointer-events: auto;
  }
  .lb-side :global(.notes-wrap) {
    height: 100%;
  }

  .lb-sheet {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    pointer-events: auto;
    transition: height 0.2s ease;
  }
  .sheet-handle {
    flex: 0 0 auto;
    min-height: 44px;
    width: 100%;
    padding-bottom: env(safe-area-inset-bottom, 0);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: #fff;
    color: var(--text);
    border: none;
    border-top: 1px solid var(--border);
    font-size: 14px;
  }
  .lb-sheet.open {
    height: 62vh;
  }
  .sheet-body {
    flex: 1;
    min-height: 0;
    background: var(--bg-elev);
    border-top: 1px solid var(--border);
    padding-bottom: env(safe-area-inset-bottom, 0);
  }
  .sheet-body :global(.notes-wrap) {
    height: 100%;
  }

  /* Custom (video) content bypasses PhotoSwipe's paddingFn, so mirror the reserved
     zones here as box-sizing padding to keep video out from under the chrome. */
  :global(.pswp__html-content) {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    padding: 56px 360px 8px 8px; /* top header, right sidebar (desktop) */
  }
  @media (max-width: 819px) {
    :global(.pswp__html-content) {
      padding: 56px 6px calc(44px + env(safe-area-inset-bottom, 0)) 6px; /* header + bottom handle */
    }
  }
  :global(.pswp-video-wrap) {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
  }
  :global(.pswp-video) {
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: auto;
    background: #000;
  }
</style>
