import type { BookData } from './stores';

export const bookBeginningSegment = `   ,   ,
  /////
 /////
|~~~|
|===|
|j  |
| g |
|  s|
|===|
'---'`;

export const bookMiddleSegment = `,   ,
/////
/////
|~~~|
|===|
|j  |
| g |
|  s|
|===|
'---'`;

const bookEndSegments = `,   ,
/////|
///// |
|~~~|  |
|===|  |
|j  |  |
| g |  |
|  s| /
|===|/
'---'`;

export const generateBookLibrary = (books: BookData[]) => {
	// let out = bookBeginningSegment;
	const agg: string[][] = [];

	let segments = bookBeginningSegment.split('\n');
	for (let index = 0; index < segments.length; index++) {
		agg.push([]);
	}
	for (let index = 0; index < segments.length; index++) {
		agg[index].push(segments[index]);
	}
	segments = bookMiddleSegment.split('\n');
	for (let index = 0; index < segments.length; index++) {
		agg[index].push(segments[index].repeat(books.length));
	}
	segments = bookEndSegments.split('\n');
	for (let index = 0; index < segments.length; index++) {
		agg[index].push(segments[index]);
	}
	// for (const segment of bookMiddleSegment.split('\n')) {
	// 	out.push(segment.repeat(books.length - 1));
	// 	out += '\n';
	// }
	// out += '\n';
	console.log(agg);
	let out = ``;

	for (const line of agg) {
		out += line.join('');
		out += '\n';
	}
	console.log(out);

	return out;
};
