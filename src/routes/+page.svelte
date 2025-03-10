<script lang="ts">
	import { labels } from '$lib/labels.svelte';
	import { l, preferences, recentBooks } from '$lib/stores.svelte';
	import { chunks } from '$lib/ui';
	import AddFile from '$ui/AddFile.svelte';
	import Cover from '$ui/Cover.svelte';
	import Shelf from '$ui/Shelf.svelte';

	let showCovers = $state(preferences.view === 'cover');

	$effect(() => {
		preferences.view = showCovers === true ? 'cover' : 'shelf';
	});

	labels.root = root;
</script>

{#snippet root()}
	<div class="row label" style="gap: 0.5rem;">
		<AddFile />
		<button
			class="btn"
			onclick={() => {
				l.clear();
			}}>Clear library</button
		>
		<button
			class="btn"
			onclick={() => {
				// $recentBooks = [];
			}}>Clear recent</button
		>
		<label class="switch">
			Show covers
			<input type="checkbox" bind:checked={showCovers} />
		</label>
		<label class="range">
			<input type="range" min="5" max="40" step="1" bind:value={preferences.booksPerShelf} />
			Books per shelf
		</label>
	</div>
{/snippet}
{#if preferences.view === 'cover'}
	<ul class="covers">
		{#each Object.keys(l.books).sort() as book}
			<Cover book={l.books[book]} />
		{/each}
	</ul>
{:else}
	<ul class="library">
		{#each chunks(Object.keys(l.books), preferences.booksPerShelf) as chunk, row}
			{@const r = chunk.map((book) => l.books[book])}
			<!-- {JSON.stringify(r)} -->
			<Shelf row={r} />
		{/each}
	</ul>
{/if}

<style>
	ul.covers {
		display: flex;
		gap: 1rem;
		padding: 1rem 1rem 0.5rem;
		flex-wrap: wrap;
		/* max-width: 85rem; */
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
