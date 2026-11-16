// Start an app: it shows the connect screen until it is connected to UUOB,
// then the app itself.
import './tokens.css';
import { mount } from 'svelte';
import Gate from './Gate.svelte';
import { uuob } from './uuob.js';

/** app: { id, name, icon, color, noun } */
export function boot(app, Main) {
  uuob.init(app);
  mount(Gate, { target: document.getElementById('app'), props: { app, Main } });
}
