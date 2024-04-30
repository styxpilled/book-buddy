<script lang="ts">
	import type { BookData } from '$lib/stores';
	import ShelfBook from './ShelfBook.svelte';

	export let row: BookData[];
</script>

<ul>
	{#each row as book, i}
		<li>
			<ShelfBook {book} index={i} end={i + 1 === row.length} />
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
