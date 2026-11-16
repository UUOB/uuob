import { boot } from '../../ui/boot.js';
import Photos from './Photos.svelte';

boot({ id: 'photos', name: 'Photos', icon: 'photo', color: 'var(--c-orange)', noun: 'photos' }, Photos);
