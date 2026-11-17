<script>
  // A time field that writes times the way the user chose (14:30 or
  // 2:30 PM), whatever the system's own setting. Typing "1430", "9",
  // "9.15" or "2pm" works too; anything else goes back to the time it had.
  // `value` is "HH:MM"; with `optional`, emptying the field makes it null.
  import { friendlyTime, parseTime } from './dates.js';
  import { settings } from './Settings.svelte';

  let { value = $bindable(), optional = false, ...rest } = $props();
  let text = $state('');
  let editing = $state(false);
  // A click focuses the field with all of it selected, so typing replaces
  // the time; the click's own mouse-up would otherwise put a cursor in it.
  let picked = false;
  const shown = () => (value ? friendlyTime(value) : '');
  $effect(() => {
    if (!editing) text = shown();
  });
  function commit() {
    const t = parseTime(text);
    if (t) value = t;
    else if (optional && !text.trim()) value = null;
    text = shown();
  }
</script>

<input
  class="field"
  autocomplete="off"
  spellcheck="false"
  inputmode={settings.hour12 ? 'text' : 'decimal'}
  {...rest}
  bind:value={text}
  onfocus={(e) => ((editing = picked = true), e.currentTarget.select())}
  onmouseup={(e) => picked && (e.preventDefault(), (picked = false))}
  onblur={() => ((editing = false), commit())}
  onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), commit(), e.currentTarget.blur())}
/>
