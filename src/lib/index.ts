/* eslint-disable @typescript-eslint/no-explicit-any */
import { makeBook } from './foliate-js/view';
import { canvasToText, createImageCanvas } from '$lib/braille';
import { l } from '$lib/stores.svelte';
import { convertFileSrc } from '@tauri-apps/api/tauri';

export const SUPPORTED_FORMATS = ['epub'];

export const percentFormat = new Intl.NumberFormat('en', { style: 'percent' });

export type Book = {
	metadata: {
		title: any;
		identifier: any;
		language: any;
		author: any[];
		translator: any[];
		contributor: any[];
		publisher: any;
		published: any;
		modified: any;
		description: any;
		subject: any[];
	};
	getCover(): Promise<Blob>;
};

export const addFile = async (file: string): Promise<Book> => {
	return (await makeBook(file)) as Book;
};

export const addBooks = async (files: string | string[] | null) => {
	if (!files) return;
	else if (Array.isArray(files)) {
		console.log(files);
		for (const file of files) {
			const fileSrc = convertFileSrc(file);
			const book = await addFile(fileSrc);
			const coverUrl = await book?.getCover();

			let cover = '';
			if (coverUrl) {
				const canvas = await createImageCanvas(coverUrl);
				// settings.last_canvas = canvas;
				// settings.last_dithering = null;
				cover = canvasToText(canvas);
			}
			const title = book?.metadata?.title;

			if (title === undefined) {
				// TODO: error toast
				continue;
			}
			l.books[book.metadata.identifier] = {
				title,
				src: fileSrc,
				metadata: book.metadata,
				// coverUrl,
				// author
				cover
			};
		}
	}
};
