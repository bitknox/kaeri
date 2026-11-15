// Byte helpers: a big-endian reader, a bit reader for codec headers, and a growable writer.

export class Reader {
	pos = 0;
	readonly view: DataView;
	constructor(
		readonly buf: Uint8Array,
		pos = 0,
	) {
		this.view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
		this.pos = pos;
	}
	get left() {
		return this.buf.length - this.pos;
	}
	u8() {
		return this.buf[this.pos++];
	}
	u16() {
		const v = this.view.getUint16(this.pos);
		this.pos += 2;
		return v;
	}
	u24() {
		const v = (this.buf[this.pos] << 16) | (this.buf[this.pos + 1] << 8) | this.buf[this.pos + 2];
		this.pos += 3;
		return v;
	}
	u32() {
		const v = this.view.getUint32(this.pos);
		this.pos += 4;
		return v;
	}
	i32() {
		const v = this.view.getInt32(this.pos);
		this.pos += 4;
		return v;
	}
	u64() {
		const hi = this.view.getUint32(this.pos);
		const lo = this.view.getUint32(this.pos + 4);
		this.pos += 8;
		return hi * 2 ** 32 + lo;
	}
	bytes(n: number) {
		const v = this.buf.subarray(this.pos, this.pos + n);
		this.pos += n;
		return v;
	}
	skip(n: number) {
		this.pos += n;
	}
	ascii(n: number) {
		return ascii(this.buf, this.pos, (this.pos += n));
	}
}

export function ascii(buf: Uint8Array, from = 0, to = buf.length) {
	let s = '';
	for (let i = from; i < to; i++) s += String.fromCharCode(buf[i]);
	return s;
}

export function utf8(buf: Uint8Array) {
	return new TextDecoder().decode(buf);
}

/** Reads bits most significant first, with Exp-Golomb codes for H.264 and HEVC headers. */
export class Bits {
	private bit = 0;
	constructor(private readonly buf: Uint8Array) {}
	get pos() {
		return this.bit;
	}
	read(n: number): number {
		let v = 0;
		for (let i = 0; i < n; i++) {
			const byte = this.buf[this.bit >> 3] ?? 0;
			v = v * 2 + ((byte >> (7 - (this.bit & 7))) & 1);
			this.bit++;
		}
		return v;
	}
	flag() {
		return this.read(1) === 1;
	}
	skip(n: number) {
		this.bit += n;
	}
	ue(): number {
		let zeros = 0;
		while (this.read(1) === 0 && zeros < 32) zeros++;
		return 2 ** zeros - 1 + this.read(zeros);
	}
	se(): number {
		const v = this.ue();
		return v & 1 ? (v + 1) / 2 : -v / 2;
	}
}

export class Writer {
	private buf = new Uint8Array(256);
	length = 0;
	private room(n: number) {
		if (this.length + n <= this.buf.length) return;
		let cap = this.buf.length * 2;
		while (cap < this.length + n) cap *= 2;
		const next = new Uint8Array(cap);
		next.set(this.buf.subarray(0, this.length));
		this.buf = next;
	}
	u8(v: number) {
		this.room(1);
		this.buf[this.length++] = v & 255;
		return this;
	}
	u16(v: number) {
		this.room(2);
		this.buf[this.length++] = (v >> 8) & 255;
		this.buf[this.length++] = v & 255;
		return this;
	}
	u24(v: number) {
		this.u8(v >> 16).u8(v >> 8).u8(v);
		return this;
	}
	u32(v: number) {
		this.room(4);
		this.buf[this.length++] = (v >>> 24) & 255;
		this.buf[this.length++] = (v >>> 16) & 255;
		this.buf[this.length++] = (v >>> 8) & 255;
		this.buf[this.length++] = v & 255;
		return this;
	}
	u64(v: number) {
		this.u32(Math.floor(v / 2 ** 32));
		this.u32(v >>> 0);
		return this;
	}
	bytes(b: ArrayLike<number>) {
		this.room(b.length);
		this.buf.set(b, this.length);
		this.length += b.length;
		return this;
	}
	ascii(s: string) {
		for (let i = 0; i < s.length; i++) this.u8(s.charCodeAt(i));
		return this;
	}
	/** Writes a 32-bit placeholder and returns its offset, for sizes known later. */
	mark() {
		const at = this.length;
		this.u32(0);
		return at;
	}
	patch32(at: number, v: number) {
		this.buf[at] = (v >>> 24) & 255;
		this.buf[at + 1] = (v >>> 16) & 255;
		this.buf[at + 2] = (v >>> 8) & 255;
		this.buf[at + 3] = v & 255;
	}
	/** Overwrites bytes already written. */
	set(at: number, bytes: ArrayLike<number>) {
		this.buf.set(bytes, at);
	}
	done() {
		return this.buf.slice(0, this.length);
	}
}

export function concat(parts: Uint8Array[]): Uint8Array {
	let n = 0;
	for (const p of parts) n += p.length;
	const out = new Uint8Array(n);
	let at = 0;
	for (const p of parts) {
		out.set(p, at);
		at += p.length;
	}
	return out;
}

export function hex(v: number, width = 2) {
	return v.toString(16).padStart(width, '0');
}
