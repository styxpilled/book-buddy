<script lang="ts">
	import { books } from '$lib/stores';
	import AddFile from '$ui/AddFile.svelte';
</script>

<h1>Welcome to SvelteKit</h1>

<div class="row">
	<button class="btn">Test No Style</button>
	<button class="btn sm">Test No Style</button>
	<button class="btn md">Test No Style</button>
	<button class="btn lg">Test No Style</button>
</div>

<AddFile />
{#each $books as book}
	<h3>
		<a class="btn lg" href="/book/{book}">{book}</a>
	</h3>
{/each}
