// UUOB SDK for apps. Your app has no backend: it keeps its data in its own
// container in UUOB, which runs on the user's devices and syncs everywhere.
//
//   import { uuob } from 'ui/uuob.js';
//   uuob.init({ id: 'tasks', name: 'Tasks' });
//   await uuob.connect();                       // user taps Allow in UUOB
//   const tasks = uuob.collection('tasks');
//   await tasks.put({ title: 'Buy milk' });     // -> { id, title }
//   const off = tasks.subscribe(list => render(list)); // live, across devices
//
// Another app's data needs the user's permission:
//   await uuob.requestAccess('tasks', 'read');
//   uuob.collection('tasks', 'tasks').list();
//
// Things people share (a list, a calendar) each get a container of their own:
//   const id = await uuob.containers.create('Groceries');
//   uuob.collection('tasks', id).put({ title: 'Milk' });
//   uuob.containers.share(id);                  // the user picks people in UUOB
//   uuob.all('tasks').subscribe(render);        // across all your containers,
//                                               // including ones shared with you
//
// On a device without UUOB the app uses the user's UUOB on another device:
// the user types a code from UUOB → Apps (`uuob.connect(code)`). Nothing
// else changes for the app.
//
// While UUOB is not reachable the app keeps working on what it last saw:
// changes to records wait in the app's own storage and are sent when UUOB
// is back (creating, renaming, deleting and sharing containers, and files,
// need UUOB).
//
// Files of any size live next to the records:
//   const { id } = await uuob.files(container).put(blob);
//   img.src = uuob.files(container).url(id);

let BASE = 'http://127.0.0.1:7745/api';
// The app shell (its `uuob-sdk` plugin) tells us where UUOB is: its port on
// this device (Android profiles differ), or, on a device without UUOB, the
// port of the tunnel to the user's UUOB on another device (see `connect`).
const shell = (cmd, args) => import('@tauri-apps/api/core').then(({ invoke }) => invoke(`plugin:uuob-sdk|${cmd}`, args));
const at = (port) => (BASE = `http://127.0.0.1:${port}/api`);
let ready = shell('port').then(at).catch(() => {});
const listeners = new Set(); // (containerId | '*') => void
const stateListeners = new Set(); // ('connected' | 'disconnected' | 'unreachable') => void
const askListeners = new Set(); // (waiting: boolean) => void
const settingsListeners = new Set(); // (settings) => void, for a change made here
const SETTINGS = { hour12: false, sunday_first: false };
let settingsChanges = 0; // changes made here, so an older read doesn't undo one
// The list this copy of the app gave UUOB last (see uuob.upcoming), and its queue.
let upcomingGiven = null;
let upcomingDone = Promise.resolve();
let remote = false; // UUOB is on another device

// A question to the user is answered in UUOB. When UUOB is on another
// device the app cannot bring it to the front, so the user is told to go
// there (see `onAsk`).
function asking(answer) {
  if (!remote) return answer;
  askListeners.forEach((f) => f(true));
  return answer.finally(() => askListeners.forEach((f) => f(false)));
}

let app = null;
let token = null;
let stream = null;

const KEY = () => `uuob.token.${app.id}`;
const store = {
  get: () => { try { return localStorage.getItem(KEY()); } catch { return null; } },
  set: (t) => { try { t ? localStorage.setItem(KEY(), t) : localStorage.removeItem(KEY()); } catch {} },
};

// Last known records per collection, so the app shows them instantly on
// launch (and while UUOB is unreachable). It lives in the app's private
// storage, which the OS keeps encrypted at rest; it's cleared on disconnect.
const cache = {
  key: (c, coll) => `uuob.cache.${app.id}.${c}.${coll}`,
  get(c, coll) { try { return JSON.parse(localStorage.getItem(this.key(c, coll))); } catch { return null; } },
  set(c, coll, v) { try { localStorage.setItem(this.key(c, coll), JSON.stringify(v)); } catch {} },
  clear() {
    try {
      Object.keys(localStorage).filter((k) => k.startsWith(`uuob.cache.${app.id}.`)).forEach((k) => localStorage.removeItem(k));
    } catch {}
  },
};

// Changes made while UUOB was unreachable, oldest first. Each is about one
// record: the fields that were changed (`set`, `unset`) or its removal.
const outbox = {
  key: () => `uuob.outbox.${app.id}`,
  all() { try { return JSON.parse(localStorage.getItem(this.key())) ?? []; } catch { return []; } },
  save(list) { try { list.length ? localStorage.setItem(this.key(), JSON.stringify(list)) : localStorage.removeItem(this.key()); } catch {} },
};
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const newId = () => [...crypto.getRandomValues(new Uint8Array(16))].map((b) => 'abcdefghijklmnopqrstuvwxyz234567'[b % 32]).join('');

// Records as they look with the waiting changes applied.
function withPending(container, collection, records) {
  records = [...records];
  for (const e of outbox.all()) {
    if (e.container !== container || e.collection !== collection) continue;
    const i = records.findIndex((r) => r.id === e.id);
    if (e.remove) {
      if (i >= 0) records.splice(i, 1);
    } else if (i >= 0 || e.fresh) {
      const r = { ...records[i], ...e.set, id: e.id };
      for (const k of e.unset) delete r[k];
      i >= 0 ? (records[i] = r) : records.push(r);
    }
  }
  return records;
}

// UUOB is away: keep what the user did to a record (`value` null: removed
// it) and show it at once. Only the fields that differ from what the app
// last saw are kept, so a change made meanwhile on another device to
// another field of the same record survives.
function later(container, collection, id, value) {
  const records = cache.get(container, collection) ?? [];
  const before = records.find((r) => r.id === id);
  let entry = { container, collection, id, remove: true };
  if (value) {
    const { id: _, ...fields } = value;
    entry = {
      container, collection, id, fresh: !before,
      set: Object.fromEntries(Object.entries(fields).filter(([k, v]) => !before || !same(before[k], v))),
      unset: before ? Object.keys(before).filter((k) => k !== 'id' && !(k in fields)) : [],
    };
  }
  outbox.save([...outbox.all(), entry]);
  const now = withPending(container, collection, records);
  cache.set(container, collection, now);
  flush();
  return value ? now.find((r) => r.id === id) : !!before;
}

// Send what waited. A change to a record that was deleted meanwhile is
// dropped, as is one UUOB refuses; if UUOB goes away again the rest waits.
let flushing = false;
async function flush() {
  if (flushing || !outbox.all().length) return;
  flushing = true;
  try {
    for (let e; (e = outbox.all()[0]); outbox.save(outbox.all().slice(1))) {
      const at = { container: e.container, collection: e.collection, id: e.id };
      try {
        if (e.remove) await call('delete', at);
        else {
          const now = await call('get', at);
          if (now || e.fresh) {
            const { id, ...value } = { ...now, ...e.set };
            for (const k of e.unset) delete value[k];
            await call('put', { ...at, value });
          }
        }
      } catch (err) {
        // Only a refusal drops a change; anything else (UUOB away, a full
        // disk, …) keeps it waiting for the next try.
        if (!['permission_denied', 'not_found', 'invalid'].includes(err.code)) return;
      }
    }
    listeners.forEach((f) => f('*'));
  } finally {
    flushing = false;
  }
}

function setState(s) {
  stateListeners.forEach((f) => f(s));
}

class UuobError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

// On Android another app could take UUOB's port while UUOB is not running
// and collect what apps send it. So before anything goes to the port, it must
// answer a random nonce as UUOB on this phone answers it through Android
// (`prove`, which only the real UUOB receives; see uuob_api::proof). A tunnel
// to another device is the app's own, and needs no such check.
const android = /Android/.test(globalThis.navigator?.userAgent ?? '');
let trusted = !android;
async function verify() {
  const nonce = newId();
  let hello;
  try {
    hello = await (await fetch(`${BASE}?prove=${nonce}`)).json();
  } catch {
    throw new UuobError('unreachable', 'UUOB isn’t running on this device');
  }
  if (!hello.remote) {
    const mine = await shell('prove', { nonce }).catch(() => null);
    if (!mine || mine !== hello.proof) {
      console.warn('UUOB: the server on UUOB’s port is not UUOB; nothing was sent to it');
      throw new UuobError('unreachable', 'Another app is using UUOB’s port');
    }
  }
  trusted = true;
}

// `type`: the body is sent as it is (a file) instead of as JSON.
async function request(path, body, { signal, type } = {}) {
  await ready;
  if (!trusted) await verify();
  let res;
  try {
    res = await fetch(BASE + path, {
      method: body ? 'POST' : 'GET',
      headers: { 'Content-Type': type ?? 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) },
      body: body ? (type ? body : JSON.stringify(body)) : undefined,
      signal,
    });
  } catch (e) {
    if (e.name === 'AbortError') throw e;
    // Whatever answers on the port next must prove itself again.
    trusted = !android;
    throw new UuobError('unreachable', 'UUOB isn’t running on this device');
  }
  const data = await res.json().catch(() => ({}));
  if (res.status === 401) {
    forget();
    throw new UuobError('unauthorized', 'Not connected to UUOB');
  }
  if (data.error) throw new UuobError(data.error.code, data.error.message);
  return data;
}

async function call(method, params = {}) {
  try {
    return (await request('/call', { method, params })).result;
  } catch (e) {
    if (e.code === 'unreachable') setState('unreachable');
    throw e;
  }
}

// After changing our containers, refresh subscribers right away.
function changed(r) {
  listeners.forEach((f) => f('*'));
  return r;
}

function forget() {
  token = null;
  store.set(null);
  cache.clear();
  outbox.save([]);
  stream?.abort();
  stream = null;
  setState('disconnected');
}

// Server-sent events over fetch (EventSource can't send the token).
async function follow() {
  stream?.abort();
  const ctl = (stream = new AbortController());
  while (!ctl.signal.aborted) {
    try {
      await ready;
      if (!trusted) await verify();
      const res = await fetch(BASE + '/events', { headers: { Authorization: `Bearer ${token}` }, signal: ctl.signal });
      if (res.status === 401) return forget();
      setState('connected');
      flush();
      const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
      let buf = '';
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buf += value;
        let i;
        while ((i = buf.indexOf('\n\n')) >= 0) {
          const line = buf.slice(0, i).split('\n').find((l) => l.startsWith('data:'));
          buf = buf.slice(i + 2);
          if (line) listeners.forEach((f) => f(JSON.parse(line.slice(5)).container));
        }
      }
    } catch (e) {
      if (ctl.signal.aborted) return;
      trusted = !android;
    }
    // UUOB went away: retry every few seconds until it's back.
    if (!ctl.signal.aborted) {
      setState('unreachable');
      await new Promise((r) => setTimeout(r, 3000));
    }
  }
}

// A phone freezes apps in the background, and the stream can die without
// an error. Coming back to the front starts a fresh one, which also reloads
// everything that changed meanwhile.
document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && token && stream && follow());

// On Android, once per connection: tell UUOB which Android app this is, so
// UUOB on this phone hands it its own alerts and widget data (UUOB's ToApps.kt).
async function sayAndroidPackage() {
  if (remote || !token || !/Android/.test(navigator.userAgent)) return;
  const key = `uuob.android.${app.id}`;
  if (localStorage.getItem(key) === token.slice(-8)) return;
  const { getIdentifier } = await import('@tauri-apps/api/app');
  await request('/call', { method: 'android', params: { package: await getIdentifier() } });
  localStorage.setItem(key, token.slice(-8));
}

export const uuob = {
  /** Who this app is. Call once, first. */
  init(manifest) {
    app = manifest;
    token = store.get();
  },

  get self() {
    return app?.id;
  },

  /** Connected before (has a key)? Then the app can show cached data right away. */
  get hasToken() {
    return !!token;
  },

  /** 'connected' | 'disconnected' (UUOB is here, not connected) | 'unreachable' (no UUOB). */
  async check() {
    let hello;
    try {
      hello = await request('');
      uuob.device = hello.device;
      remote = !!hello.remote;
    } catch {
      // Linked to a UUOB on another device, where the user disconnected
      // this app? Then that link is gone, like a refused token.
      if (token && (await shell('removed').catch(() => false))) {
        forget();
        ready = shell('port').then(at).catch(() => {});
      }
      return 'unreachable';
    }
    // On another device and without a token: only a new code helps.
    if (!token) return hello.remote ? 'unreachable' : 'disconnected';
    try {
      await request('/call', { method: 'containers' });
    } catch (e) {
      return e.code === 'unreachable' ? 'unreachable' : 'disconnected';
    }
    if (!stream) follow();
    sayAndroidPackage().catch(() => {});
    return 'connected';
  },

  /** Connect to UUOB. Without a code the user is asked in UUOB (this waits
   *  for their answer). With a code from UUOB → Apps it connects right away;
   *  if this device has no UUOB, the code links the app to the UUOB on the
   *  device that showed it (and from then on to any of the user's devices). */
  async connect(code, { signal } = {}) {
    const here = code && (await request('').then((hello) => !hello.remote, () => false));
    let r;
    if (code && !here) {
      r = await shell('link', { code, id: app.id, name: app.name }).catch((e) => {
        throw new UuobError('pairing', String(e));
      });
      at(r.port);
      remote = true;
    } else r = await request('/connect', { app: app.id, name: app.name, ...(code ? { code } : {}) }, { signal });
    token = r.token;
    store.set(token);
    follow();
    setState('connected');
    sayAndroidPackage().catch(() => {});
  },

  /** How many changes are waiting for UUOB to come back. */
  get pending() {
    return outbox.all().length;
  },

  /** 'connected' | 'disconnected' | 'unreachable' changes. */
  onState(fn) {
    stateListeners.add(fn);
    return () => stateListeners.delete(fn);
  },

  /** A collection in your container, or in `container` if you were granted access. */
  collection(collection, container) {
    const base = { collection, ...(container ? { container } : {}) };
    const own = () => container ?? app.id;
    // After our own writes, refresh subscribers right away (no round trip).
    const changed = (r) => {
      listeners.forEach((f) => f(own()));
      return r;
    };
    // Unreachable (or earlier changes still waiting, which go first): the
    // change waits too.
    const away = (e) => {
      if (e.code !== 'unreachable') throw e;
    };
    const send = (method, params) => (outbox.all().length ? Promise.reject(new UuobError('unreachable', '')) : call(method, params));
    const known = () => cache.get(own(), collection) ?? [];
    return {
      put: (value, id = value.id) =>
        send('put', { ...base, id, value })
          .catch((e) => (away(e), later(own(), collection, id ?? newId(), value)))
          .then(changed),
      get: (id) => call('get', { ...base, id }).catch((e) => (away(e), known().find((r) => r.id === id) ?? null)),
      delete: (id) =>
        send('delete', { ...base, id })
          .catch((e) => (away(e), later(own(), collection, id, null)))
          .then(changed),
      list: () => call('list', base).then((r) => ((r = withPending(own(), collection, r)), cache.set(own(), collection, r), r)),
      /** Calls `fn(records)` now (from cache, instantly) and whenever the data
       *  changes, on any device. */
      subscribe(fn) {
        const cached = cache.get(own(), collection);
        if (cached) fn(cached);
        // While UUOB is away: what is known, with the waiting changes.
        const load = () => this.list().then(fn, (e) => e.code === 'unreachable' && cache.get(own(), collection) && fn(known()));
        load();
        return uuob.onChange((c) => (c === '*' || c === own()) && load());
      },
    };
  },

  /** Choices for all of the user's apps, on every device:
   *  { hour12: false, sunday_first: false } until changed. Each app shows them
   *  in its Settings (ui/Settings.svelte); a change in one reaches all. */
  settings: {
    get: () =>
      call('settings').then(
        (s) => (cache.set('', 'settings', s), s),
        () => ({ ...SETTINGS, ...cache.get('', 'settings') }), // UUOB away, or too old to know them
      ),
    /** Changes some of them, e.g. `{ hour12: true }`; shown at once. */
    set(change) {
      const next = { ...SETTINGS, ...cache.get('', 'settings'), ...change };
      settingsChanges++;
      cache.set('', 'settings', next);
      settingsListeners.forEach((f) => f(next));
      return call('set_settings', change).catch(() => next);
    },
    /** Calls `fn(settings)` now and whenever they change, in any app. */
    subscribe(fn) {
      fn({ ...SETTINGS, ...cache.get('', 'settings') });
      settingsListeners.add(fn);
      // A read that set off before a change made here would undo it on screen;
      // UUOB announces the change, and the read after that has it.
      const load = () => {
        const at = settingsChanges;
        return this.get().then((s) => at === settingsChanges && fn(s));
      };
      load();
      const off = uuob.onChange((c) => c === '*' && load());
      return () => (off(), settingsListeners.delete(fn));
    },
  },

  /** What is coming up in this app, for the user's devices to show (the
   *  app's own widget) and ring (alerts), also while the app is closed: hands
   *  UUOB the whole list, `[{ id, date: 'YYYY-MM-DD', time?: 'HH:MM', end_date?,
   *  end?, tz?, title, body?, alert?, color? }]`. An item starts at `date` `time` (all
   *  day without a time) on the wall clock of `tz`, or of the device without
   *  one, and is over after `end_date` `end` (never without them: a task that
   *  is due stays until the app takes it off the list). `body` is a short
   *  second line (a place, a list's name); `alert` is how many minutes before
   *  the start to ring (none without it); `color` one of the calendar colours
   *  of tokens.css ('blue', 'red', 'orange', 'yellow', 'green', 'teal',
   *  'purple', 'pink').
   *
   *  Give ids that stay the same for the same item: every copy of the app, on
   *  any device, writes the same list, and only what differs is written.
   *  Items that left the list are removed. While UUOB is unreachable, nothing
   *  happens; call it again whenever what the list comes from changes. A list
   *  is written only when it differs from the one this copy of the app gave
   *  last time: two copies that see things a little differently (another time
   *  zone, an older version) then take turns instead of rewriting each other
   *  for good. */
  upcoming(list) {
    const key = JSON.stringify(list);
    return (upcomingDone = upcomingDone.then(async () => {
      if (key === upcomingGiven) return;
      const c = uuob.collection('upcoming');
      const now = await call('list', { collection: 'upcoming' }).catch(() => null);
      if (!now) return;
      const same = (a, b) => JSON.stringify(a, Object.keys(a).sort()) === JSON.stringify(b, Object.keys(b).sort());
      const want = new Map(list.map(({ id, ...a }) => [String(id), Object.fromEntries(Object.entries(a).filter(([, v]) => v != null))]));
      for (const { id } of now) if (!want.has(id)) await c.delete(id);
      for (const [id, a] of want) {
        const { id: _, ...had } = now.find((r) => r.id === id) ?? {};
        if (!same(had, a)) await c.put(a, id);
      }
      upcomingGiven = key;
    }).catch(() => {}));
  },

  /** This app's containers: its first, private one (`main`) plus one per list,
   *  calendar, … including those other people shared with the user.
   *  Each is `{ id, name, main, canWrite, from: {id, name} | null, sharedWith: [{id, name, access}],
   *  keep: 'all' | 'list' | 'none', here }` (`here`: its records are on this device). */
  containers: {
    list: () =>
      call('containers').then((all) => {
        const mine = all
          .filter((c) => c.owner_app === app.id)
          .map((c) => ({ id: c.id, name: c.name, main: c.id === app.id, canWrite: c.can_write, from: c.from, sharedWith: c.shared_with, keep: c.keep, here: c.here }));
        cache.set('', 'containers', mine);
        return mine;
      }),
    /** Calls `fn(containers)` now (from cache) and whenever they change. */
    subscribe(fn) {
      const cached = cache.get('', 'containers');
      if (cached) fn(cached);
      const load = () => this.list().then(fn, () => {});
      load();
      return uuob.onChange((c) => c === '*' && load());
    },
    /** A new container; returns its id. (`key`: reuse an existing random id, so
     *  that moving old data over gives the same container on every device.) */
    create: (name, key) => call('create_container', { name, ...(key ? { key } : {}) }).then(changed),
    rename: (id, name) => call('rename_container', { id, name }).then(changed),
    /** Deletes it everywhere; for one shared with the user, leaves it. */
    delete: (id) => call('delete_container', { id }).then(changed),
    /** How much of it this device keeps: 'all' (the default), 'list' (its
     *  records; files come when opened and are cached) or 'none'. The user's
     *  storage devices keep everything anyway, and nothing leaves this
     *  device before they have it. Asking for 'list' or 'all' again brings
     *  it back; until then reading it fails with code 'unavailable'. */
    keep: (id, keep) => call('keep', { container: id, keep }).then(changed),
    /** Opens UUOB, where the user chooses who to share it with. */
    async share(id) {
      if (!remote && /Android/.test(navigator.userAgent)) {
        import('@tauri-apps/plugin-opener').then(({ openUrl }) => openUrl('uuob://share')).catch(() => {});
      }
      return asking(call('share', { container: id }));
    },
  },

  /** Files (photos, video, anything) in one of your containers. A file is
   *  stored once and never changes; whoever can see the container can see
   *  its files, and deleting it removes it on every device. */
  files(container) {
    const c = container ?? app.id;
    const q = `container=${encodeURIComponent(c)}`;
    const changed = (r) => (listeners.forEach((f) => f(c)), r);
    return {
      /** Store a Blob or File → `{ id, size, type }`. Keep the id in a record. */
      put: (blob) =>
        request(`/files?${q}`, blob, { type: blob.type || 'application/octet-stream' })
          .catch((e) => {
            if (e.code === 'unreachable') setState('unreachable');
            throw e;
          })
          .then(changed),
      /** An address for `<img>`, `<video>` (seeking works) or `fetch`. Empty
       *  while the server on the port has not proven it is UUOB (Android):
       *  the address carries the app's key. */
      url: (id) => (trusted ? `${BASE}/files/${id}?${q}&key=${token}` : ''),
      /** `[{ id, size, type, added, here }]`; `here`: the bytes are on this device. */
      list: () => call('files', { container: c }),
      delete: (id) => call('delete_file', { container: c, id }).then(changed),
    };
  },

  /** Every record of `collection` across all of this app's containers, live:
   *  `fn(records, complete)`, each record with the id of its `container`;
   *  `complete` once every container has delivered its records. */
  all(collection) {
    return {
      subscribe(fn) {
        const subs = new Map();
        const data = new Map();
        const emit = () =>
          fn(
            [...data].flatMap(([container, rs]) => rs.map((r) => ({ ...r, container }))),
            [...subs.keys()].every((id) => data.has(id)),
          );
        const off = uuob.containers.subscribe((cs) => {
          const ids = new Set(cs.map((c) => c.id));
          for (const [id, stop] of subs) {
            if (!ids.has(id)) (stop(), subs.delete(id), data.delete(id));
          }
          for (const id of ids) {
            if (!subs.has(id)) subs.set(id, uuob.collection(collection, id).subscribe((rs) => (data.set(id, rs), emit())));
          }
          emit();
        });
        return () => (off(), subs.forEach((stop) => stop()));
      },
    };
  },

  /** Ask the user for access to another app's data ('read' | 'write'). */
  requestAccess: (container, access = 'read') => asking(call('request_access', { container, access })),

  /** `fn(true)` while a question waits for the user in UUOB on another
   *  device (where the app cannot open it for them), then `fn(false)`. */
  onAsk(fn) {
    askListeners.add(fn);
    return () => askListeners.delete(fn);
  },

  /** `fn(containerId | '*')` whenever data you can see changes. */
  onChange(fn) {
    listeners.add(fn);
    return () => listeners.delete(fn);
  },
};
