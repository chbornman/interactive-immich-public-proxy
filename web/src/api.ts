import type {
  AlbumInfo,
  AssetsPage,
  AssetMeta,
  FilterName,
  KindFilter,
  MarkResult,
  Note,
  SizeName,
  Visitor,
} from './types';

/** Extract the share key from a `/share/<key>` (Immich format) or `/s/<key>` pathname. */
export function getShareKey(): string {
  const m = window.location.pathname.match(/\/(?:share|s)\/([^/?#]+)/);
  return m ? decodeURIComponent(m[1]) : '';
}

const shareKey = getShareKey();

export class ApiError extends Error {
  passwordRequired = false;
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

const fetchOpts: RequestInit = { credentials: 'same-origin' };

/** True when the browser can share files via the native share sheet (iOS/Android). */
export const supportsShareFiles: boolean = (() => {
  try {
    const nav = navigator as Navigator & { canShare?: (d?: ShareData) => boolean };
    return (
      typeof nav.canShare === 'function' &&
      nav.canShare({ files: [new File([new Blob([''])], 'x.jpg', { type: 'image/jpeg' })] })
    );
  } catch {
    return false;
  }
})();

async function jsonFetch<T>(url: string, init?: RequestInit): Promise<T> {
  const res = await fetch(url, { ...fetchOpts, ...init });
  if (!res.ok) {
    let pw = false;
    try {
      const b = (await res.clone().json()) as { passwordRequired?: boolean };
      pw = !!b?.passwordRequired;
    } catch {
      /* non-JSON error body */
    }
    const err = new ApiError(res.status, `Request failed (${res.status}) for ${url}`);
    err.passwordRequired = pw;
    throw err;
  }
  return (await res.json()) as T;
}

/** Submit the album password; throws ApiError on wrong password. */
export async function unlockShare(password: string): Promise<void> {
  const res = await fetch(s('/unlock'), {
    ...fetchOpts,
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  });
  if (!res.ok) {
    throw new ApiError(
      res.status,
      res.status === 401 || res.status === 403 ? 'Incorrect password' : `Unlock failed (${res.status})`,
    );
  }
}

function s(path: string): string {
  return `/api/s/${encodeURIComponent(shareKey)}${path}`;
}

export function getAlbum(): Promise<AlbumInfo> {
  return jsonFetch<AlbumInfo>(s('/album'));
}

export function getAssets(opts: {
  cursor?: string;
  limit?: number;
  filter?: FilterName;
  kind?: KindFilter;
  q?: string;
}): Promise<AssetsPage> {
  const params = new URLSearchParams();
  params.set('cursor', opts.cursor ?? '');
  params.set('limit', String(opts.limit ?? 100));
  params.set('filter', opts.filter ?? 'all');
  if (opts.kind && opts.kind !== 'all') params.set('kind', opts.kind);
  if (opts.q) params.set('q', opts.q);
  return jsonFetch<AssetsPage>(s(`/assets?${params.toString()}`));
}

/** URL for an asset's bytes at a given size (img/video src). */
export function assetUrl(id: string, size: SizeName): string {
  return s(`/media/${encodeURIComponent(id)}/${size}`);
}

export function getAssetMeta(id: string): Promise<AssetMeta> {
  return jsonFetch<AssetMeta>(s(`/asset/${encodeURIComponent(id)}/meta`));
}

export function toggleMark(id: string): Promise<MarkResult> {
  return jsonFetch<MarkResult>(s(`/asset/${encodeURIComponent(id)}/mark`), {
    method: 'POST',
  });
}

/** Mark or unmark many assets at once. Returns updated mark counts per id. */
export function bulkMark(
  ids: string[],
  marked = true,
): Promise<{ items: { id: string; markCount: number }[] }> {
  return jsonFetch<{ items: { id: string; markCount: number }[] }>(s('/mark'), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ids, marked }),
  });
}

export function addNote(id: string, body: string): Promise<Note> {
  return jsonFetch<Note>(s(`/asset/${encodeURIComponent(id)}/note`), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ body }),
  });
}

export function getVisitor(): Promise<Visitor> {
  return jsonFetch<Visitor>('/api/visitor/me');
}

export function setVisitorName(name: string): Promise<{ name: string }> {
  return jsonFetch<{ name: string }>('/api/visitor/name', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  });
}

function filenameFromDisposition(disposition: string | null): string | null {
  if (!disposition) return null;
  const star = disposition.match(/filename\*=(?:UTF-8'')?([^;]+)/i);
  if (star) {
    try {
      return decodeURIComponent(star[1].replace(/^"|"$/g, ''));
    } catch {
      /* fall through */
    }
  }
  const plain = disposition.match(/filename="?([^";]+)"?/i);
  return plain ? plain[1] : null;
}

/** POST ids to /download and trigger a browser save of the returned blob. */
export async function downloadAssets(ids: string[]): Promise<void> {
  const res = await fetch(s('/download'), {
    ...fetchOpts,
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ ids }),
  });
  if (!res.ok) {
    throw new ApiError(res.status, `Download failed (${res.status})`);
  }
  const blob = await res.blob();
  const name =
    filenameFromDisposition(res.headers.get('Content-Disposition')) ??
    (ids.length === 1 ? 'download' : 'album.zip');

  // Prefer the native share sheet (iOS/Android) — lets users Save to Photos / AirDrop.
  const type = blob.type || (ids.length === 1 ? 'application/octet-stream' : 'application/zip');
  const file = new File([blob], name, { type });
  const nav = navigator as Navigator & { canShare?: (d?: ShareData) => boolean };
  if (typeof nav.canShare === 'function' && nav.canShare({ files: [file] })) {
    try {
      await nav.share({ files: [file] });
      return;
    } catch (e) {
      // User dismissed the sheet — done; any other error falls back to download.
      if ((e as Error)?.name === 'AbortError') return;
    }
  }

  // Fallback: trigger a normal browser download.
  const objUrl = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = objUrl;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(objUrl), 10000);
}
