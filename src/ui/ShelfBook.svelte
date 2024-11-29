<script lang="ts">
	import type { BookData } from '$lib/stores.svelte';
	import { bookEndSegment, bookSegment } from '$lib/ui';
	import Dropdown from './Dropdown.svelte';

	interface Props {
		book: BookData;
		end: boolean;
		index: number;
	}

	let { book, end, index }: Props = $props();

	// const display =
	// 	order === 'start'
	// 		? bookSegment
	// 		: order === 'middle'
	// 			? bookMiddleSegment
	// 			: bookEndSegment;
</script>

<a href="/book/{book.title}">
	<Dropdown style="position: absolute; --index: {index * 2.25}rem; left: var(--index);">
		<!-- @migration-task: migrate this slot by hand, `drop-btn` is an invalid identifier -->
		<pre slot="drop-btn" class="book-shelf" style:--index="{0}rem">{end
				? bookEndSegment(book.title)
				: bookSegment(book.title)}</pre>
		<div>
			<h3>
				{book.title}
				{#if book.author}
					by {book.author}
				{/if}
			</h3>
		</div>
	</Dropdown>
</a>
