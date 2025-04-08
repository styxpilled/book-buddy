<script lang="ts">
	import '$styles/remedy.css';
	import '$styles/app.css';
	import '$styles/inputs.css';
	import '$styles/animations.css';
	import '@fontsource/geist-mono';
	import '@fontsource/commit-mono';
	// import '$styles/book.css';
	import { currentBook, mainLabel, recentBooks } from '$lib/stores.svelte';
	import Dropdown from '$ui/Dropdown.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { appWindow } from '@tauri-apps/api/window';
	import { labels } from '$lib/labels.svelte';
	import { listen } from '@tauri-apps/api/event';
	import { addBooks } from '$lib';
	import CommandBar from '$ui/CommandBar.svelte';

	interface Props {
		children?: import('svelte').Snippet;
	}
	addBooks;

	let { children }: Props = $props();
	let pulse = $state(false);

	onMount(() => {
		if (currentBook !== null) {
			// goto(`/book/${currentBook.title}`);
		}
	});

	listen('tauri://file-drop-hover', () => {
		$mainLabel.value = 'adding-books';
		pulse = true;
	});

	listen('tauri://file-drop-cancelled', () => {
		$mainLabel.value = '';
		pulse = false;
	});

	listen('tauri://file-drop', (event) => {
		if (Array.isArray(event.payload)) {
			addBooks(event.payload);
		}
	});
</script>

<div data-tauri-drag-region class="titlebar">
	<nav
		data-tauri-drag-region
		class="container nav low"
		class:pulse
		style:--label="'{$mainLabel.value || 'book-buddy'}'"
	>
		<div class="row">
			<a href="/" class="btn lg">HOME</a>
			<Dropdown>
				{#snippet label()}
					<button class="btn lg upper">Recently read</button>
				{/snippet}
				<ul>
					<!-- {#each $recentBooks.reverse() as recent}
            <li><a href="/book/{recent}">{recent}</a></li>
          {/each} -->
				</ul>
			</Dropdown>
			<a href="/design" class="btn lg">DESIGN</a>
			<a href="/settings" class="btn lg">SETTINGS</a>
			<a href="/help" class="btn lg">HELP</a>
		</div>
		<CommandBar />
		<div class="dummy"></div>
		<div class="label right low">
			<button class="btn" id="titlebar-minimize" onclick={() => appWindow.minimize()}>
				minimize
			</button>
			<button class="btn" id="titlebar-maximize" onclick={() => appWindow.toggleMaximize()}>
				maximize
			</button>
			<button class="btn" id="titlebar-close" onclick={() => appWindow.close()}> close </button>
		</div>
	</nav>
</div>
<div
	id="root"
	class="container pad"
	class:pulse
	class:manual={$mainLabel.mode === 'manual'}
	style:--label="'{$mainLabel.value}'"
>
	{@render labels.root?.()}
	<div id="main" class="proxy">
		{@render children?.()}
	</div>
</div>
<div></div>

<style>
	.titlebar {
		padding-top: 0.375rem;
		background-color: var(--bg-color);
		user-select: none;
		display: flex;
		justify-content: flex-end;
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		z-index: 50;
	}

	#root {
		height: calc(100vh - 5.25rem);
		flex-direction: column;
		margin-top: 4.85rem;
		z-index: 100;
	}

	#main {
		overflow-y: scroll;
		overflow-x: hidden;
		height: calc(100vh - 7.5rem);
	}

	nav.container.nav {
		flex-grow: 1;
		/* width: 100%; */
		height: min-content;
		/* width: 0; */
		display: flex;
		/* justify-content: space-between; */
		/* border-bottom: none; */
	}
</style>
