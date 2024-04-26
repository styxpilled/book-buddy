import { persist, createLocalStorage } from '@macfja/svelte-persistent-store';
import { writable } from 'svelte/store';

export type BookData = {
	title: string;
	cover: string;
	coverUrl?: string;
	author?: string;
};

export type Label = {
	mode: 'manual' | 'automatic';
	value: string;
};

export const mainLabel = writable<Label>({
	mode: 'automatic',
	value: ''
});

export const books = persist(writable<BookData[]>([]), createLocalStorage(), 'books');
