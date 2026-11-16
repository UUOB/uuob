<script>
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { slide, fade } from 'svelte/transition';
  import Icon from '../../ui/Icon.svelte';
  import { uuob } from '../../ui/uuob.js';
  import { today, friendly, friendlyTime, parseQuick } from '../../ui/dates.js';

  // Every list is a container of its own, so it can be shared with other
  // people. Tasks that belong to no list live in the app's first container.
  const tasksIn = (container) => uuob.collection('tasks', container);
  const COLORS = ['blue', 'red', 'orange', 'yellow', 'green', 'teal', 'purple', 'pink'];

  let all = $state([]); // tasks from every container
  let containers = $state([]);
  let infos = $state([]); // each list's { color, created }, kept inside the list
  const tasks = $derived(all.map((x) => ({ ...x, list: x.container === uuob.self ? null : x.container })));
  const lists = $derived(
    containers
      .filter((c) => !c.main)
      .map((c) => ({ created: 0, color: 'blue', ...infos.find((i) => i.container === c.id), ...c }))
      .sort((x, y) => x.created - y.created),
  );
  const canEdit = (x) => containers.find((c) => c.id === x.container)?.canWrite !== false;
  const stored = ({ container, list, ...x }) => x; // what is saved: without the fields we add
  let view = $state('today'); // smart view id or list id
  let mobileList = $state(false); // phone: showing a list (vs. the overview)
  let draft = $state('');
  let openId = $state(null);
  let finishing = $state({}); // id -> true while the check animation plays
  let listSheet = $state(null); // { id?, name, color }

  const t = $derived(today());
  const smart = $derived([
    { id: 'today', name: 'Today', icon: 'sun', color: 'var(--c-blue)', filter: (x) => !x.done && x.due && x.due <= t },
    { id: 'upcoming', name: 'Upcoming', icon: 'calendar', color: 'var(--c-red)', filter: (x) => !x.done && x.due && x.due > t },
    { id: 'all', name: 'All', icon: 'inbox', color: 'var(--text-2)', filter: (x) => !x.done },
    { id: 'done', name: 'Completed', icon: 'check', color: 'var(--c-green)', filter: (x) => x.done },
  ]);
  const current = $derived(
    smart.find((s) => s.id === view) ??
      (() => {
        const l = lists.find((l) => l.id === view);
        return l ? { id: l.id, name: l.name, color: `var(--c-${l.color})`, list: l, filter: (x) => x.list === l.id && !x.done } : smart[0];
      })(),
  );
  const visible = $derived(
    tasks
      .filter((x) => current.filter(x) || finishing[x.id])
      .sort((a, b) =>
        view === 'done'
          ? (b.done_at ?? 0) - (a.done_at ?? 0)
          : (a.due ?? '9999').localeCompare(b.due ?? '9999') || (a.time ?? '99').localeCompare(b.time ?? '99') || a.created - b.created,
      ),
  );
  const count = (filter) => tasks.filter(filter).length;
  const listOf = (x) => lists.find((l) => l.id === x.list);
  const ringColor = (x) => (listOf(x) ? `var(--c-${listOf(x).color})` : 'var(--accent)');

  // Group upcoming by day, like Reminders.
  const groups = $derived.by(() => {
    if (view !== 'upcoming') return [{ key: '', items: visible }];
    const m = new Map();
    for (const x of visible) {
      if (!m.has(x.due)) m.set(x.due, []);
      m.get(x.due).push(x);
    }
    return [...m].map(([key, items]) => ({ key: friendly(key), items }));
  });

  async function add(e) {
    e.preventDefault();
    if (!draft.trim()) return;
    const q = parseQuick(draft);
    const due = q.due ?? (view === 'today' ? t : null);
    draft = '';
    await tasksIn(current.list?.id).put({ title: q.title || 'New task', notes: '', done: false, due, time: q.time, created: Date.now() });
  }

  function toggle(x) {
    if (!canEdit(x)) return;
    const done = !x.done;
    if (done) {
      finishing = { ...finishing, [x.id]: true };
      setTimeout(() => {
        const { [x.id]: _, ...rest } = finishing;
        finishing = rest;
      }, 700);
    }
    save({ ...x, done, done_at: done ? Date.now() : null });
  }

  function save(x) {
    all = all.map((y) => (y.id === x.id ? x : y)); // optimistic
    tasksIn(x.container).put(stored(x));
  }

  async function remove(x) {
    openId = null;
    all = all.filter((y) => y.id !== x.id);
    await tasksIn(x.container).delete(x.id);
  }

  // Moving a task to another list moves it to that list's container.
  async function move(x, list) {
    const to = list || uuob.self;
    if (to === x.container) return;
    all = all.map((y) => (y.id === x.id ? { ...x, container: to } : y));
    await tasksIn(to).put(stored(x));
    await tasksIn(x.container).delete(x.id);
  }

  async function saveList() {
    const { id, name, color, created } = listSheet;
    if (!name.trim()) return;
    listSheet = null;
    if (id) {
      if (name.trim() !== lists.find((l) => l.id === id)?.name) await uuob.containers.rename(id, name.trim());
      await uuob.collection('list', id).put({ color, created }, 'info');
    } else {
      const list = await uuob.containers.create(name.trim());
      await uuob.collection('list', list).put({ color, created: Date.now() }, 'info');
      view = list;
      mobileList = true;
    }
  }

  // Deletes the list everywhere; a list someone shared with you is only left.
  async function deleteList(l) {
    listSheet = null;
    view = 'today';
    await uuob.containers.delete(l.id);
  }

  function share(l) {
    listSheet = null;
    uuob.containers.share(l.id).catch(() => {});
  }

  const sharedNote = (l) =>
    l.from
      ? `From ${l.from.name}${l.canWrite ? '' : ' · view only'}`
      : l.sharedWith?.length
        ? `Shared with ${l.sharedWith.map((p) => p.name).join(', ')}`
        : '';

  // Lists used to be records in the app's first container. Move each one
  // into a container of its own (keyed by its old id, so every device ends
  // up with the same container).
  async function moveOldLists() {
    const oldLists = uuob.collection('lists');
    const old = await oldLists.list();
    if (!old.length) return;
    const loose = await tasksIn().list();
    for (const l of old) {
      const list = await uuob.containers.create(l.name || 'List', l.id).catch(() => uuob.containers.create(l.name || 'List'));
      await uuob.collection('list', list).put({ color: l.color ?? 'blue', created: l.created ?? 0 }, 'info');
      for (const { list: was, ...x } of loose.filter((x) => x.list === l.id)) {
        await tasksIn(list).put(x);
        await tasksIn().delete(x.id);
      }
      await oldLists.delete(l.id);
    }
  }

  function go(id) {
    view = id;
    openId = null;
    mobileList = true;
  }

  onMount(() => {
    const a = uuob.all('tasks').subscribe((r) => (all = r));
    const b = uuob.all('list').subscribe((r) => (infos = r));
    const c = uuob.containers.subscribe((r) => (containers = r));
    moveOldLists().catch(() => {});
    return () => (a(), b(), c());
  });
</script>

<div class="app" class:mobile-list={mobileList}>
  <aside class="sidebar scroll">
    <h1 class="large-title only-mobile">Tasks</h1>
    <div class="smart">
      {#each smart as s (s.id)}
        <button class="smart-tile" class:on={view === s.id} onclick={() => go(s.id)}>
          <span class="circle" style="background:{s.color}"><Icon name={s.icon} size={17} stroke={2.2} /></span>
          <span class="n">{count(s.filter)}</span>
          <span class="label">{s.name}</span>
        </button>
      {/each}
    </div>

    <div class="section-label lists-label">My lists</div>
    {#each lists as l (l.id)}
      <button class="list-row" class:on={view === l.id} onclick={() => go(l.id)}>
        <span class="circle sm" style="background:var(--c-{l.color})"><Icon name="list" size={14} stroke={2.4} /></span>
        <span class="name">{l.name}</span>
        {#if l.from || l.sharedWith?.length}<span class="shared-mark" title={sharedNote(l)}><Icon name="people" size={15} /></span>{/if}
        <span class="caption">{count((x) => x.list === l.id && !x.done)}</span>
      </button>
    {:else}
      <p class="caption hint">Group tasks into lists like Home, Work or Groceries.</p>
    {/each}
    <button class="btn ghost add-list" onclick={() => (listSheet = { name: '', color: COLORS[lists.length % COLORS.length], created: Date.now() })}>
      <Icon name="plus" size={18} stroke={2.2} /> Add list
    </button>
  </aside>

  <section class="main scroll">
    <header>
      <button class="icon-btn only-mobile back" onclick={() => (mobileList = false)}><Icon name="back" /></button>
      <h1 class="large-title" style="color:{current.color === 'var(--text-2)' ? 'var(--text)' : current.color}">{current.name}</h1>
      {#if current.list}
        <button class="icon-btn" title={current.list.from ? 'About this list' : 'Edit list'} onclick={() => (listSheet = { ...current.list })}><Icon name={current.list.from ? 'people' : 'edit'} size={18} /></button>
        {#if !current.list.from}
          <button class="icon-btn" title="Share list" onclick={() => share(current.list)}><Icon name="share" size={18} /></button>
        {/if}
      {/if}
      <span class="spacer"></span>
      <span class="count">{visible.length}</span>
    </header>
    {#if current.list && sharedNote(current.list)}
      <div class="shared-note caption"><Icon name="people" size={15} /> {sharedNote(current.list)}</div>
    {/if}

    {#if view !== 'done' && current.list?.canWrite !== false}
      <form class="quick" onsubmit={add}>
        <span class="plus"><Icon name="plus" size={20} stroke={2.2} /></span>
        <input bind:value={draft} placeholder={'New task — try “call mom fri 5pm”'} />
        {#if draft}
          {@const q = parseQuick(draft)}
          {#if q.due}<span class="preview" transition:fade={{ duration: 120 }}>{friendly(q.due)}{q.time ? `, ${friendlyTime(q.time)}` : ''}</span>{/if}
        {/if}
      </form>
    {/if}

    {#each groups as g (g.key)}
      {#if g.key}<div class="day">{g.key}</div>{/if}
      <ul>
        {#each g.items as x (x.id)}
          <li animate:flip={{ duration: 260 }} class:finishing={finishing[x.id]} class:open={openId === x.id}>
            <div class="task">
              <button class="check" class:on={x.done} style="--ring:{ringColor(x)}" onclick={() => toggle(x)} disabled={!canEdit(x)} aria-label="Complete"></button>
              <button class="body" onclick={() => canEdit(x) && (openId = openId === x.id ? null : x.id)}>
                <span class="t" class:done={x.done}>{x.title}</span>
                {#if x.notes && openId !== x.id}<span class="notes">{x.notes}</span>{/if}
                {#if x.due || (listOf(x) && current.list?.id !== x.list)}
                  <span class="meta">
                    {#if x.due}<span class:late={!x.done && x.due < t}>{friendly(x.due)}{x.time ? `, ${friendlyTime(x.time)}` : ''}</span>{/if}
                    {#if listOf(x) && current.list?.id !== x.list}<span class="lst" style="color:var(--c-{listOf(x).color})">{listOf(x).name}</span>{/if}
                  </span>
                {/if}
              </button>
            </div>
            {#if openId === x.id}
              <div class="editor" transition:slide={{ duration: 200 }}>
                <input class="field" value={x.title} onchange={(e) => save({ ...x, title: e.target.value })} />
                <textarea class="field" placeholder="Notes" value={x.notes ?? ''} onchange={(e) => save({ ...x, notes: e.target.value })}></textarea>
                <div class="grid">
                  <label class="stack">Date<input class="field" type="date" value={x.due ?? ''} onchange={(e) => save({ ...x, due: e.target.value || null })} /></label>
                  <label class="stack">Time<input class="field" type="time" value={x.time ?? ''} onchange={(e) => save({ ...x, time: e.target.value || null })} /></label>
                  <label class="stack">List
                    <select class="field" value={x.list ?? ''} onchange={(e) => move(x, e.target.value)}>
                      <option value="">None</option>
                      {#each lists.filter((l) => l.canWrite) as l}<option value={l.id}>{l.name}</option>{/each}
                    </select>
                  </label>
                </div>
                <div class="row-actions">
                  <button class="btn danger" onclick={() => remove(x)}><Icon name="trash" size={16} /> Delete</button>
                  <button class="btn" onclick={() => (openId = null)}>Done</button>
                </div>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/each}

    {#if visible.length === 0}
      <div class="empty" in:fade>
        <div class="glyph">{view === 'done' ? '🏁' : view === 'today' ? '☀️' : '✨'}</div>
        <div class="headline">{view === 'done' ? 'Nothing completed yet' : 'All clear'}</div>
        <div class="caption">{view === 'done' ? 'Finished tasks show up here.' : 'Add a task above. Dates like “tomorrow 9am” just work.'}</div>
      </div>
    {/if}
  </section>
</div>

{#if listSheet?.from}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (listSheet = null)} role="presentation">
    <div class="sheet">
      <div class="list-hero" style="background:var(--c-{listSheet.color})"><Icon name="list" size={30} stroke={2.4} /></div>
      <h2 class="title center">{listSheet.name}</h2>
      <p class="secondary center">
        {listSheet.from.name} shares this list with you. {listSheet.canWrite ? 'You can both change it.' : 'You can view it, but not change it.'}
      </p>
      <div class="actions">
        <button type="button" class="btn lg danger" onclick={() => deleteList(listSheet)}>Leave list</button>
        <button type="button" class="btn lg primary" onclick={() => (listSheet = null)}>Done</button>
      </div>
    </div>
  </div>
{:else if listSheet}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (listSheet = null)} role="presentation">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), saveList())}>
      <div class="list-hero" style="background:var(--c-{listSheet.color})"><Icon name="list" size={30} stroke={2.4} /></div>
      <input class="field name-field" placeholder="List name" bind:value={listSheet.name} autofocus />
      <div class="swatches">
        {#each COLORS as c}
          <button type="button" class="swatch" class:on={listSheet.color === c} style="background:var(--c-{c})" onclick={() => (listSheet.color = c)} aria-label={c}></button>
        {/each}
      </div>
      {#if listSheet.id}
        <button type="button" class="btn share-row" onclick={() => share(listSheet)}>
          <Icon name="share" size={18} />
          <span>{listSheet.sharedWith?.length ? sharedNote(listSheet) : 'Share list…'}</span>
        </button>
      {/if}
      <div class="actions">
        {#if listSheet.id}
          <button type="button" class="btn lg danger" onclick={() => deleteList(listSheet)}>Delete</button>
        {:else}
          <button type="button" class="btn lg" onclick={() => (listSheet = null)}>Cancel</button>
        {/if}
        <button class="btn lg primary" disabled={!listSheet.name.trim()}>{listSheet.id ? 'Save' : 'Create'}</button>
      </div>
    </form>
  </div>
{/if}

<style>
  .shared-mark { color: var(--text-3); display: grid; }
  .shared-note { display: flex; align-items: center; gap: 6px; margin: -6px 0 14px; }
  .share-row { width: 100%; height: 46px; justify-content: flex-start; gap: 10px; margin-top: 18px; color: var(--accent); }
  .share-row span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .center { text-align: center; }
  .check:disabled { opacity: 0.45; cursor: default; }
  .app { display: flex; height: 100vh; }
  .sidebar { width: 290px; flex: none; padding: 28px 16px; border-right: 1px solid var(--line); background: var(--bg-elevated); }
  .smart { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .smart-tile {
    display: grid; grid-template-columns: auto 1fr; grid-template-rows: auto auto; align-items: center; gap: 8px 6px;
    padding: 12px; border-radius: 14px; border: 1px solid var(--line); background: var(--surface); cursor: pointer; text-align: left;
    transition: background 0.16s var(--ease), transform 0.12s var(--ease), border-color 0.16s;
  }
  .smart-tile:hover { background: color-mix(in srgb, var(--surface), var(--text) 4%); }
  .smart-tile:active { transform: scale(0.97); }
  .smart-tile.on { border-color: color-mix(in srgb, var(--accent) 50%, transparent); background: var(--accent-soft); }
  .circle { width: 30px; height: 30px; border-radius: 50%; display: grid; place-items: center; color: #fff; }
  .circle.sm { width: 26px; height: 26px; }
  .n { justify-self: end; font: 700 24px/1 var(--font-display); }
  .label { grid-column: 1 / -1; font-size: 15px; font-weight: 600; color: var(--text-2); }
  .lists-label { margin: 26px 8px 8px; }
  .list-row {
    display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px; border: 0; background: transparent; border-radius: 10px;
    cursor: pointer; text-align: left; transition: background 0.16s;
  }
  .list-row:hover { background: var(--hover); }
  .list-row.on { background: var(--active); }
  .list-row .name { flex: 1; font-weight: 500; }
  .hint { padding: 0 8px; margin: 0 0 6px; }
  .add-list { margin-top: 8px; padding: 0 10px; }

  .main { flex: 1; padding: 40px 48px 80px; min-width: 0; }
  .main > * { max-width: 760px; margin-left: auto; margin-right: auto; }
  header { display: flex; align-items: center; gap: 8px; margin-bottom: 18px; }
  .count { font: 700 34px/1 var(--font-display); color: var(--text-3); }
  .quick {
    display: flex; align-items: center; gap: 12px; padding: 0 16px; height: 54px; border-radius: 14px; margin-bottom: 16px;
    background: var(--surface); border: 1px solid var(--line); transition: border-color 0.18s, box-shadow 0.18s;
  }
  .quick:focus-within { border-color: var(--accent); box-shadow: 0 0 0 4px var(--accent-soft); }
  .quick .plus { color: var(--accent); display: grid; }
  .quick input { flex: 1; border: 0; outline: 0; background: transparent; font-size: 17px; min-width: 0; }
  .quick input::placeholder { color: var(--text-3); }
  .preview { font-size: 13px; font-weight: 600; color: var(--accent); background: var(--accent-soft); padding: 4px 10px; border-radius: 999px; white-space: nowrap; }
  .day { font-weight: 700; font-size: 15px; margin: 22px 4px 6px; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { border-radius: 14px; transition: background 0.18s, opacity 0.5s 0.25s, transform 0.5s 0.25s; }
  li:hover { background: var(--hover); }
  li.open { background: var(--surface); box-shadow: var(--shadow); margin: 8px 0; }
  li.finishing { opacity: 0.35; }
  .task { display: flex; align-items: flex-start; gap: 14px; padding: 12px 12px; }
  .task .check { margin-top: 1px; }
  .body { flex: 1; min-width: 0; display: grid; gap: 2px; border: 0; background: transparent; padding: 0; text-align: left; cursor: pointer; }
  .t { font-size: 17px; overflow-wrap: anywhere; }
  .t.done { color: var(--text-3); text-decoration: line-through; text-decoration-color: var(--text-3); }
  .notes { font-size: 14px; color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta { display: flex; gap: 10px; font-size: 14px; color: var(--text-2); }
  .late { color: var(--danger); font-weight: 500; }
  .lst { font-weight: 500; }
  .editor { display: grid; gap: 10px; padding: 2px 14px 14px 48px; }
  .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
  .row-actions { display: flex; justify-content: space-between; margin-top: 4px; }

  .list-hero { width: 72px; height: 72px; border-radius: 50%; margin: 0 auto 18px; display: grid; place-items: center; color: #fff; box-shadow: var(--shadow); }
  .name-field { text-align: center; font-size: 20px; font-weight: 600; height: 54px; }
  .swatches { display: flex; justify-content: center; gap: 12px; margin: 18px 0 6px; flex-wrap: wrap; }
  .swatch { width: 32px; height: 32px; border-radius: 50%; border: 0; cursor: pointer; transition: transform 0.15s var(--spring); }
  .swatch:hover { transform: scale(1.1); }
  .swatch.on { box-shadow: 0 0 0 3px var(--bg-elevated), 0 0 0 5px currentColor; outline: 2px solid var(--text-2); outline-offset: 3px; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }

  .only-mobile { display: none; }
  @media (max-width: 720px) {
    .only-mobile { display: initial; }
    .sidebar { width: 100%; border: 0; padding: 24px 16px; background: var(--bg); }
    .sidebar .large-title { margin-bottom: 18px; display: block; }
    .main { display: none; padding: 16px 16px 60px; }
    .mobile-list .sidebar { display: none; }
    .mobile-list .main { display: block; }
    .back { margin-left: -8px; display: inline-grid; }
    .large-title { font-size: 30px; }
    .grid { grid-template-columns: 1fr 1fr; }
    .editor { padding-left: 14px; }
  }
</style>
