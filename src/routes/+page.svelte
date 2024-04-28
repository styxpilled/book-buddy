<script lang="ts">
	import { books, mainLabel } from '$lib/stores';
	import {
		bookBeginningSegmentDisplay,
		bookEndSegmentDisplay,
		bookMiddleSegment,
		bookMiddleSegmentDisplay,
		generateBookLibrary
	} from '$lib/ui';
	import AddFile from '$ui/AddFile.svelte';
	import Cover from '$ui/Cover.svelte';
	import Dropdown from '$ui/Dropdown.svelte';
	import Icon from '$ui/Icon.svelte';

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
		<!-- SPACER -->
		{#if $books.length > 1}
			<pre class="spacer book-shelf">{bookBeginningSegmentDisplay('Kosiarz')}</pre>
		{/if}
		<!--  -->
		{#if $books.length > 1}
			<Dropdown style="position: absolute; --index: {0}rem; left: var(--index);">
				<pre slot="drop-btn" class="book-shelf" style:--index="{0}rem">{bookBeginningSegmentDisplay(
						$books[0].title
					)}</pre>
				<div>
					<h3>
						{$books[0].title}
						{#if $books[0].author}
							by {$books[0].author}
						{/if}
					</h3>
				</div>
			</Dropdown>
		{/if}
		{#each $books.slice(1, -1) as book, i}
			<Dropdown style="position: absolute; --index: {(i + 1) * 2.25}rem; left: var(--index);">
				<pre
					slot="drop-btn"
					style:--index="{(i + 1) * 2.25}rem"
					class="book-shelf">{bookMiddleSegmentDisplay(book.title)}</pre>
				<div>
					<h3>
						{book.title}
						{#if book.author}
							by {book.author}
						{/if}
					</h3>
				</div>
			</Dropdown>
		{/each}
		{#if $books.length > 2}
			{@const finalBook = $books[$books.length - 1]}
			<Dropdown
				style="position: absolute; --index: {($books.length - 1) * 2.25}rem; left: var(--index);"
			>
				<pre
					slot="drop-btn"
					class="book-shelf"
					style:--index="{($books.length - 1) * 2.25}rem">{bookEndSegmentDisplay(
						finalBook.title
					)}</pre>
				<div>
					<h3>
						{finalBook.title}
						{#if finalBook.author}
							by {finalBook.author}
						{/if}
					</h3>
				</div>
			</Dropdown>
		{/if}
	</ul>
{/if}

<style>
	ul.covers {
		display: flex;
		gap: 1rem;
		padding: 1rem 1rem 0.5rem;
	}

	ul.library {
		width: 81.5rem;
		display: flex;
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
