<script module>
  // Settings, the same sheet in every app. The choices at the top belong to
  // all of the user's apps (kept in UUOB, so a change here reaches every app
  // on every device); an app adds its own groups below them; About comes last.
  //
  //   <Settings bind:open name="Calendar" week>…the app's own groups…</Settings>
  //
  // `settings` is reactive: read `settings.sunday_first` and the screen
  // follows. Times (friendlyTime in dates.js) follow `hour12` by themselves.
  import { useClock } from './dates.js';

  export const settings = $state({ hour12: false, sunday_first: false });
  useClock(settings);
</script>

<script>
  import { onMount } from 'svelte';
  import { uuob } from './uuob.js';
  import Icon from './Icon.svelte';

  let { open = $bindable(false), name, week = false, children } = $props();
  let version = $state('');

  onMount(() => {
    const off = uuob.settings.subscribe((s) => Object.assign(settings, s));
    import('@tauri-apps/api/app')
      .then(({ getVersion }) => getVersion())
      .then((v) => (version = v), () => {});
    return off;
  });

  const change = (c) => (Object.assign(settings, c), uuob.settings.set(c));
</script>

<svelte:window onkeydown={(e) => open && e.key === 'Escape' && (open = false)} />

{#if open}
  <div class="scrim" onclick={(e) => e.target === e.currentTarget && (open = false)} role="presentation">
    <div class="sheet settings" role="dialog" aria-label="Settings">
      <div class="head">
        <h2 class="title">Settings</h2>
        <button class="icon-btn" onclick={() => (open = false)} aria-label="Close" title="Close"><Icon name="close" size={19} /></button>
      </div>

      <div class="group-label">All your apps</div>
      <div class="group">
        <div class="srow">
          <span class="label">Time</span>
          <div class="segmented" role="group" aria-label="Time format">
            <button class:on={!settings.hour12} aria-pressed={!settings.hour12} onclick={() => change({ hour12: false })}>14:30</button>
            <button class:on={settings.hour12} aria-pressed={settings.hour12} onclick={() => change({ hour12: true })}>2:30 PM</button>
          </div>
        </div>
        {#if week}
          <div class="srow">
            <span class="label">Week starts</span>
            <div class="segmented" role="group" aria-label="First day of the week">
              <button class:on={!settings.sunday_first} aria-pressed={!settings.sunday_first} onclick={() => change({ sunday_first: false })}>Monday</button>
              <button class:on={settings.sunday_first} aria-pressed={settings.sunday_first} onclick={() => change({ sunday_first: true })}>Sunday</button>
            </div>
          </div>
        {/if}
      </div>
      <p class="foot">Every app follows these, on all your devices.</p>

      {@render children?.()}

      <div class="group-label">About</div>
      <div class="group">
        <div class="srow">
          <span class="label">{name}</span>
          <span class="value">{version ? `Version ${version}` : ''}</span>
        </div>
      </div>
      <p class="foot">Open source: github.com/UUOB</p>
    </div>
  </div>
{/if}

<style>
  .settings { padding: 22px 22px 26px; }
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
  /* Groups, like iPhone Settings. :global so an app's own groups look the same. */
  .settings :global(.group-label) {
    font-size: 13px; font-weight: 600; color: var(--text-3); text-transform: uppercase; letter-spacing: 0.04em;
    margin: 22px 4px 8px;
  }
  .settings :global(.group) { background: var(--surface); border: 1px solid var(--line); border-radius: 14px; overflow: hidden; }
  .settings :global(.srow) {
    display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 52px; padding: 8px 14px;
    flex-wrap: wrap;
  }
  .settings :global(.srow + .srow) { border-top: 1px solid var(--line); }
  .settings :global(.srow .label) { font-size: 16px; font-weight: 500; min-width: 0; overflow-wrap: anywhere; }
  .settings :global(.srow .value) { font-size: 15px; color: var(--text-2); }
  .settings :global(.foot) { font-size: 13px; color: var(--text-2); margin: 8px 4px 0; }
  .segmented button { min-width: 74px; font-variant-numeric: tabular-nums; }
</style>
