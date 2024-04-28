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

// export const bookBeginningSegmentDisplay = `   ,   ,
//   /////
//  /////
// |~~~|
// |===|
// |j  |
// | g |
// |  s|
// |===|
// '---'`;

const getLettersFromTitle = (input: string) => {
	let letters = [];
	const googa = input.split(' ');
	letters = googa.map((word) =>
		word
			.substring(0, Math.floor(9 / googa.length))
			.toUpperCase()
			.split('')
	);
	letters = letters.flat();
	return letters;
};

export const bookBeginningSegmentDisplay = (input: string) => {
	const l = getLettersFromTitle(input);
	return `   ,   ,
  /////
 /////
|~~~|
|===|
|${l[0]}${l[1]}${l[2]}|
|${l?.[3] || ' '}${l?.[4] || ' '}${l?.[5] || ' '}|
|${l?.[6] || ' '}${l?.[7] || ' '}${l?.[8] || ' '}|
|===|
'---'`;
};

export const bookMiddleSegmentDisplay = (input: string) => {
	const l = getLettersFromTitle(input);
	return `   ,   ,
  /////
 /////
|~~~|
|===|
|${l?.[0] || ' '}${l?.[1] || ' '}${l?.[2] || ' '}|
|${l?.[3] || ' '}${l?.[4] || ' '}${l?.[5] || ' '}|
|${l?.[6] || ' '}${l?.[7] || ' '}${l?.[8] || ' '}|
|===|
'---'`;
};

// export const bookMiddleSegmentDisplay = `   ,   ,
//   /////
//  /////
// |~~~|
// |===|
// |j  |
// | g |
// |  s|
// |===|
// '---'`;

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

export const bookEndSegmentDisplay = (input: string) => {
	const l = getLettersFromTitle(input);

	return `   ,   ,
  //// |
 ////  |
|~~~|  |
|===|  |
|${l?.[0] || ' '}${l?.[1] || ' '}${l?.[2] || ' '}|  |
|${l?.[3] || ' '}${l?.[4] || ' '}${l?.[5] || ' '}|  |
|${l?.[6] || ' '}${l?.[7] || ' '}${l?.[8] || ' '}| /
|===|/
'---'`;
};

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
