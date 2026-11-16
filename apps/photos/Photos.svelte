<script>
  import { onMount } from 'svelte';
  import Icon from '../../ui/Icon.svelte';
  import { uuob } from '../../ui/uuob.js';

  // Photos keeps one container per month ("2026-11"), so no single document
  // grows without end and a device can hold some months without the rest.
  // A photo is a record with a small preview inside it, which is why the
  // grid works offline; the original is a file. On phones every month is
  // "list only": originals come from the user's storage devices when opened.
  const TRASH_DAYS = 30;
  const DAY = 86400000;
  const isPhone = /Android|iPhone|iPad/.test(navigator.userAgent);
  const pad = (n) => String(n).padStart(2, '0');
  const monthKey = (t) => ((d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}`)(new Date(t));
  const monthName = (key) => new Date(+key.slice(0, 4), +key.slice(5) - 1).toLocaleDateString('en', { month: 'long', year: 'numeric' });
  const stored = ({ container, ...p }) => p; // what is saved: without the field the SDK adds

  let months = $state([]); // this app's containers
  let all = $state([]); // every photo record, each with its `container`
  let view = $state('photos'); // 'photos' | 'trash'
  let open = $state(null); // the photo shown full size
  let missing = $state(false); // its original could not be loaded
  let adding = $state(null); // { done, total } while photos are being added
  let error = $state('');
  let confirm = $state(null); // the photo about to be deleted for good
  let picker = $state();

  const photos = $derived(all.filter((p) => !p.deleted).sort((a, b) => b.taken - a.taken));
  const trash = $derived(all.filter((p) => p.deleted).sort((a, b) => b.deleted - a.deleted));
  const shown = $derived(view === 'trash' ? trash : photos);
  const groups = $derived.by(() => {
    const byMonth = new Map();
    for (const p of shown) {
      const key = monthKey(p.taken);
      if (!byMonth.has(key)) byMonth.set(key, []);
      byMonth.get(key).push(p);
    }
    return [...byMonth];
  });
  const isVideo = (p) => p.type?.startsWith('video/');
  const original = (p) => uuob.files(p.container).url(p.file);
  const daysLeft = (p) => Math.max(1, Math.ceil((p.deleted + TRASH_DAYS * DAY - Date.now()) / DAY));
  const length = (s) => `${Math.floor(s / 60)}:${pad(Math.round(s % 60))}`;
  const when = (p) => new Date(p.taken).toLocaleString('en', { dateStyle: 'medium', timeStyle: 'short' });

  // The container of a month, created when its first photo is added. Its id
  // is the same on every device, so two devices that start the same month
  // while apart end up in one container.
  const made = {};
  async function monthContainer(key) {
    const have = months.find((c) => c.name === key)?.id ?? made[key];
    return have ?? (made[key] = await uuob.containers.create(key, `month${key.replace('-', '')}00000`));
  }

  function scaled(source, w, h) {
    const scale = Math.min(1, 240 / Math.min(w, h));
    const canvas = document.createElement('canvas');
    canvas.width = Math.round(w * scale);
    canvas.height = Math.round(h * scale);
    canvas.getContext('2d').drawImage(source, 0, 0, canvas.width, canvas.height);
    return canvas.toDataURL('image/jpeg', 0.6);
  }

  async function imagePreview(file) {
    const image = await createImageBitmap(file);
    return { w: image.width, h: image.height, thumb: scaled(image, image.width, image.height) };
  }

  // The first frame of a video, and how long it is.
  function videoPreview(file) {
    return new Promise((resolve) => {
      const video = document.createElement('video');
      const url = URL.createObjectURL(file);
      const done = (meta) => (URL.revokeObjectURL(url), resolve(meta));
      video.muted = true;
      video.playsInline = true;
      video.preload = 'auto';
      video.onloadeddata = () => (video.currentTime = Math.min(0.1, video.duration || 0));
      video.onseeked = () => done({ w: video.videoWidth, h: video.videoHeight, duration: video.duration, thumb: scaled(video, video.videoWidth, video.videoHeight) });
      video.onerror = () => done({});
      setTimeout(() => done({}), 8000);
      video.src = url;
    });
  }

  async function add(files) {
    error = '';
    adding = { done: 0, total: files.length };
    for (const file of files) {
      try {
        const taken = file.lastModified || Date.now();
        const container = await monthContainer(monthKey(taken));
        // A format this device cannot draw still gets stored, without a preview.
        const preview = await (file.type.startsWith('video/') ? videoPreview(file) : imagePreview(file)).catch(() => ({}));
        const { id } = await uuob.files(container).put(file);
        await uuob.collection('photos', container).put({ taken, added: Date.now(), name: file.name, type: file.type, size: file.size, file: id, ...preview });
      } catch (e) {
        error = `${file.name} could not be added. ${e.message}`;
      }
      adding.done++;
    }
    adding = null;
  }

  const save = (p) => uuob.collection('photos', p.container).put(stored(p));

  // Deleting moves a photo to Recently Deleted; it is gone for good later.
  async function remove(p) {
    step(1) || (open = null);
    await save({ ...p, deleted: Date.now() });
  }

  async function restore(p) {
    open = null;
    const { deleted, ...rest } = p;
    await save(rest);
  }

  async function destroy(p) {
    confirm = open = null;
    await uuob.files(p.container).delete(p.file).catch(() => {});
    await uuob.collection('photos', p.container).delete(p.id);
  }

  function show(p) {
    missing = false;
    open = p;
  }

  // Move to the next (or previous) photo; false at the end.
  function step(by) {
    const next = shown[shown.findIndex((p) => p.id === open?.id) + by];
    if (next) show(next);
    return !!next;
  }

  // Phones: swipe sideways to move between photos.
  let touchX = 0;
  const swipe = (e) => {
    const dx = e.changedTouches[0].clientX - touchX;
    if (Math.abs(dx) > 60 && !isVideo(open)) step(dx < 0 ? 1 : -1);
  };

  function key(e) {
    if (!open || confirm) return;
    if (e.key === 'Escape') open = null;
    if (e.key === 'ArrowRight') step(1);
    if (e.key === 'ArrowLeft') step(-1);
  }

  onMount(() => {
    const asked = new Set();
    let purged = false;
    const a = uuob.containers.subscribe((cs) => {
      months = cs.filter((c) => !c.main);
      // Phones keep the lists; the originals stay on the storage devices.
      for (const c of months) {
        if (isPhone && c.keep === 'all' && !asked.has(c.id)) {
          asked.add(c.id);
          uuob.containers.keep(c.id, 'list').catch(() => {});
        }
      }
    });
    const b = uuob.all('photos').subscribe((records) => {
      all = records;
      if (!purged && records.length) {
        purged = true;
        records.filter((p) => p.deleted && Date.now() - p.deleted > TRASH_DAYS * DAY).forEach(destroy);
      }
    });
    return () => (a(), b());
  });
</script>

<svelte:window onkeydown={key} />

<div class="page scroll">
  <header>
    {#if view === 'trash'}
      <button class="icon-btn" aria-label="Back to photos" onclick={() => (view = 'photos')}><Icon name="back" /></button>
      <h1 class="large-title">Recently Deleted</h1>
    {:else}
      <h1 class="large-title">Photos</h1>
      <span class="count">{photos.length || ''}</span>
      <span class="spacer"></span>
      {#if trash.length}
        <button class="icon-btn" aria-label="Recently deleted" title="Recently deleted" onclick={() => (view = 'trash')}><Icon name="trash" size={20} /></button>
      {/if}
      <button class="btn primary" onclick={() => picker.click()} disabled={!!adding}><Icon name="plus" size={18} stroke={2.2} /> Add</button>
    {/if}
  </header>
  {#if view === 'trash'}
    <p class="secondary lead">Photos stay here for {TRASH_DAYS} days after you delete them, then they are removed from all your devices.</p>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#each groups as [month, items] (month)}
    <h2 class="section-label month">{monthName(month)}</h2>
    <div class="grid">
      {#each items as p (p.id)}
        <button class="cell" onclick={() => show(p)} aria-label="{isVideo(p) ? 'Video' : 'Photo'} from {when(p)}">
          {#if p.thumb}<img src={p.thumb} alt="" loading="lazy" />{:else}<span class="blank"><Icon name={isVideo(p) ? 'play' : 'photo'} size={28} /></span>{/if}
          {#if view === 'trash'}
            <span class="badge">{daysLeft(p)} {daysLeft(p) === 1 ? 'day' : 'days'}</span>
          {:else if isVideo(p)}
            <span class="badge"><Icon name="play" size={11} stroke={2.4} /> {p.duration ? length(p.duration) : ''}</span>
          {/if}
        </button>
      {/each}
    </div>
  {:else}
    <div class="card intro">
      <div class="glyph"><Icon name="photo" size={30} /></div>
      {#if view === 'trash'}
        <div class="headline">Nothing here</div>
      {:else}
        <div class="headline">No photos yet</div>
        <p class="caption">Add photos and videos from this device. They are kept in UUOB, on your own devices.</p>
        <button class="btn primary lg" onclick={() => picker.click()} disabled={!!adding}><Icon name="plus" size={20} stroke={2.2} /> Add photos</button>
      {/if}
    </div>
  {/each}
</div>

<input class="picker" type="file" accept="image/*,video/*" multiple bind:this={picker} onchange={(e) => (add([...e.target.files]), (e.target.value = ''))} />

{#if adding}
  <div class="progress" role="status">Adding {Math.min(adding.done + 1, adding.total)} of {adding.total}…</div>
{/if}

{#if open}
  <div class="viewer" role="dialog" aria-modal="true" aria-label="{isVideo(open) ? 'Video' : 'Photo'} from {when(open)}">
    <div class="bar">
      <button class="icon-btn" aria-label="Close" onclick={() => (open = null)}><Icon name="close" /></button>
      <span class="when">{when(open)}</span>
      <span class="spacer"></span>
      {#if open.deleted}
        <button class="btn" onclick={() => restore(open)}>Restore</button>
        <button class="btn danger" onclick={() => (confirm = open)}>Delete</button>
      {:else}
        <button class="icon-btn" aria-label="Delete" title="Delete" onclick={() => remove(open)}><Icon name="trash" size={20} /></button>
      {/if}
    </div>
    <!-- The preview shows at once; the original replaces it when it has loaded. -->
    <div class="stage" style={open.thumb ? `background-image:url(${open.thumb})` : ''} ontouchstart={(e) => (touchX = e.touches[0].clientX)} ontouchend={swipe} role="presentation">
      {#key open.id}
        {#if missing}
          <p class="notice" role="status">The original is on a storage device that can’t be reached right now.</p>
        {:else if isVideo(open)}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video src={original(open)} poster={open.thumb} controls autoplay playsinline onerror={() => (missing = true)}></video>
        {:else}
          <img src={original(open)} alt={open.name} onerror={() => (missing = true)} />
        {/if}
      {/key}
    </div>
    <button class="icon-btn nav prev" aria-label="Previous" onclick={() => step(-1)}><Icon name="left" /></button>
    <button class="icon-btn nav next" aria-label="Next" onclick={() => step(1)}><Icon name="right" /></button>
  </div>
{/if}

{#if confirm}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (confirm = null)} role="presentation">
    <div class="sheet">
      <h2 class="title">Delete this {isVideo(confirm) ? 'video' : 'photo'}?</h2>
      <p class="secondary">It is removed from all your devices. This can’t be undone.</p>
      <div class="actions">
        <button class="btn lg" onclick={() => (confirm = null)}>Cancel</button>
        <button class="btn lg danger" onclick={() => destroy(confirm)}>Delete</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { height: 100vh; padding: 48px 32px 48px; }
  .page > * { max-width: 1040px; margin-left: auto; margin-right: auto; }
  header { display: flex; align-items: center; gap: 10px; margin-bottom: 18px; }
  .count { font: 600 22px/1 var(--font-display); color: var(--text-3); margin-left: 4px; }
  .lead { margin: -6px auto 18px; }
  .error { color: var(--danger); margin: 0 auto 14px; }
  .month { margin: 26px auto 10px; }
  .month:first-of-type { margin-top: 6px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 4px; }
  .cell {
    position: relative; aspect-ratio: 1; padding: 0; border: 0; border-radius: 6px; overflow: hidden;
    background: var(--surface-2); color: var(--text-3); cursor: pointer;
  }
  .cell img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .cell:focus-visible { outline: 3px solid var(--accent); outline-offset: -3px; }
  .blank { display: grid; place-items: center; height: 100%; }
  .badge {
    position: absolute; right: 6px; bottom: 6px; display: inline-flex; align-items: center; gap: 4px;
    padding: 2px 7px; border-radius: 999px; font-size: 12px; font-weight: 600; color: #fff; background: rgba(0, 0, 0, 0.55);
  }
  .intro { padding: 44px 24px; text-align: center; margin-top: 24px; }
  .intro .glyph { width: 60px; height: 60px; border-radius: 18px; margin: 0 auto 12px; display: grid; place-items: center; background: var(--surface-2); color: var(--c-orange); }
  .intro p { margin: 4px auto 18px; max-width: 380px; }
  .picker { display: none; }
  .progress {
    position: fixed; left: 50%; bottom: 20px; transform: translateX(-50%); z-index: 300; white-space: nowrap;
    background: var(--bg-elevated); border: 1px solid var(--line-strong); box-shadow: var(--shadow);
    padding: 10px 18px; border-radius: 999px; font-size: 14px;
  }

  .viewer { position: fixed; inset: 0; z-index: 400; background: #000; display: flex; flex-direction: column; color: #fff; }
  .bar { display: flex; align-items: center; gap: 8px; padding: 10px 12px; padding-top: calc(10px + env(safe-area-inset-top)); }
  .bar .icon-btn, .nav { color: #fff; }
  .when { font-size: 15px; color: rgba(255, 255, 255, 0.8); }
  .stage { flex: 1; min-height: 0; display: grid; place-items: center; background: center / contain no-repeat; }
  .stage img, .stage video { width: 100%; height: 100%; object-fit: contain; }
  .notice {
    margin: 0 20px; padding: 12px 18px; border-radius: 14px; max-width: 420px; text-align: center;
    background: rgba(0, 0, 0, 0.7); font-size: 15px;
  }
  .nav { position: absolute; top: 50%; width: 44px; height: 44px; background: rgba(0, 0, 0, 0.4); border-radius: 50%; }
  .prev { left: 14px; }
  .next { right: 14px; }
  .actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 22px; }

  @media (max-width: 720px) {
    .page { padding: 24px 12px 32px; }
    .large-title { font-size: 30px; }
    .grid { grid-template-columns: repeat(3, 1fr); gap: 3px; }
    .cell { border-radius: 3px; }
    /* On a phone you swipe; the arrows would cover the photo. */
    .nav { display: none; }
  }
</style>
