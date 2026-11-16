<script>
  import { onMount } from 'svelte';
  import Icon from '../../../ui/Icon.svelte';
  import Code from './Code.svelte';
  import { call, onEvent, message, scanCode, isPhone, autostart, setAutostart } from './api.js';

  let { onToast } = $props();
  let devices = $state([]);
  let copies = $state(1);
  let sheet = $state(null); // 'add' | 'join' | 'leave' | { remove: device }
  let invite = $state(null);
  let joined = $state(null);
  let code = $state('');
  let busy = $state(false);
  let error = $state('');
  let editing = $state(false);
  let name = $state('');

  const alone = $derived(devices.length <= 1);
  const me = $derived(devices.find((d) => d.is_self));

  const keepers = $derived(devices.filter((d) => d.keeper).length);

  // Start UUOB in the background: at login on a computer, when a phone turns on. Null where there is no such choice.
  let startsAtLogin = $state(null);
  async function setStartsAtLogin(on) {
    try {
      await setAutostart(on);
      startsAtLogin = await autostart();
    } catch (e) {
      onToast(message(e));
    }
  }

  const load = async () => {
    startsAtLogin = await autostart();
    devices = await call('devices');
    copies = (await call('status')).network.copies ?? 1;
  };

  // A storage device keeps everything; other devices may then free space.
  async function setKeeper(d, on) {
    try {
      await call('set_keeper', { device: d.id, on });
      load();
    } catch (e) {
      onToast(message(e));
    }
  }

  async function setCopies(n) {
    try {
      await call('set_copies', { copies: n });
      load();
    } catch (e) {
      onToast(message(e));
    }
  }

  async function startInvite() {
    sheet = 'add';
    invite = null;
    joined = null;
    error = '';
    try {
      invite = await call('create_invite');
    } catch (e) {
      error = message(e);
    }
  }

  function closeSheet() {
    if (invite && !joined) call('cancel_invite', { code: invite.code });
    sheet = null;
    invite = null;
    code = '';
    error = '';
    busy = false;
  }

  async function join() {
    busy = true;
    error = '';
    try {
      await call('join', { code });
      onToast('Linked! Your data will sync in a moment.');
      closeSheet();
      load();
    } catch (e) {
      error = message(e);
      busy = false;
    }
  }

  // Phones: scan the QR code shown on the other device.
  async function scanQr() {
    error = '';
    try {
      code = await scanCode();
      await join();
    } catch (e) {
      error = message(e);
    }
  }

  async function remove(d) {
    await call('remove_device', { id: d.id });
    sheet = null;
    onToast(`${d.name} was removed`);
    load();
  }

  async function leave() {
    await call('leave_network');
    sheet = null;
    onToast('This device now has its own network');
    load();
  }

  async function saveName() {
    try {
      await call('set_device_name', { name });
      editing = false;
      load();
    } catch (e) {
      onToast(message(e));
    }
  }

  const icon = (d) => (/android|ios/.test(d.platform) ? 'phone' : 'laptop');
  const platformName = (p) => ({ linux: 'Linux', macos: 'macOS', windows: 'Windows', android: 'Android', ios: 'iOS' })[p] ?? p;

  onMount(() => {
    load();
    const un = onEvent((e) => {
      if (e.type === 'device_joined' && sheet === 'add') joined = e.name;
      load();
    });
    return () => un.then((f) => f());
  });
</script>

<div class="page scroll">
  <header>
    <h1 class="large-title">Devices</h1>
    <p class="secondary lead">Your devices form one private network. Every app's data lives on them, synced and end-to-end encrypted. No cloud, no accounts.</p>
  </header>

  <div class="group">
    {#each devices as d (d.id)}
      <div class="row device">
        <span class="dev-icon"><Icon name={icon(d)} size={24} /></span>
        <div class="grow">
          {#if d.is_self && editing}
            <form class="rename" onsubmit={(e) => (e.preventDefault(), saveName())}>
              <input class="field" bind:value={name} autofocus />
              <button class="btn primary">Save</button>
            </form>
          {:else}
            <div class="headline">{d.name} {#if d.is_self}<span class="tag">This device</span>{/if}</div>
            <div class="caption">
              <span class="dot" style="background:{d.online ? 'var(--ok)' : 'var(--text-3)'}"></span>
              {d.online ? 'Online' : 'Offline'} · {platformName(d.platform)}
            </div>
          {/if}
        </div>
        {#if d.is_self && !editing}
          <button class="icon-btn" title="Rename" onclick={() => ((name = d.name), (editing = true))}><Icon name="edit" size={18} /></button>
        {:else if !d.is_self}
          <button class="icon-btn" title="Remove from network" onclick={() => (sheet = { remove: d })}><Icon name="trash" size={18} /></button>
        {/if}
      </div>
    {/each}
  </div>

  {#if !alone}
    <div class="section-label storage-label">Storage devices</div>
    <div class="group">
      {#each devices as d (d.id)}
        <div class="row">
          <div class="grow">
            <div class="headline">{d.name}</div>
            <div class="caption">{d.keeper ? 'Keeps everything' : 'Keeps what its apps ask for'}</div>
          </div>
          <button class="switch" role="switch" aria-checked={d.keeper} aria-label="Keep everything on {d.name}" onclick={() => setKeeper(d, !d.keeper)}></button>
        </div>
      {/each}
      {#if keepers}
        <div class="row">
          <div class="grow">
            <div class="headline">Copies</div>
            <div class="caption">Storage devices that must have something before another device frees its own copy.</div>
          </div>
          <div class="segmented" role="group" aria-label="Copies">
            {#each [1, 2] as n}
              <button class:on={copies === n} aria-pressed={copies === n} onclick={() => setCopies(n)}>{n}</button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
    <p class="caption storage-note">
      {#if keepers}
        Apps with large data, like photos, can keep less on your other devices. Nothing is removed from a device until {copies === 1 ? 'a storage device has' : `${copies} storage devices have`} it.
      {:else}
        Turn this on for a device with plenty of space that is usually on. Apps with large data, like photos, can then keep less on your other devices.
      {/if}
    </p>
  {/if}

  {#if startsAtLogin !== null}
    <div class="section-label storage-label">{isPhone ? 'This phone' : 'This computer'}</div>
    <div class="group">
      <div class="row">
        <div class="grow">
          <div class="headline">{isPhone ? 'Start when the phone turns on' : 'Start at login'}</div>
          <div class="caption">UUOB starts in the background {isPhone ? 'once you unlock the phone' : 'when you log in'}, so your apps and devices stay in sync.</div>
        </div>
        <button class="switch" role="switch" aria-checked={startsAtLogin} aria-label={isPhone ? 'Start UUOB when the phone turns on' : 'Start UUOB at login'} onclick={() => setStartsAtLogin(!startsAtLogin)}></button>
      </div>
    </div>
  {/if}

  <div class="cta">
    <button class="btn primary lg" onclick={startInvite}><Icon name="plus" size={20} stroke={2.2} /> Add a device</button>
    {#if alone}
      <button class="btn lg" onclick={() => ((sheet = 'join'), (error = ''))}><Icon name="link" size={20} /> Join a network</button>
    {:else}
      <button class="btn ghost" onclick={() => (sheet = 'leave')}>Leave this network</button>
    {/if}
  </div>
</div>

{#if sheet === 'add'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && closeSheet()} role="presentation">
    <div class="sheet center">
      {#if joined}
        <div class="success"><Icon name="check" size={40} stroke={2.5} /></div>
        <h2 class="title">{joined} joined</h2>
        <p class="secondary">Your apps and data are syncing to it now.</p>
        <button class="btn primary lg wide" onclick={closeSheet}>Done</button>
      {:else if invite}
        <h2 class="title">Add a device</h2>
        <p class="secondary steps">On the other device, open UUOB, choose <b>Join a network</b> and enter this code.</p>
        <Code {invite} waiting="Waiting for your device…" />
        <button class="btn wide" onclick={closeSheet}>Cancel</button>
      {:else if error}
        <h2 class="title">Couldn’t create a code</h2>
        <p class="secondary">{error}</p>
        <button class="btn wide" onclick={closeSheet}>Close</button>
      {:else}
        <div class="waiting caption"><span class="spinner"></span> Creating a code…</div>
      {/if}
    </div>
  </div>
{:else if sheet === 'join'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && closeSheet()} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), join())}>
      <h2 class="title">Join a network</h2>
      <p class="secondary">On a device that’s already set up, open <b>Devices → Add a device</b> and type the code it shows.</p>
      {#if isPhone}
        <button type="button" class="btn primary lg scan" onclick={scanQr}><Icon name="search" size={20} /> Scan QR code</button>
        <div class="or caption">or type the code</div>
      {/if}
      <input class="field code-input" placeholder="e.g. 42 amber river falcon" bind:value={code} autofocus autocomplete="off" autocorrect="off" autocapitalize="none" spellcheck="false" enterkeyhint="go" />
      {#if error}<p class="error">{error}</p>{/if}
      <p class="caption note">Anything already on this device is kept and merged into the network.</p>
      <div class="actions">
        <button type="button" class="btn lg" onclick={closeSheet}>Cancel</button>
        <button class="btn lg primary" disabled={busy || code.trim().split(/[\s-]+/).length < 4}>
          {#if busy}<span class="spinner light"></span> Linking…{:else}Join{/if}
        </button>
      </div>
    </form>
  </div>
{:else if sheet === 'leave'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Leave this network?</h2>
      <p class="secondary">This device stops syncing with your other devices. Everything already on it stays here.</p>
      <div class="actions">
        <button class="btn lg" onclick={() => (sheet = null)}>Cancel</button>
        <button class="btn lg danger" onclick={leave}>Leave</button>
      </div>
    </div>
  </div>
{:else if sheet?.remove}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Remove “{sheet.remove.name}”?</h2>
      <p class="secondary">It stops receiving updates right away. Data it already has stays on it. To use it again, add it back with a new code.</p>
      <div class="actions">
        <button class="btn lg" onclick={() => (sheet = null)}>Cancel</button>
        <button class="btn lg danger" onclick={() => remove(sheet.remove)}>Remove</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { height: 100%; padding: 56px 32px 48px; }
  .page > * { max-width: 680px; margin-left: auto; margin-right: auto; }
  header { margin-bottom: 28px; }
  .lead { margin: 10px 0 0; font-size: 17px; max-width: 560px; }
  .device { padding: 16px 18px; }
  .dev-icon { width: 44px; height: 44px; border-radius: 12px; background: var(--surface-2); display: grid; place-items: center; color: var(--text-2); }
  .grow { flex: 1; min-width: 0; }
  .caption { display: flex; align-items: center; gap: 6px; margin-top: 2px; }
  .tag { white-space: nowrap; font-size: 12px; font-weight: 600; color: var(--accent); background: var(--accent-soft); padding: 2px 8px; border-radius: 999px; margin-left: 6px; vertical-align: 2px; }
  .rename { display: flex; gap: 8px; }
  .rename .field { height: 40px; }
  .cta { display: flex; gap: 12px; flex-wrap: wrap; margin-top: 24px; align-items: center; }
  .storage-label { margin-top: 28px; }
  .storage-note { display: block; margin-top: 10px; padding: 0 4px; }
  .center { text-align: center; }
  .steps { margin: 6px auto 22px; max-width: 340px; }
  .waiting { justify-content: center; margin: 4px 0 20px; }
  .wide { width: 100%; }
  .success {
    width: 76px; height: 76px; border-radius: 50%; margin: 4px auto 16px; display: grid; place-items: center;
    background: var(--ok); color: #fff; animation: pop 0.45s var(--spring);
  }
  .success + .title { margin-bottom: 6px; }
  .success ~ .btn { margin-top: 22px; }
  .scan { width: 100%; margin-top: 18px; }
  .or { justify-content: center; margin: 14px 0 -6px; }
  .code-input { margin-top: 18px; height: 54px; font-size: 20px; text-align: center; }
  .error { color: var(--danger); font-size: 15px; margin: 10px 0 0; }
  .note { margin: 14px 0 0; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--line-strong); border-top-color: var(--accent);
    animation: spin 0.8s linear infinite; display: inline-block;
  }
  .spinner.light { border-color: rgba(255,255,255,.35); border-top-color: #fff; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 720px) { .page { padding: 28px 16px 32px; } .large-title { font-size: 30px; } }
</style>
