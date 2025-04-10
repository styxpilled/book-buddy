<script lang="ts">
	import type { BookData } from '$lib/stores.svelte';
	import { fallback } from '$lib/ui';

	interface Props {
		book: BookData;
	}

	let { book }: Props = $props();
</script>

<a href="/book/{book.metadata.identifier}" class="cover-anchor">
	<li class="container interactable nomar" style:--label="'{book.title}'">
		<div class="spacer" style="--steps: {((book.cover || fallback).match(/\n/g) || []).length}">
			<div class="cover">
				<pre class="magic">{book.cover || fallback}</pre>
				<pre class="cover-1">{book.cover || fallback}</pre>
				<!-- <pre class="cover-2">{book.cover || fallback}</pre> -->
			</div>
		</div>
	</li>
</a>

<style>
	.magic {
		visibility: hidden;
		color: transparent;
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
		overflow: hidden;
	}

	div.cover {
		width: max-content;
		height: max-content;
		padding: 0.5rem;
		/* line-height: 1; */
		position: relative;
		overflow: hidden;
	}

	.cover > pre {
		display: block;
		font-size: 0.5rem;
		user-select: none;
		font-family: 'Commit Mono';
		letter-spacing: 1.25px;
	}

	.cover-1,
	.cover-2 {
		position: absolute;
	}

	.cover-1 {
		top: 0;
		filter: brightness(2);
		/* animation: scroll-1 5s steps(var(--steps)) infinite paused forwards; */
	}

	.cover-anchor:hover > .container {
		border-style: dashed;
		animation: pulse-border 1.5s linear infinite;
	}

	.cover-anchor:hover > .container::before {
		animation: pulse-color 1.5s linear infinite;
	}

	.cover-2 {
		animation: pulse 1.5s linear infinite;
		/* animation: scroll-2 5s steps(var(--steps)) infinite paused forwards; */
	}

	@keyframes p-br {
		0% {
			border-style: dashed;
		}
		50% {
			border-style: solid;
		}
		100% {
			border-style: dashed;
		}
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
