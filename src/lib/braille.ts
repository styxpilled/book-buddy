export const WIDTH = 64;
export const HEIGHT = 64;
export const GRAYSCALE_MODE: 'luminance' | 'lightness' | 'average' | 'value' = 'luminance';

export function createImageCanvas(blob: Blob | string): Promise<HTMLCanvasElement> {
	return new Promise((resolve, reject) => {
		const canvas = document.createElement('CANVAS') as HTMLCanvasElement;
		const image = new Image();

		image.onload = () => {
			let width = image.width;
			let height = image.height;
			if (image.width != HEIGHT * 2) {
				width = HEIGHT * 2;
				height = (width * image.height) / image.width;
			}

			//nearest multiple
			canvas.width = width - (width % 2);
			canvas.height = height - (height % 4);

			const ctx = canvas.getContext('2d');
			if (ctx === null) {
				reject(`Couldn't create canvas!`);
				return;
			}
			ctx.fillStyle = '#FFFFFF'; //get rid of alpha
			ctx.fillRect(0, 0, canvas.width, canvas.height);

			// @ts-expect-error no it exists actually
			ctx.mozImageSmoothingEnabled = false;
			// @ts-expect-error no it exists actually
			ctx.webkitImageSmoothingEnabled = false;
			// @ts-expect-error no it exists actually
			ctx.msImageSmoothingEnabled = false;
			ctx.imageSmoothingEnabled = false;

			ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
			resolve(canvas);
		};
		if (typeof blob === 'string') {
			image.src = blob;
		} else {
			image.src = URL.createObjectURL(blob);
		}
	});
}

function pixelsToCharacter(pixels_lo_hi: number[]) {
	//expects an array of 8 bools
	//Codepoint reference - https://www.ssec.wisc.edu/~tomw/java/unicode.html#x2800
	const shift_values = [0, 1, 2, 6, 3, 4, 5, 7]; //correspond to dots in braille chars compared to the given array
	let codepoint_offset = 0;
	for (const i in pixels_lo_hi) {
		codepoint_offset += +pixels_lo_hi[i] << shift_values[i];
	}

	//  && settings.monospace === false
	if (codepoint_offset === 0) {
		//pixels were all blank
		// codepoint_offset = 4;
		//0x2800 is a blank braille char, 0x2804 is a single dot
	}
	return String.fromCharCode(0x2800 + codepoint_offset);
}

export function toGreyscale(r: number, g: number, b: number) {
	switch (GRAYSCALE_MODE) {
		case 'luminance':
			return 0.22 * r + 0.72 * g + 0.06 * b;

		case 'lightness':
			return (Math.max(r, g, b) + Math.min(r, g, b)) / 2;

		case 'average':
			return (r + g + b) / 3;

		case 'value':
			return Math.max(r, g, b);

		default:
			console.error('Greyscale mode is not valid');
			return 0;
	}
}

export function canvasToText(canvas: HTMLCanvasElement) {
	const ctx = canvas.getContext('2d') as CanvasRenderingContext2D;
	const width = canvas.width;
	const height = canvas.height;

	let image_data = [];
	// if (settings.dithering) {
	// 	if (settings.last_dithering === null || settings.last_dithering.canvas !== canvas) {
	// 		settings.last_dithering = new Dithering(canvas);
	// 	}
	// 	image_data = settings.last_dithering.image_data;
	// } else {
	// @ts-expect-error yeah it's fine actually don't worry about it
	image_data = new Uint8Array(ctx.getImageData(0, 0, width, height).data.buffer);
	// }
	let output = '';

	for (let imgy = 0; imgy < height; imgy += 4) {
		for (let imgx = 0; imgx < width; imgx += 2) {
			const braille_info = [0, 0, 0, 0, 0, 0, 0, 0];
			let dot_index = 0;
			for (let x = 0; x < 2; x++) {
				for (let y = 0; y < 4; y++) {
					const index = (imgx + x + width * (imgy + y)) * 4;
					const pixel_data = image_data.slice(index, index + 4);
					//ctx.getImageData(imgx+x,imgy+y,1,1).data
					if (pixel_data[3] >= 128) {
						//account for alpha
						const grey = toGreyscale(pixel_data[0], pixel_data[1], pixel_data[2]);
						// if (settings.inverted) {
						// if (grey >= 128) braille_info[dot_index] = 1;
						// } else {
						if (grey <= 128) braille_info[dot_index] = 1;
						// }
					}
					dot_index++;
				}
			}
			output += pixelsToCharacter(braille_info);
		}
		output += '\n';
	}

	return output;
}
