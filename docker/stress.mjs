// A stress test for files and storage devices: three windowless UUOBs (a
// "phone" that keeps only lists, a "laptop", a storage "server"), a few
// thousand pictures and one big video added on the phone through the same
// local API an app uses. Reports times, memory and disk use.
//   docker/dev.sh sh -c 'cargo build -p uuob-cli && node docker/stress.mjs [pictures] [videoMiB]'
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomBytes, randomFillSync } from 'node:crypto';
import { readFileSync, rmSync } from 'node:fs';

const PICTURES = Number(process.argv[2] ?? 2000);
const VIDEO_MIB = Number(process.argv[3] ?? 1024);
const MONTHS = 24;
const BIN = process.env.UUOB_BIN ?? 'target/debug/uuob';
const ROOT = 'target/stress';
const devices = Object.fromEntries(
  ['phone', 'laptop', 'server'].map((name, i) => [name, { name, env: { UUOB_DATA_DIR: `${ROOT}/${name}`, UUOB_PORT: String(17770 + i) }, peak: 0 }]),
);
const { phone, laptop, server } = devices;
const cli = (d, ...args) => execFileSync(BIN, args, { env: { ...process.env, ...d.env }, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
const call = (d, method, params = {}) => JSON.parse(cli(d, 'call', method, JSON.stringify(params)));
const start = (d) => (d.process = spawn(BIN, ['run', '--name', d.name], { env: { ...process.env, ...d.env }, stdio: 'ignore' }));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, f, ms = 3600_000) {
  for (const end = Date.now() + ms; ; await sleep(500)) {
    try {
      if (await f()) return;
    } catch {}
    if (Date.now() > end) throw new Error(`timed out waiting for: ${what}`);
  }
}
const mb = (bytes) => `${(bytes / 1048576).toFixed(0)} MB`;
const rss = (d) => Number(/VmRSS:\s+(\d+)/.exec(readFileSync(`/proc/${d.process.pid}/status`, 'utf8'))?.[1] ?? 0) * 1024;
// (du complains when a file disappears while it counts; its total is still good enough.)
const disk = (d, sub = '') => {
  let out;
  try {
    out = execFileSync('du', ['-sb', `${d.env.UUOB_DATA_DIR}/${sub}`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
  } catch (e) {
    out = e.stdout;
  }
  return Number(out.split('\t')[0]);
};
const sampler = setInterval(() => Object.values(devices).forEach((d) => { try { d.peak = Math.max(d.peak, rss(d)); } catch {} }), 1000);
const report = {};
const timed = async (what, f) => {
  const t = Date.now();
  const result = await f();
  report[what] = `${((Date.now() - t) / 1000).toFixed(1)} s`;
  console.log(`${what}: ${report[what]}`);
  return result;
};
process.on('exit', () => Object.values(devices).forEach((d) => d.process?.kill()));

rmSync(ROOT, { recursive: true, force: true });
Object.values(devices).forEach(start);
await until('the three UUOBs are up', () => Object.values(devices).every((d) => cli(d, 'status')));
for (const d of [laptop, server]) cli(d, 'join', cli(phone, 'invite').split('\n')[0]);
cli(server, 'keep', 'on');

// The "app" on the phone: connected like Photos, over the local API.
const api = `http://127.0.0.1:${phone.env.UUOB_PORT}/api`;
const json = (r) => r.json().then((j) => (j.error ? Promise.reject(new Error(j.error.message)) : (j.result ?? j)));
const post = (path, body, headers = {}) => fetch(api + path, { method: 'POST', headers: { 'Content-Type': 'application/json', ...headers }, body, duplex: 'half' }).then(json);
const { token } = await post('/connect', JSON.stringify({ app: 'photos', name: 'Photos', code: cli(phone, 'app-code').split('\n')[0] }));
const auth = { Authorization: `Bearer ${token}` };
const app = (method, params) => post('/call', JSON.stringify({ method, params }), auth);
const upload = (container, body, type) => post(`/files?container=${container}`, body, { ...auth, 'Content-Type': type });

const months = [];
for (let m = 0; m < MONTHS; m++) {
  const name = `${2025 + Math.floor(m / 12)}-${String((m % 12) + 1).padStart(2, '0')}`;
  const id = await app('create_container', { name, key: `month${name.replace('-', '')}00000` });
  await app('keep', { container: id, keep: 'list' }); // the phone keeps the lists, not the originals
  months.push(id);
}

const sha = (b) => createHash('sha256').update(b).digest('hex');
const thumb = () => `data:image/jpeg;base64,${randomBytes(6000).toString('base64')}`;
const sample = []; // some pictures to read back later: { container, id, hash }
let bytes = 0;
await timed(`adding ${PICTURES} pictures on the phone`, async () => {
  const picture = Buffer.alloc(600_000);
  for (let i = 0; i < PICTURES; i++) {
    const size = 200_000 + ((i * 7919) % 400_000);
    const data = randomFillSync(picture).subarray(0, size);
    const container = months[i % MONTHS];
    const file = await upload(container, data, 'image/jpeg');
    await app('put', { container, collection: 'photos', value: { taken: Date.now() - i * 60_000, name: `IMG_${i}.jpg`, type: 'image/jpeg', size, file: file.id, thumb: thumb() } });
    if (i % 100 === 0) sample.push({ container, id: file.id, hash: sha(data) });
    bytes += size;
  }
});

// One big video: bytes that can be recomputed anywhere, so a part can be checked.
const block = randomBytes(1 << 20);
const videoByte = (offset) => block[offset % block.length] ^ (Math.floor(offset / block.length) & 0xff);
const video = await timed(`adding a ${VIDEO_MIB} MiB video on the phone`, () => {
  let i = 0;
  const body = new ReadableStream({
    pull(controller) {
      if (i === VIDEO_MIB) return controller.close();
      const chunk = Buffer.from(block);
      for (let k = 0; k < chunk.length; k++) chunk[k] ^= i & 0xff;
      controller.enqueue(chunk);
      i++;
    },
  });
  return upload(months[0], body, 'video/mp4');
});
await app('put', { container: months[0], collection: 'photos', value: { taken: Date.now(), name: 'VID_0.mp4', type: 'video/mp4', size: video.size, file: video.id } });
bytes += video.size;
const total = PICTURES + 1;
report.data = `${total} files, ${mb(bytes)}`;

const filesHere = (d) => call(d, 'containers').filter((c) => c.owner_app === 'photos').reduce((n, c) => n + c.files_here, 0);
await timed('until the laptop and the storage server have every file', () => until('sync', () => filesHere(server) === total && filesHere(laptop) === total));
// The phone may now let go: it keeps lists only, and caches up to 1 GiB of files.
await timed('until the phone has freed the originals beyond its cache', () => until('eviction', () => disk(phone, 'files') <= (1 << 30) + (64 << 20)));
report['files left on the phone'] = `${filesHere(phone)} of ${total}`;

// Opening pictures that are no longer on the phone fetches them from another device.
const get = (container, id, range) => fetch(`${api}/files/${id}?container=${container}`, { headers: { ...auth, ...(range ? { Range: range } : {}) } });
const gone = sample.filter((s) => true);
await timed(`opening ${gone.length} pictures on the phone (fetched on demand when not there)`, async () => {
  for (const s of gone) {
    const r = await get(s.container, s.id);
    assert.equal(r.status, 200);
    assert.equal(sha(Buffer.from(await r.arrayBuffer())), s.hash, 'the picture comes back unchanged');
  }
});
// Seeking in the video: one mebibyte from the middle.
await timed('reading 1 MiB from the middle of the video', async () => {
  const from = Math.floor(video.size / 2) + 12345;
  const r = await get(months[0], video.id, `bytes=${from}-${from + (1 << 20) - 1}`);
  assert.equal(r.status, 206);
  const part = Buffer.from(await r.arrayBuffer());
  assert.equal(part.length, 1 << 20);
  for (let k = 0; k < part.length; k += 997) assert.equal(part[k], videoByte(from + k));
});

for (const d of Object.values(devices)) {
  d.busy = rss(d);
  d.diskFiles = disk(d, 'files');
  d.diskDb = disk(d) - d.diskFiles;
}

// Restart everything: devices compare fingerprints and should load next to nothing.
await timed('restarting all three and reconnecting', async () => {
  for (const d of Object.values(devices)) {
    d.process.kill();
    await new Promise((r) => d.process.on('exit', r));
  }
  Object.values(devices).forEach(start);
  await until('all online again', () => Object.values(devices).every((d) => call(d, 'status').devices_online === 3));
});
await sleep(5000);
clearInterval(sampler);
console.log(`\n${report.data}, ${MONTHS} month containers\n`);
console.log('device   files on disk   documents on disk   memory: peak / after the load / after restart   documents in memory after restart');
for (const d of Object.values(devices)) {
  console.log(`${d.name.padEnd(8)} ${mb(d.diskFiles).padStart(13)}   ${mb(d.diskDb).padStart(17)}   ${mb(d.peak)} / ${mb(d.busy)} / ${mb(rss(d))}   ${call(d, 'status').documents_in_memory}`);
}
console.log(JSON.stringify(report, null, 2));
process.exit(0);
