<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { PaperPlaneTilt, UserCirclePlus } from 'phosphor-svelte';
  import type { Asset, AssetMeta, Note } from '../types';
  import { relativeTime } from '../layout';
  import { setVisitorName } from '../api';

  export let meta: AssetMeta | null = null;
  export let loading = false;
  export let noteBusy = false;
  export let asset: Asset | null = null;
  export let showInfo = false;
  export let visitorName = '';

  const dispatch = createEventDispatcher<{
    addNote: { body: string };
    setname: { name: string };
  }>();

  let draft = '';
  let nameDraft = '';
  let nameBusy = false;
  let nameError = '';
  let nameFocused = false;

  $: needsName = !visitorName || !visitorName.trim();

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

  async function saveName() {
    const trimmed = nameDraft.trim();
    if (!trimmed || nameBusy) return;
    nameBusy = true;
    nameError = '';
    try {
      const res = await setVisitorName(trimmed);
      dispatch('setname', { name: res.name });
    } catch {
      nameError = 'Could not save. Try again.';
    } finally {
      nameBusy = false;
    }
  }

  function onNameKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      saveName();
    }
  }

  /** Enter submits the note (chat convention); Shift+Enter inserts a newline. */
  function onComposerKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (!noteBusy && !needsName) submitNote();
    }
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
    {#if needsName}
      <div class="name-prompt" class:focused={nameFocused}>
        <div class="np-head">
          <UserCirclePlus size={16} weight="fill" />
          <span>Set a name so notes are attributed to you.</span>
        </div>
        <div class="np-row">
          <input
            type="text"
            placeholder="Your name"
            bind:value={nameDraft}
            on:keydown={onNameKey}
            on:focus={() => (nameFocused = true)}
            on:blur={() => (nameFocused = false)}
            maxlength="60"
            aria-label="Display name"
            autocomplete="name"
            enterkeyhint="done"
          />
          <button class="np-save" on:click={saveName} disabled={nameBusy || !nameDraft.trim()}>
            {nameBusy ? 'Saving…' : 'Save'}
          </button>
        </div>
        {#if nameError}<span class="np-err">{nameError}</span>{/if}
      </div>
    {/if}
    <textarea
      bind:value={draft}
      placeholder={needsName ? 'Add a note (set your name first)…' : 'Add a note…'}
      rows="2"
      maxlength="2000"
      disabled={needsName}
      enterkeyhint="send"
      on:keydown={onComposerKey}
    ></textarea>
    <button class="primary" on:click={submitNote} disabled={noteBusy || needsName || !draft.trim()}>
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
    transition: background 0.15s ease, border-color 0.15s ease;
  }
  .composer .primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }
  .composer .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
  textarea:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .name-prompt {
    background: var(--bg-elev-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 9px 10px;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .name-prompt.focused {
    border-color: var(--accent);
  }
  .np-head {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-dim);
    font-size: 12px;
  }
  .np-row {
    display: flex;
    gap: 6px;
  }
  .np-row input {
    flex: 1;
    min-width: 0;
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 6px 9px;
    font: inherit;
  }
  .np-row input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .np-save {
    border: 1px solid var(--accent);
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius);
    padding: 6px 12px;
    font: inherit;
    transition: background 0.15s ease, border-color 0.15s ease;
  }
  .np-save:hover:not(:disabled) {
    background: var(--accent-strong);
  }
  .np-save:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .np-err {
    color: var(--danger);
    font-size: 12px;
  }
</style>
