<script lang="ts">
  import { onMount } from 'svelte';
  import type { Asset, AlbumInfo, FilterName, KindFilter, Visitor } from './types';
  import {
    getShareKey,
    getAlbum,
    getAssets,
    getVisitor,
    downloadAssets,
    bulkMark,
    unlockShare,
    ApiError,
  } from './api';
  import Toolbar from './components/Toolbar.svelte';
  import Gallery from './components/Gallery.svelte';
  import Lightbox from './components/Lightbox.svelte';
  import NameBanner from './components/NameBanner.svelte';
  import Slideshow from './components/Slideshow.svelte';
  import PasswordGate from './components/PasswordGate.svelte';
  import AlbumIndex from './components/AlbumIndex.svelte';

  const PAGE = 100;
  const shareKey = getShareKey();

  let album: AlbumInfo | null = null;
  let assets: Asset[] = [];
  let cursor = '';
  let nextCursor: string | null = '';
  let loading = false;
  // Monotonic load id: each loadMore() takes a snapshot so a stale in-flight
  // response (from a since-superseded filter) can be dropped instead of applied.
  let loadSeq = 0;
  let fatalError = '';
  let passwordRequired = false;
  let unlockBusy = false;
  let unlockError = '';

  let filter: FilterName = 'all';
  let kind: KindFilter = 'all';
  let query = '';

  let selectMode = false;
  let selectedIds = new Set<string>();
  let downloading = false;
  let marking = false;

  let openIndex: number | null = null;
  let slideshowStart: number | null = null;

  let visitor: Visitor | null = null;
  let showNameBanner = false;
  $: visitorName = visitor?.name ?? '';

  let tileSize = Number(localStorage.getItem('ipp-tile')) || 220;
  function onSize(e: CustomEvent<{ value: number }>) {
    tileSize = e.detail.value;
    localStorage.setItem('ipp-tile', String(tileSize));
  }

  $: hasMore = nextCursor !== null;
  // Tab title reflects the album instead of the generic shell title.
  $: if (album?.title) document.title = album.title;
  $: emptyMessage =
    query || filter !== 'all' ? 'No photos match your filter.' : 'This album is empty.';

  async function resetAndLoad() {
    assets = [];
    cursor = '';
    nextCursor = '';
    // Release the in-flight guard so this newest reset isn't dropped by loadMore's
    // `if (loading) return` — the loadSeq bump below invalidates the old request.
    loading = false;
    await loadMore();
  }

  async function loadMore() {
    if (loading || nextCursor === null) return;
    loading = true;
    const seq = ++loadSeq;
    try {
      const page = await getAssets({ cursor, limit: PAGE, filter, kind, q: query });
      if (seq !== loadSeq) return; // a newer load superseded this one — drop the response
      // Guard against duplicate ids if the backend overlaps pages.
      const seen = new Set(assets.map((a) => a.id));
      const fresh = page.items.filter((a) => !seen.has(a.id));
      assets = [...assets, ...fresh];
      nextCursor = page.nextCursor;
      cursor = page.nextCursor ?? '';
    } catch (e) {
      if (seq !== loadSeq) return; // stale failure — the current load owns the error state
      if (e instanceof ApiError && e.passwordRequired) {
        passwordRequired = true;
      } else if (e instanceof ApiError && e.status === 404 && assets.length === 0) {
        fatalError = 'This shared album could not be found. The link may be invalid or expired.';
      } else if (assets.length === 0) {
        fatalError = 'Could not load photos. Please try again later.';
      }
      nextCursor = null; // stop infinite scroll on error
    } finally {
      if (seq === loadSeq) loading = false;
    }
  }

  function onFilter(e: CustomEvent<{ filter: FilterName }>) {
    if (filter === e.detail.filter) return;
    filter = e.detail.filter;
    fatalError = '';
    resetAndLoad();
  }

  function onSearch(e: CustomEvent<{ q: string }>) {
    if (query === e.detail.q) return;
    query = e.detail.q;
    fatalError = '';
    resetAndLoad();
  }

  function onKind(e: CustomEvent<{ kind: KindFilter }>) {
    if (kind === e.detail.kind) return;
    kind = e.detail.kind;
    fatalError = '';
    resetAndLoad();
  }

  function onToggleSelectMode() {
    selectMode = !selectMode;
    if (!selectMode) selectedIds = new Set();
  }

  function onToggleSelect(e: CustomEvent<{ asset: Asset }>) {
    const id = e.detail.asset.id;
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }

  async function onDownload() {
    if (selectedIds.size === 0) return;
    downloading = true;
    try {
      await downloadAssets([...selectedIds]);
    } catch {
      /* surface nothing fatal */
    } finally {
      downloading = false;
    }
  }

  /** Transient message for download-all outcomes (too large, failed). */
  let downloadNotice = '';
  let downloadNoticeTimer: ReturnType<typeof setTimeout> | undefined;
  function notice(msg: string) {
    downloadNotice = msg;
    clearTimeout(downloadNoticeTimer);
    downloadNoticeTimer = setTimeout(() => (downloadNotice = ''), 6000);
  }

  /**
   * Download everything in the current view (respects filter/kind/search).
   *
   * Gathers ids page by page, stopping the moment the count exceeds the server's
   * cap — so an oversized album costs one extra request, not a full walk of the
   * album followed by a rejection.
   */
  async function onDownloadAll() {
    if (downloading) return;
    const cap = album?.maxDownload ?? 0;

    downloading = true;
    try {
      const ids: string[] = [...assets.map((a) => a.id)];
      let next: string | null = nextCursor;
      let cur: string = cursor;
      const seen = new Set(ids);

      while (next !== null && (cap <= 0 || ids.length <= cap)) {
        const page = await getAssets({ cursor: cur, limit: 200, filter, kind, q: query });
        for (const a of page.items) {
          if (!seen.has(a.id)) {
            seen.add(a.id);
            ids.push(a.id);
          }
        }
        next = page.nextCursor;
        cur = page.nextCursor ?? '';
      }

      if (ids.length === 0) {
        notice('Nothing to download.');
        return;
      }
      if (cap > 0 && ids.length > cap) {
        notice(
          `That's ${ids.length.toLocaleString()} photos — more than the ${cap.toLocaleString()} per-download limit. Use Select to pick a smaller batch.`,
        );
        return;
      }
      await downloadAssets(ids);
    } catch (e) {
      notice(
        e instanceof ApiError && e.status === 413
          ? 'That selection is too large to download at once.'
          : 'Download failed. Please try again.',
      );
    } finally {
      downloading = false;
    }
  }

  async function bulkSetMark(marked: boolean) {
    if (selectedIds.size === 0) return;
    marking = true;
    try {
      const res = await bulkMark([...selectedIds], marked);
      const map = new Map(res.items.map((i) => [i.id, i.markCount]));
      assets = assets.map((a) => (map.has(a.id) ? { ...a, markCount: map.get(a.id) as number } : a));
    } catch {
      /* surface nothing fatal */
    } finally {
      marking = false;
    }
  }

  async function onUnlock(e: CustomEvent<{ password: string }>) {
    if (unlockBusy) return;
    unlockBusy = true;
    unlockError = '';
    try {
      await unlockShare(e.detail.password);
      passwordRequired = false;
      fatalError = '';
      album = await getAlbum().catch(() => null);
      await resetAndLoad();
    } catch {
      unlockError = 'Incorrect password. Try again.';
    } finally {
      unlockBusy = false;
    }
  }

  function onActivate(e: CustomEvent<{ asset: Asset; index: number }>) {
    openIndex = e.detail.index;
  }

  /** Scroll target for Gallery after lightbox/slideshow closes. */
  let galleryScrollTarget: number | null = null;

  /** Scroll the grid to `index`, then clear the target after the animation. */
  function scrollGridTo(index: number) {
    galleryScrollTarget = index;
    setTimeout(() => {
      galleryScrollTarget = null;
    }, 800);
  }

  function onLightboxClose(e: CustomEvent<{ index: number }>) {
    // Return to the grid at the last photo viewed in the lightbox.
    scrollGridTo(e.detail.index);
  }

  function onSlideshowClose(e: CustomEvent<{ index: number }>) {
    stopSlideshow();
    // The slideshow is always the sole overlay (launching it closes the lightbox),
    // so closing it always returns to the grid at the last-viewed photo.
    scrollGridTo(e.detail.index);
  }

  let slideshowLoading = false;

  /** Eagerly load every page of the current filter so the slideshow covers the full library. */
  async function loadAll() {
    if (nextCursor === null) return; // already fully loaded
    slideshowLoading = true;
    try {
      let cur: string = cursor;
      let next: string | null = nextCursor;
      while (next !== null) {
        const page = await getAssets({ cursor: cur, limit: 200, filter, kind, q: query });
        const seen = new Set(assets.map((a) => a.id));
        const fresh = page.items.filter((a) => !seen.has(a.id));
        assets = [...assets, ...fresh];
        next = page.nextCursor;
        cur = page.nextCursor ?? '';
      }
      nextCursor = null;
      cursor = '';
    } catch {
      /* stop on error — slideshow runs with whatever loaded */
    } finally {
      slideshowLoading = false;
    }
  }

  async function startSlideshow(index = 0) {
    if (assets.length === 0) return;
    // Kick off the slideshow immediately with what we have; load the rest in the
    // background so advancing reaches the full library.
    slideshowStart = index;
    if (nextCursor !== null) loadAll();
  }

  function stopSlideshow() {
    slideshowStart = null;
  }

  function onSetName(e: CustomEvent<{ name: string }>) {
    if (visitor) visitor = { ...visitor, name: e.detail.name };
  }

  function onAssetChange(e: CustomEvent<{ id: string; markCount: number; hasNote: boolean }>) {
    const { id, markCount, hasNote } = e.detail;
    assets = assets.map((a) => (a.id === id ? { ...a, markCount, hasNote } : a));
  }

  onMount(async () => {
    // No share key => the album index renders instead; skip the gallery load.
    if (!shareKey) return;
    // Load album + assets + visitor in parallel.
    const albumP = getAlbum().catch(() => null);
    const visitorP = getVisitor().catch(() => null);
    await resetAndLoad();
    album = await albumP;
    visitor = await visitorP;
    if (visitor && !visitor.name.trim()) showNameBanner = true;
  });
</script>

<main>
  {#if !shareKey}
    <AlbumIndex />
  {:else if passwordRequired}
    <PasswordGate error={unlockError} busy={unlockBusy} on:submit={onUnlock} />
  {:else}
  <Toolbar
    title={album?.title ?? ''}
    total={album?.total ?? assets.length}
    photos={album?.photos ?? 0}
    videos={album?.videos ?? 0}
    {filter}
    {kind}
    {query}
    {selectMode}
    selectedCount={selectedIds.size}
    {downloading}
    {marking}
    allowDownload={album?.allowDownload ?? false}
    filtered={query !== '' || filter !== 'all' || kind !== 'all'}
    visitorName={visitor?.name ?? ''}
    {tileSize}
    on:filter={onFilter}
    on:kind={onKind}
    on:search={onSearch}
    on:toggleSelect={onToggleSelectMode}
    on:download={onDownload}
    on:downloadAll={onDownloadAll}
    on:markSelected={() => bulkSetMark(true)}
    on:unmarkSelected={() => bulkSetMark(false)}
    on:editName={() => (showNameBanner = true)}
    on:slideshow={() => startSlideshow(0)}
    on:size={onSize}
  />

  {#if downloadNotice}
    <div class="download-notice" role="status">{downloadNotice}</div>
  {/if}

  <NameBanner
    visible={showNameBanner}
    current={visitor?.name ?? ''}
    on:saved={(e) => {
      if (visitor) visitor = { ...visitor, name: e.detail.name };
      showNameBanner = false;
    }}
    on:dismiss={() => (showNameBanner = false)}
  />

  <Gallery
    {assets}
    {loading}
    {hasMore}
    error={fatalError}
    {emptyMessage}
    {selectMode}
    {selectedIds}
    targetRowHeight={tileSize}
    {galleryScrollTarget}
    on:loadMore={loadMore}
    on:activate={onActivate}
    on:toggleSelect={onToggleSelect}
  />

  <Lightbox
    items={assets}
    bind:openIndex
    {visitorName}
    on:assetchange={onAssetChange}
    on:slideshow={(e) => startSlideshow(e.detail.index)}
    on:close={onLightboxClose}
    on:setname={onSetName}
  />

  <Slideshow
    items={assets}
    bind:startIndex={slideshowStart}
    loading={slideshowLoading}
    on:close={onSlideshowClose}
  />
  {/if}
</main>

<style>
  main {
    min-height: 100vh;
  }
  .download-notice {
    margin: 8px 10px 0;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
    background: var(--bg-elev);
    color: var(--text);
    font-size: 13px;
  }
</style>
