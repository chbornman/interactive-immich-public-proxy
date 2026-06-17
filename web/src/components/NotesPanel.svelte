<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { PaperPlaneTilt } from 'phosphor-svelte';
  import type { Asset, AssetMeta, Note } from '../types';
  import { relativeTime } from '../layout';

  export let meta: AssetMeta | null = null;
  export let loading = false;
  export let noteBusy = false;
  export let asset: Asset | null = null;
  export let showInfo = false;

  const dispatch = createEventDispatcher<{ addNote: { body: string } }>();

  let draft = '';

  function fmtDate(secs: number): string {
    if (!secs) return '';
    const ms = secs < 1e12 ? secs * 1000 : secs;
    try {
      return new Date(ms).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
    } catch {
      return '';
    }
  }

  function submitNote() {
    const body = draft.trim();
    if (!body) return;
    dispatch('addNote', { body });
    draft = '';
  }

  function displayName(n: Note): string {
    return n.name && n.name.trim() ? n.name : 'Anonymous';
  }

  function fmtSize(bytes: number): string {
    if (!bytes) return '';
    return bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  $: exif = (meta?.exif ?? null) as Record<string, any> | null;
  $: infoRows = buildInfoRows(asset, exif);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function buildInfoRows(a: Asset | null, e: Record<string, any> | null): [string, string][] {
    const x = e ?? {};
    const rows: [string, string][] = [];
    if (a?.filename) rows.push(['File', a.filename]);

    let takenSecs = 0;
    if (typeof x.dateTimeOriginal === 'string') {
      const t = Date.parse(x.dateTimeOriginal);
      if (!Number.isNaN(t)) takenSecs = Math.floor(t / 1000);
    }
    if (!takenSecs && a?.takenAt) takenSecs = a.takenAt;
    if (takenSecs) rows.push(['Taken', fmtDate(takenSecs)]);

    const cam = [x.make, x.model].filter(Boolean).join(' ');
    if (cam) rows.push(['Camera', cam]);
    if (x.lensModel) rows.push(['Lens', String(x.lensModel)]);

    const exp = [
      x.exposureTime ? `${x.exposureTime}s` : '',
      x.fNumber ? `ƒ/${x.fNumber}` : '',
      x.iso ? `ISO ${x.iso}` : '',
      x.focalLength ? `${Math.round(Number(x.focalLength))}mm` : '',
    ]
      .filter(Boolean)
      .join(' · ');
    if (exp) rows.push(['Exposure', exp]);

    const w = Number(x.exifImageWidth) || (a && a.width > 3 ? a.width : 0);
    const h = Number(x.exifImageHeight) || (a && a.height > 2 ? a.height : 0);
    if (w && h) rows.push(['Size', `${w} × ${h}`]);

    if (x.fileSizeInByte) rows.push(['File size', fmtSize(Number(x.fileSizeInByte))]);

    const loc = [x.city, x.state, x.country].filter(Boolean).join(', ');
    if (loc) rows.push(['Location', loc]);
    if (x.description) rows.push(['Caption', String(x.description)]);

    rows.push(['Type', a?.kind === 'VIDEO' ? 'Video' : 'Photo']);
    return rows;
  }
</script>

<div class="notes-wrap">
  <div class="notes">
    {#if showInfo && asset}
      <dl class="info">
        {#each infoRows as [label, value] (label)}
          <div><dt>{label}</dt><dd title={value}>{value}</dd></div>
        {/each}
      </dl>
    {/if}
    <h3>Notes {#if meta?.notes?.length}({meta.notes.length}){/if}</h3>

    {#if loading}
      <p class="muted">Loading…</p>
    {:else if meta && meta.notes.length === 0}
      <p class="muted">No notes yet. Be the first to comment.</p>
    {:else if meta}
      <ul>
        {#each meta.notes as n (n.id)}
          <li>
            <div class="note-head">
              <span class="who">{displayName(n)}</span>
              <span class="when">{relativeTime(n.createdAt)}</span>
            </div>
            <div class="body">{n.body}</div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <div class="composer">
    <textarea bind:value={draft} placeholder="Add a note…" rows="2" maxlength="2000"></textarea>
    <button class="primary" on:click={submitNote} disabled={noteBusy || !draft.trim()}>
      <PaperPlaneTilt size={15} weight="fill" />
      <span>{noteBusy ? 'Adding…' : 'Add note'}</span>
    </button>
  </div>
</div>

<style>
  .notes-wrap {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--bg-elev);
    color: var(--text);
  }
  .notes {
    flex: 1;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    padding: 4px 14px 8px;
  }
  .notes h3 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-dim);
    margin: 10px 0 8px;
    position: sticky;
    top: 0;
    background: var(--bg-elev);
    padding-bottom: 4px;
  }
  .muted {
    color: var(--text-dim);
    font-size: 14px;
  }
  .info {
    margin: 8px 0 4px;
    padding: 8px 10px;
    background: var(--bg-elev-2);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }
  .info > div {
    display: flex;
    gap: 8px;
  }
  .info dt {
    flex: 0 0 48px;
    color: var(--text-dim);
  }
  .info dd {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  li {
    background: var(--bg-elev-2);
    border-radius: var(--radius);
    padding: 9px 11px;
  }
  .note-head {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 4px;
  }
  .who {
    font-weight: 600;
    font-size: 13px;
  }
  .when {
    color: var(--text-dim);
    font-size: 12px;
    white-space: nowrap;
  }
  .body {
    font-size: 14px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .composer {
    flex: 0 0 auto;
    border-top: 1px solid var(--border);
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--bg-elev);
  }
  textarea {
    resize: none;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 8px 10px;
    min-height: 40px;
    font: inherit;
  }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
  }
  .composer .primary {
    align-self: flex-end;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--accent);
    border: 1px solid var(--accent);
    color: #fff;
    border-radius: var(--radius);
    padding: 7px 14px;
  }
  .composer .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
