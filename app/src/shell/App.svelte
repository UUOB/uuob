<script>
  import { onMount } from 'svelte';
  import Icon from '../../../ui/Icon.svelte';
  import Logo from '../../../ui/Logo.svelte';
  import Update from '../../../ui/Update.svelte';
  import Devices from './Devices.svelte';
  import People from './People.svelte';
  import Share from './Share.svelte';
  import Apps from './Apps.svelte';
  import Ai from './Ai.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { call, onEvent, initial } from './api.js';
  import { appIcon, appColor } from './apps.js';

  const screens = [
    { id: 'devices', name: 'Devices', icon: 'devices' },
    { id: 'people', name: 'People', icon: 'people' },
    { id: 'apps', name: 'Apps', icon: 'data' },
    { id: 'ai', name: 'AI access', icon: 'ai' },
  ];

  let status = $state(null);
  let view = $state('devices');
  let asks = $state([]); // questions from apps, answered here
  let toast = $state(null);
  let tick = $state(0); // bumped on network changes so screens reload
  const ask = $derived(asks[0]);
  let people = $state([]);
  let later = $state([]); // offers put aside until UUOB is opened again
  // Things other people offered to share, waiting for an answer.
  const offers = $derived(people.flatMap((p) => p.receiving.filter((s) => s.offered).map((s) => ({ ...s, person: p }))));
  const offer = $derived(offers.find((o) => !later.includes(o.container)));

  let serverError = $state(null);
  const refresh = async () => {
    [status, people] = await Promise.all([call('status'), call('people')]);
    serverError = await invoke('server_error');
  };
  const loadAsks = async () => (asks = await call('asks'));

  function showToast(text) {
    toast = text;
    clearTimeout(showToast.t);
    showToast.t = setTimeout(() => (toast = null), 3200);
  }

  async function answer(allow) {
    const a = ask;
    await call('answer', { id: a.id, allow }).catch(() => {});
    if (allow && a.kind === 'connect') showToast(`${a.name} is connected`);
    loadAsks();
    tick++;
  }

  async function answerOffer(accept) {
    const o = offer;
    later = [...later, o.container];
    await call(accept ? 'accept_share' : 'delete_container', accept ? { container: o.container } : { id: o.container }).catch(() => {});
    if (accept) showToast(`“${o.name}” was added to ${o.app_name}`);
    refresh();
    tick++;
  }

  onMount(() => {
    refresh();
    loadAsks();
    const un = onEvent((e) => {
      if (e.type === 'container_changed') return;
      if (e.type === 'asks_changed') return void loadAsks();
      if (e.type === 'device_joined') showToast(`${e.name} joined your network`);
      if (e.type === 'removed_from_network') showToast('This device was removed from the network');
      refresh();
      tick++;
    });
    return () => un.then((f) => f());
  });
</script>

<svelte:document onvisibilitychange={() => document.visibilityState === 'visible' && loadAsks()} />
<svelte:window onkeydown={(e) => ask && ask.kind !== 'share' && (e.key === 'Escape' ? answer(false) : e.key === 'Enter' && answer(true))} />

<Update />
<div class="shell">
  <nav class="rail">
    <div class="logo" title="UUOB"><Logo size={40} radius={12} /></div>
    {#each screens as s (s.id)}
      <button class="nav" class:on={view === s.id} onclick={() => (view = s.id)}>
        <span class="tile">
          <Icon name={s.icon} size={20} />
          {#if s.id === 'devices' && status?.devices_total > 1}
            <span class="badge" style="background:{status.devices_online > 1 ? 'var(--ok)' : 'var(--text-3)'}"></span>
          {:else if s.id === 'people' && offers.length}
            <span class="badge" style="background:var(--danger)"></span>
          {/if}
        </span>
        <span class="lbl">{s.name}</span>
      </button>
    {/each}
    <div class="spacer"></div>
    <div class="lock" title="End-to-end encrypted. Your data is stored only on your own devices."><Icon name="lock" size={16} /></div>
  </nav>

  <main>
    {#if serverError}<div class="server-error">{serverError} Close other copies of UUOB and open it again.</div>{/if}
    {#if view === 'people'}
      <!-- Reloads itself; staying mounted keeps an open sheet (a code, a name) through network changes. -->
      <People onToast={showToast} />
    {:else}
      {#key tick}
        {#if view === 'devices'}
          <Devices onToast={showToast} />
        {:else if view === 'apps'}
          <Apps onToast={showToast} />
        {:else}
          <Ai onToast={showToast} />
        {/if}
      {/key}
    {/if}
  </main>
</div>

{#if ask?.kind === 'share'}
  {#key ask.id}<Share {ask} onDone={() => answer(true)} onToast={showToast} />{/key}
{:else if ask}
  <div class="scrim">
    <div class="sheet center">
      <div class="perm-icons">
        <span class="app-tile" style="background:{appColor(ask.app)}"><Icon name={appIcon(ask.app)} size={26} stroke={2} /></span>
        <span class="arrow"><Icon name="right" /></span>
        {#if ask.kind === 'connect'}
          <span class="logo big"><Logo size={58} radius={16} /></span>
        {:else}
          <span class="app-tile" style="background:{appColor(ask.container)}"><Icon name={appIcon(ask.container)} size={26} stroke={2} /></span>
        {/if}
      </div>
      {#if ask.kind === 'connect'}
        <h2 class="title">“{ask.name}” wants to use your network</h2>
        <p class="secondary">It gets its own place for its data, synced to all your devices. It can’t see anything else unless you allow it.</p>
      {:else}
        <h2 class="title">“{ask.app_name}” wants to {ask.access === 'write' ? 'read and change' : 'read'} your {ask.container_name} data</h2>
        <p class="secondary">You can change this any time in Apps.</p>
      {/if}
      <div class="actions">
        <button class="btn lg" onclick={() => answer(false)}>Don’t allow</button>
        <button class="btn lg primary" onclick={() => answer(true)}>Allow</button>
      </div>
    </div>
  </div>
{:else if offer}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (later = [...later, offer.container])} role="presentation">
    <div class="sheet center">
      <div class="perm-icons">
        <span class="avatar">{initial(offer.person.name)}</span>
        <span class="arrow"><Icon name="right" /></span>
        <span class="app-tile" style="background:{appColor(offer.app)}"><Icon name={appIcon(offer.app)} size={26} stroke={2} /></span>
      </div>
      <h2 class="title">{offer.person.name} shared “{offer.name}” with you</h2>
      <p class="secondary">
        It will appear in {offer.app_name} on all your devices. {offer.access === 'write' ? 'You can both change it.' : 'You can view it, but not change it.'}
      </p>
      <div class="actions">
        <button class="btn lg" onclick={() => answerOffer(false)}>Decline</button>
        <button class="btn lg primary" onclick={() => answerOffer(true)}>Accept</button>
      </div>
    </div>
  </div>
{/if}

{#if toast}<div class="toast">{toast}</div>{/if}

<style>
  .shell { display: flex; height: 100vh; }
  .rail {
    width: 84px; flex: none; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 16px 0 14px;
    background: var(--sidebar); backdrop-filter: saturate(180%) blur(30px); border-right: 1px solid var(--line);
  }
  .logo { flex: none; margin-bottom: 14px; border-radius: 12px; box-shadow: 0 4px 14px rgba(0, 0, 0, 0.4); }
  .logo.big { margin: 0; border-radius: 16px; }
  .nav {
    display: flex; flex-direction: column; align-items: center; gap: 4px; width: 76px; border: 0; background: transparent;
    padding: 6px 0; border-radius: 14px; cursor: pointer; color: var(--text-2); transition: color 0.16s;
  }
  .nav:hover { color: var(--text); }
  .nav.on { color: var(--text); }
  .tile { width: 40px; height: 40px; border-radius: 12px; display: grid; place-items: center; flex: none; position: relative; transition: background 0.16s; }
  .nav:hover .tile { background: var(--hover); }
  .nav.on .tile { background: var(--active); }
  .lbl { font-size: 11px; font-weight: 600; letter-spacing: 0; }
  .badge { position: absolute; top: 6px; right: 6px; width: 8px; height: 8px; border-radius: 50%; border: 2px solid var(--bg-elevated); box-sizing: content-box; }
  .lock { color: var(--text-3); padding: 8px; }
  main { flex: 1; position: relative; min-width: 0; background: var(--bg); display: flex; flex-direction: column; }
  main > :global(.page) { flex: 1; min-height: 0; height: auto; }
  .server-error { padding: 12px 20px; background: var(--danger-soft); color: var(--danger); font-size: 15px; text-align: center; }
  .center { text-align: center; }
  .center .title { margin: 18px 0 6px; }
  .perm-icons { display: flex; align-items: center; justify-content: center; gap: 12px; color: var(--text-3); }
  .app-tile { width: 58px; height: 58px; border-radius: 16px; display: grid; place-items: center; color: #fff; }
  .avatar {
    width: 58px; height: 58px; border-radius: 50%; display: grid; place-items: center; color: #fff;
    font: 600 24px/1 var(--font-display); background: linear-gradient(140deg, #30c48d, #2a9df4);
  }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 24px; }
  .toast {
    position: fixed; left: 50%; bottom: 28px; transform: translateX(-50%); z-index: 200;
    background: var(--bg-elevated); border: 1px solid var(--line-strong); box-shadow: var(--shadow);
    padding: 12px 20px; border-radius: 999px; font-size: 15px; font-weight: 500; animation: pop 0.3s var(--spring);
  }

  /* Phones: the rail becomes a bottom tab bar. */
  @media (max-width: 720px) {
    .shell { flex-direction: column-reverse; }
    .rail {
      width: 100%; flex-direction: row; justify-content: space-around; padding: 4px 4px calc(4px + env(safe-area-inset-bottom));
      border-right: 0; border-top: 1px solid var(--line);
    }
    .rail > .logo, .spacer, .lock { display: none; }
    .nav { width: auto; flex: 1; }
    .toast { bottom: 88px; }
  }
</style>
