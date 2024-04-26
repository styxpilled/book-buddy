<script lang="ts">
	import { books, mainLabel } from '$lib/stores';
	import AddFile from '$ui/AddFile.svelte';

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
</div>

<ul>
	{#each $books as book}
		<li>
			<a href="/book/{book.title}" class="cover-anchor">
				<div class="container cover interactable" style:--label="'{book.title}'">
					<pre>{book.cover}</pre>
				</div>
			</a>
		</li>
	{/each}
</ul>

<style>
	ul {
		display: flex;
	}

	.cover-anchor {
		width: max-content;
		height: max-content;
	}

	div.cover {
		width: max-content;
		height: max-content;
		padding: 0.5rem;
	}

	pre {
		display: block;
		height: max-content;
		width: max-content;
		font-size: 0.5rem;
	}
</style>
