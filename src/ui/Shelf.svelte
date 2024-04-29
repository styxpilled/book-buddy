<script lang="ts">
	import type { BookData } from '$lib/stores';
	import ShelfBook from './ShelfBook.svelte';

	export let row: BookData[];
</script>

<ul>
	{#each row as book, i}
		<li>
			<ShelfBook
				{book}
				index={i}
				order={i === 0 ? 'start' : i + 1 === row.length ? 'end' : 'middle'}
			/>
		</li>
	{/each}
</ul>

<style>
	ul {
		position: relative;
		display: block;
		height: 12rem;
	}
</style>
