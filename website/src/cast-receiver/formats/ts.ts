// MPEG-TS (and Blu-ray's M2TS): programs, PES packets and their access units, for TS files and
// HLS segments. Times are the stream's own 90 kHz clock in seconds, carried on past 2^33.

import { ac3, ac3Box, adts, annexB, avcC, frames, h264Size, hevcSps, hvcC, lengthPrefixed, mpa } from './codecs';
import type { ByteSource } from './source';
import type { Demuxer, Sample, Track } from './types';

interface Stream {
	pid: number;
	type: number;
	codec: string;
	kind: Track['kind'];
	language?: string;
	pes: Uint8Array[];
	pesSize: number;
	pts: number | null;
	dts: number | null;
	/** Audio bytes left over from the previous PES. */
	carry: Uint8Array | null;
	/** Next audio frame time when a PES carries no time. */
	next: number | null;
	track: Track | null;
	sps: Uint8Array[];
	pps: Uint8Array[];
	vps: Uint8Array[];
}

const WRAP = 2 ** 33;

function streamCodec(type: number, descriptors: Map<number, Uint8Array>, registration: string): [string, Track['kind']] | null {
	switch (type) {
		case 0x1b: return ['h264', 'video'];
		case 0x24: return ['hevc', 'video'];
		case 0x01: case 0x02: return ['mpeg2', 'video'];
		case 0x0f: return ['aac', 'audio'];
		case 0x11: return ['aac_latm', 'audio'];
		case 0x03: case 0x04: return ['mpa', 'audio'];
		case 0x81: return ['ac3', 'audio'];
		case 0x87: case 0x84: case 0xa1: return ['eac3', 'audio'];
		case 0x82: case 0x85: case 0x86: case 0x8a: case 0xa2: return ['dts', 'audio'];
		case 0x83: return ['truehd', 'audio'];
		case 0x80: return ['pcm_bluray', 'audio'];
		case 0x90: return ['pgs', 'text'];
		case 0x06:
			if (descriptors.has(0x6a) || registration === 'AC-3') return ['ac3', 'audio'];
			if (descriptors.has(0x7a) || registration === 'EAC3') return ['eac3', 'audio'];
			if (descriptors.has(0x7b) || /^DTS/.test(registration)) return ['dts', 'audio'];
			if (registration === 'Opus') return ['opus', 'audio'];
			return null;
		default:
			return null;
	}
}

export class TsParser {
	/** Tracks by PID, in PMT order, once each has shown enough to configure it. */
	readonly tracks: Track[] = [];
	private streams = new Map<number, Stream>();
	private pmt = -1;
	private packet = 188;
	private last = -1;
	private wrap = 0;
	private rest: Uint8Array | null = null;
	/** Only these PIDs are turned into samples; null for all. */
	wanted: Set<number> | null = null;

	/** Feeds bytes; returns the samples completed by them. */
	push(input: Uint8Array): Sample[] {
		let b = input;
		if (this.rest) {
			const joined = new Uint8Array(this.rest.length + b.length);
			joined.set(this.rest);
			joined.set(b, this.rest.length);
			b = joined;
			this.rest = null;
		}
		const out: Sample[] = [];
		let at = this.sync(b, 0);
		const offset = this.packet === 192 ? 4 : 0;
		while (at >= 0 && at + this.packet <= b.length) {
			const p = at + offset;
			if (b[p] !== 0x47) {
				at = this.sync(b, at + 1);
				continue;
			}
			this.onPacket(b.subarray(p, p + 188), out);
			at += this.packet;
		}
		if (at >= 0 && at < b.length) this.rest = b.slice(at);
		return out;
	}

	/** Completes the PES packets still open (end of a segment or file). */
	flush(): Sample[] {
		const out: Sample[] = [];
		for (const s of this.streams.values()) this.finish(s, out);
		return out;
	}

	/** Every stream in the program has a track. */
	get complete() {
		return this.streams.size > 0 && [...this.streams.values()].every((s) => s.track);
	}

	/** A parser that knows this one's program and clock, for reading elsewhere in the file. */
	fork(): TsParser {
		const p = new TsParser();
		p.pmt = this.pmt;
		p.packet = this.packet;
		p.last = this.last;
		p.wrap = this.wrap;
		for (const [k, s] of this.streams) p.streams.set(k, { ...s, pes: [], pesSize: 0, carry: null, next: null });
		return p;
	}

	/** Forgets partial packets and the clock's anchor, for a seek. */
	reset() {
		this.rest = null;
		this.last = -1;
		this.wrap = 0;
		for (const s of this.streams.values()) {
			s.pes = [];
			s.pesSize = 0;
			s.carry = null;
			s.next = null;
		}
	}

	private sync(b: Uint8Array, from: number): number {
		for (let i = from; i + 188 * 2 < b.length || (i < b.length && i + 188 <= b.length); i++) {
			if (b[i] === 0x47 && b[i + 188] === 0x47 && (i + 376 >= b.length || b[i + 376] === 0x47)) {
				this.packet = 188;
				return i;
			}
			if (i >= 4 && b[i] === 0x47 && b[i + 192] === 0x47 && (i + 384 >= b.length || b[i + 384] === 0x47)) {
				this.packet = 192;
				return i - 4;
			}
		}
		return -1;
	}

	private onPacket(p: Uint8Array, out: Sample[]) {
		const pusi = (p[1] & 0x40) !== 0;
		const pid = ((p[1] & 0x1f) << 8) | p[2];
		const afc = (p[3] >> 4) & 3;
		let at = 4;
		if (afc === 2 || afc === 0) return;
		if (afc === 3) at += 1 + p[4];
		if (at >= 188) return;
		const payload = p.subarray(at);
		if (pid === 0) {
			if (pusi) this.pat(payload.subarray(1 + payload[0]));
			return;
		}
		if (pid === this.pmt) {
			if (pusi) this.pmtSection(payload.subarray(1 + payload[0]));
			return;
		}
		const s = this.streams.get(pid);
		if (!s) return;
		if (pusi) {
			this.finish(s, out);
			s.pes = [payload.slice()];
			s.pesSize = payload.length;
		} else if (s.pes.length) {
			s.pes.push(payload.slice());
			s.pesSize += payload.length;
		}
	}

	private pat(b: Uint8Array) {
		const len = ((b[1] & 15) << 8) | b[2];
		for (let i = 8; i + 4 <= 3 + len - 4; i += 4) {
			const program = (b[i] << 8) | b[i + 1];
			const pid = ((b[i + 2] & 0x1f) << 8) | b[i + 3];
			if (program !== 0) {
				this.pmt = pid;
				return;
			}
		}
	}

	private pmtSection(b: Uint8Array) {
		if (b[0] !== 2) return;
		const len = ((b[1] & 15) << 8) | b[2];
		const end = 3 + len - 4;
		const info = ((b[10] & 15) << 8) | b[11];
		let i = 12 + info;
		while (i + 5 <= end) {
			const type = b[i];
			const pid = ((b[i + 1] & 0x1f) << 8) | b[i + 2];
			const esLen = ((b[i + 3] & 15) << 8) | b[i + 4];
			const descriptors = new Map<number, Uint8Array>();
			let registration = '';
			let language: string | undefined;
			for (let d = i + 5; d + 2 <= i + 5 + esLen; ) {
				const tag = b[d];
				const dl = b[d + 1];
				const body = b.subarray(d + 2, d + 2 + dl);
				descriptors.set(tag, body);
				if (tag === 5 && dl >= 4) registration = String.fromCharCode(...body.subarray(0, 4));
				if (tag === 0x0a && dl >= 3) language = String.fromCharCode(...body.subarray(0, 3));
				d += 2 + dl;
			}
			i += 5 + esLen;
			if (this.streams.has(pid)) continue;
			const c = streamCodec(type, descriptors, registration);
			if (!c) continue;
			this.streams.set(pid, {
				pid,
				type,
				codec: c[0],
				kind: c[1],
				language: language && language !== 'und' ? language : undefined,
				pes: [],
				pesSize: 0,
				pts: null,
				dts: null,
				carry: null,
				next: null,
				track: null,
				sps: [],
				pps: [],
				vps: [],
			});
		}
	}

	private time(raw: number): number {
		// Unwrap the 33-bit clock against the last time seen.
		let t = raw + this.wrap;
		if (this.last >= 0) {
			if (t < this.last - WRAP / 2) {
				this.wrap += WRAP;
				t += WRAP;
			} else if (t > this.last + WRAP / 2 && this.wrap > 0) {
				t -= WRAP;
			}
		}
		this.last = Math.max(this.last, t);
		return t / 90000;
	}

	private finish(s: Stream, out: Sample[]) {
		if (!s.pes.length) return;
		const pes = s.pes.length === 1 ? s.pes[0] : join(s.pes, s.pesSize);
		s.pes = [];
		s.pesSize = 0;
		if (pes.length < 9 || pes[0] !== 0 || pes[1] !== 0 || pes[2] !== 1) return;
		const flags = pes[7];
		const headerLen = pes[8];
		let pts: number | null = null;
		let dts: number | null = null;
		if (flags & 0x80) pts = this.time(stamp(pes, 9));
		if (flags & 0x40) dts = this.time(stamp(pes, 14));
		const data = pes.subarray(9 + headerLen);
		if (!data.length) return;
		if (this.wanted && !this.wanted.has(s.pid) && s.track) return;
		if (s.kind === 'video') this.video(s, data, pts, dts ?? pts, out);
		else if (s.kind === 'audio') this.audio(s, data, pts, out);
		else if (s.codec === 'pgs' && pts != null) this.emit(s, out, { pts, dts: pts, duration: 0, key: true, data: data.slice() }, {});
	}

	private emit(s: Stream, out: Sample[], sample: Omit<Sample, 'track'>, fields: Partial<Track>) {
		if (!s.track) {
			s.track = { id: s.pid, kind: s.kind, codec: s.codec, language: s.language, ...fields };
			this.tracks.push(s.track);
		}
		if (this.wanted && !this.wanted.has(s.pid)) return;
		out.push({ track: s.pid, ...sample });
	}

	private video(s: Stream, data: Uint8Array, pts: number | null, dts: number | null, out: Sample[]) {
		if (pts == null) pts = dts = s.next ?? 0;
		const nals = annexB(data);
		const hevc = s.codec === 'hevc';
		let key = false;
		const keep: Uint8Array[] = [];
		for (const n of nals) {
			if (!n.length) continue;
			const type = hevc ? (n[0] >> 1) & 63 : n[0] & 31;
			if (hevc) {
				if (type === 35) continue;
				if (type === 32) remember(s.vps, n);
				else if (type === 33) remember(s.sps, n);
				else if (type === 34) remember(s.pps, n);
				else if (type >= 16 && type <= 21) key = true;
			} else {
				if (type === 9) continue;
				if (type === 7) remember(s.sps, n);
				else if (type === 8) remember(s.pps, n);
				else if (type === 5) key = true;
			}
			keep.push(n);
		}
		if (s.codec === 'mpeg2') {
			// MPEG-2 video can't go to MediaSource; it is listed so the error can name it.
			this.emit(s, out, { pts, dts: dts ?? pts, duration: 0, key: true, data: new Uint8Array(0) }, {});
			return;
		}
		if (!s.track) {
			// Wait for the parameter sets before the track exists.
			if (!s.sps.length || !s.pps.length || (hevc && !s.vps.length) || !key) return;
			const config = hevc ? hvcC(s.vps, s.sps, s.pps) : avcC(s.sps, s.pps);
			if (!config) return;
			const size = hevc ? hevcSps(s.sps[0]) : h264Size(s.sps[0]);
			this.emit(s, out, { pts, dts: dts ?? pts, duration: 0, key, data: lengthPrefixed(keep) }, {
				config,
				width: size?.width,
				height: size?.height,
			});
		} else this.emit(s, out, { pts, dts: dts ?? pts, duration: 0, key, data: lengthPrefixed(keep) }, {});
		s.next = pts;
	}

	private audio(s: Stream, data: Uint8Array, pts: number | null, out: Sample[]) {
		if (s.carry) {
			data = join([s.carry, data], s.carry.length + data.length);
			s.carry = null;
			// The PES time belongs to its first whole frame; the carried bytes come before it.
			pts = null;
		}
		let t = pts ?? s.next;
		if (t == null) return;
		const codec = s.codec;
		if (codec === 'aac') {
			const { list, rest } = frames(data, adts);
			for (const f of list) {
				const raw = data.slice(f.at + f.header, f.at + f.size);
				this.emit(s, out, { pts: t, dts: t, duration: f.samples / f.rate, key: true, data: raw }, {
					config: f.config,
					sampleRate: f.rate,
					channels: f.channels,
				});
				t += f.samples / f.rate;
			}
			if (rest < data.length) s.carry = data.slice(rest);
		} else if (codec === 'ac3' || codec === 'eac3') {
			const { list, rest } = frames(data, ac3);
			for (const f of list) {
				const frame = data.slice(f.at, f.at + f.size);
				this.emit(s, out, { pts: t, dts: t, duration: f.samples / f.rate, key: true, data: frame }, {
					codec: f.eac3 ? 'eac3' : 'ac3',
					config: ac3Box(data, f.at),
					sampleRate: f.rate,
					channels: f.channels,
				});
				t += f.samples / f.rate;
			}
			if (rest < data.length) s.carry = data.slice(rest);
		} else if (codec === 'mpa') {
			const { list, rest } = frames(data, mpa);
			for (const f of list) {
				const frame = data.slice(f.at, f.at + f.size);
				this.emit(s, out, { pts: t, dts: t, duration: f.samples / f.rate, key: true, data: frame }, {
					codec: f.layer === 3 ? 'mp3' : f.layer === 2 ? 'mp2' : 'mp1',
					sampleRate: f.rate,
					channels: f.channels,
				});
				t += f.samples / f.rate;
			}
			if (rest < data.length) s.carry = data.slice(rest);
		} else {
			// DTS, TrueHD, LATM, Blu-ray LPCM, Opus: the decoder's parser finds the frames.
			this.emit(s, out, { pts: t, dts: t, duration: 0, key: true, data: data.slice() }, { unframed: true });
			s.next = null;
			return;
		}
		s.next = t;
	}
}

function remember(list: Uint8Array[], nal: Uint8Array) {
	if (list.some((x) => x.length === nal.length && x.every((v, i) => v === nal[i]))) return;
	if (list.length >= 4) list.shift();
	list.push(nal.slice());
}

function stamp(b: Uint8Array, at: number): number {
	return (
		((b[at] >> 1) & 7) * 2 ** 30 +
		((b[at + 1] << 22) | ((b[at + 2] >> 1) << 15) | (b[at + 3] << 7) | (b[at + 4] >> 1))
	);
}

function join(parts: Uint8Array[], size: number): Uint8Array {
	const out = new Uint8Array(size);
	let at = 0;
	for (const p of parts) {
		out.set(p, at);
		at += p.length;
	}
	return out;
}

/** A TS or M2TS file read over HTTP ranges. Seeking bisects over the video's PES times. */
export class TsFile implements Demuxer {
	tracks: Track[] = [];
	duration = NaN;
	private parser = new TsParser();
	private pos = 0;
	private origin = 0;
	private pending: Sample[] = [];
	private readonly step = 1 << 19;

	constructor(private readonly src: ByteSource) {}

	async open() {
		// Read until every stream in the PMT has a track, or 4 MB.
		for (let at = 0; at < 4 << 20; at += this.step) {
			const b = await this.src.bytes(at, this.step);
			if (!b.length) break;
			this.pending.push(...this.parser.push(b));
			this.pos = at + b.length;
			if (this.parser.complete) break;
		}
		this.tracks = this.parser.tracks;
		if (!this.tracks.length) throw new Error('no streams');
		const first = this.pending.find((s) => this.tracks.find((t) => t.id === s.track)?.kind === 'video') ?? this.pending[0];
		this.origin = first ? Math.min(first.dts, first.pts) : 0;
		const last = Number.isFinite(this.src.size)
			? await this.timeNear(Math.max(0, this.src.size - (1 << 20)), this.src.size, true)
			: null;
		if (last != null) this.duration = last - this.origin;
		await this.seek(0);
	}

	select(ids: number[]) {
		this.parser.wanted = new Set(ids);
	}

	/** The first (or, with `last`, the last) video PES time between two offsets. */
	private async timeNear(from: number, to: number, last = false): Promise<number | null> {
		const p = this.parser.fork();
		p.wanted = null;
		const video = this.tracks.find((t) => t.kind === 'video')?.id;
		const mine = (list: Sample[]) => list.filter((s) => video == null || s.track === video);
		let found: number | null = null;
		for (let at = from; at < to; at += this.step) {
			const b = await this.src.bytes(at, Math.min(this.step, to - at));
			if (!b.length) break;
			const got = mine(p.push(b));
			if (got.length) {
				if (!last) return got[0].pts;
				found = got[got.length - 1].pts;
			}
		}
		if (last) {
			const tail = mine(p.flush());
			if (tail.length) found = tail[tail.length - 1].pts;
		}
		return found;
	}

	async seek(time: number): Promise<number> {
		this.parser.reset();
		this.pending = [];
		if (time <= 0 || !Number.isFinite(this.src.size)) {
			this.pos = 0;
			return 0;
		}
		const target = time - 3 + this.origin;
		let lo = 0;
		let hi = this.src.size;
		let best = 0;
		let bestTime = this.origin;
		for (let i = 0; i < 20 && hi - lo > this.step; i++) {
			const mid = Math.floor((lo + hi) / 2 / 188) * 188;
			const t = await this.timeNear(mid, Math.min(hi, mid + this.step * 4));
			if (t == null) {
				hi = mid;
				continue;
			}
			if (t <= target) {
				best = mid;
				bestTime = t;
				lo = mid;
			} else hi = mid;
		}
		this.pos = best;
		return Math.max(0, bestTime - this.origin);
	}

	async read(): Promise<Sample[] | null> {
		if (this.pending.length) {
			const p = this.pending;
			this.pending = [];
			return this.shift(p);
		}
		if (Number.isFinite(this.src.size) && this.pos >= this.src.size) {
			const tail = this.parser.flush();
			this.pos = Infinity;
			return tail.length ? this.shift(tail) : null;
		}
		if (this.pos === Infinity) return null;
		const b = await this.src.bytes(this.pos, this.step);
		if (!b.length) {
			this.pos = Infinity;
			const tail = this.parser.flush();
			return tail.length ? this.shift(tail) : null;
		}
		this.pos += b.length;
		this.src.prefetch(this.pos + this.step);
		return this.shift(this.parser.push(b));
	}

	private shift(samples: Sample[]): Sample[] {
		for (const s of samples) {
			s.pts -= this.origin;
			s.dts -= this.origin;
		}
		return samples;
	}

	close() {
		this.src.close();
	}
}
