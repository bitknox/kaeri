// A small FLAC encoder for decoded sound that goes to the TV's MediaSource as FLAC in MP4: 16-bit
// stereo, one frame per block of decoded sound (variable block size, the sample number in each
// header), each channel the cheapest of a constant or a fixed predictor of order 0 to 4, with
// Rice-coded residuals in up to 16 partitions. It runs in the decoder's worker.

/** The most samples in one frame; longer blocks are split. */
export const MAX_BLOCK = 4608;

/** "fLaC" and a STREAMINFO block (marked last) for 16-bit sound at `rate` with `channels`. */
export function streamInfo(rate: number, channels: number): Uint8Array {
	const b = new BitWriter(42);
	b.bits(0x664c6143, 32); // fLaC
	b.bits(0x80, 8); // STREAMINFO, last
	b.bits(34, 24);
	b.bits(16, 16); // least block size
	b.bits(65535, 16); // largest block size
	b.bits(0, 24); // frame sizes unknown
	b.bits(0, 24);
	b.bits(rate, 20);
	b.bits(channels - 1, 3);
	b.bits(15, 5); // 16 bits per sample
	b.bits(0, 4); // total samples unknown (36 bits)
	b.bits(0, 32);
	for (let i = 0; i < 4; i++) b.bits(0, 32); // no MD5
	return b.done();
}

/**
 * One FLAC frame for `frames` samples of interleaved float `pcm` with `channels` channels,
 * starting at sample number `first` of the stream.
 */
export function encodeFrame(pcm: Float32Array, frames: number, channels: number, first: number): Uint8Array {
	const b = new BitWriter(frames * channels * 2 + 64);
	b.bits(0x7ffc, 15); // sync (14 bits), reserved bit
	b.bits(1, 1); // variable block size: the header holds the sample number
	b.bits(frames <= 256 ? 6 : 7, 4); // block size at the end of the header, 8 or 16 bits
	b.bits(0, 4); // sample rate from STREAMINFO
	b.bits(channels - 1, 4); // independent channels
	b.bits(4, 3); // 16 bits per sample
	b.bits(0, 1);
	utf8(b, first);
	if (frames <= 256) b.bits(frames - 1, 8);
	else b.bits(frames - 1, 16);
	b.bits(crc8(b.buf, b.byteLength()), 8);
	const x = new Int32Array(frames);
	const r = new Int32Array(frames);
	for (let c = 0; c < channels; c++) {
		for (let i = 0; i < frames; i++) {
			const v = Math.round(pcm[i * channels + c] * 32767);
			x[i] = v > 32767 ? 32767 : v < -32768 ? -32768 : v;
		}
		subframe(b, x, r, frames);
	}
	b.align();
	const len = b.byteLength();
	b.bits(crc16(b.buf, len), 16);
	return b.done();
}

/** Writes one channel's subframe: constant when every sample is the same, else a fixed predictor. */
function subframe(b: BitWriter, x: Int32Array, r: Int32Array, n: number) {
	let same = true;
	for (let i = 1; i < n && same; i++) same = x[i] === x[0];
	if (same) {
		b.bits(0, 8); // constant
		b.bits(x[0] & 0xffff, 16);
		return;
	}
	const order = n < 5 ? 0 : bestOrder(x, n);
	residual(x, r, n, order);
	b.bits(0x10 | (order << 1), 8); // padding bit, fixed predictor of `order`, no wasted bits
	for (let i = 0; i < order; i++) b.bits(x[i] & 0xffff, 16);
	rice(b, r, n, order);
}

/** The fixed predictor order (0 to 4) with the least total residual, as libFLAC picks it. */
function bestOrder(x: Int32Array, n: number): number {
	let e0 = 0;
	let e1 = 0;
	let e2 = 0;
	let e3 = 0;
	let e4 = 0;
	let a = x[3];
	let d1 = x[3] - x[2];
	let d2 = d1 - (x[2] - x[1]);
	let d3 = d2 - (x[2] - x[1] - (x[1] - x[0]));
	for (let i = 4; i < n; i++) {
		const v = x[i];
		const n1 = v - a;
		const n2 = n1 - d1;
		const n3 = n2 - d2;
		const n4 = n3 - d3;
		e0 += v < 0 ? -v : v;
		e1 += n1 < 0 ? -n1 : n1;
		e2 += n2 < 0 ? -n2 : n2;
		e3 += n3 < 0 ? -n3 : n3;
		e4 += n4 < 0 ? -n4 : n4;
		a = v;
		d1 = n1;
		d2 = n2;
		d3 = n3;
	}
	const e = [e0, e1, e2, e3, e4];
	let best = 0;
	for (let o = 1; o < 5; o++) if (e[o] < e[best]) best = o;
	return best;
}

function residual(x: Int32Array, r: Int32Array, n: number, order: number) {
	for (let i = order; i < n; i++) {
		switch (order) {
			case 0:
				r[i] = x[i];
				break;
			case 1:
				r[i] = x[i] - x[i - 1];
				break;
			case 2:
				r[i] = x[i] - 2 * x[i - 1] + x[i - 2];
				break;
			case 3:
				r[i] = x[i] - 3 * x[i - 1] + 3 * x[i - 2] - x[i - 3];
				break;
			default:
				r[i] = x[i] - 4 * x[i - 1] + 6 * x[i - 2] - 4 * x[i - 3] + x[i - 4];
		}
	}
}

/** The highest Rice partition order tried (16 partitions). */
const MAX_PARTITION = 4;

/** Rice-codes residuals r[order..n) in the partition order that comes out shortest. */
function rice(b: BitWriter, r: Int32Array, n: number, order: number) {
	let top = 0;
	while (top < MAX_PARTITION && n % (2 << top) === 0 && (n >> (top + 1)) > order) top++;
	// Sums of the folded residuals per partition at the finest order.
	const parts = 1 << top;
	const size = n >> top;
	const sums = new Float64Array(parts);
	for (let p = 0, i = order; p < parts; p++) {
		const end = (p + 1) * size;
		let s = 0;
		for (; i < end; i++) {
			const v = r[i];
			s += v < 0 ? -2 * v - 1 : 2 * v;
		}
		sums[p] = s;
	}
	let bestBits = Infinity;
	let best = 0;
	let bestParams: number[] = [];
	for (let po = top; po >= 0; po--) {
		const count = 1 << po;
		const merge = 1 << (top - po);
		const params: number[] = [];
		let bits = 0;
		for (let p = 0; p < count; p++) {
			let s = 0;
			for (let j = 0; j < merge; j++) s += sums[p * merge + j];
			const m = (n >> po) - (p === 0 ? order : 0);
			const k = param(s, m);
			params.push(k);
			bits += 4 + m * (k + 1) + Math.floor(s / 2 ** k);
		}
		if (bits < bestBits) {
			bestBits = bits;
			best = po;
			bestParams = params;
		}
	}
	b.bits(0, 2); // 4-bit Rice parameters
	b.bits(best, 4);
	const count = 1 << best;
	for (let p = 0, i = order; p < count; p++) {
		const k = bestParams[p];
		b.bits(k, 4);
		const end = (p + 1) * (n >> best);
		for (; i < end; i++) {
			const v = r[i];
			const u = v < 0 ? -2 * v - 1 : 2 * v;
			b.zeros(u >>> k);
			b.bits(1, 1);
			if (k) b.bits(u & ((1 << k) - 1), k);
		}
	}
}

/** The Rice parameter for `m` values summing to `s`: about log2 of their mean, at most 14. */
function param(s: number, m: number): number {
	if (m <= 0 || s <= m) return 0;
	let k = 0;
	while (k < 14 && m * 2 ** (k + 1) < s) k++;
	return k;
}

/** The frame header's sample number, coded like UTF-8 (up to 36 bits). */
function utf8(b: BitWriter, v: number) {
	if (v < 0x80) {
		b.bits(v, 8);
		return;
	}
	let bytes = 2;
	while (bytes < 7 && v >= 2 ** (5 * bytes + 1)) bytes++;
	const lead = 0xff00 >> bytes;
	const top = Math.floor(v / 2 ** (6 * (bytes - 1)));
	b.bits((lead | top) & 0xff, 8);
	for (let i = bytes - 2; i >= 0; i--) b.bits(0x80 | (Math.floor(v / 2 ** (6 * i)) & 0x3f), 8);
}

const CRC8 = new Uint8Array(256);
const CRC16 = new Uint16Array(256);
for (let i = 0; i < 256; i++) {
	let c = i;
	for (let j = 0; j < 8; j++) c = c & 0x80 ? ((c << 1) ^ 0x07) & 0xff : (c << 1) & 0xff;
	CRC8[i] = c;
	let d = i << 8;
	for (let j = 0; j < 8; j++) d = d & 0x8000 ? ((d << 1) ^ 0x8005) & 0xffff : (d << 1) & 0xffff;
	CRC16[i] = d;
}

function crc8(buf: Uint8Array, len: number): number {
	let c = 0;
	for (let i = 0; i < len; i++) c = CRC8[c ^ buf[i]];
	return c;
}

function crc16(buf: Uint8Array, len: number): number {
	let c = 0;
	for (let i = 0; i < len; i++) c = ((c << 8) ^ CRC16[(c >> 8) ^ buf[i]]) & 0xffff;
	return c;
}

/** Writes bits most significant first. */
class BitWriter {
	buf: Uint8Array;
	private at = 0;
	private acc = 0;
	private n = 0;

	constructor(size: number) {
		this.buf = new Uint8Array(size);
	}

	private room(bytes: number) {
		if (this.at + bytes <= this.buf.length) return;
		const next = new Uint8Array(Math.max(this.buf.length * 2, this.at + bytes + 64));
		next.set(this.buf.subarray(0, this.at));
		this.buf = next;
	}

	/** Writes the low `count` bits of `v` (count up to 32). */
	bits(v: number, count: number) {
		if (count > 24) {
			this.bits(Math.floor(v / 65536) & 0xffff, count - 16);
			this.bits(v & 0xffff, 16);
			return;
		}
		this.room(4);
		this.acc = ((this.acc << count) | (v & ((1 << count) - 1))) >>> 0;
		this.n += count;
		while (this.n >= 8) {
			this.n -= 8;
			this.buf[this.at++] = (this.acc >>> this.n) & 0xff;
		}
		this.acc &= (1 << this.n) - 1;
	}

	/** Writes `count` zero bits. */
	zeros(count: number) {
		while (count > 16) {
			this.bits(0, 16);
			count -= 16;
		}
		if (count) this.bits(0, count);
	}

	/** Pads with zeros to a whole byte. */
	align() {
		if (this.n) this.bits(0, 8 - this.n);
	}

	/** Bytes written so far (whole bytes only). */
	byteLength() {
		return this.at;
	}

	done(): Uint8Array {
		this.align();
		return this.buf.slice(0, this.at);
	}
}
