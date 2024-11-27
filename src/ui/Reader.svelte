<script lang="ts">
	import { type Rendition } from 'epubjs';

	// import inline from 'epubjs/src/managers/views/inline';

	import { addFile } from '$lib';
	import { open } from '@tauri-apps/api/dialog';
	import { convertFileSrc } from '@tauri-apps/api/tauri';

	import bookStyle from '$styles/book.css?url';
	import { onDestroy, onMount } from 'svelte';
	import { books, recentBooks, type BookData } from '$lib/stores';
	import inline from '$lib/view';
	// import script from '$lib/test?url';

	export let id: string;

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
		try {
			const book = await addFile($books[id].src);
			await book.ready;
			book.spine.get('reader');
			try {
				rendition = book.renderTo(reader, {
					width,
					height,
					allowScriptedContent: true,
					stylesheet: bookStyle,
					// script: script,
					// in iframe view, scrolling is done via controlling scrollLeft
					// the element has a super big width, and is just scrolling along that
					// https://github.com/futurepress/epub.js/blob/f09089cf77c55427bfdac7e0a4fa130e373a19c8/src/managers/default/index.js#L451
					view: inline
				});
				await rendition.display($books[id]?.cfi);
				rendition = rendition;
				innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
				if ($books[id].scroll) {
					innerReader.scrollLeft = $books[id]?.scroll || 0;
				}
			} catch (error) {}
		} catch (error) {
			console.log(error);
		}
	});

	const keybindHandler = async (e: KeyboardEvent) => {
		const key = e.key.toLowerCase();
		const tagName = document.activeElement?.tagName.toLocaleLowerCase();
		if (tagName === 'input' || tagName === 'textarea') return;
		switch (key) {
			case 'arrowleft': {
				e.preventDefault();
				await prevPage();
				break;
			}
			case 'arrowright': {
				e.preventDefault();
				await nextPage();
				break;
			}
			default:
				break;
		}
	};

	const prevPage = async () => {
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

	onDestroy(() => {
		// @ts-expect-error this is fake news
		$books[id].cfi = rendition.currentLocation().start.cfi;
		$books[id].scroll = innerReader.scrollLeft;
		// JSON.parse(localStorage.getItem(id) || 'null')
	});
</script>

<svelte:window on:keydown={keybindHandler} />

<div class="label row space-between">
	<h3 class="inline">{id}</h3>
	<button class="btn" on:click={prevPage}>Previous page</button>
	<button class="btn" on:click={nextPage}>Next page</button>
	<button class="btn" on:click={() => console.log(rendition.currentLocation())}>
		{$books[id].cfi}
	</button>
</div>

<div id="reader" style:--width="{width}px" style:--height="{height}px" bind:this={reader}></div>

<style>
	#reader {
		/* height: 30rem;
		width: 30rem; */
		/* pointer-events: none; */
		width: var(--width);
		height: var(--height);
		text-indent: 1rem;
		font-size: 1rem;
		/* display: flex;
		flex-direction: column; */
	}

	/* :global(#reader div) {
		display: contents;
	} */

	:global(#reader *) {
		margin: 0;
		/* margin-bottom: 0.5rem; */
		padding: 0;

		& p.calibre_3:has(+ br) {
			/* height: 1rem; */
			margin-bottom: 1rem;
		}

		& p.calibre_ {
			margin-bottom: 0.625rem;
			/* border: 1px solid whitesmoke; */
			width: 100%;
			font-size: 1.5rem;
		}

		/* & *::-webkit-scrollbar {
			width: 0 !important;
			background-color: orange;
			display: none !important;
		}

		& *::-webkit-scrollbar-thumb {
			display: none !important;
		}

		& *::-webkit-scrollbar-track {
			background-color: transparent !important;
		} */

		& div[id^='epubjs-view'] {
			overflow: scroll !important;
			/* width: 500px; */
			height: 500px;
			display: flex;
			flex-wrap: wrap;
			flex-direction: column;
			/* align-items: center; */
			/* background: red; */

			& svg {
				align-self: center;
			}
			/* scrollbar-width: thin; */
		}

		& .epub-view {
			display: flex !important;
			align-items: center;
			justify-content: center;
		}

		& div {
			display: contents;
		}

		& br {
			display: none;
		}

		&.calibre1 {
			overflow: scroll;
		}

		& section {
			max-height: 100%;
			width: 1000px;
			display: flex;
			flex-wrap: wrap;
			flex-direction: column;
			/* column-gap: 5rem; */

			& > * {
				padding: 0 5rem;
				max-width: 1000px;
			}
		}
	}

	:global(#reader *) {
		scrollbar-width: none;
	}
</style>
