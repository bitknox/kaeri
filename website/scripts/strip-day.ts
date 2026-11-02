/**
 * The light theme's pixel strip, made from the dusk one: public/scene-strip.png and
 * public/scene-strip-bare.png are recoloured pixel for pixel into public/scene-strip-day.png and
 * public/scene-strip-day-bare.png, so the buildings, the roofline and the rooftop stay exactly
 * where they are and Nagare's spot and run path still fit.
 *
 *   bun scripts/strip-day.ts            write them
 *   bun scripts/strip-day.ts --check    write nothing; fail when one is not what the dusk strips make
 *
 * Every pixel of the bare strip is one of the scene's colours, dimmed a little for grain (see the
 * skyline in src/game/run.ts): the dusk sky, a star, a far block, a near tower, a lit window in
 * amber or in water, the railing, the roof's edge or the roof. Each is given its daytime colour
 * with the same grain: a pale sky that starts as the light band above it, hazy blocks, panes that
 * only catch the light, and a roof that ends in the light footer's colour. Nagare's pixels in the
 * baked strip (her standing art from src/game/sprites.ts) are copied over as they are.
 */
import sharp from 'sharp';
import { join } from 'node:path';
import { NAGARE } from '../src/game/sprites';

type RGB = [number, number, number];

const PUBLIC = join(import.meta.dir, '..', 'public');
const W = 480;
const H = 44;
/** Nagare's top left in the baked strip. */
const BAKED_AT = { x: 331, y: 9 };

// The dusk scene's colours, as run.ts draws them.
const FAR: RGB = [38, 44, 78];
const WINDOW_OFF: RGB = [21, 24, 46];
const AMBER: RGB = [242, 166, 75];
const WATER: RGB = [127, 199, 217];
const RAILING: RGB = [15, 17, 32];
const EDGE: RGB = [52, 58, 92];
const FLOOR: RGB = [31, 35, 62];
const GROUND: RGB = [22, 26, 46];

// The day's colours, from the light theme in src/styles/site.css.
const DAY = {
	/**
	 * The sky starts as the band above the strip on the home page (--ani-band, #e3e1d9) and turns a
	 * pale blue at the horizon. The top is a shade over the band, so with its grain it averages it.
	 */
	skyTop: [230, 228, 219] as RGB,
	skyLow: [198, 215, 231] as RGB,
	/** The row the sky reaches its horizon colour, behind the blocks. */
	skyRows: 11,
	far: [196, 202, 220] as RGB,
	near: [150, 157, 186] as RGB,
	/** What an amber window and a water window become: glass catching a little warmth, or the sky. */
	warm: [236, 214, 182] as RGB,
	glass: [212, 226, 238] as RGB,
	railing: [94, 100, 130] as RGB,
	edge: [244, 242, 236] as RGB,
	floor: [196, 199, 212] as RGB,
	/** The roof's last rows, the footer's colour (--ani-foot), so the two meet without a seam. */
	ground: [227, 225, 217] as RGB,
};
/** How much of the dusk's grain the day keeps: pale colours show it more. */
const GRAIN = 0.55;

const mix = (a: RGB, b: RGB, k: number): RGB => [
	a[0] + (b[0] - a[0]) * k,
	a[1] + (b[1] - a[1]) * k,
	a[2] + (b[2] - a[2]) * k,
];
const scale = (c: RGB, k: number): RGB => [c[0] * k, c[1] * k, c[2] * k];
const sum = (c: RGB) => c[0] + c[1] + c[2];
const grain = (k: number) => 1 - (1 - Math.min(1, k)) * GRAIN;

/** How much `base` was dimmed to give `p`, or -1 when `p` is not `base` dimmed within `lo`..`hi`. */
function dimmed(p: RGB, base: RGB, lo: number, hi: number): number {
	const k = sum(p) / sum(base);
	if (k < lo - 0.01 || k > hi + 0.01) return -1;
	return base.every((c, i) => Math.abs(c * k - p[i]) <= 1.6) ? k : -1;
}

function daySky(y: number): RGB {
	const t = Math.min(1, y / DAY.skyRows);
	return mix(DAY.skyTop, DAY.skyLow, t * t * (3 - 2 * t));
}

async function read(name: string) {
	const { data, info } = await sharp(join(PUBLIC, name)).raw().toBuffer({ resolveWithObject: true });
	if (info.width !== W || info.height !== H) throw new Error(`${name} is ${info.width}×${info.height}, not ${W}×${H}`);
	const px = (x: number, y: number): RGB => {
		const i = (y * W + x) * info.channels;
		return [data[i], data[i + 1], data[i + 2]];
	};
	return px;
}

/** The bare strip by day. */
function day(px: (x: number, y: number) => RGB): Uint8Array {
	const out = new Uint8Array(W * H * 3);
	for (let y = 0; y < H; y++) {
		// The sky's colour undimmed in this row: near the top of what the row holds, past the stars.
		const sums: number[] = [];
		const sky: boolean[] = [];
		for (let x = 0; x < W; x++) {
			const p = px(x, y);
			const isSky =
				p[0] > p[1] &&
				dimmed(p, AMBER, 0.44, 0.96) < 0 &&
				dimmed(p, WATER, 0.44, 0.96) < 0 &&
				y < 31;
			sky.push(isSky);
			if (isSky) sums.push(sum(p));
		}
		sums.sort((a, b) => a - b);
		const skyRef = sums.length ? sums[Math.floor(sums.length * 0.9)] : 0;
		for (let x = 0; x < W; x++) {
			const p = px(x, y);
			let c: RGB;
			let k: number;
			if (p.every((v, i) => v === RAILING[i])) c = DAY.railing;
			else if ((k = dimmed(p, FAR, 0.95, 1)) >= 0) c = scale(DAY.far, grain(k));
			else if ((k = dimmed(p, WINDOW_OFF, 0.94, 1)) >= 0) c = scale(DAY.near, grain(k));
			else if ((k = dimmed(p, AMBER, 0.44, 0.96)) >= 0) c = mix(DAY.near, DAY.warm, 0.3 + 0.5 * (k - 0.45));
			else if ((k = dimmed(p, WATER, 0.44, 0.96)) >= 0) c = mix(DAY.near, DAY.glass, 0.4 + 0.5 * (k - 0.45));
			else if ((k = dimmed(p, EDGE, 0.93, 1)) >= 0) c = scale(DAY.edge, grain(k));
			else if (y >= 31) {
				// The roof: dusk's floor fading to its ground, with grain.
				const t = (sum(p) - sum(GROUND)) / (sum(FLOOR) - sum(GROUND));
				c = mix(DAY.ground, DAY.floor, Math.max(0, Math.min(1.2, t)));
			} else if (sky[x]) {
				// A star is the sky undimmed; the rest keep their grain.
				const k = skyRef ? sum(p) / skyRef : 1;
				c = scale(daySky(y), grain(k));
			} else throw new Error(`pixel ${x},${y} (${p}) is none of the scene's colours`);
			out.set(c.map(Math.round), (y * W + x) * 3);
		}
	}
	return out;
}

const png = (raw: Uint8Array) =>
	sharp(Buffer.from(raw), { raw: { width: W, height: H, channels: 3 } })
		.png({ compressionLevel: 9, palette: false })
		.toBuffer();

const bare = await read('scene-strip-bare.png');
const baked = await read('scene-strip.png');
const dayBare = day(bare);
// Nagare: her pixels in the baked strip, where she stands in it, copied as they are. The pixels
// around her are the bare strip's, by day.
const dayBaked = dayBare.slice();
const art = NAGARE.stand;
for (let j = 0; j < art.h; j++) {
	for (let i = 0; i < art.w; i++) {
		// The baked strip has her facing left, the art turned as the strip's canvas turns it.
		const c = art.px[j * art.w + (art.w - 1 - i)];
		if (!c) continue;
		const x = BAKED_AT.x + i;
		const y = BAKED_AT.y + j;
		const b = baked(x, y);
		if (b.some((v, k) => Math.abs(v - c[k]) > 2)) throw new Error(`Nagare is not at ${BAKED_AT.x},${BAKED_AT.y} in scene-strip.png`);
		dayBaked.set(b, (y * W + x) * 3);
	}
}

const outputs: [string, Uint8Array][] = [
	['scene-strip-day-bare.png', dayBare],
	['scene-strip-day.png', dayBaked],
];
const check = process.argv.includes('--check');
let stale = false;
for (const [name, raw] of outputs) {
	const path = join(PUBLIC, name);
	if (check) {
		const file = Bun.file(path);
		const have = (await file.exists()) ? await sharp(path).raw().toBuffer() : null;
		if (!have || Buffer.compare(have, Buffer.from(raw)) !== 0) {
			console.error(`${name} is not what the dusk strips make; run bun scripts/strip-day.ts`);
			stale = true;
		}
	} else {
		await Bun.write(path, await png(raw));
		console.log(`wrote public/${name}`);
	}
}
if (stale) process.exit(1);
