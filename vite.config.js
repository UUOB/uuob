// One config for the four frontends: TARGET=uuob | tasks | calendar | photos.
// Each builds into its own <root>/dist for its Tauri app.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const targets = { uuob: ['app', 1420], tasks: ['apps/tasks', 1421], calendar: ['apps/calendar', 1422], photos: ['apps/photos', 1423] };
const [root, port] = targets[process.env.TARGET ?? 'uuob'];

export default defineConfig({
  plugins: [svelte()],
  root,
  clearScreen: false,
  server: { port, strictPort: true, fs: { allow: ['.'] } },
  build: { outDir: 'dist', emptyOutDir: true, target: 'es2022' },
});
