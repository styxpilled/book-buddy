<script lang="ts">
	import { type Rendition } from 'epubjs';
	import inline from 'epubjs/src/managers/views/inline';
	import { addFile } from '$lib';
	import { open } from '@tauri-apps/api/dialog';
	import { convertFileSrc } from '@tauri-apps/api/tauri';
	import bookStyle from '$styles/book.css?url';
	import script from '$lib/test?url';

	let reader: HTMLDivElement;
	let innerReader: HTMLDivElement;
	let rendition: Rendition;
	let height = 750;
	let width = 1000;

	const getBooks = async () => {
		const files = await open({
			multiple: true,
			filters: [
				{
					extensions: ['epub'],
					name: 'Supported formats: epub'
				}
			]
		});
		console.log(files);

		if (!files) return;
		else if (Array.isArray(files)) {
			for (let file of files) {
				file = convertFileSrc(file);
				try {
					const book = await addFile(file);
					await book.ready;
					console.log(book);
					book.spine.get('reader');
					console.log(book);

					try {
						rendition = book.renderTo(reader, {
							width,
							height,
							allowScriptedContent: true,
							stylesheet: bookStyle,
							script: script,
							// in iframe view, scrolling is done via controlling scrollLeft
							// the element has a super big width, and is just scrolling along that
							// https://github.com/futurepress/epub.js/blob/f09089cf77c55427bfdac7e0a4fa130e373a19c8/src/managers/default/index.js#L451
							view: inline
						});
						await rendition.display();
						rendition = rendition;
					} catch (error) {}

					// document.querySelectorAll('div').forEach((el) => {
					//   console.log(el.scrollLeft)
					// })
					// const t= document.querySelector('.calibre');
					// if (t) {
					//   t.addEventListener('scroll', (e) => {
					//     console.log(e)
					//   })
					// }
					// rendition.
					// await rendition.display();
					// rendition.resize(500, 500);
					// innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
					console.log(innerReader);
				} catch (error) {
					console.log(error);
				}
			}
		} else {
			console.log(await addFile(files));
		}
	};

	const keybindHandler = async (e: KeyboardEvent) => {
		const key = e.key.toLowerCase();
		const tagName = document.activeElement?.tagName.toLocaleLowerCase();
		if (tagName === 'input' || tagName === 'textarea') return;
		switch (key) {
			case 'arrowleft': {
				await nextPage();
				break;
			}
			case 'arrowright': {
				await prevPage();
				break;
			}
			default:
				break;
		}
	};

	const nextPage = async () => {
		if (innerReader?.scrollLeft === undefined) {
			await rendition.prev();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		}
		const previousScrollLeft = innerReader.scrollLeft;
		innerReader.scrollLeft -= width;
		if (innerReader.scrollLeft === previousScrollLeft) {
			await rendition.prev();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		}
	};

	const prevPage = async () => {
		if (innerReader?.scrollLeft === undefined) {
			await rendition.next();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		}
		const previousScrollLeft = innerReader.scrollLeft;
		innerReader.scrollLeft += width;
		if (innerReader.scrollLeft === previousScrollLeft) {
			await rendition.next();
			innerReader = document.querySelector(`div[id^='epubjs-view']`) as HTMLDivElement;
		}
	};

	$: console.log(rendition?.book?.packaging?.metadata?.title);
</script>

<svelte:window on:keydown={keybindHandler} />

<!-- {#if rendition}
	{rendition?.currentLocation()}
{/if} -->
{rendition?.book?.packaging?.metadata?.title}
<button class="btn" on:click={getBooks}>add books</button>
<button class="btn" on:click={prevPage}>previous page</button>
<button class="btn" on:click={nextPage}>next page</button>
<button class="btn" on:click={() => (innerReader.scrollLeft += width)}>uhuh</button>
<div
	id="reader"
	class="container"
	style:--label="'{rendition?.book?.packaging?.metadata?.title}'"
	style:--width="{width}px"
	style:--height="{height}px"
	bind:this={reader}
></div>

<!-- {' '}{innerReader?.scrollLeft || ''} -->
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

		& div[id^='epubjs-view'] {
			overflow: scroll !important;
			width: 500px;
			height: 500px;
			display: flex;
			flex-wrap: wrap;
			flex-direction: column;
			/* align-items: center; */
			/* background: red; */

			& svg {
				align-self: center;
			}

			& * {
				/* scrollbar-width: thin; */
				scrollbar-color: transparent;
			}

			&::-webkit-scrollbar {
				width: 0;
			}

			&::-webkit-scrollbar-thumb {
				display: none;
			}

			&::-webkit-scrollbar-track {
				background-color: transparent;
			}
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
	}
</style>
