import type { Snippet } from 'svelte';

export const labels = $state<{
	root?: Snippet;
}>({
	root: undefined
});
