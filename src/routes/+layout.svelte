<script lang="ts">
	import '$styles/remedy.css';
	import '$styles/app.css';
	import '$styles/book.css';
	import { mainLabel } from '$lib/stores';
</script>

<div id="root">
	<div class="container nav" style:--label="'book-buddy'">
		<nav>
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
	}

	#main {
		width: max-content;
		min-width: max(50rem, 75%);
		min-height: 10rem;
	}

	nav {
		padding: 0.5rem 0;
		width: max(50rem, 100%);
	}
	/* #main {
		font-family: monospace;
		background-color: #181818;
	} */
</style>
