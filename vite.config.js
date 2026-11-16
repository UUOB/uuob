// The front end of UUOB, the backend app, is in app/ and builds into app/dist
// for its Tauri shell. (Tasks, Calendar and Photos have repositories of their own.)
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  root: 'app',
  clearScreen: false,
  server: { port: 1420, strictPort: true, fs: { allow: ['.'] } },
  build: { outDir: 'dist', emptyOutDir: true, target: 'es2022' },
});
