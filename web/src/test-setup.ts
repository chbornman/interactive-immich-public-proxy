import '@testing-library/jest-dom/vitest';

// ---- jsdom seams the components rely on ----

// Lightbox resolves its mobile/desktop breakpoint via matchMedia.
window.matchMedia = (query: string): MediaQueryList =>
  ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  }) as unknown as MediaQueryList;

// Default no-op observers; tests that need to drive entries stub their own.
class NoopObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords() {
    return [];
  }
}
window.ResizeObserver = NoopObserver as unknown as typeof ResizeObserver;
window.IntersectionObserver = NoopObserver as unknown as typeof IntersectionObserver;

// Deterministic requestAnimationFrame: run on the macrotask queue so
// post-render work (Gallery's scroll-to-tile) fires after Svelte has
// flushed the DOM, and a plain `await setTimeout(0)` drains it.
window.requestAnimationFrame = ((cb: FrameRequestCallback) =>
  setTimeout(() => cb(performance.now()), 0)) as unknown as typeof requestAnimationFrame;
window.cancelAnimationFrame = ((id: number) => clearTimeout(id)) as typeof cancelAnimationFrame;

// jsdom leaves media playback unimplemented; Slideshow/Lightbox call these.
HTMLMediaElement.prototype.play = () => Promise.resolve();
HTMLMediaElement.prototype.pause = () => {};
