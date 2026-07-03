import { describe, it, expect, vi, afterEach } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import Slideshow from './Slideshow.svelte';
import type { Asset } from '../types';

describe('Slideshow', () => {
  const asset = (id: string, kind: Asset['kind'], filename: string): Asset => ({
    id,
    kind,
    width: 1920,
    height: 1080,
    takenAt: 1700000000,
    filename,
    markCount: 0,
    hasNote: false,
  });

  const images: Asset[] = [
    asset('s1', 'IMAGE', 'one.jpg'),
    asset('s2', 'IMAGE', 'two.jpg'),
    asset('s3', 'IMAGE', 'three.jpg'),
  ];
  const mixed: Asset[] = [asset('m1', 'IMAGE', 'still.jpg'), asset('m2', 'VIDEO', 'clip.mp4')];

  afterEach(() => {
    vi.useRealTimers();
  });

  function renderShow(items: Asset[], startIndex: number | null) {
    const onClose = vi.fn();
    const utils = render(Slideshow, {
      props: { items, startIndex },
      events: { close: onClose },
    });
    return { ...utils, onClose };
  }

  /** Dispatch a window keydown and report whether the slideshow consumed it. */
  async function pressKey(key: string) {
    const e = new KeyboardEvent('keydown', { key, cancelable: true });
    window.dispatchEvent(e);
    await tick();
    return e.defaultPrevented;
  }

  const counter = (container: HTMLElement) =>
    container.querySelector('.counter')?.textContent?.trim() ?? '';

  it('is inert at startIndex null: no DOM and no window keydown listener', async () => {
    const { container, onClose } = renderShow(images, null);

    expect(container.querySelector('.slide-host')).toBeNull();
    // The key handler is only attached in begin(); nothing may consume keys.
    expect(await pressKey('ArrowRight')).toBe(false);
    expect(await pressKey('Escape')).toBe(false);
    expect(container.querySelector('.slide-host')).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('begins at startIndex and arrows navigate with wrap-around', async () => {
    const { container } = renderShow(images, 1);

    expect(container.querySelector('.slide-host')).not.toBeNull();
    expect(counter(container)).toContain('2 / 3');
    expect(container.querySelector('img')?.getAttribute('src')).toContain('s2');

    expect(await pressKey('ArrowRight')).toBe(true);
    expect(counter(container)).toContain('3 / 3');
    expect(await pressKey('ArrowRight')).toBe(true); // wraps to the first slide
    expect(counter(container)).toContain('1 / 3');
    expect(await pressKey('ArrowLeft')).toBe(true); // wraps back to the last
    expect(counter(container)).toContain('3 / 3');
  });

  it('Escape closes once, even when fullscreenchange re-enters close()', async () => {
    const { container, onClose } = renderShow(images, 0);

    expect(await pressKey('Escape')).toBe(true);
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(onClose.mock.calls[0][0].detail).toEqual({ index: 0 });
    expect(container.querySelector('.slide-host')).toBeNull();

    // Leaving fullscreen fires fullscreenchange, which calls close() again —
    // the closing guard must keep it a single close event.
    document.dispatchEvent(new Event('fullscreenchange'));
    await tick();
    expect(onClose).toHaveBeenCalledTimes(1);

    // The window keydown listener is gone after close.
    expect(await pressKey('ArrowRight')).toBe(false);
  });

  it('auto-advances image slides after the image duration', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const { container } = renderShow(images, 0);
    await tick(); // let queued attachMediaHandlers arm the timer

    expect(counter(container)).toContain('1 / 3');
    vi.advanceTimersByTime(5000);
    await tick();
    expect(counter(container)).toContain('2 / 3');
    vi.advanceTimersByTime(5000);
    await tick();
    expect(counter(container)).toContain('3 / 3');
  });

  it('video slides advance on ended, not on the image timer', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const { container } = renderShow(mixed, 1);
    await tick();

    const video = container.querySelector('video');
    expect(video).not.toBeNull();
    expect(counter(container)).toContain('2 / 2');

    // No image timer is armed for a video slide.
    vi.advanceTimersByTime(60000);
    await tick();
    expect(counter(container)).toContain('2 / 2');

    video!.dispatchEvent(new Event('ended'));
    await tick();
    expect(counter(container)).toContain('1 / 2'); // wrapped to the image
    expect(container.querySelector('img')).not.toBeNull();
    expect(container.querySelector('video')).toBeNull();
  });
});
