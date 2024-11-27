<script lang="ts">
	import { addFile } from '$lib';
	import { canvasToText, createImageCanvas } from '$lib/braille';
	import { books } from '$lib/stores';
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
				await book.ready;
				const coverUrl = await book.coverUrl();
				let cover = '';
				if (coverUrl) {
					const canvas = await createImageCanvas(coverUrl);
					// settings.last_canvas = canvas;
					// settings.last_dithering = null;
					cover = canvasToText(canvas);
				}
				const title = book?.packaging?.metadata?.title;
				if (title === undefined) {
					// TODO: error toast
					continue;
				}
				$books[title] = {
					title,
					src: fileSrc,
					coverUrl,
					// author
					cover
				};
				// $books = $books;
				// localStorage.setItem(title, file);
			}
		}
	};
</script>

<button class="btn" on:click={getBooks}>Add books</button>
