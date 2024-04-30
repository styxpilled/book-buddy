<script lang="ts">
	import { books, mainLabel, preferences, recentBooks } from '$lib/stores';
	import { chunks } from '$lib/ui';
	import AddFile from '$ui/AddFile.svelte';
	import Cover from '$ui/Cover.svelte';
	import Shelf from '$ui/Shelf.svelte';

	let showCovers = $preferences.view === 'cover';

	$: $preferences.view = showCovers === true ? 'cover' : 'shelf';

	$mainLabel = {
		mode: 'manual',
		value: 'Books'
	};
</script>

<div class="row label" style="gap: 1rem;">
	<AddFile />
	<button
		class="btn"
		on:click={() => {
			$books = [];
		}}>Clear library</button
	>
	<button
		class="btn"
		on:click={() => {
			$recentBooks = [];
		}}>Clear recent</button
	>
	<label class="switch">
		Show covers
		<input type="checkbox" bind:checked={showCovers} />
	</label>
	<label class="range">
		<input type="range" min="5" max="40" step="5" bind:value={$preferences.booksPerShelf} />
		Books per shelf
	</label>
</div>
{#if $preferences.view === 'cover'}
	<ul class="covers">
		{#each $books as book}
			<Cover {book} />
		{/each}
	</ul>
{:else}
	<ul class="library">
		{#each chunks($books, $preferences.booksPerShelf) as chunk, row}
			{#if row}
				<!--  -->
			{/if}
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
