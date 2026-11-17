// The app SDK (uuob.js) against a real UUOB: connecting, reading and writing,
// and changes made while UUOB is stopped. Inside the dev container:
//   docker/dev.sh sh -c 'cargo build -p uuob-cli && node ui/uuob.test.mjs'
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { rmSync } from 'node:fs';

const BIN = 'target/debug/uuob';
const A = { UUOB_DATA_DIR: '/tmp/sdk-test-a', UUOB_PORT: '17760' }; // the device the app runs on
const B = { UUOB_DATA_DIR: '/tmp/sdk-test-b', UUOB_PORT: '17761' }; // another device of the network
const cli = (dev, ...args) => execFileSync(BIN, args, { env: { ...process.env, ...dev }, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
const run = (dev, name) => spawn(BIN, ['run', '--name', name], { env: { ...process.env, ...dev }, stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, f, ms = 30000) {
  for (const end = Date.now() + ms; ; await sleep(200)) {
    try {
      if (await f()) return;
    } catch {}
    if (Date.now() > end) throw new Error(`timed out waiting for: ${what}`);
  }
}
const at = { container: 'tasks', collection: 'tasks' };
const list = (dev) => JSON.parse(cli(dev, 'call', 'list', JSON.stringify(at))).sort((x, y) => x.id.localeCompare(y.id));

// What a webview gives the SDK.
const stored = new Map();
globalThis.localStorage = {
  getItem: (k) => stored.get(k) ?? null,
  setItem: (k, v) => stored.set(k, String(v)),
  removeItem: (k) => stored.delete(k),
};
globalThis.document = { addEventListener() {}, visibilityState: 'visible' };
// The app shell says where UUOB is (and that this app is not linked to one on another device).
globalThis.window = { __TAURI_INTERNALS__: { invoke: async (cmd) => (cmd.endsWith('port') ? Number(A.UUOB_PORT) : false) } };

for (const dev of [A, B]) rmSync(dev.UUOB_DATA_DIR, { recursive: true, force: true });
let a = run(A, 'A');
const b = run(B, 'B');
process.on('exit', () => (a.kill(), b.kill()));
await until('both UUOBs are up', () => (cli(A, 'status'), cli(B, 'status')));
cli(B, 'join', cli(A, 'invite').split('\n')[0]);

const { uuob } = await import('./uuob.js');
uuob.init({ id: 'tasks', name: 'Tasks' });
await uuob.connect(cli(A, 'app-code').split('\n')[0]);
let state;
uuob.onState((s) => (state = s));
const tasks = uuob.collection('tasks');
let seen = [];
tasks.subscribe((records) => (seen = records));
const titles = () => seen.map((t) => t.title).sort();

// UUOB is running: changes go straight to it.
await tasks.put({ title: 'one', done: false }, 't1');
await tasks.put({ title: 'two', done: false }, 't2');
await tasks.put({ title: 'three', done: false }, 't3');
assert.equal(uuob.pending, 0);
await until('the app sees its tasks', () => seen.length === 3);
await until('they reach the other device', () => list(B).length === 3);

// UUOB stops. The app keeps working on what it last saw.
a.kill();
await new Promise((r) => a.on('exit', r));
await tasks.put({ ...seen.find((t) => t.id === 't1'), done: true }); // one field of an existing record
const made = await tasks.put({ title: 'made while away', done: false }); // a new record
assert.equal(await tasks.delete('t2'), true);
await tasks.put({ title: 'three, edited', done: false }, 't3');
assert.equal(state, 'unreachable');
assert.equal(uuob.pending, 4);
await until('the app shows the changes', () => JSON.stringify(titles()) === JSON.stringify(['made while away', 'one', 'three, edited']), 5000);
assert.equal(seen.find((t) => t.id === 't1').done, true);
assert.equal((await tasks.get(made.id)).title, 'made while away');
assert.equal(await tasks.get('t2'), null);
// Only what changed is waiting, not whole records.
assert.deepEqual(JSON.parse(stored.get('uuob.outbox.tasks'))[0], { ...at, id: 't1', fresh: false, set: { done: true }, unset: [] });

// Meanwhile on the other device: t1 is renamed and t3 is deleted.
cli(B, 'call', 'put', JSON.stringify({ ...at, id: 't1', value: { title: 'renamed elsewhere', done: false } }));
cli(B, 'call', 'delete', JSON.stringify({ ...at, id: 't3' }));

// UUOB is back: what waited is sent, and both changes to t1 survive.
a = run(A, 'A');
await until('nothing is waiting any more', () => uuob.pending === 0);
await until('both devices agree', () => list(A).length === 2 && JSON.stringify(list(A)) === JSON.stringify(list(B)));
const now = Object.fromEntries(list(A).map((t) => [t.id, t]));
assert.deepEqual(now.t1, { id: 't1', title: 'renamed elsewhere', done: true });
assert.equal(now[made.id].title, 'made while away');
assert.ok(!now.t2, 'the delete made while away was sent');
assert.ok(!now.t3, 'a change to a record deleted meanwhile is dropped');
await until('the app shows the same', () => JSON.stringify(titles()) === JSON.stringify(['made while away', 'renamed elsewhere']));
assert.equal(state, 'connected');

// Settings for all apps: 24-hour time until changed; a change reaches the other device.
let settings;
const offSettings = uuob.settings.subscribe((s) => (settings = s));
await until('settings arrive', () => settings && settings.hour12 === false && settings.sunday_first === false);
await uuob.settings.set({ hour12: true });
assert.equal(settings.hour12, true, 'shown at once');
await until('the other device has it', () => JSON.parse(cli(B, 'call', 'settings', '{}')).hour12 === true);
cli(B, 'call', 'set_settings', JSON.stringify({ sunday_first: true }));
await until('a change made elsewhere reaches the app', () => settings.sunday_first === true && settings.hour12 === true);
offSettings();

console.log('ok: the SDK reads, writes, and sends later what was changed while UUOB was away; settings reach every app');
process.exit(0);
