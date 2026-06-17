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
