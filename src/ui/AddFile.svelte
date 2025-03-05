<script lang="ts">
	import { addFile } from '$lib';
	import { canvasToText, createImageCanvas } from '$lib/braille';
	import { books } from '$lib/stores.svelte';
	import { open } from '@tauri-apps/api/dialog';
	import { convertFileSrc } from '@tauri-apps/api/tauri';

	const getBooks = async () => {
		const files = await open({
			multiple: true,
			filters: [
				{
					extensions: ['epub'],
					name: 'Supported formats: epub'
				}
			]
		});
		if (!files) return;
		else if (Array.isArray(files)) {
			for (let file of files) {
				const fileSrc = convertFileSrc(file);
				const book = await addFile(fileSrc);
				console.log(book);

				const coverUrl = await book?.getCover();
				console.log(coverUrl);

				let cover = '';
				if (coverUrl) {
					const canvas = await createImageCanvas(coverUrl);
					// settings.last_canvas = canvas;
					// settings.last_dithering = null;
					cover = canvasToText(canvas);
				}
				const title = book?.metadata?.title;
				console.log(title);

				if (title === undefined) {
					// TODO: error toast
					continue;
				}
				$books[title] = {
					title,
					src: fileSrc,
					// coverUrl,
					// author
					cover
				};

				// $books = $books;
				// localStorage.setItem(title, file);
			}
		}
	};
</script>

<button class="btn" onclick={getBooks}>Add books</button>
