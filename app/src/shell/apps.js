// How known apps look in UUOB. Unknown apps get a neutral tile.
const known = {
  tasks: { icon: 'tasks', color: 'var(--c-blue)' },
  calendar: { icon: 'calendar', color: 'var(--c-red)' },
  photos: { icon: 'photo', color: 'var(--c-orange)' },
};
export const appIcon = (id) => known[id]?.icon ?? 'data';
export const appColor = (id) => known[id]?.color ?? 'var(--c-purple)';
