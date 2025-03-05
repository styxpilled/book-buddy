<script lang="ts">
	import type { BookData } from '$lib/stores.svelte';
	import { fallback } from '$lib/ui';

	interface Props {
		book: BookData;
	}

	let { book }: Props = $props();
</script>

<a href="/book/{book.title}" class="cover-anchor">
	<li class="container interactable nomar" style:--label="'{book.title}'">
		<div class="spacer">
			<div class="cover">
				<pre class="magic">{book.cover || fallback}</pre>
				<pre class="cover-1">{book.cover || fallback}</pre>
				<pre class="cover-2">{book.cover || fallback}</pre>
			</div>
		</div>
	</li>
</a>

<style>
	.magic {
		visibility: hidden;
	}

	.cover-anchor {
		width: max-content;
		height: max-content;
	}

	.spacer {
		display: flex;
		justify-content: center;
		align-items: center;
		width: 25rem;
		height: 34rem;
	}

	div.cover {
		width: max-content;
		height: max-content;
		padding: 0.5rem;
		line-height: 0.7rem;
		position: relative;
		overflow: hidden;
	}

	pre {
		display: block;
		font-size: 0.5rem;
		user-select: none;
	}

	.cover-1,
	.cover-2 {
		position: absolute;
	}

	.cover-1 {
		animation: scroll-1 5s linear infinite paused forwards;
	}

	.cover-2 {
		animation: scroll-2 5s linear infinite paused forwards;
	}

	.cover-anchor:hover .cover-1,
	.cover-anchor:hover .cover-2 {
		animation-play-state: running;
	}

	@keyframes scroll-1 {
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
