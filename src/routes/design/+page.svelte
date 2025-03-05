<h1>Header 1</h1>
<h2>Header 2</h2>
<h3>Header 3</h3>
<h4>Header 4-6</h4>

<div class="row">
	<button class="btn">btn Sample Text</button>
	<button class="btn sm">btn sm Sample Text</button>
	<button class="btn md">btn md Sample Text</button>
	<button class="btn lg">btn lg Sample Text</button>
</div>

<div class="container" style:--label="'Inputs'">
	<div class="row" style="gap: 1rem;">
		<div class="row sm">
			<span class="caret"></span>
			<input type="text" />
		</div>
		<div class="row md">
			<span class="caret"></span>
			<input type="text" />
		</div>
		<div class="row lg">
			<span class="caret"></span>
			<input type="text" />
		</div>
	</div>
	<div class="row lg">
		<label class="switch">
			Switch
			<input type="checkbox" />
		</label>
	</div>
	<div class="row lg">
		<label class="range">
			<input type="range" min="5" max="40" step="5" />
			Range
		</label>
	</div>
</div>
<span class="loading">This is loading...</span>
<div class="blink block">Blink</div>
<div class="stroke block">Stroke</div>
<div class="block-in block">Block-in</div>

<div></div>

<style>
	.block {
		height: 4rem;
		width: 4rem;
	}
</style>
