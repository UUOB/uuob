<script>
  import { onMount } from 'svelte';
  import Icon from '../../../ui/Icon.svelte';
  import { call, mcpUrl, message } from './api.js';

  let { onToast } = $props();
  let clients = $state({});
  let containers = $state([]);
  let url = $state('');
  let sheet = $state(null); // 'new' | { created } | { remove }
  let name = $state('');

  const load = async () => {
    [clients, containers, url] = await Promise.all([call('clients'), call('containers'), mcpUrl()]);
  };

  // Access is given per app and covers all of that app's lists, calendars, …
  const apps = $derived([...new Map(containers.map((c) => [c.owner_app, c])).values()]);
  const appName = (k) => containers.find((c) => c.id === k.owner_app)?.name ?? k.name;
  const level = (cid, k) => k.access.find((a) => a.client === `ai:${cid}`)?.access ?? 'none';

  async function setLevel(cid, k, access) {
    await call('set_grant', { client: `ai:${cid}`, app: k.owner_app, access: access === 'none' ? null : access });
    load();
  }

  async function create() {
    try {
      const r = await call('create_client', { name });
      sheet = { created: { ...r, name } };
      name = '';
      load();
    } catch (e) {
      onToast(message(e));
    }
  }

  async function remove(cid) {
    await call('delete_client', { id: cid });
    sheet = null;
    load();
  }

  const claudeCmd = (t) => `claude mcp add --transport http uuob ${url} --header "Authorization: Bearer ${t}"`;
  const jsonCfg = (t) =>
    JSON.stringify({ mcpServers: { uuob: { type: 'http', url, headers: { Authorization: `Bearer ${t}` } } } }, null, 2);
  const copy = async (text, what) => {
    await navigator.clipboard.writeText(text);
    onToast(`${what} copied`);
  };

  onMount(load);
</script>

<div class="page scroll">
  <header>
    <h1 class="large-title">AI access</h1>
    <p class="secondary lead">Let an AI assistant (Claude, or any app that supports MCP) work with your data. It sees only what you allow, app by app. Nothing by default.</p>
  </header>

  {#each Object.entries(clients).filter(([, c]) => !c.app) as [cid, c] (cid)}
    <section class="card client">
      <div class="top">
        <span class="tile"><Icon name="ai" size={22} stroke={2} /></span>
        <div class="grow">
          <div class="title">{c.name}</div>
          <div class="caption">Connected {new Date(c.created_at).toLocaleDateString()}</div>
        </div>
        <button class="icon-btn" title="Disconnect" onclick={() => (sheet = { remove: { cid, ...c } })}><Icon name="trash" size={18} /></button>
      </div>
      <div class="grants">
        {#each apps as k (k.owner_app)}
          <div class="grant">
            <span class="headline">{appName(k)}</span>
            <div class="segmented">
              {#each [['none', 'No access'], ['read', 'Read'], ['write', 'Read & write']] as [v, label]}
                <button class:on={level(cid, k) === v} onclick={() => setLevel(cid, k, v)}>{label}</button>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    </section>
  {:else}
    <div class="card intro">
      <div class="glyph"><Icon name="ai" size={30} /></div>
      <div class="headline">No AI connected</div>
      <p class="caption">Connect one, then choose exactly which data it may read or change.</p>
    </div>
  {/each}

  {#if url}
    <div class="cta">
      <button class="btn primary lg" onclick={() => (sheet = 'new')}><Icon name="plus" size={20} stroke={2.2} /> Connect an AI</button>
    </div>
    <p class="caption server">Server: {url} · only reachable from this computer</p>
  {:else}
    <p class="caption server">Connect AIs from UUOB on a computer. You can change what they can access from here, and it applies on all your devices.</p>
  {/if}
</div>

{#if sheet === 'new'}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), create())}>
      <h2 class="title">Connect an AI</h2>
      <p class="secondary">Give it a name you’ll recognize. It starts with no access.</p>
      <input class="field" placeholder="e.g. Claude on my laptop" bind:value={name} autofocus />
      <div class="actions">
        <button type="button" class="btn lg" onclick={() => (sheet = null)}>Cancel</button>
        <button class="btn lg primary" disabled={!name.trim()}>Create</button>
      </div>
    </form>
  </div>
{:else if sheet?.created}
  {@const t = sheet.created.token}
  <div class="scrim">
    <div class="sheet wide-sheet">
      <h2 class="title">“{sheet.created.name}” is ready</h2>
      <p class="secondary">Add UUOB to your AI with one of these. The key is shown only once.</p>
      <div class="label-row"><span class="section-label">Claude Code</span><button class="btn ghost sm" onclick={() => copy(claudeCmd(t), 'Command')}><Icon name="copy" size={15} /> Copy</button></div>
      <pre>{claudeCmd(t)}</pre>
      <div class="label-row"><span class="section-label">Other MCP apps (JSON)</span><button class="btn ghost sm" onclick={() => copy(jsonCfg(t), 'Config')}><Icon name="copy" size={15} /> Copy</button></div>
      <pre>{jsonCfg(t)}</pre>
      <p class="caption">Next, choose what it can access on this screen.</p>
      <button class="btn primary lg full" onclick={() => (sheet = null)}>Done</button>
    </div>
  </div>
{:else if sheet?.remove}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (sheet = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Disconnect “{sheet.remove.name}”?</h2>
      <p class="secondary">Its key stops working immediately on all your devices.</p>
      <div class="actions">
        <button class="btn lg" onclick={() => (sheet = null)}>Cancel</button>
        <button class="btn lg danger" onclick={() => remove(sheet.remove.cid)}>Disconnect</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { height: 100%; padding: 56px 32px 48px; }
  .page > * { max-width: 680px; margin-left: auto; margin-right: auto; }
  header { margin-bottom: 28px; }
  .lead { margin: 10px 0 0; max-width: 580px; }
  .client { padding: 22px; margin-bottom: 16px; }
  .top { display: flex; align-items: center; gap: 14px; }
  .tile { width: 46px; height: 46px; border-radius: 13px; display: grid; place-items: center; color: #fff; flex: none;
    background: linear-gradient(140deg, #9b5cff, #ff5c9b); }
  .grow { flex: 1; }
  .grants { margin-top: 16px; display: grid; }
  .grant { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 0; border-top: 1px solid var(--line); flex-wrap: wrap; }
  .intro { padding: 36px 24px; text-align: center; }
  .intro .glyph { width: 60px; height: 60px; border-radius: 18px; margin: 0 auto 12px; display: grid; place-items: center; background: var(--surface-2); color: var(--c-purple); }
  .intro p { margin: 4px 0 0; }
  .cta { margin-top: 24px; }
  .server { margin-top: 14px; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }
  form .field { margin-top: 16px; }
  .wide-sheet { width: min(620px, 100%); }
  pre {
    margin: 0 0 16px; padding: 14px 16px; border-radius: var(--r); background: var(--surface-2); border: 1px solid var(--line);
    font: 13px/1.5 ui-monospace, "SF Mono", "JetBrains Mono", monospace; white-space: pre-wrap; word-break: break-all;
  }
  .label-row { display: flex; align-items: center; justify-content: space-between; margin-top: 18px; }
  .label-row .section-label { margin: 0; }
  .btn.sm { height: 30px; padding: 0 10px; font-size: 13px; }
  .full { width: 100%; margin-top: 14px; }
  @media (max-width: 720px) { .page { padding: 28px 16px 32px; } }
</style>
