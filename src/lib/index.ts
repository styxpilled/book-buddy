// import epub from 'epubjs/src/index';

// import { EPUB } from './foliate-js/epub';
import { makeBook } from './foliate-js/view';

export const SUPPORTED_FORMATS = ['epub'];

export const percentFormat = new Intl.NumberFormat('en', { style: 'percent' });

export const addFile = async (file: string) => {
	return await makeBook(file);
};
