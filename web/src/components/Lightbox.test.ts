import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import { tick } from 'svelte';
import Lightbox from './Lightbox.svelte';
import type { Asset } from '../types';

/**
 * PhotoSwipe mock: records constructor options, captures handlers registered
 * via pswp.on(), and lets tests fire lifecycle events with emit(). close()
 * jumps straight to the destroy event (the real library animates first).
 */
const { MockPhotoSwipe } = vi.hoisted(() => {
  type Handler = (e?: unknown) => void;
  class MockPhotoSwipe {
    static instances: MockPhotoSwipe[] = [];
    options: Record<string, unknown>;
    offset = { x: 0, y: 0 };
    currIndex: number;
    currSlide: unknown = null;
    initCalled = false;
    private handlers: Record<string, Handler[]> = {};
    constructor(options: Record<string, unknown>) {
      this.options = options;
      this.currIndex = (options.index as number) ?? 0;
      MockPhotoSwipe.instances.push(this);
    }
    on(event: string, fn: Handler) {
      (this.handlers[event] ??= []).push(fn);
    }
    emit(event: string, e?: unknown) {
      for (const fn of this.handlers[event] ?? []) fn(e);
    }
    init() {
      this.initCalled = true;
    }
    updateSize() {}
    close() {
      this.emit('destroy');
    }
    destroy() {
      this.emit('destroy');
    }
  }
  return { MockPhotoSwipe };
});

vi.mock('photoswipe', () => ({ default: MockPhotoSwipe }));
vi.mock('photoswipe/style.css', () => ({}));

vi.mock('../api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api')>();
  return {
    ...actual,
    getAssetMeta: vi.fn(async () => ({ markCount: 0, marked: false, notes: [], exif: null })),
    toggleMark: vi.fn(),
    addNote: vi.fn(),
    downloadAssets: vi.fn(),
  };
});

describe('Lightbox', () => {
  const asset = (id: string, kind: Asset['kind'], filename: string): Asset => ({
    id,
    kind,
    width: 1600,
    height: 1200,
    takenAt: 1700000000,
    filename,
    markCount: 0,
    hasNote: false,
  });

  const items: Asset[] = [
    asset('a1', 'IMAGE', 'one.jpg'),
    asset('a2', 'VIDEO', 'clip.mp4'),
    asset('a3', 'IMAGE', 'three.jpg'),
  ];

  beforeEach(() => {
    MockPhotoSwipe.instances.length = 0;
  });

  /** Render with openIndex set and wait for the async open flow to finish. */
  async function renderOpen(index: number) {
    const onClose = vi.fn();
    const onSlideshow = vi.fn();
    const utils = render(Lightbox, {
      props: { items, openIndex: index },
      events: { close: onClose, slideshow: onSlideshow },
    });
    // openLightbox mounts the shell, awaits a tick, then constructs PhotoSwipe.
    await tick();
    await tick();
    const pswp = MockPhotoSwipe.instances.at(-1);
    if (!pswp) throw new Error('PhotoSwipe was not constructed');
    return { ...utils, onClose, onSlideshow, pswp };
  }

  it('renders nothing and constructs no PhotoSwipe while openIndex is null', async () => {
    const { container } = render(Lightbox, { props: { items, openIndex: null } });
    await tick();
    expect(container.querySelector('.lb')).toBeNull();
    expect(MockPhotoSwipe.instances).toHaveLength(0);
    expect(document.body.style.overflow).toBe('');
  });

  it('mounts the grid shell and boots PhotoSwipe into the stage cell', async () => {
    const { container, pswp } = await renderOpen(0);

    const stage = container.querySelector('.lb-stage');
    expect(container.querySelector('.lb')).not.toBeNull();
    expect(stage).not.toBeNull();
    expect(pswp.options.appendToEl).toBe(stage);
    expect(pswp.options.index).toBe(0);
    expect(pswp.initCalled).toBe(true);

    // Images become src slides; videos become html slides carrying a <video>.
    const dataSource = pswp.options.dataSource as Array<{ src?: string; html?: string }>;
    expect(dataSource).toHaveLength(items.length);
    expect(dataSource[0].src).toContain('a1');
    expect(dataSource[0].html).toBeUndefined();
    expect(dataSource[1].html).toContain('<video');
    expect(dataSource[1].html).toContain('a2');

    // Page behind the lightbox is scroll-locked while open.
    expect(document.body.style.overflow).toBe('hidden');
  });

  it('dispatches close exactly once with the final index when PhotoSwipe is destroyed', async () => {
    const { container, pswp, onClose } = await renderOpen(0);

    pswp.currIndex = 2; // user navigated to the last slide
    pswp.emit('destroy');
    await tick();

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose.mock.calls[0][0].detail).toEqual({ index: 2 });
    expect(container.querySelector('.lb')).toBeNull();
    expect(document.body.style.overflow).toBe('');
  });

  it('hands off to the slideshow: dispatches slideshow with the current index and suppresses close', async () => {
    const { container, pswp, onClose, onSlideshow, getByRole } = await renderOpen(0);

    pswp.currIndex = 1;
    await fireEvent.click(getByRole('button', { name: 'Slideshow' }));

    expect(onSlideshow).toHaveBeenCalledTimes(1);
    expect(onSlideshow.mock.calls[0][0].detail).toEqual({ index: 1 });
    // pswp.close() ran the destroy handler, but the close event is suppressed.
    expect(onClose).not.toHaveBeenCalled();
    expect(container.querySelector('.lb')).toBeNull();
    expect(document.body.style.overflow).toBe('');
  });

  it('keydown guard stands down PhotoSwipe for field/video targets but not the body', async () => {
    const { pswp } = await renderOpen(0);

    const fire = (target: EventTarget | null) => {
      const e = {
        originalEvent: { target },
        defaultPrevented: false,
        preventDefault() {
          this.defaultPrevented = true;
        },
      };
      pswp.emit('keydown', e);
      return e.defaultPrevented;
    };

    expect(fire(document.createElement('input'))).toBe(true);
    expect(fire(document.createElement('textarea'))).toBe(true);
    expect(fire(document.createElement('video'))).toBe(true);
    expect(fire(document.body)).toBe(false);
    expect(fire(null)).toBe(false);
  });
});
