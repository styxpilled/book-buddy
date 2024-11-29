<script lang="ts">
	import type { BookData } from '$lib/stores.svelte';
	import ShelfBook from './ShelfBook.svelte';

	interface Props {
		row: BookData[];
	}

	let { row }: Props = $props();
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
