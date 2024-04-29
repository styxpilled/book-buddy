<script lang="ts">
	import { books, mainLabel } from '$lib/stores';
	import { chunks } from '$lib/ui';
	import AddFile from '$ui/AddFile.svelte';
	import Cover from '$ui/Cover.svelte';
	import Shelf from '$ui/Shelf.svelte';

	let settings = {
		showCovers: false
	};

	$mainLabel = {
		mode: 'manual',
		value: 'Books'
	};
</script>

<div class="row label">
	<AddFile />
	<button
		class="btn"
		on:click={() => {
			$books = [];
		}}>Clear library</button
	>
	<label class="switch">
		Show covers
		<input type="checkbox" bind:checked={settings.showCovers} />
	</label>
</div>

{#if settings.showCovers}
	<ul class="covers">
		{#each $books as book}
			<Cover {book} />
		{/each}
	</ul>
{:else}
	<ul class="library">
		{#each chunks($books, 35) as chunk, row}
			<Shelf row={chunk} />
		{/each}
	</ul>
{/if}

<style>
	ul.covers {
		display: flex;
		gap: 1rem;
		padding: 1rem 1rem 0.5rem;
		flex-wrap: wrap;
		max-width: 85rem;
	}

	ul.library {
		width: 81.5rem;
		display: flex;
		flex-direction: column;
		padding: 1rem 1rem 0.5rem;
		position: relative;

		& .book-shelf.spacer {
			visibility: hidden;
		}

		& .book-shelf:not(.spacer) {
			/* position: absolute; */
			/* left: var(--index); */
			user-select: none;
			cursor: pointer;
			filter: brightness(0.75);
			&:hover {
				/* color: red; */
				filter: brightness(1) drop-shadow(1px 1px 2px var(--color));
			}
		}
	}
</style>
