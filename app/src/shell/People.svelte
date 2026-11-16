<script>
  import { onMount } from 'svelte';
  import Icon from '../../../ui/Icon.svelte';
  import Code from './Code.svelte';
  import { call, onEvent, message, scanCode, isPhone, initial } from './api.js';
  import { appIcon, appColor } from './apps.js';

  let { onToast } = $props();
  let people = $state([]);
  let me = $state(''); // the name other people see
  let sheet = $state(null); // 'name' | 'add' | 'meet' | { remove: person }
  let after = null; // what to open once the name is set
  let name = $state('');
  let invite = $state(null);
  let linked = $state(null);
  let code = $state('');
  let busy = $state(false);
  let error = $state('');

  async function load() {
    const [status, list] = await Promise.all([call('status'), call('people')]);
    me = status.network.name ?? '';
    // While a code is showing, a new person in the list is the one who used it.
    if (sheet === 'add' && invite && !linked) linked = list.find((p) => !people.some((q) => q.id === p.id))?.name ?? null;
    people = list;
  }

  // Linking needs a name for the other person to see.
  function start(what) {
    error = '';
    if (!me) {
      after = what;
      name = '';
      return void (sheet = 'name');
    }
    sheet = what;
    if (what === 'add') newInvite();
  }

  async function saveName() {
    try {
      await call('set_name', { name });
      me = name.trim();
      const next = after;
      after = null;
      next ? start(next) : (sheet = null);
    } catch (e) {
      error = message(e);
    }
  }

  async function newInvite() {
    invite = null;
    linked = null;
    try {
      invite = await call('create_person_invite');
    } catch (e) {
      error = message(e);
    }
  }

  function close() {
    if (invite && !linked) call('cancel_invite', { code: invite.code });
    sheet = null;
    invite = null;
    linked = null;
    code = '';
    error = '';
    busy = false;
  }

  async function meet() {
    busy = true;
    error = '';
    try {
      await call('meet', { code });
      close();
      await load();
      onToast(`Linked with ${people.at(-1)?.name ?? 'them'}`);
    } catch (e) {
      error = message(e);
      busy = false;
    }
  }

  async function scan() {
    error = '';
    try {
      code = await scanCode();
      await meet();
    } catch (e) {
      error = message(e);
    }
  }

  async function act(method, params, toast) {
    try {
      await call(method, params);
      if (toast) onToast(toast);
    } catch (e) {
      onToast(message(e));
    }
    load();
  }

  const can = (access) => (access === 'write' ? 'Can edit' : 'Can view');

  onMount(() => {
    load();
    const un = onEvent((e) => e.type !== 'container_changed' && load());
    return () => un.then((f) => f());
  });
</script>

<div class="page scroll">
  <header>
    <h1 class="large-title">People</h1>
    <p class="secondary lead">Share a list or a calendar with someone who has their own UUOB. They see only what you share with them. Everything else stays private.</p>
  </header>

  {#if me}
    <div class="group you">
      <div class="row">
        <span class="avatar">{initial(me)}</span>
        <div class="grow">
          <div class="headline">{me} <span class="tag">You</span></div>
          <div class="caption">The name other people see</div>
        </div>
        <button class="icon-btn" title="Change name" onclick={() => ((name = me), (after = null), (error = ''), (sheet = 'name'))}><Icon name="edit" size={18} /></button>
      </div>
    </div>
  {/if}

  {#each people as p (p.id)}
    <section class="card person">
      <div class="top">
        <span class="avatar other">{initial(p.name)}</span>
        <div class="grow">
          <div class="title">{p.name}</div>
          <div class="caption status"><span class="dot" style="background:{p.online ? 'var(--ok)' : 'var(--text-3)'}"></span>{p.online ? 'Online' : 'Offline'}</div>
        </div>
        <button class="icon-btn" title="Remove {p.name}" onclick={() => (sheet = { remove: p })}><Icon name="trash" size={18} /></button>
      </div>

      {#if p.receiving.length}
        <div class="section-label">{p.name} shares with you</div>
        {#each p.receiving as s (s.container)}
          <div class="item">
            <span class="tile" style="background:{appColor(s.app)}"><Icon name={appIcon(s.app)} size={17} stroke={2} /></span>
            <div class="grow">
              <div class="headline">{s.name}</div>
              <div class="caption">{s.app_name} · {can(s.access)}</div>
            </div>
            {#if s.offered}
              <button class="btn" onclick={() => act('delete_container', { id: s.container })}>Decline</button>
              <button class="btn primary" onclick={() => act('accept_share', { container: s.container }, `“${s.name}” was added to ${s.app_name}`)}>Accept</button>
            {:else}
              <button class="btn ghost danger-text" onclick={() => act('delete_container', { id: s.container }, `You left “${s.name}”`)}>Leave</button>
            {/if}
          </div>
        {/each}
      {/if}

      {#if p.sharing.length}
        <div class="section-label">You share with {p.name}</div>
        {#each p.sharing as s (s.container)}
          <div class="item">
            <span class="tile" style="background:{appColor(s.app)}"><Icon name={appIcon(s.app)} size={17} stroke={2} /></span>
            <div class="grow">
              <div class="headline">{s.name}</div>
              <div class="caption">{s.app_name}</div>
            </div>
            <div class="segmented">
              {#each [['read', 'Can view'], ['write', 'Can edit']] as [v, label]}
                <button class:on={s.access === v} onclick={() => act('share_with', { container: s.container, person: p.id, access: v })}>{label}</button>
              {/each}
            </div>
            <button class="icon-btn small" title="Stop sharing" onclick={() => act('share_with', { container: s.container, person: p.id, access: null }, `Stopped sharing “${s.name}” with ${p.name}`)}><Icon name="close" size={15} /></button>
          </div>
        {/each}
      {/if}

      {#if !p.receiving.length && !p.sharing.length}
        <p class="caption nothing">Nothing shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.</p>
      {/if}
    </section>
  {:else}
    <div class="card intro">
      <div class="glyph"><Icon name="people" size={30} /></div>
      <div class="headline">No people yet</div>
      <p class="caption">Link with a partner, a family member or a colleague. Then share a list or a calendar with them from Tasks or Calendar.</p>
    </div>
  {/each}

  <div class="cta">
    <button class="btn primary lg" onclick={() => start('add')}><Icon name="plus" size={20} stroke={2.2} /> Add a person</button>
    <button class="btn lg" onclick={() => start('meet')}><Icon name="link" size={20} /> Enter a code</button>
  </div>
</div>

{#if sheet === 'name'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && close()} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), saveName())}>
      <h2 class="title">Your name</h2>
      <p class="secondary">This is how you appear to people you share with.</p>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="field name" placeholder="e.g. Alex" bind:value={name} autofocus autocomplete="off" enterkeyhint="done" />
      {#if error}<p class="error">{error}</p>{/if}
      <div class="actions">
        <button type="button" class="btn lg" onclick={close}>Cancel</button>
        <button class="btn lg primary" disabled={!name.trim()}>{after ? 'Continue' : 'Save'}</button>
      </div>
    </form>
  </div>
{:else if sheet === 'add'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && close()} role="presentation">
    <div class="sheet center">
      {#if linked}
        <div class="success"><Icon name="check" size={40} stroke={2.5} /></div>
        <h2 class="title">Linked with {linked}</h2>
        <p class="secondary">Nothing is shared yet. Open a list in Tasks or a calendar in Calendar and choose Share.</p>
        <button class="btn primary lg wide" onclick={close}>Done</button>
      {:else if invite}
        <h2 class="title">Add a person</h2>
        <p class="secondary steps">They open UUOB on their device, go to <b>People → Enter a code</b> and type this.</p>
        <Code {invite} waiting="Waiting for them…" />
        <p class="caption safe">This links the two of you. It doesn’t share anything, and it doesn’t add them to your devices.</p>
        <button class="btn wide" onclick={close}>Cancel</button>
      {:else if error}
        <h2 class="title">Couldn’t create a code</h2>
        <p class="secondary">{error}</p>
        <button class="btn wide" onclick={close}>Close</button>
      {:else}
        <div class="caption creating"><span class="spinner"></span> Creating a code…</div>
      {/if}
    </div>
  </div>
{:else if sheet === 'meet'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && close()} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), meet())}>
      <h2 class="title">Enter a code</h2>
      <p class="secondary">Ask the other person to open <b>People → Add a person</b> in their UUOB (or to share something with someone new), and type the code they see.</p>
      {#if isPhone}
        <button type="button" class="btn primary lg scan" onclick={scan}><Icon name="search" size={20} /> Scan QR code</button>
        <div class="or caption">or type the code</div>
      {/if}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="field code-input" placeholder="e.g. 42 amber river falcon" bind:value={code} autofocus autocomplete="off" autocorrect="off" autocapitalize="none" spellcheck="false" enterkeyhint="go" />
      {#if error}<p class="error">{error}</p>{/if}
      <p class="caption note">Your devices and data stay your own. You see only what they share with you.</p>
      <div class="actions">
        <button type="button" class="btn lg" onclick={close}>Cancel</button>
        <button class="btn lg primary" disabled={busy || code.trim().split(/[\s-]+/).length < 4}>
          {#if busy}<span class="spinner light"></span> Linking…{:else}Link{/if}
        </button>
      </div>
    </form>
  </div>
{:else if sheet?.remove}
  {@const p = sheet.remove}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Remove {p.name}?</h2>
      <p class="secondary">Everything you share with each other stops. What {p.name} shared disappears from your devices, and what you shared disappears from theirs the next time they connect.</p>
      <div class="actions">
        <button class="btn lg" onclick={() => (sheet = null)}>Cancel</button>
        <button class="btn lg danger" onclick={() => ((sheet = null), act('remove_person', { id: p.id }, `${p.name} was removed`))}>Remove</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { height: 100%; padding: 56px 32px 48px; }
  .page > * { max-width: 680px; margin-left: auto; margin-right: auto; }
  header { margin-bottom: 28px; }
  .lead { margin: 10px 0 0; max-width: 580px; }
  .you { margin-bottom: 16px; }
  .grow { flex: 1; min-width: 0; }
  .avatar {
    width: 44px; height: 44px; border-radius: 50%; flex: none; display: grid; place-items: center; color: #fff;
    font: 600 18px/1 var(--font-display); background: linear-gradient(140deg, #5e8bff, #9b5cff);
  }
  .avatar.other { background: linear-gradient(140deg, #30c48d, #2a9df4); }
  .tag { white-space: nowrap; font-size: 12px; font-weight: 600; color: var(--accent); background: var(--accent-soft); padding: 2px 8px; border-radius: 999px; margin-left: 6px; vertical-align: 2px; }
  .person { padding: 22px; margin-bottom: 16px; }
  .top { display: flex; align-items: center; gap: 14px; }
  .status { display: flex; align-items: center; gap: 6px; margin-top: 2px; }
  .section-label { margin-top: 20px; }
  .item { display: flex; align-items: center; gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); flex-wrap: wrap; }
  .section-label + .item { border-top: 0; }
  .item .grow { min-width: 120px; }
  .tile { width: 34px; height: 34px; border-radius: 10px; display: grid; place-items: center; color: #fff; flex: none; }
  .icon-btn.small { width: 28px; height: 28px; }
  .danger-text { color: var(--danger); }
  .danger-text:hover { background: var(--danger-soft); }
  .nothing { margin: 16px 0 0; }
  .intro { padding: 36px 24px; text-align: center; }
  .intro .glyph { width: 60px; height: 60px; border-radius: 18px; margin: 0 auto 12px; display: grid; place-items: center; background: var(--surface-2); color: var(--accent); }
  .intro p { margin: 4px auto 0; max-width: 420px; }
  .cta { display: flex; gap: 12px; flex-wrap: wrap; margin-top: 24px; }
  .center { text-align: center; }
  .steps { margin: 6px auto 22px; max-width: 340px; }
  .safe { margin: 0 auto 18px; max-width: 340px; }
  .creating { display: flex; align-items: center; gap: 6px; justify-content: center; }
  .wide { width: 100%; }
  .success {
    width: 76px; height: 76px; border-radius: 50%; margin: 4px auto 16px; display: grid; place-items: center;
    background: var(--ok); color: #fff; animation: pop 0.45s var(--spring);
  }
  .success + .title { margin-bottom: 6px; }
  .success ~ .btn { margin-top: 22px; }
  .name { margin-top: 18px; }
  .scan { width: 100%; margin-top: 18px; }
  .or { text-align: center; margin: 14px 0 -6px; }
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
