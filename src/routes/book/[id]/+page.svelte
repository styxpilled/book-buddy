<script lang="ts">
	import { page } from '$app/stores';
	import { mainLabel } from '$lib/stores';
	import Reader from '$ui/Reader.svelte';

	$mainLabel.mode = 'manual';
</script>

<Reader id={$page.params.id} />
