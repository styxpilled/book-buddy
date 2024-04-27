<script lang="ts">
	import { books, mainLabel } from '$lib/stores';
	import { generateBookLibrary } from '$lib/ui';
	import AddFile from '$ui/AddFile.svelte';
	import Cover from '$ui/Cover.svelte';
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

<ul>
	{#if settings.showCovers}
		{#each $books as book}
			<Cover {book} />
		{/each}
	{:else}
		<pre>{generateBookLibrary($books)}</pre>
		<!-- {#each $books as book}
			<Icon />
		{/each} -->
	{/if}
</ul>

<style>
	ul {
		display: flex;
		gap: 1rem;
		padding: 1rem 1rem 0.5rem;
	}
</style>
