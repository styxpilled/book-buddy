<script lang="ts">
	import { page } from '$app/stores';
	import Reader from '$ui/Reader.svelte';
</script>

<h1>hi {$page.params.id}</h1>
<Reader id={$page.params.id} />
