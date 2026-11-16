<script>
  // An invite code with its QR code and a countdown. Used for linking a
  // device, for linking with a person, and (typed, so without the QR code)
  // for connecting an app.
  import { onMount } from 'svelte';
  import Qr from './Qr.svelte';

  let { invite, waiting = 'Waiting…', qr = true } = $props();
  let now = $state(Date.now());
  const left = $derived(Math.max(0, Math.round((invite.expires_at - now) / 1000)));
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });
</script>

<div class="code">
  {#each invite.code.split('-') as part, i}
    <span class:num={i === 0}>{part}</span>
  {/each}
</div>
{#if qr}<div class="qr-wrap"><Qr text={invite.code} size={150} /></div>{/if}
<div class="waiting caption">
  <span class="spinner"></span> {waiting} Code expires in {Math.floor(left / 60)}:{String(left % 60).padStart(2, '0')}
</div>

<style>
  .code { display: flex; flex-wrap: wrap; gap: 8px; justify-content: center; margin-bottom: 22px; }
  .code span {
    font: 650 22px/1 var(--font-display); letter-spacing: -0.01em; padding: 12px 14px; border-radius: 12px;
    background: var(--surface-2); border: 1px solid var(--line);
  }
  .code span.num { background: var(--accent-soft); color: var(--accent); border-color: transparent; }
  .qr-wrap { display: flex; justify-content: center; margin-bottom: 18px; }
  .waiting { display: flex; align-items: center; gap: 6px; justify-content: center; margin: 4px 0 20px; }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--line-strong); border-top-color: var(--accent);
    animation: spin 0.8s linear infinite; display: inline-block; flex: none;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
