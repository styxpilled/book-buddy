<script lang="ts">
	import '$styles/remedy.css';
	import '$styles/app.css';
	import '$styles/inputs.css';
	import '@fontsource/geist-mono';
	// import '$styles/book.css';
	import { currentBook, mainLabel, recentBooks } from '$lib/stores.svelte';
	import Dropdown from '$ui/Dropdown.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	interface Props {
		children?: import('svelte').Snippet;
	}

	let { children }: Props = $props();

	onMount(() => {
		if ($currentBook !== null) {
			goto(`/book/${$currentBook.title}`);
		}
	});
</script>

<div id="root">
	<div class="uhuh">
		<nav class="container nav" style:--label="'book-buddy'">
			<a href="/" class="btn lg">HOME</a>
			<Dropdown>
				{#snippet label()}
					<button class="btn lg upper">Recently read</button>
				{/snippet}
				<ul>
					{#each $recentBooks.reverse() as recent}
						<li><a href="/book/{recent}">{recent}</a></li>
					{/each}
				</ul>
			</Dropdown>
			<a href="/design" class="btn lg">DESIGN</a>
			<a href="/settings" class="btn lg">SETTINGS</a>
		</nav>
		<div class="container row lg" style:--label="'search'">
			<span class="caret"></span>
			<input type="text" />
		</div>
	</div>
	<div
		id="main"
		class="container"
		class:manual={$mainLabel.mode === 'manual'}
		style:--label="'{$mainLabel.value}'"
	>
		{@render children?.()}
	</div>
</div>

<style>
	#root {
		/* width: auto; */
		/* display: inline-block; */
		flex-direction: column;
	}

	#main {
	}

	.uhuh {
		display: flex;
		justify-content: space-between;
	}

	nav.container.nav {
		flex-grow: 1;
		height: min-content;
		width: 0;
		display: flex;
	}
</style>
