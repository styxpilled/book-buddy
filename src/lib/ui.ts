export function* chunks<T>(arr: T[], n: number): Generator<T[], void> {
	for (let i = 0; i < arr.length; i += n) {
		yield arr.slice(i, i + n);
	}
}

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

export const bookBeginningSegment = (input: string) => {
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

export const bookMiddleSegment = (input: string) => {
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

export const bookEndSegment = (input: string) => {
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
