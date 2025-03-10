<script lang="ts">
	import '$lib/foliate-js/view.js';
	import { createTOCView } from '$lib/foliate-js/ui/tree.js';
	import { createMenu } from '$lib/foliate-js/ui/menu.js';
	import { Paginator } from '$lib/foliate-js/paginator';

	import bookStyle from '$styles/book.css?inline';
	import { onDestroy, onMount } from 'svelte';
	import { l, recentBooks, type BookData } from '$lib/stores.svelte';
	import type { View } from '$lib/foliate-js/view.js';
	import { canvasToText, createImageCanvas } from '$lib/braille';
	import { labels } from '$lib/labels.svelte';
	import { percentFormat } from '$lib';

	interface Props {
		id: string;
	}

	let { id }: Props = $props();

	let view = $state() as View;
	let showing = $state<null | {
		x: number;
		y: number;
		src: string;
	}>(null);
	let locationLabel = $state('');
	let search = $state('');

	onMount(async () => {
		// recentBooks.$push(id);
		console.log(l.books[id]);

		await view.open(l.books[id].src);
		// await view.next();
		await view.goTo(l.books[id]?.cfi || 0);
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
	};

	const nextPage = async () => {
		await view.next();
	};

	const mouseHandler = (e: MouseEvent) => {
		showing = null;
		if (!e.target) return;
		if (e.button !== 2) return;
		const el = e.target as HTMLElement;
		if (el.nodeName === 'img') {
			const img = el as HTMLImageElement;
			e.preventDefault();
			showing = {
				x: e.x + view.offsetLeft,
				y: e.y + view.offsetTop,
				src: img.src
			};
		}
	};

	onDestroy(() => {
		view.close();
	});

	labels.root = root;
</script>

<svelte:window onkeydown={keybindHandler} />

{#snippet root()}
	<div class="label row space-between">
		<button class="btn" onclick={prevPage}>Previous page</button>
		<button class="btn" onclick={nextPage}>Next page</button>
		<input
			bind:value={search}
			oninput={async (e) => {
				for await (const t of view.search({
					query: search,
					index: null,
					matchCase: false,
					matchWholeWords: true
				})) {
					console.log(t);
					// console.log(view.searchResults);
				}
				// const t = await view
				// 	.search({
				// 		query: search
				// 	})
				// 	.next();
				// console.log(t);
			}}
		/>
	</div>
	<div class="label row center">
		<h3 class="inline">{l.books[id].title}</h3>
	</div>
	<div class="label right low">
		<span>{locationLabel}</span>
		<span>{l.books[id]?.cfi}</span>
	</div>
{/snippet}

<foliate-view
	bind:this={view}
	onrelocate={({ detail }: any) => {
		const { fraction, location, tocItem, pageItem } = detail;
		const percent = percentFormat.format(fraction);
		const loc = pageItem ? `Page ${pageItem.label}` : `Loc ${location.current}`;
		locationLabel = `${percent} · ${loc}`;
		l.books[id].cfi = detail.cfi;
	}}
	onload={(e: { detail: { doc: Document } }) => {
		const { doc } = e.detail;
		const style = document.createElement('style');
		style.textContent = bookStyle;
		doc.head.append(style);
		doc.addEventListener('keydown', keybindHandler);
		doc.addEventListener('mouseup', mouseHandler);
		doc.addEventListener('wheel', async (e) => {
			if (e.deltaY > 0) {
				await nextPage();
			} else {
				await prevPage();
			}
		});
	}}
></foliate-view>
<div
	id="image-contextmenu"
	class:showing
	style={showing ? `top: ${showing.y}px; left: ${showing.x}px;` : ''}
>
	<button
		onclick={async () => {
			if (showing === null) return;
			const canvas = await createImageCanvas(showing.src);
			// settings.last_canvas = canvas;
			// settings.last_dithering = null;
			l.books[id].cover = canvasToText(canvas);
		}}>Use as Cover</button
	>
</div>

<!-- <div id="reader" style:--width="{width}px" style:--height="{height}px" bind:this={reader}></div> -->

<style>
	foliate-view {
		display: block;
		width: var(--width);
		height: var(--height);
		height: calc(100dvh - 7.5rem);
		width: calc(100dvw - 2rem);
		color: white !important;
		position: relative;
		z-index: 1;
	}

	#image-contextmenu {
		position: absolute;
		top: 100px;
		left: 100px;
		z-index: 5000;
		filter: invert(1);
		background: var(--bg-color);
		display: none;
	}

	#image-contextmenu.showing {
		display: block;
	}
</style>
