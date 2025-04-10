<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		style,
		clamp = false,
		align = 'left',
		label,
		children
	}: {
		style?: string;
		clamp?: boolean;
		align?: 'left' | 'center';
		label: Snippet;
		children: Snippet;
	} = $props();
</script>

<div class="drop-container" {style}>
	{@render label?.()}
	<div class="drop-content" class:clamp class:align>
		<div class="drop-content-inner" class:clamp>
			{@render children?.()}
		</div>
	</div>
</div>

<style>
	.drop-container {
		display: flex;
		flex-direction: row;
		position: relative;
	}

	.drop-container:hover > .drop-content,
	.drop-container:focus-within > .drop-content {
		display: block;
		position: absolute;
		z-index: 120;
		top: 100%;
		background-color: var(--bg-color);
		padding: 0.25rem;
	}

	.drop-content:global(.center) {
		left: 50%;
		transform: translate(-50%);
	}

	.drop-content-inner {
		display: block;
		width: max-content;
		border: 2px solid var(--color-inactive);
		padding: 0.5rem;
	}

	.clamp {
		max-width: 100%;
	}

	.drop-content-inner:hover {
		border-color: var(--color);
	}

	.drop-content {
		display: none;
	}
</style>
