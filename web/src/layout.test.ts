import { describe, it, expect } from 'vitest';
import { layoutJustified, groupByDay, dayLabel } from './layout';
import type { Asset } from './types';

describe('layoutJustified', () => {
  let seq = 0;
  const asset = (width: number, height: number): Asset => ({
    id: `a${++seq}`,
    kind: 'IMAGE',
    width,
    height,
    takenAt: 0,
    filename: `a${seq}.jpg`,
    markCount: 0,
    hasNote: false,
  });

  const opts = { containerWidth: 1000, targetRowHeight: 200, gap: 4 };

  /** A varied mix of landscape, portrait and square shapes. */
  const varied = () =>
    Array.from({ length: 17 }, (_, i) =>
      i % 3 === 0 ? asset(1600, 900) : i % 3 === 1 ? asset(900, 1600) : asset(1000, 1000),
    );

  it('returns no rows for empty input or a zero-width container', () => {
    expect(layoutJustified([], opts)).toEqual([]);
    expect(layoutJustified([asset(800, 600)], { ...opts, containerWidth: 0 })).toEqual([]);
  });

  it('places every asset exactly once, preserving order', () => {
    const assets = varied();
    const rows = layoutJustified(assets, opts);
    const placed = rows.flatMap((r) => r.tiles.map((t) => t.asset.id));
    expect(placed).toEqual(assets.map((a) => a.id));
  });

  it('bounds every row height by the target height', () => {
    const rows = layoutJustified(varied(), opts);
    expect(rows.length).toBeGreaterThan(1);
    for (const row of rows) {
      expect(row.height).toBeGreaterThan(0);
      expect(row.height).toBeLessThanOrEqual(opts.targetRowHeight);
      // Every tile shares the row height.
      for (const tile of row.tiles) expect(tile.height).toBe(row.height);
    }
  });

  it('fills the container width exactly on all but the last row', () => {
    const rows = layoutJustified(varied(), opts);
    for (const row of rows.slice(0, -1)) {
      const width =
        row.tiles.reduce((sum, t) => sum + t.width, 0) + opts.gap * (row.tiles.length - 1);
      expect(width).toBeCloseTo(opts.containerWidth, 6);
    }
  });

  it('keeps each tile at its source aspect ratio', () => {
    const assets = [asset(1600, 900), asset(900, 1600), asset(1000, 1000)];
    const rows = layoutJustified(assets, opts);
    for (const tile of rows.flatMap((r) => r.tiles)) {
      const { width, height } = tile.asset;
      expect(tile.width / tile.height).toBeCloseTo(width / height, 6);
    }
  });

  it('does not stretch a partial last row past the target height', () => {
    // A single 2:1 asset in a 1000px container would need a 500px row to fill it.
    const rows = layoutJustified([asset(2000, 1000)], opts);
    expect(rows).toHaveLength(1);
    expect(rows[0].height).toBe(opts.targetRowHeight);
    expect(rows[0].tiles[0].width).toBeCloseTo(2 * opts.targetRowHeight, 6);

    // A lower maxRowHeight caps the unstretched row further.
    const capped = layoutJustified([asset(2000, 1000)], { ...opts, maxRowHeight: 120 });
    expect(capped[0].height).toBe(120);
  });

  it('falls back to a square aspect for non-positive dimensions', () => {
    const rows = layoutJustified([asset(0, 0)], opts);
    expect(rows).toHaveLength(1);
    const tile = rows[0].tiles[0];
    expect(tile.width).toBeCloseTo(tile.height, 6);
  });
});

describe('groupByDay', () => {
  let seq = 0;
  /** Build an asset taken at a specific LOCAL wall-clock time. */
  const at = (y: number, m: number, d: number, hh = 12): Asset => ({
    id: `g${++seq}`,
    kind: 'IMAGE',
    width: 1600,
    height: 900,
    takenAt: Math.floor(new Date(y, m - 1, d, hh, 0, 0).getTime() / 1000),
    filename: `g${seq}.jpg`,
    markCount: 0,
    hasNote: false,
  });

  it('returns nothing for an empty list', () => {
    expect(groupByDay([])).toEqual([]);
  });

  it('groups consecutive same-day assets and splits on a date change', () => {
    const items = [at(2024, 5, 3), at(2024, 5, 3, 9), at(2024, 5, 2), at(2024, 5, 2, 8)];
    const groups = groupByDay(items);
    expect(groups.map((g) => g.key)).toEqual(['2024-05-03', '2024-05-02']);
    expect(groups.map((g) => g.assets.length)).toEqual([2, 2]);
  });

  it('preserves the input order exactly across groups', () => {
    const items = [at(2024, 5, 3), at(2024, 5, 2), at(2024, 5, 1)];
    const flat = groupByDay(items).flatMap((g) => g.assets.map((a) => a.id));
    expect(flat).toEqual(items.map((a) => a.id));
  });

  it('does not merge same-day runs that are not adjacent', () => {
    // Out-of-order input must not be silently reordered — the gallery's
    // index-based scrolling depends on render order matching the array.
    const items = [at(2024, 5, 3), at(2024, 5, 2), at(2024, 5, 3, 8)];
    const groups = groupByDay(items);
    expect(groups.map((g) => g.key)).toEqual(['2024-05-03', '2024-05-02', '2024-05-03']);
  });

  it('buckets undated assets under a single "unknown" key', () => {
    const undated = { ...at(2024, 5, 3), takenAt: 0 };
    const groups = groupByDay([undated, { ...undated, id: 'x' }]);
    expect(groups).toHaveLength(1);
    expect(groups[0].key).toBe('unknown');
    expect(groups[0].label).toBe('Undated');
  });

  it('labels today and yesterday by name', () => {
    const now = new Date(2024, 4, 3, 15, 0, 0);
    expect(dayLabel(Math.floor(new Date(2024, 4, 3, 9).getTime() / 1000), now)).toBe('Today');
    expect(dayLabel(Math.floor(new Date(2024, 4, 2, 9).getTime() / 1000), now)).toBe('Yesterday');
  });

  it('omits the year for the current year and includes it for older photos', () => {
    const now = new Date(2024, 4, 3, 15, 0, 0);
    expect(dayLabel(Math.floor(new Date(2024, 0, 15, 9).getTime() / 1000), now)).not.toMatch(/2024/);
    expect(dayLabel(Math.floor(new Date(2021, 0, 15, 9).getTime() / 1000), now)).toMatch(/2021/);
  });
});
