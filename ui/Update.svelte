<script>
  // "Update to 0.1.2": appears when a newer version of this app is published.
  // One tap downloads it, checks its signature and installs it; the app then
  // restarts (computers) or closes (Android). See crates/uuob-update.
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let version = $state(null);
  let busy = $state(false);
  let error = $state('');
  let checked = 0;
  // Android replaces the app while it is closed; it doesn't start it again.
  const closes = /Android/.test(navigator.userAgent);

  // When the app opens and when the user comes back to it, at most hourly.
  async function check() {
    if (busy || document.hidden || Date.now() - checked < 3600_000) return;
    checked = Date.now();
    try {
      version = await invoke('plugin:uuob-update|check');
    } catch {}
  }

  async function install() {
    busy = true;
    error = '';
    try {
      await invoke('plugin:uuob-update|install');
    } catch (e) {
      if (e !== 'cancelled') error = String(e);
    }
    busy = false;
  }

  onMount(() => {
    check();
    document.addEventListener('visibilitychange', check);
    return () => document.removeEventListener('visibilitychange', check);
  });
</script>

{#if version}
  <div class="update">
    <button onclick={install} disabled={busy}>{busy ? 'Updating…' : error ? 'Try again' : `Update to ${version}`}</button>
    {#if error}<p class="error">{error}</p>{:else if busy && closes}<p>The app closes to finish. Open it again.</p>{/if}
  </div>
{/if}

<style>
  .update {
    position: fixed; left: 50%; top: 10px; transform: translateX(-50%); z-index: 90; /* under sheets and dialogs */
    display: grid; justify-items: center; gap: 6px; max-width: calc(100vw - 32px);
  }
  button {
    height: 34px; padding: 0 16px; border: 0; border-radius: 999px; cursor: pointer; white-space: nowrap;
    background: var(--accent); color: var(--on-accent); font: inherit; font-size: 14px; font-weight: 600;
    box-shadow: var(--shadow); transition: transform 0.15s var(--ease), background 0.15s;
  }
  button:hover { background: color-mix(in srgb, var(--accent), #fff 12%); }
  button:active { transform: scale(0.97); }
  button:disabled { opacity: 0.7; pointer-events: none; }
  p {
    margin: 0; padding: 8px 14px; border-radius: var(--r); font-size: 13px; text-align: center;
    background: var(--bg-elevated); border: 1px solid var(--line-strong); color: var(--text-2); box-shadow: var(--shadow);
  }
  .error { color: var(--danger); }
</style>
