import type { Asset } from './types';

export interface LaidOutTile {
  asset: Asset;
  /** Display width in px. */
  width: number;
  /** Display height in px (equals the row height). */
  height: number;
}

export interface LaidOutRow {
  tiles: LaidOutTile[];
  height: number;
}

export interface LayoutOptions {
  containerWidth: number;
  targetRowHeight: number;
  gap: number;
  /** Rows whose final height exceeds this are not stretched (last row). */
  maxRowHeight?: number;
}

/**
 * Justified-rows layout (Google Photos / Immich style).
 *
 * Pure function: given assets and container metrics, returns rows where each
 * row's tiles share a common height and the row fills the container width.
 */
export function layoutJustified(
  assets: Asset[],
  opts: LayoutOptions,
): LaidOutRow[] {
  const { containerWidth, targetRowHeight, gap } = opts;
  const maxRowHeight = opts.maxRowHeight ?? targetRowHeight * 1.5;
  const rows: LaidOutRow[] = [];

  if (containerWidth <= 0 || assets.length === 0) return rows;

  let current: Asset[] = [];
  let aspectSum = 0; // sum of width/height ratios in the current row

  const aspect = (a: Asset): number => {
    const w = a.width > 0 ? a.width : 1;
    const h = a.height > 0 ? a.height : 1;
    return w / h;
  };

  const flush = (stretch: boolean): void => {
    if (current.length === 0) return;
    const totalGap = gap * (current.length - 1);
    const availWidth = containerWidth - totalGap;
    // Height that makes the row exactly fill availWidth.
    let rowHeight = availWidth / aspectSum;
    if (!stretch) {
      rowHeight = Math.min(rowHeight, maxRowHeight, targetRowHeight);
    }
    const tiles: LaidOutTile[] = current.map((asset) => ({
      asset,
      width: aspect(asset) * rowHeight,
      height: rowHeight,
    }));
    rows.push({ tiles, height: rowHeight });
    current = [];
    aspectSum = 0;
  };

  for (const asset of assets) {
    current.push(asset);
    aspectSum += aspect(asset);
    const totalGap = gap * (current.length - 1);
    const projectedHeight = (containerWidth - totalGap) / aspectSum;
    // When the row would shrink to/below the target height, it's full.
    if (projectedHeight <= targetRowHeight) {
      flush(true);
    }
  }
  // Last (possibly partial) row: keep tiles at target height, don't stretch.
  flush(false);

  return rows;
}

export interface DayGroup {
  /** Stable local-date key, e.g. "2025-05-17"; "unknown" when undated. */
  key: string;
  /** Human label for the separator, e.g. "Saturday, 17 May 2025". */
  label: string;
  assets: Asset[];
}

/**
 * Local-date key for an asset timestamp. Dates are bucketed in the VIEWER's
 * timezone: Immich gives us a UTC instant and we don't cache the capture
 * offset, so a photo taken near midnight can land on the adjacent day for a
 * viewer in a distant timezone. Consistent within a session, which is what
 * matters for grouping.
 */
export function dayKey(epochSeconds: number): string {
  if (!epochSeconds || epochSeconds <= 0) return 'unknown';
  const d = new Date(epochSeconds * 1000);
  if (Number.isNaN(d.getTime())) return 'unknown';
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${m}-${day}`;
}

/** Display label for a day key. Today/Yesterday are called out by name. */
export function dayLabel(epochSeconds: number, now: Date = new Date()): string {
  if (!epochSeconds || epochSeconds <= 0) return 'Undated';
  const d = new Date(epochSeconds * 1000);
  if (Number.isNaN(d.getTime())) return 'Undated';

  if (dayKey(epochSeconds) === dayKey(Math.floor(now.getTime() / 1000))) return 'Today';
  const yesterday = new Date(now.getTime() - 86400_000);
  if (dayKey(epochSeconds) === dayKey(Math.floor(yesterday.getTime() / 1000))) {
    return 'Yesterday';
  }

  // Same calendar year reads fine without the year; older photos need it.
  const sameYear = d.getFullYear() === now.getFullYear();
  return d.toLocaleDateString(undefined, {
    weekday: 'long',
    month: 'long',
    day: 'numeric',
    ...(sameYear ? {} : { year: 'numeric' }),
  });
}

/**
 * Split an asset list into consecutive same-day runs.
 *
 * Assumes the caller's ordering (the API returns takenAt DESC) and only ever
 * breaks between adjacent items, so it never reorders or drops assets — the
 * flat render order is preserved, which the gallery's index-based scrolling
 * and lightbox navigation depend on.
 */
export function groupByDay(assets: Asset[], now: Date = new Date()): DayGroup[] {
  const groups: DayGroup[] = [];
  for (const asset of assets) {
    const key = dayKey(asset.takenAt);
    const last = groups[groups.length - 1];
    if (last && last.key === key) {
      last.assets.push(asset);
    } else {
      groups.push({ key, label: dayLabel(asset.takenAt, now), assets: [asset] });
    }
  }
  return groups;
}

export function relativeTime(epochSeconds: number): string {
  const ms = epochSeconds < 1e12 ? epochSeconds * 1000 : epochSeconds;
  const diff = Date.now() - ms;
  const sec = Math.round(diff / 1000);
  if (sec < 45) return 'just now';
  const min = Math.round(sec / 60);
  if (min < 60) return `${min} min${min === 1 ? '' : 's'} ago`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr} hour${hr === 1 ? '' : 's'} ago`;
  const day = Math.round(hr / 24);
  if (day < 30) return `${day} day${day === 1 ? '' : 's'} ago`;
  return new Date(ms).toLocaleDateString();
}
