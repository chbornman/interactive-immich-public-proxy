<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { LockKey } from 'phosphor-svelte';

  export let error = '';
  export let busy = false;

  const dispatch = createEventDispatcher<{ submit: { password: string } }>();
  let password = '';

  function submit() {
    if (!password) return;
    dispatch('submit', { password });
  }
</script>

<div class="gate">
  <form class="card" on:submit|preventDefault={submit}>
    <LockKey size={34} weight="duotone" />
    <h2>Password required</h2>
    <p>This album is protected. Enter its password to view.</p>
    <input
      type="password"
      bind:value={password}
      placeholder="Album password"
      autocomplete="current-password"
      aria-label="Album password"
    />
    <button type="submit" disabled={busy || !password}>
      {busy ? 'Unlocking…' : 'Unlock'}
    </button>
    {#if error}<p class="err">{error}</p>{/if}
  </form>
</div>

<style>
  .gate {
    min-height: 70vh;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 100%;
    max-width: 320px;
    text-align: center;
    color: var(--text);
  }
  h2 {
    margin: 4px 0 0;
    font-size: 18px;
  }
  p {
    margin: 0;
    color: var(--text-dim);
    font-size: 14px;
  }
  input {
    width: 100%;
    background: var(--bg-elev);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius);
    padding: 9px 12px;
    margin-top: 6px;
  }
  input:focus {
    outline: none;
    border-color: var(--accent);
  }
  button {
    width: 100%;
    background: var(--accent);
    border: 1px solid var(--accent);
    color: #fff;
    border-radius: var(--radius);
    padding: 9px 12px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .err {
    color: var(--danger);
  }
</style>
