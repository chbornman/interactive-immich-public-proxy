export type AssetKind = 'IMAGE' | 'VIDEO';

export type SizeName = 'thumbnail' | 'preview' | 'original';

export type FilterName = 'all' | 'marked' | 'noted';
export type KindFilter = 'all' | 'image' | 'video';

export interface Asset {
  id: string;
  kind: AssetKind;
  width: number;
  height: number;
  takenAt: number;
  filename: string;
  markCount: number;
  hasNote: boolean;
}

export interface AlbumInfo {
  title: string;
  total: number;
  photos: number;
  videos: number;
}

export interface AssetsPage {
  items: Asset[];
  nextCursor: string | null;
}

export interface Note {
  id: number;
  name: string;
  body: string;
  createdAt: number;
}

export interface AssetMeta {
  markCount: number;
  marked: boolean;
  notes: Note[];
  exif?: Record<string, unknown> | null;
}

export interface MarkResult {
  marked: boolean;
  markCount: number;
}

export interface Visitor {
  id: string;
  name: string;
}
