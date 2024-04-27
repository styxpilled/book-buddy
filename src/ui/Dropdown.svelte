<div class="drop-container">
	<slot name="drop-btn" />
	<div class="drop-content">
		<div class="drop-content-inner">
			<slot />
		</div>
	</div>
</div>

<style>
	.drop-container {
		position: relative;
	}

	.drop-container:hover > .drop-content {
		display: block;
		position: absolute;
		z-index: 10;
		left: 0.25rem;
		background-color: var(--bg-color);
		padding: 0.5rem;
	}

	.drop-content-inner {
		border: 1px solid var(--color);
		padding: 0.5rem;
	}

	.drop-content {
		display: none;
	}
</style>
