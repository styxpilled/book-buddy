import { persist, createLocalStorage, type PersistentStore } from '@macfja/svelte-persistent-store';
import { writable } from 'svelte/store';

export type BookData = {
	title: string;
	src: string;
	cover: string;
	cfi?: string;
	scroll?: number;
	coverUrl?: string;
	author?: string;
};

export const currentBook = persist(
	writable<BookData | null>(null),
	createLocalStorage(),
	'currentBook'
);

export type BookList = {
	[key: string]: BookData;
};

export const books = persist(writable<BookList>({}), createLocalStorage(), 'books');

export type Label = {
	mode: 'manual' | 'automatic';
	value: string;
};

export const mainLabel = writable<Label>({
	mode: 'automatic',
	value: ''
});

export type Preferences = {
	view: 'shelf' | 'cover';
	booksPerShelf: number;
};

export const preferences = persist(
	writable<Preferences>({
		view: 'cover',
		booksPerShelf: 25
	}),
	createLocalStorage(),
	'preferences'
);

export const createSetStore = <T>(store: PersistentStore<T[]>, maxLength: number) => {
	const { subscribe, set, update } = store;

	return {
		subscribe,
		update,
		$push: (v: T) =>
			update((arr) => {
				const index = arr.indexOf(v);
				if (index !== -1) {
					arr.splice(index, 1);
				} else if (arr.length > maxLength) {
					arr.shift();
				}
				arr.push(v);
				return arr;
			}),
		set
	};
};

export const recentBooks = createSetStore(
	persist(writable<string[]>([]), createLocalStorage(), 'recentBooks'),
	15
);
