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
		<li class="container interactable nomar" style:--label="'{book.title}'">
			<a href="/book/{book.title}" class="cover-anchor">
				<div class="cover">
					<pre class="magic">{book.cover}</pre>
					<pre class="cover-1">{book.cover}</pre>
					<pre class="cover-2">{book.cover}</pre>
					<!-- <pre>{book.cover}</pre> -->
				</div>
			</a>
		</li>
	{/each}
</ul>

<style>
	ul {
		display: flex;
		gap: 1rem;
		padding: 1rem 1rem 0.5rem;
	}

	.magic {
		visibility: hidden;
	}

	.cover-anchor {
		width: max-content;
		height: max-content;
	}

	div.cover {
		width: max-content;
		height: max-content;
		padding: 0.5rem;
		position: relative;
		overflow: hidden;
	}

	pre {
		display: block;
		height: max-content;
		width: max-content;
		font-size: 0.5rem;
	}

	pre:not(.magic).cover-1 {
		position: absolute;
		top: 0;
		animation: scroll-2 5s linear infinite;
		animation-fill-mode: forwards;
		animation-play-state: paused;
	}

	pre.cover-2 {
		position: absolute;
		animation: scroll 5s linear infinite;
		animation-fill-mode: forwards;
		animation-play-state: paused;
	}

	div.cover:hover > pre:not(.magic) {
		position: absolute;
		top: 0;

		animation-play-state: running;
	}

	div.cover:hover > pre.cover-2 {
		display: block;
		animation-play-state: running;
	}

	@keyframes scroll {
		0% {
			top: 0%;
		}
		100% {
			top: 100%;
		}
	}

	@keyframes scroll-2 {
		0% {
			top: -100%;
		}
		100% {
			top: 0%;
		}
	}
</style>
