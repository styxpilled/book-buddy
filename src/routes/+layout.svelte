<script lang="ts">
	import '$styles/remedy.css';
	import '$styles/app.css';
	import '$styles/book.css';
</script>

<nav>
	<a href="/">home</a>
</nav>
<div id="main">
	<slot />
</div>

<style>
	/* #main {
		font-family: monospace;
		background-color: #181818;
	} */
</style>
