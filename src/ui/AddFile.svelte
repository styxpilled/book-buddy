<script lang="ts">
	import { addFile } from '$lib';
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
				// TODO: book cover
				// coverurl gives a blob url, covert that into braille
				// const coverUrl = await book.coverUrl();
				const title = book?.packaging?.metadata?.title;
				if (title === undefined) {
					// TODO: error toast
					continue;
				}
				$books.push(title);
				$books = $books;
				localStorage.setItem(title, file);
			}
		}
	};
</script>

<button class="btn" on:click={getBooks}>add book</button>
