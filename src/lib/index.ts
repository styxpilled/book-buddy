import epub from 'epubjs/src/index';

export const SUPPORTED_FORMATS = ['epub'];

export const addFile = async (file: string) => {
	const filetype = file.split('.').slice(-1)[0];
	if (!SUPPORTED_FORMATS.includes(filetype)) {
		throw 'Unsupported Filetype: ' + file.split('/').slice(-1)[0];
	}
	return epub(file);
};
