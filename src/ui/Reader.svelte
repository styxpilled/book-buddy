<script lang="ts">
	import '$lib/foliate-js/view.js';
	import { createTOCView } from '$lib/foliate-js/ui/tree.js';
	import { createMenu } from '$lib/foliate-js/ui/menu.js';
	import { Paginator } from '$lib/foliate-js/paginator';

	import bookStyle from '$styles/book.css?inline';
	import { onDestroy, onMount } from 'svelte';
	import { books, recentBooks, type BookData } from '$lib/stores.svelte';
	import type { View } from '$lib/foliate-js/view.js';

	interface Props {
		id: string;
	}

	let { id }: Props = $props();

	let view = $state() as View;

	onMount(async () => {
		recentBooks.$push(id);
		await view.open($books[id].src);
		await view.next();
		await view.goTo(0);
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

	onDestroy(() => {
		view.close();
	});
</script>

<svelte:window onkeydown={keybindHandler} />

<div class="label row space-between">
	<h3 class="inline">{id}</h3>
	<button class="btn" onclick={prevPage}>Previous page</button>
	<button class="btn" onclick={nextPage}>Next page</button>
</div>

<foliate-view
	bind:this={view}
	onload={(e: { detail: { doc: Document } }) => {
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
		height: calc(100dvh - 7rem);
		width: calc(100dvw - 2rem);
		color: white !important;
	}
</style>
