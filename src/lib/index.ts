import epub from 'epubjs';

export const SUPPORTED_FORMATS = ['epub'];

// place files you want to import through the `$lib` alias in this folder.
export const addFile = async (file: string) => {
	const filetype = file.split('.').slice(-1)[0];
	if (!SUPPORTED_FORMATS.includes(filetype)) {
		throw 'Unsupported Filetype: ' + file.split('/').slice(-1)[0];
	}
	return epub(file);
};
