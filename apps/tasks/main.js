import { boot } from '../../ui/boot.js';
import Tasks from './Tasks.svelte';

boot({ id: 'tasks', name: 'Tasks', icon: 'tasks', color: 'var(--c-blue)', noun: 'tasks' }, Tasks);
