<script lang="ts">
	import '$styles/remedy.css';
	import '$styles/app.css';
	import '$styles/book.css';
	import { mainLabel } from '$lib/stores';
</script>

<div id="root">
	<div class="uhuh">
		<nav class="container nav" style:--label="'book-buddy'">
			<a href="/" class="btn lg">HOME</a>
		</nav>
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
	}

	nav.container.nav {
		flex-grow: 1;
		height: min-content;
		width: 0;
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
