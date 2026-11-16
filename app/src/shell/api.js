// Thin wrapper over the Tauri command and the node's event stream.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { mount, unmount } from 'svelte';
import Scan from './Scan.svelte';

export const call = (method, params = {}) => invoke('call', { method, params });
export const mcpUrl = () => invoke('mcp_url');
/** Does UUOB start by itself (at login on a computer, when a phone turns on)? Null where there is no such choice. */
export const autostart = () => invoke('autostart');
export const setAutostart = (on) => invoke('set_autostart', { on });
export const onEvent = (fn) => listen('uuob', (e) => fn(e.payload));

/** "permission_denied: …" -> readable message */
export const message = (err) => String(err?.message ?? err).replace(/^[a-z_]+: /, '');

/** Phones: show the camera until it sees a QR code, and return its text.
 *  Cancelling rejects with an empty message. */
export function scanCode() {
  return new Promise((resolve, reject) => {
    const view = mount(Scan, {
      target: document.body,
      props: {
        done: (text, error) => {
          unmount(view);
          error ? reject(error) : resolve(text);
        },
      },
    });
  });
}

export const isPhone = /Android|iPhone|iPad/.test(navigator.userAgent);
export const initial = (name) => (name ?? '').trim().charAt(0).toUpperCase() || '?';
