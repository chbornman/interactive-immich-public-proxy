<script lang="ts">
  import { createEventDispatcher, onDestroy, tick } from 'svelte';
  import PhotoSwipe from 'photoswipe';
  import 'photoswipe/style.css';
  import { Star, Info, DownloadSimple, Export, X, CaretDown, CaretUp, ProjectorScreen, Question } from 'phosphor-svelte';
  import type { Asset, AssetMeta } from '../types';
  import { assetUrl, getAssetMeta, toggleMark, addNote, downloadAssets, supportsShareFiles } from '../api';
  import NotesPanel from './NotesPanel.svelte';

  export let items: Asset[] = [];
  /** Index to open; set to a number to open, null to keep closed. */
  export let openIndex: number | null = null;
  /** Current visitor name (for inline note attribution prompt). */
  export let visitorName = '';

  const dispatch = createEventDispatcher<{
    assetchange: { id: string; markCount: number; hasNote: boolean };
    slideshow: { index: number };
    close: { index: number };
    setname: { name: string };
  }>();

  /** The only place the breakpoint exists; CSS keys off the .mobile class. */
  const MOBILE_MQ = '(max-width: 819px)';

  let pswp: PhotoSwipe | null = null;
  /** Stage grid cell that PhotoSwipe mounts into (via appendToEl). */
  let stageEl: HTMLDivElement | null = null;
  let ro: ResizeObserver | null = null;
  let mql: MediaQueryList | null = null;
  /** True while the shell is mounted; pswp lives inside it. */
  let lbOpen = false;
  /** Guards the reactive open gate across the tick between mount and pswp.init(). */
  let opening = false;
  /** True while closing to hand off to the slideshow, so we don't scroll the grid. */
  let handingOff = false;
  let currentAsset: Asset | null = null;
  /** Current slide index, mirrored for the header position counter. */
  let curIndex = 0;
  let isMobile = false;
  let sheetOpen = false;
  let showInfo = false;
  /** Keyboard-shortcuts popover (desktop only; toggled by the ? key too). */
  let showHelp = false;

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

  /** Try unmuted autoplay; on browser rejection fall back to muted (per Immich's approach). */
  async function playVideo(video: HTMLVideoElement) {
    video.muted = false;
    try {
      await video.play();
    } catch (err) {
      if (err instanceof DOMException && err.name === 'NotAllowedError') {
        video.muted = true;
        try {
          await video.play();
        } catch {
          /* muted autoplay blocked too — user can press play */
        }
      }
    }
  }

  /**
   * PhotoSwipe gestures compute pageX/pageY minus pswp.offset and assume the
   * viewport starts at the page origin. Contained in the stage cell, rewrite
   * the offset from the cell's page coordinates (undocumented internal, but
   * the updateScrollOffset listener runs after the stock assignment and is
   * allowed to mutate instance state).
   */
  function syncStageOffset() {
    if (!pswp || !stageEl) return;
    const r = stageEl.getBoundingClientRect();
    pswp.offset.x = r.left + window.scrollX;
    pswp.offset.y = r.top + window.scrollY;
  }

  function onMqlChange(e: MediaQueryListEvent) {
    // Swapping sidebar<->sheet resizes the stage cell; the ResizeObserver refits.
    isMobile = e.matches;
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

  async function openLightbox(index: number) {
    opening = true;
    // Resolve the breakpoint BEFORE mounting so the shell renders the correct
    // grid template and pswp.init() measures the real stage cell (a phone must
    // never mount the desktop 360px sidebar, even for one tick).
    mql = window.matchMedia(MOBILE_MQ);
    isMobile = mql.matches;
    mql.addEventListener('change', onMqlChange);
    // Mount the shell so the stage cell exists (and has its grid size) before
    // pswp.init() measures it.
    lbOpen = true;
    await tick();
    try {
      open(index);
    } catch {
      /* unwound below */
    }
    if (!pswp) {
      // Failed to initialize: unwind so the page isn't left scroll-locked and
      // the reactive gate doesn't respin on a null pswp.
      lbOpen = false;
      openIndex = null;
      document.body.style.overflow = '';
      mql.removeEventListener('change', onMqlChange);
      mql = null;
    }
    opening = false;
  }

  function open(index: number) {
    if (pswp || !stageEl) return;
    const stage = stageEl;
    sheetOpen = false;
    handingOff = false;
    // Lock the page behind the lightbox: no scroll-behind, and the stage's
    // page offset stays put for the gesture math in syncStageOffset.
    document.body.style.overflow = 'hidden';

    pswp = new PhotoSwipe({
      dataSource: buildDataSource(items),
      index,
      appendToEl: stage,
      getViewportSizeFn: () => ({ x: stage.clientWidth, y: stage.clientHeight }),
      bgOpacity: 1,
      showHideAnimationType: 'fade',
      trapFocus: false,
      wheelToZoom: true,
    });

    pswp.on('updateScrollOffset', syncStageOffset);

    // pswp's keydown is document-level and focus-agnostic: stand down while a
    // field or a video's native controls own the keys (Esc/arrows/z).
    pswp.on('keydown', (e) => {
      // In fullscreen, Esc should only exit fullscreen (the browser handles
      // that natively), not also close the lightbox.
      if (e.originalEvent.key === 'Escape' && document.fullscreenElement) {
        e.preventDefault();
        return;
      }
      const t = e.originalEvent.target as HTMLElement | null;
      if (!t) return;
      if (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'VIDEO' || t.isContentEditable) {
        e.preventDefault();
      }
    });

    // Stage resizes during the ~333ms opening fade are only half-applied
    // (updateSize skips slide.resize until the opener reports open) — replay
    // the final size once the animation completes.
    pswp.on('openingAnimationEnd', () => {
      pswp?.updateSize(true);
      syncStageOffset();
    });

    // The built-in wheel zoom anchors on window-client coordinates and ignores
    // pswp.offset — contained, that drifts the zoom point by the stage's page
    // offset. Reproduce the stock zoom math with a stage-relative center.
    pswp.on('wheel', (e) => {
      const ev = e.originalEvent;
      const slide = pswp?.currSlide;
      if (!slide || !slide.isZoomable()) return;
      e.preventDefault();
      let zoomFactor = -ev.deltaY;
      if (ev.deltaMode === 1) zoomFactor *= 0.05;
      else zoomFactor *= ev.deltaMode ? 1 : 0.002;
      const r = stage.getBoundingClientRect();
      slide.zoomTo(slide.currZoomLevel * 2 ** zoomFactor, {
        x: ev.clientX - r.left,
        y: ev.clientY - r.top,
      });
    });

    // Cell resizes that never fire window.resize (sheet open/close, breakpoint
    // flip): upstream updateSize is only wired to window resize.
    ro = new ResizeObserver(() => {
      pswp?.updateSize(true);
      syncStageOffset();
    });
    ro.observe(stage);

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
      if (v) {
        v.pause();
        v.currentTime = 0;
      }
    });

    pswp.on('contentActivate', (e) => {
      const video = e.content.element?.querySelector('video.pswp-video') as HTMLVideoElement | null;
      if (video) {
        // Capture the slide this activation is for; if the user navigates away
        // before it's ready, a late canplay must not play a now-inactive slide.
        const activatedIndex = e.content.index;
        const tryPlay = () => {
          if (pswp && pswp.currIndex === activatedIndex) playVideo(video);
        };
        if (video.readyState >= 2) tryPlay();
        else video.addEventListener('canplay', tryPlay, { once: true });
      }
    });

    pswp.on('change', () => {
      const idx = pswp?.currIndex ?? 0;
      // Guarantee exactly one video ever plays: pause+reset every slide's video
      // except the one now active.
      const activeVideo = pswp?.currSlide?.content?.element?.querySelector(
        'video.pswp-video',
      ) as HTMLVideoElement | null;
      for (const v of document.querySelectorAll<HTMLVideoElement>('video.pswp-video')) {
        if (v !== activeVideo) {
          v.pause();
          v.currentTime = 0;
        }
      }
      curIndex = idx;
      currentAsset = items[idx] ?? null;
      if (currentAsset) loadMeta(currentAsset);
    });

    pswp.on('destroy', () => {
      // Report the last-viewed index so the grid can scroll back to it — unless
      // we're handing off to the slideshow, which owns the return-to-grid.
      if (!handingOff) dispatch('close', { index: pswp?.currIndex ?? index });
      pswp = null;
      currentAsset = null;
      meta = null;
      sheetOpen = false;
      showHelp = false;
      openIndex = null;
      lbOpen = false;
      ro?.disconnect();
      ro = null;
      mql?.removeEventListener('change', onMqlChange);
      mql = null;
      document.body.style.overflow = '';
      window.removeEventListener('keydown', onKey);
    });

    window.addEventListener('keydown', onKey);
    pswp.init();
    // init() reassigns offset.y directly (no updateScrollOffset dispatch), so
    // re-sync deterministically instead of relying on the RO's initial fire.
    syncStageOffset();
    curIndex = index;
    currentAsset = items[index] ?? null;
    if (currentAsset) loadMeta(currentAsset);
  }

  // PhotoSwipe's built-in keyboard nav handles arrows (navigate) and Escape (close);
  // we add Space (toggle the current video — native <video> needs focus
  // otherwise), F (fullscreen, gone from PhotoSwipe since v5), M (mark), and
  // Home/End (jump to first/last).
  const OWN_KEYS = [' ', 'f', 'F', 'm', 'M', 'Home', 'End', '?'];
  function onKey(e: KeyboardEvent) {
    if (!OWN_KEYS.includes(e.key)) return;
    const el = e.target as HTMLElement | null;
    // Don't hijack keys while typing a note, or while the video itself has
    // focus (its native controls already handle them; avoid a double toggle).
    if (
      el &&
      (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'VIDEO' || el.isContentEditable)
    )
      return;
    if (e.key === '?') {
      e.preventDefault();
      showHelp = !showHelp;
      return;
    }
    if (e.key === 'f' || e.key === 'F') {
      e.preventDefault();
      toggleFullscreen();
      return;
    }
    if (e.key === 'm' || e.key === 'M') {
      e.preventDefault();
      onToggleMark();
      return;
    }
    if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      pswp?.goTo(e.key === 'Home' ? 0 : items.length - 1);
      return;
    }
    const video = pswp?.currSlide?.content?.element?.querySelector(
      'video.pswp-video',
    ) as HTMLVideoElement | null;
    if (!video) return;
    e.preventDefault();
    if (video.paused) playVideo(video);
    else video.pause();
  }

  /** Fullscreen the MEDIA, not the chrome: the video player itself on video
      slides (native controls take over), the stage cell on photos (pure view,
      arrows/keys still work — the stage RO refits automatically). */
  async function toggleFullscreen() {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
        return;
      }
      const video = pswp?.currSlide?.content?.element?.querySelector(
        'video.pswp-video',
      ) as HTMLVideoElement | null;
      if (video) await video.requestFullscreen?.();
      else await stageEl?.requestFullscreen?.();
    } catch {
      /* fullscreen unavailable — ignore */
    }
  }

  $: if (openIndex !== null && !pswp && !opening) openLightbox(openIndex);

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

  function startSlideshow() {
    if (!currentAsset) return;
    const idx = pswp?.currIndex ?? 0;
    // Hand off to the slideshow as the sole owner: close the lightbox so its
    // video/keyboard can't keep running underneath (no doubled audio or drift).
    handingOff = true;
    dispatch('slideshow', { index: idx });
    pswp?.close();
  }

  function toggleInfo() {
    showInfo = !showInfo;
    if (showInfo && isMobile) sheetOpen = true;
  }

  onDestroy(() => {
    pswp?.destroy();
    ro?.disconnect();
    mql?.removeEventListener('change', onMqlChange);
    window.removeEventListener('keydown', onKey);
    document.body.style.overflow = '';
  });
</script>

{#if lbOpen}
  <div class="lb" class:mobile={isMobile}>
    <header class="lb-header">
      <span class="fname" title={currentAsset?.filename}>{currentAsset?.filename ?? ''}</span>
      <span class="pos">{curIndex + 1} / {items.length}</span>
      <button
        class="hbtn mark"
        class:on={meta?.marked}
        on:click={onToggleMark}
        disabled={markBusy || metaLoading}
        title={meta?.marked ? 'Unmark (M)' : 'Mark (M)'}
        aria-label={meta?.marked ? 'Unmark' : 'Mark'}
      >
        <Star size={18} weight={meta?.marked ? 'fill' : 'regular'} />
        <span class="cnt">{meta?.markCount ?? 0}</span>
      </button>
      <button class="hbtn" class:on={showInfo} on:click={toggleInfo} title="Image info" aria-label="Image info">
        <Info size={18} weight={showInfo ? 'fill' : 'regular'} />
      </button>
      <button class="hbtn" on:click={startSlideshow} title="Slideshow" aria-label="Slideshow">
        <ProjectorScreen size={18} />
      </button>
      <button
        class="hbtn"
        on:click={onDownload}
        title={supportsShareFiles ? 'Share' : 'Download'}
        aria-label={supportsShareFiles ? 'Share' : 'Download'}
      >
        {#if supportsShareFiles}<Export size={18} />{:else}<DownloadSimple size={18} />{/if}
      </button>
      {#if !isMobile}
        <span class="help-wrap">
          <button
            class="hbtn"
            class:on={showHelp}
            on:click={() => (showHelp = !showHelp)}
            title="Keyboard shortcuts (?)"
            aria-label="Keyboard shortcuts"
            aria-expanded={showHelp}
          >
            <Question size={18} weight={showHelp ? 'fill' : 'regular'} />
          </button>
          {#if showHelp}
            <div class="help-pop" role="note" aria-label="Keyboard shortcuts">
              <div class="hp-row"><kbd>&larr;</kbd><kbd>&rarr;</kbd><span>navigate</span></div>
              <div class="hp-row"><kbd>Space</kbd><span>play / pause video</span></div>
              <div class="hp-row"><kbd>M</kbd><span>mark</span></div>
              <div class="hp-row"><kbd>F</kbd><span>fullscreen</span></div>
              <div class="hp-row"><kbd>Home</kbd><kbd>End</kbd><span>first / last</span></div>
              <div class="hp-row"><kbd>Esc</kbd><span>close</span></div>
            </div>
          {/if}
        </span>
      {/if}
      <button class="hbtn" on:click={() => pswp?.close()} title="Close (Esc)" aria-label="Close">
        <X size={18} />
      </button>
    </header>

    <div class="lb-stage" bind:this={stageEl}></div>

    {#if !isMobile}
      <aside class="lb-side">
        <NotesPanel
          {meta}
          loading={metaLoading}
          {noteBusy}
          asset={currentAsset}
          {showInfo}
          {visitorName}
          on:addNote={onAddNote}
          on:setname={(e) => dispatch('setname', e.detail)}
        />
      </aside>
    {:else}
      <div class="lb-sheet" class:open={sheetOpen}>
        <button class="sheet-handle" on:click={() => (sheetOpen = !sheetOpen)}>
          {#if sheetOpen}<CaretDown size={16} />{:else}<CaretUp size={16} />{/if}
          <span>Notes{meta?.notes?.length ? ` (${meta.notes.length})` : ''}</span>
        </button>
        {#if sheetOpen}
          <div class="sheet-body">
            <NotesPanel
              {meta}
              loading={metaLoading}
              {noteBusy}
              asset={currentAsset}
              {showInfo}
              {visitorName}
              on:addNote={onAddNote}
              on:setname={(e) => dispatch('setname', e.detail)}
            />
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  /* ---- One source of truth for lightbox geometry ---- */
  .lb {
    --lb-side-w: 360px;
    --lb-stage-bg: #eceee8; /* shared by the stage cell and --pswp-bg: fade never flashes */
    position: fixed;
    inset: 0;
    z-index: 2000000; /* unchanged rung: below NameBanner (3M) and Slideshow (4M) */
    display: grid;
    grid-template-areas:
      'header header'
      'stage  side';
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr) var(--lb-side-w);
  }

  /* Mobile: three stacked rows. The breakpoint lives ONLY in the JS matchMedia
     string (MOBILE_MQ); CSS keys off the .mobile class, so JS and CSS cannot drift. */
  .lb.mobile {
    grid-template-areas:
      'header'
      'stage'
      'sheet';
    grid-template-rows: auto minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
  }

  .lb-header {
    grid-area: header;
    min-height: 56px;
    padding-top: env(safe-area-inset-top, 0);
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: max(10px, env(safe-area-inset-left, 0));
    padding-right: max(10px, env(safe-area-inset-right, 0));
    background: var(--bg-elev);
    border-bottom: 1px solid var(--border);
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
  .pos {
    font-size: 12px;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .help-wrap {
    position: relative;
    display: inline-flex;
  }
  .help-pop {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 10; /* above the stage cell within the .lb stacking context */
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
    padding: 10px 14px;
    display: grid;
    gap: 7px;
    white-space: nowrap;
  }
  .hp-row {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12.5px;
    color: var(--text-dim);
  }
  .hp-row span {
    margin-left: 4px;
  }
  .help-pop kbd {
    font-family: ui-monospace, monospace;
    font-size: 11px;
    color: var(--text);
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
    line-height: 1.5;
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
    transition: background 0.15s ease, border-color 0.15s ease;
  }
  .hbtn:hover:not(:disabled) {
    background: var(--bg-elev-2);
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
    opacity: 0.5;
    cursor: default;
  }

  /* ---- The stage cell: the one element PhotoSwipe owns ---- */
  .lb-stage {
    grid-area: stage;
    position: relative; /* containing block for the contained .pswp */
    min-width: 0;
    min-height: 0;
    overflow: hidden; /* clips drag-to-close motion to the cell */
    background: var(--lb-stage-bg);
  }

  /* Containment override: photoswipe.css ships .pswp as position:fixed full-
     viewport; pin it to the stage cell instead. The scoped selector out-
     specifies the stock .pswp rule, and nothing in pswp JS reads the
     positioning. Solid stage bg, dark icons for the arrows. */
  .lb-stage :global(.pswp) {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    z-index: auto; /* neutralize --pswp-root-z-index inside the .lb stacking context */
    --pswp-bg: var(--lb-stage-bg);
    --pswp-icon-color: #2b2e2a;
    --pswp-icon-color-secondary: #fff;
    --pswp-icon-stroke-color: #fff;
  }
  .lb-stage :global(.pswp__top-bar) {
    display: none !important; /* our header replaces it; built-in arrows remain */
  }

  /* Video (html) slides fill the stage; no padding mirror, geometry is the cell. */
  .lb-stage :global(.pswp__html-content),
  .lb-stage :global(.pswp-video-wrap) {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
  }
  .lb-stage :global(.pswp-video) {
    max-width: 100%;
    max-height: 100%;
    width: auto;
    height: auto;
    background: #000;
  }

  .lb-side {
    grid-area: side;
    min-height: 0;
    border-left: 1px solid var(--border);
    background: var(--bg-elev);
  }
  .lb-side :global(.notes-wrap) {
    height: 100%;
  }

  .lb-sheet {
    grid-area: sheet;
    display: flex;
    flex-direction: column;
  }
  .lb-sheet.open {
    height: 62vh; /* row is auto: open sheet takes 62vh, stage shrinks, RO refits the photo */
    height: 62dvh; /* dynamic-viewport units keep the composer clear of iOS browser chrome */
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
    background: var(--bg-elev);
    color: var(--text);
    border: none;
    border-top: 1px solid var(--border);
    font-size: 14px;
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
</style>
