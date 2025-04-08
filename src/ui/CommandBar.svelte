<script lang="ts">
	import { commands } from '$lib/stores.svelte';
</script>

<div id="commandbar" class="container row center label outter" style:--label="'{commands.mode}'">
	<input
		bind:this={commands.commandbar}
		bind:value={commands.raw}
		placeholder="Search library, or append @ to search book contents"
		type="text"
	/>
</div>

<style>
	#commandbar {
		top: unset;
		bottom: 0.5ch;
		height: 110%;

		&:has(> input:focus-visible),
		&:has(> input:focus-visible)::before {
			color: var(--color);
			border-color: var(--color);
		}
	}

	input {
		width: 30rem;
		padding: 1rem;
		/* background-color: red; */
	}
</style>
