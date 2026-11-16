<script>
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';
  import Update from './Update.svelte';
  import { uuob } from './uuob.js';

  let { app, Main } = $props();
  // Connected before: show the app at once (from cache) and check in the background.
  let state = $state(uuob.hasToken ? 'ready' : 'checking'); // checking | unreachable | connect | waiting | ready
  let offline = $state(false); // connected before, UUOB not reachable right now
  let asking = $state(false); // a question waits in UUOB on another device
  let code = $state('');
  let useCode = $state(false); // code entry is the secondary path
  let busy = $state(false); // a code is being tried
  let error = $state('');
  // A code is a number and three words, like 42-amber-river-falcon.
  const codeOk = $derived(code.split(/[^a-z0-9]+/i).filter(Boolean).length === 4);
  let ctl = null;
  const isAndroid = /Android/.test(navigator.userAgent);

  async function check() {
    const s = await uuob.check();
    if (state === 'waiting') return;
    if (s === 'unreachable' && isAndroid) startUuob();
    if (s === 'unreachable' && state === 'ready') return void (offline = true);
    state = s === 'connected' ? 'ready' : s === 'disconnected' ? 'connect' : 'unreachable';
  }

  // Phones: UUOB isn't running (e.g. after a restart). Start it in the
  // background once; it steps aside right away and we reconnect.
  let started = false;
  async function startUuob() {
    if (started) return;
    started = true;
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl('uuob://start');
    } catch {}
  }

  async function connect(withCode) {
    error = '';
    const from = state;
    if (!withCode) {
      state = 'waiting';
      if (isAndroid) openUuob();
    }
    ctl = new AbortController();
    busy = withCode;
    try {
      await uuob.connect(withCode ? code : undefined, { signal: ctl.signal });
      state = 'ready';
      offline = false;
      code = '';
    } catch (e) {
      if (e.name === 'AbortError') return void (state = 'connect');
      error = e.message;
      // A refused code leaves the user where they typed it.
      state = e.code === 'unreachable' ? 'unreachable' : withCode ? from : 'connect';
    } finally {
      busy = false;
    }
  }

  // Phones: bring UUOB to the front so the user can tap Allow.
  async function openUuob() {
    try {
      const { openUrl } = await import('@tauri-apps/plugin-opener');
      await openUrl('uuob://connect');
    } catch {}
  }

  onMount(() => {
    check();
    const off = uuob.onState((s) => {
      if (s === 'disconnected' && state === 'ready') state = 'connect';
      offline = s === 'unreachable' && state === 'ready';
    });
    const offAsk = uuob.onAsk((a) => (asking = a));
    const t = setInterval(() => (state === 'unreachable' || offline) && !busy && check(), 3000);
    return () => (off(), offAsk(), clearInterval(t));
  });
</script>

{#snippet codeForm()}
  <form class="code" onsubmit={(e) => (e.preventDefault(), connect(true))}>
    <input class="field" bind:value={code} placeholder="42-amber-river-falcon" autocapitalize="none" autocorrect="off" spellcheck="false" autocomplete="off" enterkeyhint="go" aria-label="Code from UUOB" />
    <button class="btn lg" disabled={!codeOk || busy}>{busy ? 'Connecting…' : 'Connect'}</button>
  </form>
{/snippet}

<Update />
{#if state === 'ready'}
  <Main />
  {#if offline}
    <div class="offline">Waiting for UUOB. Your changes are kept and sent when it’s back.</div>
  {:else if asking}
    <div class="offline">Answer in UUOB on one of your devices.</div>
  {/if}
{:else if state !== 'checking'}
  <div class="gate scroll">
    <div class="box">
      <div class="pair" class:busy={state === 'waiting'}>
        <span class="tile" style="background:{app.color}"><Icon name={app.icon} size={30} stroke={2} /></span>
        <span class="link"><i></i><i></i><i></i></span>
        <span class="logo"><Logo size={68} radius={19} /></span>
      </div>

      {#if state === 'unreachable'}
        <h1 class="large-title">{app.name} needs UUOB</h1>
        <p class="secondary">{app.name} has no server of its own. It keeps your {app.noun} in UUOB, on your own devices, end-to-end encrypted.</p>
        <p class="secondary">Install UUOB on this device and open it.</p>
        <div class="looking caption"><span class="spinner"></span> Looking for UUOB…</div>
        {#if isAndroid}<button class="btn lg wide" onclick={openUuob}>Open UUOB</button>{/if}

        <div class="other">
          <h2 class="headline">Or use UUOB on another device</h2>
          <p class="caption">In UUOB there, open Apps, choose <b>Connect with a code</b>, and type the code here.</p>
          {@render codeForm()}
          {#if error}<p class="error">{error}</p>{/if}
        </div>
      {:else if state === 'waiting'}
        <h1 class="large-title">Allow it in UUOB</h1>
        <p class="secondary">UUOB is asking whether {app.name} may keep its {app.noun} in your network.</p>
        <div class="looking caption"><span class="spinner"></span> Waiting for your answer…</div>
        <button class="btn lg wide" onclick={() => ctl?.abort()}>Cancel</button>
      {:else}
        <h1 class="large-title">Connect {app.name}</h1>
        <p class="secondary">{app.name} has no server of its own. Your {app.noun} live in UUOB, on your own devices, end-to-end encrypted.</p>
        <div class="found caption"><span class="dot" style="background:var(--ok)"></span> UUOB is running on {uuob.device ?? 'this device'}</div>
        <button class="btn primary lg wide" onclick={() => connect(false)}>Connect to UUOB</button>

        {#if error}<p class="error">{error}</p>{/if}
        {#if useCode}
          {@render codeForm()}
          <p class="caption hint">In UUOB, open Apps and choose <b>Connect with a code</b>.</p>
        {:else}
          <button class="code-link" onclick={() => (useCode = true)}>Use a code instead</button>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .gate { height: 100vh; display: grid; place-items: center; padding: 32px 20px; }
  .box { width: min(420px, 100%); text-align: center; }
  .pair { display: flex; align-items: center; justify-content: center; gap: 14px; margin-bottom: 28px; }
  .tile, .logo { width: 68px; height: 68px; border-radius: 19px; display: grid; place-items: center; color: #fff; flex: none; }
  .logo { box-shadow: 0 6px 20px rgba(110, 90, 255, 0.35); }
  .link { display: flex; gap: 6px; }
  .link i { width: 6px; height: 6px; border-radius: 50%; background: var(--text-3); }
  .busy .link i { animation: blink 1.2s infinite; }
  .busy .link i:nth-child(2) { animation-delay: 0.2s; }
  .busy .link i:nth-child(3) { animation-delay: 0.4s; }
  @keyframes blink { 0%, 100% { opacity: 0.25; } 40% { opacity: 1; background: var(--accent); } }
  .large-title { font-size: 30px; }
  .secondary { margin: 12px 0 0; }
  .found, .looking { justify-content: center; margin: 22px 0 14px; display: flex; align-items: center; gap: 8px; }
  .wide { width: 100%; }
  .code-link { display: block; margin: 18px auto 0; border: 0; background: none; color: var(--accent); font: inherit; font-size: 15px; font-weight: 500; cursor: pointer; padding: 8px 12px; border-radius: 10px; }
  .code-link:hover { background: var(--accent-soft); }
  .code { display: flex; gap: 10px; margin-top: 24px; }
  .code .field { flex: 1; min-width: 0; height: 50px; font-size: 18px; text-align: center; }
  .other { margin-top: 32px; padding-top: 28px; border-top: 1px solid var(--line); }
  .other .headline { font-size: 17px; margin: 0; }
  .other .caption { margin: 8px 0 0; }
  .other .code { margin-top: 18px; }
  .hint { margin: 10px 0 0; }
  .error { color: var(--danger); margin: 12px 0 0; }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--line-strong); border-top-color: var(--accent);
    animation: spin 0.8s linear infinite; display: inline-block;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .offline {
    position: fixed; left: 50%; bottom: 20px; transform: translateX(-50%); z-index: 300;
    width: max-content; max-width: calc(100vw - 24px); text-align: center; text-wrap: balance;
    background: var(--bg-elevated); border: 1px solid var(--line-strong); box-shadow: var(--shadow);
    padding: 10px 18px; border-radius: 22px; font-size: 14px; color: var(--warn);
  }
</style>
