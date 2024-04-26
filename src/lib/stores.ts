import { persist, createLocalStorage } from '@macfja/svelte-persistent-store';
import { writable } from 'svelte/store';

export const books = persist(writable<string[]>([]), createLocalStorage(), 'books');
