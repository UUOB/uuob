<script>
  // The share sheet: an app asked to share one of its lists or calendars,
  // and the user chooses here who gets it. Changes apply as they are made.
  import { onMount } from 'svelte';
  import Icon from '../../../ui/Icon.svelte';
  import Code from './Code.svelte';
  import { call, onEvent, message, initial } from './api.js';
  import { appIcon, appColor } from './apps.js';

  let { ask, onDone, onToast } = $props();
  let people = $state([]);
  let me = $state('');
  let step = $state('people'); // 'people' | 'name' | 'new'
  let name = $state('');
  let access = $state('write'); // for someone new
  let invite = $state(null);
  let error = $state('');

  const level = (p) => p.sharing.find((s) => s.container === ask.container)?.access ?? 'none';
  const LEVELS = [['none', 'No access'], ['read', 'Can view'], ['write', 'Can edit']];

  async function load() {
    const [status, list] = await Promise.all([call('status'), call('people')]);
    me = status.network.name ?? '';
    // While a code is showing, a new person in the list is the one who used it.
    const joined = step === 'new' && invite && list.find((p) => !people.some((q) => q.id === p.id));
    people = list;
    if (joined) {
      invite = null;
      step = 'people';
      onToast(`Linked with ${joined.name}`);
    }
  }

  async function set(p, v) {
    try {
      await call('share_with', { container: ask.container, person: p.id, access: v === 'none' ? null : v });
    } catch (e) {
      onToast(message(e));
    }
    load();
  }

  function someoneNew() {
    error = '';
    if (!me) return void (step = 'name');
    step = 'new';
    newInvite();
  }

  async function saveName() {
    try {
      await call('set_name', { name });
      me = name.trim();
      someoneNew();
    } catch (e) {
      error = message(e);
    }
  }

  // The code both links the person and shares this with them.
  async function newInvite() {
    cancelInvite();
    try {
      invite = await call('create_person_invite', { container: ask.container, access });
    } catch (e) {
      error = message(e);
    }
  }

  function cancelInvite() {
    if (invite) call('cancel_invite', { code: invite.code });
    invite = null;
  }

  function back() {
    cancelInvite();
    error = '';
    step = 'people';
  }

  function done() {
    cancelInvite();
    onDone();
  }

  onMount(() => {
    load();
    const un = onEvent((e) => e.type !== 'container_changed' && e.type !== 'asks_changed' && load());
    return () => un.then((f) => f());
  });
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (step === 'people' ? done() : back())} />

<div class="scrim" onclick={(e) => e.target === e.currentTarget && done()} role="presentation">
  <div class="sheet">
    {#if step === 'name'}
      <form onsubmit={(e) => (e.preventDefault(), saveName())}>
        <h2 class="title">Your name</h2>
        <p class="secondary">This is how you appear to people you share with.</p>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="field name" placeholder="e.g. Alex" bind:value={name} autofocus autocomplete="off" enterkeyhint="done" />
        {#if error}<p class="error">{error}</p>{/if}
        <div class="actions">
          <button type="button" class="btn lg" onclick={back}>Back</button>
          <button class="btn lg primary" disabled={!name.trim()}>Continue</button>
        </div>
      </form>
    {:else if step === 'new'}
      <div class="center">
        <h2 class="title">Share with someone new</h2>
        <p class="secondary steps">They open UUOB on their device, go to <b>People → Enter a code</b> and type this.</p>
        <div class="segmented choose">
          {#each LEVELS.slice(1) as [v, label]}
            <button class:on={access === v} onclick={() => ((access = v), newInvite())}>{label}</button>
          {/each}
        </div>
        {#if invite}
          <Code {invite} waiting="Waiting for them…" />
        {:else if error}
          <p class="error">{error}</p>
        {:else}
          <div class="caption creating"><span class="spinner"></span> Creating a code…</div>
        {/if}
        <button class="btn wide" onclick={back}>Back</button>
      </div>
    {:else}
      <div class="head">
        <span class="tile" style="background:{appColor(ask.app)}"><Icon name={appIcon(ask.app)} size={24} stroke={2} /></span>
        <div class="grow">
          <h2 class="title">Share “{ask.container_name}”</h2>
          <div class="caption">From {ask.app_name}. People you choose get this and nothing else.</div>
        </div>
      </div>

      <div class="people">
        {#each people as p (p.id)}
          <div class="person">
            <span class="avatar">{initial(p.name)}</span>
            <span class="headline grow">{p.name}</span>
            <div class="segmented">
              {#each LEVELS as [v, label]}
                <button class:on={level(p) === v} onclick={() => set(p, v)}>{label}</button>
              {/each}
            </div>
          </div>
        {/each}
        <button class="person add" onclick={someoneNew}>
          <span class="avatar plus"><Icon name="plus" size={20} stroke={2.2} /></span>
          <span class="headline grow">Someone new…</span>
          <Icon name="chevron" size={18} />
        </button>
      </div>
      <button class="btn primary lg wide" onclick={done}>Done</button>
    {/if}
  </div>
</div>

<style>
  .head { display: flex; align-items: center; gap: 14px; }
  .tile { width: 52px; height: 52px; border-radius: 15px; display: grid; place-items: center; color: #fff; flex: none; }
  .grow { flex: 1; min-width: 0; }
  .head .caption { margin-top: 2px; }
  .people { margin: 20px 0 22px; border-top: 1px solid var(--line); }
  .person {
    display: flex; align-items: center; gap: 12px; padding: 12px 0; min-height: 60px; flex-wrap: wrap;
    border: 0; border-bottom: 1px solid var(--line); background: none; width: 100%; text-align: left; color: var(--text); font: inherit;
  }
  .person .grow { min-width: 90px; }
  .add { cursor: pointer; color: var(--accent); border-radius: 0; transition: background 0.16s; }
  .add:hover { background: var(--hover); }
  .add :global(svg) { color: var(--text-3); }
  .avatar {
    width: 36px; height: 36px; border-radius: 50%; flex: none; display: grid; place-items: center; color: #fff;
    font: 600 15px/1 var(--font-display); background: linear-gradient(140deg, #30c48d, #2a9df4);
  }
  .avatar.plus { background: var(--accent-soft); }
  .avatar.plus :global(svg) { color: var(--accent); }
  .center { text-align: center; }
  .steps { margin: 6px auto 18px; max-width: 340px; }
  .choose { margin-bottom: 22px; }
  .creating { display: flex; align-items: center; gap: 6px; justify-content: center; margin: 10px 0 22px; }
  .wide { width: 100%; }
  .name { margin-top: 18px; }
  .error { color: var(--danger); font-size: 15px; margin: 10px 0 0; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }
  .spinner {
    width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--line-strong); border-top-color: var(--accent);
    animation: spin 0.8s linear infinite; display: inline-block;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
