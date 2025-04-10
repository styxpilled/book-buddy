<script lang="ts">
	import { commands, l } from '$lib/stores.svelte';
	import Dropdown from './Dropdown.svelte';

	const keybindHandler = async (e: KeyboardEvent) => {
		const key = e.key.toLowerCase();
		const tagName = document.activeElement?.tagName.toLocaleLowerCase();
		if (
			(tagName === 'input' || tagName === 'textarea') &&
			document.activeElement?.id !== 'commandbar-input'
		)
			return;
		switch (key) {
			case 'p': {
				if (e.ctrlKey) {
					e.preventDefault();
					commands.commandbar.focus();
				}
				break;
			}
			case 'escape': {
				if (document.activeElement?.id === 'commandbar-input') {
					commands.commandbar.blur();
				}
				break;
			}
			default:
				break;
		}
	};
</script>

<svelte:window onkeydown={keybindHandler} />

<div
	id="commandbar-wrapper"
	style:--label="'{commands.mode}'"
	class="container row center label outter"
>
	<Dropdown align="center" clamp={true}>
		{#snippet label()}
			<div id="commandbar">
				<input
					bind:this={commands.commandbar}
					bind:value={commands.raw}
					placeholder="Search library, or append @ to search book contents"
					type="text"
					id="commandbar-input"
				/>
			</div>
		{/snippet}
		{#if commands.mode === 'search library'}
			<ul>
				{#each Object.keys(l.books)
					.filter((k) => l.books[k].title.toLowerCase().includes(commands.query))
					.sort()
					.slice(0, 10) as key}
					{@const title = l.books[key].title}
					{@const p1 = title.toLowerCase().indexOf(commands.query)}
					{@const p2 = p1 + commands.query.length}
					<li>
						<a href="/book/{key}">
							{title.substring(0, p1)}<span class="highlight">{title.substring(p1, p2)}</span
							>{title.substring(p2)}
						</a>
					</li>
				{:else}
					<li>
						<span>No matching results</span>
					</li>
				{/each}
			</ul>
		{/if}
	</Dropdown>
</div>

<style>
	#commandbar-wrapper {
		top: unset;
		bottom: 0.5ch;
		height: 110%;
	}

	input {
		width: 30rem;
		padding: 1rem;
	}

	ul {
		width: 30rem;
	}

	li {
		text-align: left;
		color: var(--color-inactive);

		&:hover {
			color: var(--color);
		}
	}

	a {
		display: inline-block;
		width: 100%;

		&:focus {
			color: var(--color);
			outline: none;
		}
	}

	.highlight {
		color: var(--color);
		font-weight: bold;
	}
</style>
