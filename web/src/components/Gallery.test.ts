import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import Gallery from './Gallery.svelte';
import type { Asset } from '../types';

/** ResizeObserver stub that lets tests hand the gallery a container width. */
class ROStub {
  static instances: ROStub[] = [];
  private targets: Element[] = [];
  constructor(private cb: ResizeObserverCallback) {
    ROStub.instances.push(this);
  }
  observe(el: Element) {
    this.targets.push(el);
  }
  unobserve() {}
  disconnect() {}
  resize(width: number) {
    const entries = this.targets.map((target) => ({ target, contentRect: { width } }));
    this.cb(entries as ResizeObserverEntry[], this as unknown as ResizeObserver);
  }
}

/** IntersectionObserver stub that lets tests intersect the sentinel. */
class IOStub {
  static instances: IOStub[] = [];
  private targets: Element[] = [];
  constructor(private cb: IntersectionObserverCallback) {
    IOStub.instances.push(this);
  }
  observe(el: Element) {
    this.targets.push(el);
  }
  unobserve() {}
  disconnect() {}
  intersect(isIntersecting: boolean) {
    const entries = this.targets.map((target) => ({ target, isIntersecting }));
    this.cb(entries as IntersectionObserverEntry[], this as unknown as IntersectionObserver);
  }
}

describe('Gallery', () => {
  const assets: Asset[] = Array.from({ length: 12 }, (_, i) => ({
    id: `asset-${i}`,
    kind: i % 3 === 0 ? 'VIDEO' : 'IMAGE',
    width: i % 2 === 0 ? 800 : 600,
    height: i % 2 === 0 ? 600 : 800,
    takenAt: 1700000000 - i * 86400,
    filename: `asset-${i}.${i % 3 === 0 ? 'mp4' : 'jpg'}`,
    markCount: 0,
    hasNote: false,
  }));

  const scrolled: Element[] = [];

  beforeEach(() => {
    ROStub.instances.length = 0;
    IOStub.instances.length = 0;
    scrolled.length = 0;
    vi.stubGlobal('ResizeObserver', ROStub);
    vi.stubGlobal('IntersectionObserver', IOStub);
    Element.prototype.scrollIntoView = function () {
      scrolled.push(this as Element);
    };
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    delete (Element.prototype as { scrollIntoView?: unknown }).scrollIntoView;
  });

  async function renderGallery(props: Record<string, unknown> = {}) {
    const onActivate = vi.fn();
    const onLoadMore = vi.fn();
    const utils = render(Gallery, {
      props: { assets, ...props },
      events: { activate: onActivate, loadMore: onLoadMore },
    });
    // jsdom measures everything at 0 — report a real width so the justified
    // layout produces rows.
    ROStub.instances.at(-1)!.resize(1200);
    await tick();
    return { ...utils, onActivate, onLoadMore };
  }

  /** Drain the (setTimeout-based) requestAnimationFrame queue from test-setup. */
  const flushRaf = () => new Promise((r) => setTimeout(r, 0));

  it('lays out one tile per asset once the container reports a width', async () => {
    const { container } = await renderGallery();
    expect(container.querySelectorAll('.tile')).toHaveLength(assets.length);
    // Tiles are grouped into justified rows, not a single strip.
    expect(container.querySelectorAll('.row').length).toBeGreaterThan(1);
  });

  it('activate carries the clicked asset and its index', async () => {
    const { container, onActivate } = await renderGallery();
    const tiles = container.querySelectorAll('.tile');

    await fireEvent.click(tiles[5]);

    expect(onActivate).toHaveBeenCalledTimes(1);
    expect(onActivate.mock.calls[0][0].detail).toEqual({ asset: assets[5], index: 5 });
  });

  it('scrolls the galleryScrollTarget tile into view after render', async () => {
    const { container, rerender } = await renderGallery({ galleryScrollTarget: null });

    await rerender({ galleryScrollTarget: 7 });
    await flushRaf();

    const tiles = container.querySelectorAll('.tile');
    expect(scrolled).toEqual([tiles[7]]);
  });

  it('no-ops without error when the scroll target tile is absent', async () => {
    const { rerender } = await renderGallery({ galleryScrollTarget: null });

    await rerender({ galleryScrollTarget: 999 });
    await flushRaf();

    expect(scrolled).toHaveLength(0);
  });

  it('dispatches loadMore when the sentinel intersects and more pages exist', async () => {
    const { onLoadMore } = await renderGallery({ hasMore: true });

    IOStub.instances.at(-1)!.intersect(true);
    expect(onLoadMore).toHaveBeenCalledTimes(1);

    // A non-intersecting notification must not fetch.
    IOStub.instances.at(-1)!.intersect(false);
    expect(onLoadMore).toHaveBeenCalledTimes(1);
  });

  it('does not dispatch loadMore when out of pages or already loading', async () => {
    const done = await renderGallery({ hasMore: false });
    IOStub.instances.at(-1)!.intersect(true);
    expect(done.onLoadMore).not.toHaveBeenCalled();
    done.unmount();

    const busy = await renderGallery({ hasMore: true, loading: true });
    IOStub.instances.at(-1)!.intersect(true);
    expect(busy.onLoadMore).not.toHaveBeenCalled();
  });
});
