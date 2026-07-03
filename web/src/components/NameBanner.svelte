<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { setVisitorName } from '../api';
  import { focusOnMount } from '../actions';

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
    // The saving guard stops Enter from double-submitting mid-flight.
    if (!trimmed || saving) return;
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
    if (e.key === 'Enter') {
      save();
    } else if (e.key === 'Escape') {
      dispatch('dismiss');
    }
  }
</script>

{#if visible}
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-label="Set your display name"
    tabindex="-1"
    on:click|self={() => dispatch('dismiss')}
    on:keydown={onKey}
  >
    <div class="modal">
      <h2>Your display name</h2>
      <p class="msg">
        {current
          ? 'Change your display name (used on notes you add).'
          : 'Set a display name so your notes are attributed to you.'}
      </p>
      <input
        type="text"
        placeholder="Your name"
        bind:value={name}
        on:keydown={onKey}
        maxlength="60"
        aria-label="Display name"
        autocomplete="name"
        enterkeyhint="done"
        use:focusOnMount
      />
      {#if error}<span class="error">{error}</span>{/if}
      <div class="actions">
        <button class="ghost" on:click={() => dispatch('dismiss')}>{current ? 'Cancel' : 'Not now'}</button>
        <button class="primary" on:click={save} disabled={saving || !name.trim()}>
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 3000000;
    background: rgba(0, 0, 0, 0.4);
    display: grid;
    place-items: center;
    padding: 20px;
  }
  .modal {
    width: 100%;
    max-width: 360px;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 18px 18px 14px;
    display: flex;
    flex-direction: column;
    gap: 9px;
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.22);
  }
  h2 {
    margin: 0;
    font-size: 16px;
  }
  .msg {
    margin: 0 0 4px;
    color: var(--text-dim);
    font-size: 13px;
    line-height: 1.4;
  }
  input {
    background: var(--bg);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 9px 11px;
    font: inherit;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  button {
    border-radius: var(--radius);
    padding: 8px 16px;
    border: 1px solid var(--border);
    background: var(--bg-elev);
    color: var(--text);
    font: inherit;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    transition: background 0.15s ease, border-color 0.15s ease;
  }
  button.primary:hover:not(:disabled) {
    background: var(--accent-strong);
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
  }
</style>
