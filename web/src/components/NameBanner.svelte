<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { setVisitorName } from '../api';

  export let visible = false;
  export let current = '';

  const dispatch = createEventDispatcher<{ saved: { name: string }; dismiss: void }>();

  let name = '';
  let saving = false;
  let error = '';
  let lastVisible = false;

  // Seed the field with the current name each time the banner opens.
  $: if (visible && !lastVisible) {
    name = current;
    lastVisible = true;
  } else if (!visible) {
    lastVisible = false;
  }

  async function save() {
    const trimmed = name.trim();
    if (!trimmed) return;
    saving = true;
    error = '';
    try {
      const res = await setVisitorName(trimmed);
      dispatch('saved', { name: res.name });
    } catch {
      error = 'Could not save your name. Try again.';
    } finally {
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter') save();
  }
</script>

{#if visible}
  <div class="banner" role="region" aria-label="Set your display name">
    <span class="msg">
      {current
        ? 'Change your display name (used on notes you add).'
        : 'Set a display name so your notes are attributed to you.'}
    </span>
    <div class="controls">
      <input
        type="text"
        placeholder="Your name"
        bind:value={name}
        on:keydown={onKey}
        maxlength="60"
        aria-label="Display name"
      />
      <button class="primary" on:click={save} disabled={saving || !name.trim()}>
        {saving ? 'Saving…' : 'Save'}
      </button>
      <button class="ghost" on:click={() => dispatch('dismiss')}>Not now</button>
    </div>
    {#if error}<span class="error">{error}</span>{/if}
  </div>
{/if}

<style>
  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    background: var(--bg-elev-2);
    border-bottom: 1px solid var(--border);
    padding: 8px 12px;
  }
  .msg {
    color: var(--text-dim);
    font-size: 14px;
  }
  .controls {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }
  input {
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 6px 10px;
    min-width: 160px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  button {
    border-radius: var(--radius);
    padding: 6px 14px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text);
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  button.primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button.ghost {
    background: transparent;
  }
  .error {
    color: var(--danger);
    font-size: 13px;
    width: 100%;
  }
</style>
