<script lang="ts">
	import type { BookData } from '$lib/stores';
	import { bookBeginningSegment, bookEndSegment, bookMiddleSegment } from '$lib/ui';
	import Dropdown from './Dropdown.svelte';

	export let book: BookData;
	export let order: 'start' | 'middle' | 'end';
	export let index: number;

	const display =
		order === 'start'
			? bookBeginningSegment
			: order === 'middle'
				? bookMiddleSegment
				: bookEndSegment;
</script>

<a href="/book/{book.title}">
	<Dropdown style="position: absolute; --index: {index * 2.25}rem; left: var(--index);">
		<pre slot="drop-btn" class="book-shelf" style:--index="{0}rem">{display(book.title)}</pre>
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
