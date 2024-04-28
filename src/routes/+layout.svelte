<script lang="ts">
	import '$styles/remedy.css';
	import '$styles/app.css';
	import '$styles/book.css';
	import { mainLabel } from '$lib/stores';
	import Dropdown from '$ui/Dropdown.svelte';
	import Icon from '$ui/Icon.svelte';
</script>

<div id="root">
	<div class="uhuh">
		<nav class="container nav" style:--label="'book-buddy'">
			<a href="/" class="btn lg">HOME</a>
			<Dropdown>
				<button slot="drop-btn" class="btn lg upper"> Recently read </button>
				<ul>
					<li>book 1</li>
					<li>book 2</li>
					<li>book 3</li>
				</ul>
			</Dropdown>
			<a href="/design" class="btn lg">DESIGN</a>
		</nav>
		<div class="container row lg" style:--label="'search'">
			<span class="caret"></span>
			<input type="text" />
		</div>
		<div class="row" style="font-size: 0.5rem;">
			<!-- <Icon />
			<Icon />
			<Icon />
			<Icon /> -->
		</div>
	</div>
	<div
		id="main"
		class="container"
		class:manual={$mainLabel.mode === 'manual'}
		style:--label="'{$mainLabel.value}'"
	>
		<slot />
	</div>
</div>

<style>
	#root {
		width: auto;
		display: inline-block;
		flex-direction: column;
	}

	#main {
		width: max-content;
		min-width: max(50rem, 40%);
		min-height: 10rem;
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
		/* padding: 0.5rem 0; */
		/* max-width: 50rem; */
		/* width: ; */
		/* display: flex; */
		/* width: 100%; */
		/* width: max(50rem, 40%); */
	}
	/* #main {
		font-family: monospace;
		background-color: #181818;
	} */
</style>
