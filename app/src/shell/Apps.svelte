<script>
  import { onMount } from 'svelte';
  import Icon from '../../../ui/Icon.svelte';
  import Code from './Code.svelte';
  import { call, message } from './api.js';
  import { appIcon, appColor } from './apps.js';

  let { onToast } = $props();
  let containers = $state([]);
  let apps = $state({});
  let clients = $state({});
  let sheet = $state(null); // 'code' | { uninstall: app } | { wipe: app }
  let code = $state(null);

  let keepers = $state(0); // storage devices in the network
  let needed = $state(1); // how many of them must have something
  const load = async () => {
    const [cs, as, devices, status, cl] = await Promise.all([call('containers'), call('apps'), call('devices'), call('status'), call('clients')]);
    [containers, apps, clients] = [cs, as, cl];
    keepers = devices.filter((d) => d.keeper).length;
    needed = status.network.copies ?? 1;
  };
  // One card per app: its first container plus one per list, calendar, …
  const groups = $derived(
    [...new Set(containers.map((c) => c.owner_app))].map((id) => [id, containers.filter((c) => c.owner_app === id)]).map(([id, cs]) => ({
      id,
      name: apps[id]?.name ?? id,
      installed: !!apps[id]?.installed,
      records: cs.reduce((n, c) => n + c.records, 0),
      size_bytes: cs.reduce((n, c) => n + c.size_bytes + c.file_bytes, 0),
      files: cs.reduce((n, c) => n + c.files, 0),
      files_here: cs.reduce((n, c) => n + c.files_here, 0),
      // Connected storage devices that have all of it (of what is on this device).
      safe: Math.min(...cs.filter((c) => c.here && c.records + c.files > 0).map((c) => c.copies)),
      access: cs[0].access,
      // Copies of the app on devices without UUOB, each linked with a code.
      copies: Object.entries(clients).filter(([, c]) => c.app === id && c.endpoint).sort(([, a], [, b]) => a.created_at - b.created_at),
      parts: cs.filter((c) => c.id !== id),
      cs,
    })),
  );
  const sharedNote = (c) =>
    c.from
      ? `From ${c.from.name}${c.can_write ? '' : ' · view only'}`
      : c.shared_with.length
        ? `Shared with ${c.shared_with.map((p) => p.name).join(', ')}`
        : '';
  // Where an app's data is: files on this device, and storage devices that have it.
  const storageNote = (g) =>
    [
      g.files && (g.files_here === g.files ? `${g.files} ${g.files === 1 ? 'file' : 'files'}` : `${g.files_here} of ${g.files} files on this device`),
      keepers && Number.isFinite(g.safe) && (g.safe >= needed ? `On ${g.safe} storage ${g.safe === 1 ? 'device' : 'devices'}` : 'Waiting for a storage device'),
    ]
      .filter(Boolean)
      .join(' · ');
  const size = (b) => (b < 1024 ? `${b} B` : b < 1048576 ? `${(b / 1024).toFixed(1)} KB` : b < 1073741824 ? `${(b / 1048576).toFixed(1)} MB` : `${(b / 1073741824).toFixed(1)} GB`);

  async function revoke(c, a) {
    await call('set_grant', { client: a.client, app: c.id, access: null });
    onToast(`${a.client_name} can no longer access ${c.name}`);
    load();
  }

  const day = (ms) => new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
  async function unlink(c, id, copy) {
    try {
      await call('delete_client', { id });
      onToast(`${c.name} on ${copy.device ?? 'that device'} was disconnected`);
    } catch (e) {
      onToast(message(e));
    }
    load();
  }

  async function exportJson(g) {
    const data = {};
    for (const c of g.cs) data[c.id === g.id ? g.name : c.name] = await call('export', { container: c.id });
    await navigator.clipboard.writeText(JSON.stringify(data, null, 2));
    onToast(`Copied all ${g.name} data as JSON`);
  }

  async function uninstall(c, deleteData) {
    try {
      await call('uninstall_app', { id: c.id, delete_data: deleteData });
      onToast(deleteData ? `${c.name} was disconnected and its data deleted` : `${c.name} was disconnected; its data is kept`);
    } catch (e) {
      onToast(message(e));
    }
    sheet = null;
    load();
  }

  async function wipe(c) {
    await call('uninstall_app', { id: c.id, delete_data: true });
    sheet = null;
    onToast(`${c.name} data deleted on all devices`);
    load();
  }

  async function newCode() {
    code = null;
    sheet = 'code';
    code = await call('create_app_code');
  }

  onMount(load);
</script>

<div class="page scroll">
  <header>
    <div class="title-row">
      <h1 class="large-title">Apps</h1>
      <button class="btn" onclick={newCode}><Icon name="plus" size={18} stroke={2.2} /> Connect with a code</button>
    </div>
    <p class="secondary lead">Apps like Tasks and Calendar keep their data here, each in its own place, synced to all your devices. An app sees only its own data unless you allow more.</p>
    <p class="caption lead">To share a list or a calendar with someone, open it in its app and choose Share.</p>
  </header>

  {#each groups as c (c.id)}
    <section class="card container">
      <div class="top">
        <span class="tile" style="background:{appColor(c.id)}"><Icon name={appIcon(c.id)} size={22} stroke={2} /></span>
        <div class="grow">
          <div class="title">{c.name}</div>
          <div class="caption">
            {c.records} {c.records === 1 ? 'item' : 'items'} · {size(c.size_bytes)}
            · {#if c.installed}<span class="ok">Connected</span>{:else}<span class="warn">Not connected, data kept</span>{/if}
          </div>
          {#if storageNote(c)}<div class="caption">{storageNote(c)}</div>{/if}
        </div>
        <button class="btn ghost" onclick={() => exportJson(c)}><Icon name="copy" size={17} /> Copy as JSON</button>
      </div>

      {#if c.parts.length}
        <div class="section-label">In {c.name}</div>
        <div class="parts">
          {#each c.parts as part (part.id)}
            <div class="part">
              <span class="headline">{part.name}</span>
              <span class="caption">{part.here ? `${part.records} ${part.records === 1 ? 'item' : 'items'}` : 'Not on this device'}</span>
              {#if sharedNote(part)}<span class="caption shared"><Icon name="people" size={14} /> {sharedNote(part)}</span>{/if}
            </div>
          {/each}
        </div>
      {/if}

      <div class="section-label">Who can access it</div>
      <div class="access">
        {#each c.access as a (a.client)}
          <div class="who">
            <span class="dot" style="background:{a.client.startsWith('ai:') ? 'var(--c-purple)' : 'var(--accent)'}"></span>
            <span class="headline">{a.client_name}</span>
            <span class="caption">{a.access === 'write' ? 'Read & write' : 'Read only'}{a.client === `app:${c.id}` ? ' · owner' : ''}</span>
            {#if a.client !== `app:${c.id}`}
              <button class="icon-btn small" title="Remove access" onclick={() => revoke(c, a)}><Icon name="close" size={15} /></button>
            {/if}
          </div>
        {:else}
          <div class="caption">Nobody — only you, from this screen.</div>
        {/each}
      </div>

      {#if c.copies.length}
        <div class="section-label">On devices without UUOB</div>
        <div class="access">
          {#each c.copies as [id, copy] (id)}
            <div class="who">
              <span class="dot" style="background:var(--accent)"></span>
              <span class="copy">
                <span class="headline">{copy.device ?? 'Another device'}</span>
                <span class="caption">Connected {day(copy.created_at)}</span>
              </span>
              <button class="icon-btn small" title="Disconnect {c.name} on {copy.device ?? 'that device'}" onclick={() => unlink(c, id, copy)}><Icon name="close" size={15} /></button>
            </div>
          {/each}
        </div>
      {/if}

      <div class="foot">
        {#if c.installed}
          <button class="btn ghost danger-text" onclick={() => (sheet = { uninstall: c })}>Disconnect {c.name}…</button>
        {:else}
          <button class="btn ghost danger-text" onclick={() => (sheet = { wipe: c })}>Delete data…</button>
        {/if}
      </div>
    </section>
  {:else}
    <div class="card intro">
      <div class="glyph"><Icon name="data" size={30} /></div>
      <div class="headline">No apps yet</div>
      <p class="caption">Install an app like Tasks or Calendar on this device and open it. It will ask to connect. On a device without UUOB, the app connects with a code.</p>
    </div>
  {/each}
</div>

{#if sheet === 'code'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet center">
      <h2 class="title">Connect with a code</h2>
      <p class="secondary steps">Type this code into the app. It also works in an app on a device that has no UUOB.</p>
      {#if code}
        <Code invite={code} qr={false} waiting="Works once." />
      {/if}
      <button class="btn lg wide" onclick={() => ((sheet = null), load())}>Done</button>
    </div>
  </div>
{:else if sheet?.uninstall}
  {@const c = sheet.uninstall}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Disconnect {c.name}?</h2>
      <p class="secondary">{c.name} stops working with your network on all your devices. Choose what happens to its {c.records} {c.records === 1 ? 'item' : 'items'}.</p>
      <div class="choices">
        <button class="choice" onclick={() => uninstall(c, false)}>
          <span class="headline">Keep the data</span>
          <span class="caption">Connect it again later and pick up where you left off.</span>
        </button>
        <button class="choice bad" onclick={() => uninstall(c, true)}>
          <span class="headline">Delete the data too</span>
          <span class="caption">Permanently erased from every device.</span>
        </button>
      </div>
      <button class="btn wide" onclick={() => (sheet = null)}>Cancel</button>
    </div>
  </div>
{:else if sheet?.wipe}
  {@const c = sheet.wipe}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Delete all {c.name} data?</h2>
      <p class="secondary">{c.records} {c.records === 1 ? 'item is' : 'items are'} permanently erased from every device. This can’t be undone.</p>
      <div class="actions">
        <button class="btn lg" onclick={() => (sheet = null)}>Cancel</button>
        <button class="btn lg danger" onclick={() => wipe(c)}>Delete</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { height: 100%; padding: 56px 32px 48px; }
  .page > * { max-width: 680px; margin-left: auto; margin-right: auto; }
  header { margin-bottom: 28px; }
  .lead { margin: 10px 0 0; max-width: 560px; }
  .container { padding: 22px; margin-bottom: 16px; }
  .top { display: flex; align-items: center; gap: 14px; }
  .tile { width: 46px; height: 46px; border-radius: 13px; display: grid; place-items: center; color: #fff; flex: none; }
  .grow { flex: 1; min-width: 0; }
  .warn { color: var(--warn); }
  .ok { color: var(--ok); }
  .intro { padding: 36px 24px; text-align: center; }
  .intro .glyph { width: 60px; height: 60px; border-radius: 18px; margin: 0 auto 12px; display: grid; place-items: center; background: var(--surface-2); color: var(--accent); }
  .intro p { margin: 4px 0 0; }
  .title-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; }
  .center { text-align: center; }
  .steps { margin: 6px auto 22px; max-width: 340px; }
  .parts { display: grid; }
  .part { display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; padding: 8px 0; border-top: 1px solid var(--line); }
  .part:first-child { border-top: 0; }
  .shared { display: inline-flex; align-items: center; gap: 5px; margin-left: auto; }
  .section-label { margin-top: 20px; }
  .access { display: grid; gap: 4px; }
  .who { display: flex; align-items: center; gap: 10px; min-height: 36px; }
  .who .caption { flex: 1; }
  /* A long device name on a narrow phone: the date goes under it, whole. */
  .copy { flex: 1; min-width: 0; display: flex; flex-wrap: wrap; align-items: baseline; column-gap: 10px; overflow-wrap: anywhere; }
  .who .copy .caption { flex: none; }
  .icon-btn.small { width: 28px; height: 28px; }
  .foot { display: flex; gap: 4px; margin: 14px -10px -8px; border-top: 1px solid var(--line); padding-top: 10px; }
  .danger-text { color: var(--danger); }
  .danger-text:hover { background: var(--danger-soft); }
  .choices { display: grid; gap: 10px; margin: 20px 0 14px; }
  .choice {
    display: grid; gap: 2px; text-align: left; padding: 16px 18px; border-radius: var(--r-lg); cursor: pointer;
    background: var(--surface-2); border: 1px solid var(--line); transition: background 0.16s, border-color 0.16s;
  }
  .choice:hover { border-color: var(--accent); background: var(--accent-soft); }
  .choice.bad .headline { color: var(--danger); }
  .choice.bad:hover { border-color: var(--danger); background: var(--danger-soft); }
  .wide { width: 100%; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }
  @media (max-width: 720px) {
    .page { padding: 28px 16px 32px; }
    .top { flex-wrap: wrap; }
  }
</style>
