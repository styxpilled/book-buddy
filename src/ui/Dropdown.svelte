<!-- @migration-task Error while migrating Svelte code: This migration would change the name of a slot making the component unusable -->
<script lang="ts">
	export let style = '';
</script>

<div class="drop-container" {style}>
	<slot name="drop-btn" />
	<div class="uhuh">
		<div class="drop-content">
			<div class="drop-content-inner">
				<slot />
			</div>
		</div>
	</div>
</div>

<style>
	.drop-container {
		display: flex;
		flex-direction: row;
	}

	.uhuh {
		position: relative;
	}

	.drop-container:hover > .uhuh > .drop-content {
		display: block;
		position: absolute;
		z-index: 10;
		/* left: 0.25rem; */
		top: 0;
		background-color: var(--bg-color);
		padding: 0.5rem;
	}

	.drop-content-inner {
		display: block;
		width: max-content;
		border: 1px solid var(--color);
		padding: 0.5rem;
	}

	.drop-content {
		display: none;
	}
</style>
