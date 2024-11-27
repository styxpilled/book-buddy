<script lang="ts">
	import { type Rendition } from 'epubjs';

	// import inline from 'epubjs/src/managers/views/inline';

	import { addFile } from '$lib';
	import { open } from '@tauri-apps/api/dialog';
	import { convertFileSrc } from '@tauri-apps/api/tauri';

	import '$lib/foliate-js/view.js';
	import { createTOCView } from '$lib/foliate-js/ui/tree.js';
	import { createMenu } from '$lib/foliate-js/ui/menu.js';
	import { Paginator } from '$lib/foliate-js/paginator';

	import bookStyle from '$styles/book.css?inline';
	import { onDestroy, onMount } from 'svelte';
	import { books, recentBooks, type BookData } from '$lib/stores';
	import inline from '$lib/view';
	import type { View } from '$lib/foliate-js/view.js';
	// import script from '$lib/test?url';

	export let id: string;

	let view: View;

	let reader: HTMLDivElement;
	let innerReader: HTMLDivElement;
	let rendition: Rendition;
	let height = 750;
	let width = 1000;
	let error: string | null = null;

	onMount(async () => {
		recentBooks.$push(id);
		// const _bookstr = localStorage.getItem(id);
		// if (_bookstr === null) {
		// 	error = `Book not in library!`;
		// 	return;
		// }
		// const book: BookData = JSON.parse(_bookstr);

		await view.open($books[id].src);
		await view.next();
		await view.goTo(0);
		console.log(view?.book?.cfi);
		// try {
		// 	const book = await addFile($books[id].src);
		// 	await book.ready;
		// 	book.spine.get('reader');
		// 	try {
		// 		rendition = book.renderTo(reader, {
		// 			width,
		// 			height,
		// 			allowScriptedContent: true,
		// 			stylesheet: bookStyle,
		// 			// script: script,
		// 			// in iframe view, scrolling is done via controlling scrollLeft
		// 			// the element has a super big width, and is just scrolling along that
		// 			// https://github.com/futurepress/epub.js/blob/f09089cf77c55427bfdac7e0a4fa130e373a19c8/src/managers/default/index.js#L451
		// 			view: inline
		// 		});
		// 		await rendition.display($books[id]?.cfi);
		// 		rendition = rendition;
		// 		innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		// 		if ($books[id].scroll) {
		// 			innerReader.scrollLeft = $books[id]?.scroll || 0;
		// 		}
		// 	} catch (error) {}
		// } catch (error) {
		// 	console.log(error);
		// }
	});

	const keybindHandler = async (e: KeyboardEvent) => {
		const key = e.key.toLowerCase();
		const tagName = document.activeElement?.tagName.toLocaleLowerCase();
		if (tagName === 'input' || tagName === 'textarea') return;
		switch (key) {
			case 'arrowleft': {
				e.preventDefault();
				await view.prev();
				break;
			}
			case 'arrowright': {
				e.preventDefault();
				await view.next();
				break;
			}
			default:
				break;
		}
	};

	const prevPage = async () => {
		await view.prev();
		return;
		if (innerReader?.scrollLeft === undefined) {
			await rendition.prev();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		}
		const previousScrollLeft = innerReader.scrollLeft;
		innerReader.scrollLeft -= width;
		if (innerReader.scrollLeft === previousScrollLeft) {
			await rendition.prev();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
			innerReader.scrollLeft = innerReader.scrollWidth;
		}
	};

	const nextPage = async () => {
		await view.next();
		return;
		// console.log(innerReader.scrollLeft);
		innerReader.scrollLeft = Math.floor(innerReader.scrollLeft / 1000) * 1000;

		if (innerReader?.scrollLeft === undefined) {
			await rendition.next();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		}
		const previousScrollLeft = innerReader.scrollLeft;
		innerReader.scrollLeft = Math.floor(innerReader.scrollLeft / 1000) * 1000;
		innerReader.scrollLeft += width;

		// console.log(width);
		console.log(innerReader.scrollLeft, previousScrollLeft);

		if (innerReader.scrollLeft === previousScrollLeft) {
			await rendition.next();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
			// innerReader.scrollLeft = 200;
		}
	};

	// onDestroy(() => {
	// 	// @ts-expect-error this is fake news
	// 	$books[id].cfi = rendition.currentLocation().start.cfi;
	// 	$books[id].scroll = innerReader.scrollLeft;
	// 	// JSON.parse(localStorage.getItem(id) || 'null')
	// });
</script>

<svelte:window on:keydown={keybindHandler} />

<div class="label row space-between">
	<h3 class="inline">{id}</h3>
	<button class="btn" on:click={prevPage}>Previous page</button>
	<button class="btn" on:click={nextPage}>Next page</button>
	<button class="btn" on:click={() => console.log(rendition.currentLocation())}>
		{view?.book?.cfi + '' + Math.random()}
	</button>
</div>

<foliate-view
	bind:this={view}
	on:load={(e) => {
		const { doc } = e.detail;
		const style = document.createElement('style');
		style.textContent = bookStyle;
		doc.head.append(style);
		doc.addEventListener('keydown', keybindHandler);
	}}
></foliate-view>

<!-- <div id="reader" style:--width="{width}px" style:--height="{height}px" bind:this={reader}></div> -->

<style>
	foliate-view {
		display: block;
		width: var(--width);
		height: var(--height);
		height: 40rem;
		width: 40rem;
		color: white !important;
	}
</style>
