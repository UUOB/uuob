// Small date helpers shared by apps. Dates are local "YYYY-MM-DD" strings,
// times "HH:MM" — simple, sortable, timezone-free.

export const pad = (n) => String(n).padStart(2, '0');
export const iso = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const today = () => iso(new Date());
export const parse = (s) => {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, m - 1, d);
};
export const addDays = (s, n) => {
  const d = parse(s);
  d.setDate(d.getDate() + n);
  return iso(d);
};

const WEEKDAYS = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];

/** "Today", "Tomorrow", "Fri", "Mon 24 Nov" … */
export function friendly(s) {
  const t = today();
  if (s === t) return 'Today';
  if (s === addDays(t, 1)) return 'Tomorrow';
  if (s === addDays(t, -1)) return 'Yesterday';
  const d = parse(s);
  const diff = (d - parse(t)) / 86400000;
  if (diff > 0 && diff < 7) return d.toLocaleDateString(undefined, { weekday: 'long' });
  const sameYear = d.getFullYear() === new Date().getFullYear();
  return d.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short', ...(sameYear ? {} : { year: 'numeric' }) });
}

// How times are written: the user's choice for all apps (see Settings.svelte,
// which passes its reactive settings here, so a change shows at once).
let clock = { hour12: false };
export const useClock = (settings) => (clock = settings);

/** "14:30", or "2:30 PM" when the user chose AM/PM. */
export function friendlyTime(t) {
  const [h, m] = t.split(':').map(Number);
  if (!clock.hour12) return `${h}:${pad(m)}`;
  return `${h % 12 || 12}:${pad(m)} ${h < 12 ? 'AM' : 'PM'}`;
}

/**
 * A time as typed: "14:30", "1430", "9", "9.15", "2:30 pm", "2pm" -> "HH:MM",
 * or null if it isn't one.
 */
export function parseTime(text) {
  const m = String(text ?? '').trim().toLowerCase().match(/^(\d{1,2})(?:[:.h ]?(\d{2}))?\s*(am|pm|a|p)?$/);
  if (!m) return null;
  let h = Number(m[1]);
  const min = Number(m[2] ?? 0);
  if (m[3]) {
    if (h < 1 || h > 12) return null;
    h = (h % 12) + (m[3].startsWith('p') ? 12 : 0);
  }
  return h <= 23 && min <= 59 ? `${pad(h)}:${pad(min)}` : null;
}

/**
 * Pull a date/time out of quick-add text.
 *   "call mom fri 5pm" -> { title: "call mom", due: <next Friday>, time: "17:00" }
 * Understands: today, tonight, tomorrow, weekday names, next week,
 * in N days/weeks, 24.12. / 24.12.2026 / 12/24, 5pm, 5:30pm, 17:00, at 9.
 */
export function parseQuick(text) {
  let due = null;
  let time = null;
  let words = text.trim().split(/\s+/);
  const t = today();
  const take = (i, n = 1) => words.splice(i, n);

  for (let i = 0; i < words.length; i++) {
    const w = words[i].toLowerCase().replace(/[,.]$/, (m) => (/\d/.test(words[i]) ? m : ''));
    const next = words[i + 1]?.toLowerCase();
    let m;
    if (w === 'today') { due = t; take(i--); }
    else if (w === 'tonight') { due = t; time ??= '20:00'; take(i--); }
    else if (w === 'tomorrow' || w === 'tmr') { due = addDays(t, 1); take(i--); }
    else if (w === 'next' && next === 'week') {
      const d = parse(t).getDay();
      due = addDays(t, ((8 - d) % 7) || 7);
      take(i--, 2);
    } else if (w === 'in' && /^\d+$/.test(next ?? '') && /^(day|days|week|weeks)$/.test(words[i + 2]?.toLowerCase() ?? '')) {
      const n = Number(next) * (words[i + 2].toLowerCase().startsWith('week') ? 7 : 1);
      due = addDays(t, n);
      take(i--, 3);
    } else if ((m = WEEKDAYS.indexOf(w.slice(0, 3))) >= 0 && /^(sun|mon|tue|wed|thu|fri|sat)[a-z]*$/.test(w)) {
      const d = parse(t).getDay();
      due = addDays(t, ((m - d + 7) % 7) || 7);
      take(i--);
    } else if ((m = w.match(/^(\d{1,2})\.(\d{1,2})\.(\d{4})?$/))) {
      const y = m[3] ? Number(m[3]) : new Date().getFullYear();
      due = iso(new Date(y, Number(m[2]) - 1, Number(m[1])));
      if (!m[3] && due < t) due = iso(new Date(y + 1, Number(m[2]) - 1, Number(m[1])));
      take(i--);
    } else if ((m = w.match(/^(\d{1,2})\/(\d{1,2})$/))) {
      const y = new Date().getFullYear();
      due = iso(new Date(y, Number(m[1]) - 1, Number(m[2])));
      if (due < t) due = iso(new Date(y + 1, Number(m[1]) - 1, Number(m[2])));
      take(i--);
    } else if ((m = w.match(/^(\d{1,2})(?::(\d{2}))?(am|pm)$/))) {
      let h = Number(m[1]) % 12;
      if (m[3] === 'pm') h += 12;
      time = `${pad(h)}:${m[2] ?? '00'}`;
      take(i--);
    } else if ((m = w.match(/^([01]?\d|2[0-3]):([0-5]\d)$/))) {
      time = `${pad(Number(m[1]))}:${m[2]}`;
      take(i--);
    } else if (w === 'at' && /^\d{1,2}$/.test(next ?? '')) {
      let h = Number(next);
      if (h < 8) h += 12; // "at 5" means 5pm
      time = `${pad(h)}:00`;
      take(i--, 2);
    }
  }
  if (time && !due) due = time < `${pad(new Date().getHours())}:${pad(new Date().getMinutes())}` ? addDays(t, 1) : t;
  return { title: words.join(' ').trim(), due, time };
}
