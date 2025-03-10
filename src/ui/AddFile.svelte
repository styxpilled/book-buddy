<script lang="ts">
	import { addBooks } from '$lib';
	import { open } from '@tauri-apps/api/dialog';
</script>

<button
	class="btn"
	onclick={async () => {
		const files = await open({
			multiple: true,
			filters: [
				{
					extensions: ['epub'],
					name: 'Supported formats: epub'
				}
			]
		});
		addBooks(files);
	}}>Add books</button
>
