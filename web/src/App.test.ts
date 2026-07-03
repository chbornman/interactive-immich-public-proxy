import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import App from './App.svelte';
import type { Asset, AssetsPage } from './types';

/**
 * Deferred getAssets: every call records its args and parks until the test
 * resolves/rejects it, so responses can be settled out of order.
 */
const { assetCalls, getAssetsDeferred } = vi.hoisted(() => {
  interface AssetsCall {
    args: { cursor?: string; limit?: number; filter?: string; kind?: string; q?: string };
    resolve: (page: { items: unknown[]; nextCursor: string | null }) => void;
    reject: (err: unknown) => void;
  }
  const assetCalls: AssetsCall[] = [];
  const getAssetsDeferred = (args: AssetsCall['args']) =>
    new Promise((resolve, reject) => {
      assetCalls.push({ args, resolve, reject });
    });
  return { assetCalls, getAssetsDeferred };
});

vi.mock('./api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./api')>();
  return {
    ...actual,
    getShareKey: () => 'test-share-key',
    getAlbum: vi.fn(async () => ({ title: 'Test Album', total: 4, photos: 4, videos: 0 })),
    getVisitor: vi.fn(async () => ({ id: 'v1', name: 'Visitor' })),
    getAssets: vi.fn(getAssetsDeferred) as unknown as typeof actual.getAssets,
  };
});

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

/** IntersectionObserver stub that lets tests intersect the infinite-scroll sentinel. */
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

describe('App stale-filter race guard', () => {
  const photo = (id: string, filename: string): Asset => ({
    id,
    kind: 'IMAGE',
    width: 800,
    height: 600,
    takenAt: 1700000000,
    filename,
    markCount: 0,
    hasNote: false,
  });

  const pageA: Asset[] = [photo('a1', 'stale-one.jpg'), photo('a2', 'stale-two.jpg')];
  const pageB: Asset[] = [photo('b1', 'fresh-one.jpg'), photo('b2', 'fresh-two.jpg')];

  const page = (items: Asset[], nextCursor: string | null): AssetsPage => ({ items, nextCursor });

  beforeEach(() => {
    assetCalls.length = 0;
    ROStub.instances.length = 0;
    IOStub.instances.length = 0;
    vi.stubGlobal('ResizeObserver', ROStub);
    vi.stubGlobal('IntersectionObserver', IOStub);
    // This jsdom build ships no localStorage; App reads the tile-size pref from it.
    const store = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      getItem: (k: string) => store.get(k) ?? null,
      setItem: (k: string, v: string) => void store.set(k, String(v)),
      removeItem: (k: string) => void store.delete(k),
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** Let settled api promises run their continuations, then flush Svelte. */
  const settle = async () => {
    await new Promise((r) => setTimeout(r, 0));
    await tick();
  };

  /** Mount App (initial getAssets call #1 left pending) with a filter switch mid-flight. */
  async function renderMidFlightFilterSwitch() {
    const utils = render(App);
    // onMount kicks off the initial load synchronously up to its first await.
    expect(assetCalls).toHaveLength(1);
    // jsdom measures everything at 0 — report a real width so tiles render.
    ROStub.instances.at(-1)!.resize(1200);
    await tick();

    // Switch filters while load #1 is still in flight — starts load #2.
    await fireEvent.click(utils.getByRole('button', { name: 'Marked' }));
    expect(assetCalls).toHaveLength(2);
    expect(assetCalls[1].args.filter).toBe('marked');

    return utils;
  }

  it('drops a stale response that resolves after a newer filter load started', async () => {
    const { container, getByAltText, queryByAltText } =
      await renderMidFlightFilterSwitch();

    // The superseded load resolves late with the wrong photo set — dropped:
    // no tiles appear and load #2's skeleton stays up (its loading flag holds).
    assetCalls[0].resolve(page(pageA, 'cursor-a'));
    await settle();
    expect(queryByAltText('stale-one.jpg')).toBeNull();
    expect(container.querySelector('.loading-row')).not.toBeNull();

    // The current load resolves and wins.
    assetCalls[1].resolve(page(pageB, 'cursor-b'));
    await settle();
    expect(getByAltText('fresh-one.jpg')).toBeInTheDocument();
    expect(getByAltText('fresh-two.jpg')).toBeInTheDocument();
    expect(queryByAltText('stale-one.jpg')).toBeNull();
    expect(queryByAltText('stale-two.jpg')).toBeNull();
    expect(container.querySelector('.loading-row')).toBeNull();

    // The next page paginates from B's cursor, never the stale one.
    IOStub.instances.at(-1)!.intersect(true);
    expect(assetCalls).toHaveLength(3);
    expect(assetCalls[2].args.cursor).toBe('cursor-b');
    expect(assetCalls[2].args.filter).toBe('marked');
  });

  it('drops a stale failure that rejects after the newer load resolved', async () => {
    const { getByText, queryByText } = await renderMidFlightFilterSwitch();

    // The current load resolves first: the marked filter is empty.
    assetCalls[1].resolve(page([], null));
    await settle();
    expect(getByText('No photos match your filter.')).toBeInTheDocument();

    // Then the superseded load fails. Its error belongs to a dead request and
    // must not surface — the empty state stays, no error screen.
    assetCalls[0].reject(new Error('network dropped'));
    await settle();
    expect(queryByText('Something went wrong')).toBeNull();
    expect(getByText('No photos match your filter.')).toBeInTheDocument();
  });
});
