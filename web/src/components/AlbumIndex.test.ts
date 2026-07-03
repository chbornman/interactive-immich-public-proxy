import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import AlbumIndex from './AlbumIndex.svelte';
import type { AlbumSummary } from '../api';

vi.mock('../api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../api')>();
  return {
    ...actual,
    listAlbums: vi.fn(),
  };
});

import { listAlbums } from '../api';
const listAlbumsMock = vi.mocked(listAlbums);

describe('AlbumIndex', () => {
  const albums: AlbumSummary[] = [
    { key: 'key-a', title: 'Summer 2025', photos: 12, videos: 3, cover: 'asset-a' },
    { key: 'key-b', title: null, photos: 0, videos: 0, cover: null },
  ];

  beforeEach(() => {
    listAlbumsMock.mockReset();
  });

  /** Let the mocked listAlbums promise settle, then flush Svelte. */
  const settle = async () => {
    await new Promise((r) => setTimeout(r, 0));
    await tick();
  };

  it('renders a linked card per album with title and counts', async () => {
    listAlbumsMock.mockResolvedValue(albums);
    const { container, getByText } = render(AlbumIndex);
    await settle();

    const cards = container.querySelectorAll('a.card');
    expect(cards).toHaveLength(2);
    expect(cards[0].getAttribute('href')).toBe('/share/key-a');
    expect(cards[1].getAttribute('href')).toBe('/share/key-b');
    expect(getByText('Summer 2025')).toBeInTheDocument();
    // A null title falls back to a placeholder name.
    expect(getByText('Untitled album')).toBeInTheDocument();
    // Counts line: zero parts are omitted entirely (album b shows none).
    expect(getByText('12 photos · 3 videos')).toBeInTheDocument();
    expect(container.querySelectorAll('.counts')).toHaveLength(1);
    expect(document.title).toBe('Albums');
  });

  it('shows the empty state when there are no listed albums', async () => {
    listAlbumsMock.mockResolvedValue([]);
    const { container, getByText } = render(AlbumIndex);
    await settle();

    expect(getByText('No albums here yet.')).toBeInTheDocument();
    expect(container.querySelectorAll('a.card')).toHaveLength(0);
  });

  it('shows the error state when the fetch rejects', async () => {
    listAlbumsMock.mockRejectedValue(new Error('boom'));
    const { container, getByText } = render(AlbumIndex);
    await settle();

    expect(getByText('Could not load albums.')).toBeInTheDocument();
    expect(container.querySelectorAll('a.card')).toHaveLength(0);
  });
});
