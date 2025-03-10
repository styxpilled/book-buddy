<script lang="ts">
	import { page } from '$app/stores';
	import { mainLabel } from '$lib/stores.svelte';
	import Reader from '$ui/Reader.svelte';
	import { onMount } from 'svelte';

	$mainLabel.mode = 'manual';
	let id = decodeURIComponent($page.params.id);

	// TODO: don't store books in localstorage, wrap them in an object
	onMount(() => {
		// $currentBook = JSON.parse(localStorage.getItem(id) || 'null');
		return () => {
			// $currentBook = null;
		};
	});
</script>

<Reader {id} />
