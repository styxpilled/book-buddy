/* eslint-disable @typescript-eslint/no-explicit-any */
import type { Book } from '$lib';
import { writable } from 'svelte/store';

export type BookData = {
	title: string;
	src: string;
	cover: string;
	metadata: Book['metadata'];
	cfi?: string;
	scroll?: number;
	coverUrl?: string;
	author?: string;
};

// export const currentBook = persist(
// 	writable<BookData | null>(null),
// 	createLocalStorage(),
// 	'currentBook'
// );

export const commands = new (class Commands {
	#raw = $state('');
	mode: 'search text' | 'search library' | 'command' = $state('search library');
	query = $state('');

	commandbar = $state() as HTMLInputElement;

	set raw(str) {
		if (str.startsWith('>')) {
			this.mode = 'command';
			this.query = str.substring(1);
		} else if (str.startsWith('@')) {
			this.mode = 'search text';
			this.query = str.substring(1);
		} else {
			this.mode = 'search library';
			this.query = str;
		}
		this.#raw = str;

		switch (this.mode) {
			case 'search text':
				// TODO: implement search
				break;

			default:
				break;
		}
	}
	get raw() {
		return this.#raw;
	}
})();

export const currentBook = null;

export type BookList = {
	[key: string]: BookData;
};

// export const books = persist(writable<BookList>({}), createLocalStorage(), 'books');

export const l = new (class Library {
	books = persisted<{
		[key: string]: BookData;
	}>('books', {});
	recent = persisted<string[]>('recent', []);

	constructor() {}

	clear() {
		for (const key in this.books) {
			delete this.books[key];
		}
	}
})();

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

export const preferences = $state<Preferences>({
	view: 'cover',
	booksPerShelf: 6
});
// export const preferences = persist(
// 	writable<Preferences>({
// 		view: 'cover',
// 		booksPerShelf: 25
// 	}),
// 	createLocalStorage(),
// 	'preferences'
// );

function persisted<T>(key: string, fallback: T): T {
	const stored = readLocalStorage(key, fallback);
	console.log(stored);

	const state = $state(stored);
	localStorage.setItem(key, JSON.stringify(state));
	$effect.root(() => {
		$state.snapshot(state);
		$effect(() => {
			localStorage.setItem(key, JSON.stringify(state));
		});
	});
	return state;
}

function readLocalStorage<T>(key: string, fallback: T): T {
	const res = localStorage.getItem(key);
	if (res === null) return fallback;
	else {
		try {
			const item = JSON.parse(res);
			if (Array.isArray(item)) {
				return item as T;
			}
			return { ...fallback, ...item };
		} catch (error) {
			console.error(error);
			return fallback;
		}
	}
}

// export const recentBooks = [];

// export const recentBooks = createSetStore(
// 	persist(writable<string[]>([]), createLocalStorage(), 'recentBooks'),
// 	15
// );
