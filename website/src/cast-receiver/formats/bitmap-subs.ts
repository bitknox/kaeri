// Image subtitles: PGS (Blu-ray) and VobSub (DVD), decoded into RGBA pictures with their place on
// the stream's own canvas size.

import { Reader } from './bytes';

export interface Picture {
	x: number;
	y: number;
	width: number;
	height: number;
	rgba: Uint8ClampedArray;
}

/** What shows from `start` until the next display (or `end`, when the stream says). */
export interface Display {
	start: number;
	end: number | null;
	/** The canvas the positions refer to. */
	width: number;
	height: number;
	pictures: Picture[];
}

// ---------- PGS ----------

function ycbcr(y: number, cr: number, cb: number, a: number, hd: boolean): number {
	const c = y - 16;
	const d = cb - 128;
	const e = cr - 128;
	let r: number;
	let g: number;
	let b: number;
	if (hd) {
		r = 1.164 * c + 1.793 * e;
		g = 1.164 * c - 0.213 * d - 0.533 * e;
		b = 1.164 * c + 2.112 * d;
	} else {
		r = 1.164 * c + 1.596 * e;
		g = 1.164 * c - 0.392 * d - 0.813 * e;
		b = 1.164 * c + 2.017 * d;
	}
	const k = (v: number) => Math.max(0, Math.min(255, Math.round(v)));
	// Little-endian RGBA as one 32-bit value
	return ((a & 255) << 24) | (k(b) << 16) | (k(g) << 8) | k(r);
}

interface PgsObject {
	width: number;
	height: number;
	data: Uint8Array[];
	size: number;
}

export class PgsDecoder {
	private palettes = new Map<number, Uint32Array>();
	private objects = new Map<number, PgsObject>();
	private comp: {
		width: number;
		height: number;
		palette: number;
		items: { object: number; x: number; y: number; crop: [number, number, number, number] | null }[];
	} | null = null;

	/** Decodes the segments of one display set; returns what to show from `pts`, if complete. */
	push(data: Uint8Array, pts: number): Display | null {
		const r = new Reader(data);
		let shown: Display | null = null;
		while (r.left >= 3) {
			// Some muxers keep the .sup "PG" header and times.
			if (data[r.pos] === 0x50 && data[r.pos + 1] === 0x47 && r.left >= 13) r.skip(10);
			const type = r.u8();
			const size = r.u16();
			const body = r.bytes(size);
			if (type === 0x16) this.pcs(body);
			else if (type === 0x14) this.pds(body);
			else if (type === 0x15) this.ods(body);
			else if (type === 0x80) shown = this.end(pts);
		}
		return shown;
	}

	private pcs(b: Uint8Array) {
		const r = new Reader(b);
		const width = r.u16();
		const height = r.u16();
		r.skip(1 + 2);
		const state = r.u8();
		r.skip(1);
		const palette = r.u8();
		const n = r.u8();
		if (state & 0x80) this.objects.clear(); // epoch start
		const items = [];
		for (let i = 0; i < n && r.left >= 8; i++) {
			const object = r.u16();
			r.skip(1);
			const cropped = r.u8() & 0x80;
			const x = r.u16();
			const y = r.u16();
			let crop: [number, number, number, number] | null = null;
			if (cropped && r.left >= 8) crop = [r.u16(), r.u16(), r.u16(), r.u16()];
			items.push({ object, x, y, crop });
		}
		this.comp = { width, height, palette, items };
	}

	private pds(b: Uint8Array) {
		const r = new Reader(b);
		const id = r.u8();
		r.skip(1);
		const hd = (this.comp?.height ?? 1080) > 576;
		const pal = this.palettes.get(id) ?? new Uint32Array(256);
		while (r.left >= 5) {
			const i = r.u8();
			const y = r.u8();
			const cr = r.u8();
			const cb = r.u8();
			const a = r.u8();
			pal[i] = ycbcr(y, cr, cb, a, hd);
		}
		this.palettes.set(id, pal);
	}

	private ods(b: Uint8Array) {
		const r = new Reader(b);
		const id = r.u16();
		r.skip(1);
		const seq = r.u8();
		if (seq & 0x80) {
			r.skip(3);
			const width = r.u16();
			const height = r.u16();
			const first = r.bytes(r.left);
			this.objects.set(id, { width, height, data: [first], size: first.length });
		} else {
			const o = this.objects.get(id);
			if (!o) return;
			const more = r.bytes(r.left);
			o.data.push(more);
			o.size += more.length;
		}
	}

	private end(pts: number): Display | null {
		const c = this.comp;
		if (!c) return null;
		const pal = this.palettes.get(c.palette) ?? new Uint32Array(256);
		const pictures: Picture[] = [];
		for (const it of c.items) {
			const o = this.objects.get(it.object);
			if (!o) continue;
			const rle = o.data.length === 1 ? o.data[0] : join(o.data, o.size);
			let pic = pgsRle(rle, o.width, o.height, pal);
			let x = it.x;
			let y = it.y;
			if (it.crop) {
				const [cx, cy, cw, ch] = it.crop;
				pic = cropPicture(pic, o.width, cx, cy, cw, ch);
				x = it.x;
				y = it.y;
				pictures.push({ x, y, width: cw, height: ch, rgba: pic });
			} else pictures.push({ x, y, width: o.width, height: o.height, rgba: pic });
		}
		return { start: pts, end: null, width: c.width, height: c.height, pictures };
	}
}

function join(parts: Uint8Array[], size: number) {
	const out = new Uint8Array(size);
	let at = 0;
	for (const p of parts) {
		out.set(p, at);
		at += p.length;
	}
	return out;
}

function pgsRle(b: Uint8Array, w: number, h: number, pal: Uint32Array): Uint8ClampedArray {
	const px = new Uint32Array(w * h);
	let x = 0;
	let y = 0;
	let i = 0;
	while (i < b.length && y < h) {
		let color = b[i++];
		let run = 1;
		if (color === 0) {
			const f = b[i++];
			if (f === 0) {
				x = 0;
				y++;
				continue;
			}
			run = f & 0x40 ? ((f & 0x3f) << 8) | b[i++] : f & 0x3f;
			color = f & 0x80 ? b[i++] : 0;
		}
		const v = pal[color];
		const row = y * w;
		for (let k = 0; k < run && x < w; k++) px[row + x++] = v;
	}
	return new Uint8ClampedArray(px.buffer);
}

function cropPicture(rgba: Uint8ClampedArray, w: number, cx: number, cy: number, cw: number, ch: number): Uint8ClampedArray {
	const out = new Uint8ClampedArray(cw * ch * 4);
	for (let row = 0; row < ch; row++) out.set(rgba.subarray(((cy + row) * w + cx) * 4, ((cy + row) * w + cx + cw) * 4), row * cw * 4);
	return out;
}

// ---------- VobSub ----------

export interface VobSubSetup {
	width: number;
	height: number;
	/** 16 colours as 0xAABBGGRR (little-endian RGBA). */
	palette: Uint32Array;
}

/** The .idx text (Matroska's CodecPrivate): canvas size and palette. */
export function vobsubSetup(idx: Uint8Array | undefined): VobSubSetup {
	const text = idx ? new TextDecoder().decode(idx) : '';
	const size = text.match(/size:\s*(\d+)x(\d+)/);
	const palette = new Uint32Array(16);
	const p = text.match(/palette:\s*([0-9a-fA-F,\s]+)/);
	if (p) {
		p[1]
			.split(',')
			.map((s) => s.trim())
			.filter(Boolean)
			.slice(0, 16)
			.forEach((hex, i) => {
				const v = parseInt(hex, 16);
				palette[i] = 0xff000000 | ((v & 255) << 16) | (v & 0xff00) | ((v >> 16) & 255);
			});
	} else for (let i = 0; i < 16; i++) palette[i] = 0xff000000 | (i * 0x111111);
	return { width: size ? Number(size[1]) : 720, height: size ? Number(size[2]) : 480, palette };
}

/** One SPU packet: the picture and when it shows and hides, relative to the packet's time. */
export function vobsubPacket(b: Uint8Array, pts: number, setup: VobSubSetup): Display | null {
	if (b.length < 4) return null;
	const v = new DataView(b.buffer, b.byteOffset, b.byteLength);
	const size = v.getUint16(0);
	let dcsq = v.getUint16(2);
	if (size > b.length || dcsq >= size) return null;
	let colors = [0, 1, 2, 3];
	let alphas = [0, 15, 15, 15];
	let x1 = 0;
	let x2 = 0;
	let y1 = 0;
	let y2 = 0;
	let top = 0;
	let bottom = 0;
	let start = 0;
	let end: number | null = null;
	for (let guard = 0; guard < 64; guard++) {
		const delay = (v.getUint16(dcsq) * 1024) / 90000;
		const next = v.getUint16(dcsq + 2);
		let i = dcsq + 4;
		while (i < size) {
			const cmd = b[i++];
			if (cmd === 0xff) break;
			if (cmd === 0x00 || cmd === 0x01) start = delay;
			else if (cmd === 0x02) end = delay;
			else if (cmd === 0x03) {
				colors = [b[i] >> 4, b[i] & 15, b[i + 1] >> 4, b[i + 1] & 15];
				i += 2;
			} else if (cmd === 0x04) {
				alphas = [b[i] >> 4, b[i] & 15, b[i + 1] >> 4, b[i + 1] & 15];
				i += 2;
			} else if (cmd === 0x05) {
				x1 = (b[i] << 4) | (b[i + 1] >> 4);
				x2 = ((b[i + 1] & 15) << 8) | b[i + 2];
				y1 = (b[i + 3] << 4) | (b[i + 4] >> 4);
				y2 = ((b[i + 4] & 15) << 8) | b[i + 5];
				i += 6;
			} else if (cmd === 0x06) {
				top = v.getUint16(i);
				bottom = v.getUint16(i + 2);
				i += 4;
			} else break;
		}
		if (next === dcsq || next >= size) break;
		dcsq = next;
	}
	const w = x2 - x1 + 1;
	const h = y2 - y1 + 1;
	if (w <= 0 || h <= 0 || !top) return null;
	// Colours as stored: index 3 is the first pair's first nibble.
	const lut = new Uint32Array(4);
	for (let k = 0; k < 4; k++) {
		const c = setup.palette[colors[3 - k]] & 0xffffff;
		const a = Math.round((alphas[3 - k] * 255) / 15);
		lut[k] = ((a << 24) | c) >>> 0;
	}
	const px = new Uint32Array(w * h);
	decodeField(b, top, px, w, h, 0, lut);
	decodeField(b, bottom, px, w, h, 1, lut);
	return {
		start: pts + start,
		end: end != null ? pts + end : null,
		width: setup.width,
		height: setup.height,
		pictures: [{ x: x1, y: y1, width: w, height: h, rgba: new Uint8ClampedArray(px.buffer) }],
	};
}

function decodeField(b: Uint8Array, offset: number, px: Uint32Array, w: number, h: number, field: number, lut: Uint32Array) {
	let nib = offset * 2;
	const get = () => {
		const byte = b[nib >> 1] ?? 0;
		const v = nib & 1 ? byte & 15 : byte >> 4;
		nib++;
		return v;
	};
	for (let y = field; y < h; y += 2) {
		let x = 0;
		while (x < w) {
			let code = get();
			if (code < 4) {
				code = (code << 4) | get();
				if (code < 0x10) {
					code = (code << 4) | get();
					if (code < 0x40) code = (code << 4) | get();
				}
			}
			const color = code & 3;
			let run = code >> 2;
			if (run === 0) run = w - x;
			for (let k = 0; k < run && x < w; k++) px[y * w + x++] = lut[color];
		}
		if (nib & 1) nib++;
	}
}
