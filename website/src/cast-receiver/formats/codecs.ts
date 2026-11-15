// Codec headers: H.264 and HEVC parameter sets into avcC and hvcC, audio frame headers (ADTS,
// AC-3, E-AC-3, MPEG audio) and the codec strings MediaSource asks for.

import { Bits, Writer, hex } from './bytes';
import type { Track } from './types';

// ---------- NAL units ----------

/** NAL units of an Annex B byte stream (start codes 00 00 01 or 00 00 00 01). */
export function annexB(b: Uint8Array): Uint8Array[] {
	const out: Uint8Array[] = [];
	let start = -1;
	let i = 0;
	const n = b.length;
	while (i + 2 < n) {
		if (b[i] === 0 && b[i + 1] === 0 && b[i + 2] === 1) {
			if (start >= 0) {
				let end = i;
				while (end > start && b[end - 1] === 0) end--;
				if (end > start) out.push(b.subarray(start, end));
			}
			i += 3;
			start = i;
		} else i++;
	}
	if (start >= 0 && start < n) out.push(b.subarray(start));
	return out;
}

/** NAL units with 4-byte lengths, as MP4 carries them. */
export function lengthPrefixed(nals: Uint8Array[]): Uint8Array {
	let size = 0;
	for (const x of nals) size += 4 + x.length;
	const out = new Uint8Array(size);
	const v = new DataView(out.buffer);
	let at = 0;
	for (const x of nals) {
		v.setUint32(at, x.length);
		out.set(x, at + 4);
		at += 4 + x.length;
	}
	return out;
}

/** Removes emulation prevention bytes (00 00 03). */
export function rbsp(nal: Uint8Array): Uint8Array {
	const out = new Uint8Array(nal.length);
	let o = 0;
	for (let i = 0; i < nal.length; i++) {
		if (i >= 2 && nal[i] === 3 && nal[i - 1] === 0 && nal[i - 2] === 0) continue;
		out[o++] = nal[i];
	}
	return out.subarray(0, o);
}

export function avcC(sps: Uint8Array[], pps: Uint8Array[]): Uint8Array {
	const w = new Writer();
	const s = sps[0];
	w.u8(1).u8(s[1]).u8(s[2]).u8(s[3]).u8(0xff).u8(0xe0 | sps.length);
	for (const x of sps) w.u16(x.length).bytes(x);
	w.u8(pps.length);
	for (const x of pps) w.u16(x.length).bytes(x);
	return w.done();
}

/** Width and height from an H.264 SPS. */
export function h264Size(sps: Uint8Array): { width: number; height: number } | null {
	try {
		const b = new Bits(rbsp(sps).subarray(1));
		const profile = b.read(8);
		b.skip(16);
		b.ue();
		let chroma = 1;
		if ([100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134, 135].includes(profile)) {
			chroma = b.ue();
			if (chroma === 3) b.skip(1);
			b.ue();
			b.ue();
			b.skip(1);
			if (b.flag()) {
				for (let i = 0; i < (chroma !== 3 ? 8 : 12); i++) {
					if (!b.flag()) continue;
					let last = 8;
					let next = 8;
					for (let j = 0; j < (i < 6 ? 16 : 64); j++) {
						if (next !== 0) next = (last + b.se() + 256) % 256;
						last = next === 0 ? last : next;
					}
				}
			}
		}
		b.ue();
		const poc = b.ue();
		if (poc === 0) b.ue();
		else if (poc === 1) {
			b.skip(1);
			b.se();
			b.se();
			const n = b.ue();
			for (let i = 0; i < n; i++) b.se();
		}
		b.ue();
		b.skip(1);
		const wmbs = b.ue() + 1;
		const hmbs = b.ue() + 1;
		const frameOnly = b.flag();
		if (!frameOnly) b.skip(1);
		b.skip(1);
		let crop = [0, 0, 0, 0];
		if (b.flag()) crop = [b.ue(), b.ue(), b.ue(), b.ue()];
		const cx = chroma === 0 ? 1 : 2;
		const cy = (chroma === 1 ? 2 : 1) * (frameOnly ? 1 : 2);
		return {
			width: wmbs * 16 - cx * (crop[0] + crop[1]),
			height: (2 - (frameOnly ? 1 : 0)) * hmbs * 16 - cy * (crop[2] + crop[3]),
		};
	} catch {
		return null;
	}
}

interface HevcSps {
	ptl: Uint8Array; // 12 bytes: profile space/tier/idc, compat flags, constraint flags, level
	chroma: number;
	bitDepthLuma: number;
	bitDepthChroma: number;
	width: number;
	height: number;
	temporalLayers: number;
	nested: boolean;
}

export function hevcSps(nal: Uint8Array): HevcSps | null {
	try {
		const raw = rbsp(nal);
		const b = new Bits(raw.subarray(2));
		b.skip(4);
		const sub = b.read(3);
		const nested = b.flag();
		const ptl = raw.slice(2 + 1, 2 + 1 + 12);
		b.skip(96);
		const subProfile: boolean[] = [];
		const subLevel: boolean[] = [];
		for (let i = 0; i < sub; i++) {
			subProfile.push(b.flag());
			subLevel.push(b.flag());
		}
		if (sub > 0) for (let i = sub; i < 8; i++) b.skip(2);
		for (let i = 0; i < sub; i++) {
			if (subProfile[i]) b.skip(88);
			if (subLevel[i]) b.skip(8);
		}
		b.ue();
		const chroma = b.ue();
		if (chroma === 3) b.skip(1);
		let width = b.ue();
		let height = b.ue();
		if (b.flag()) {
			const sx = chroma === 1 || chroma === 2 ? 2 : 1;
			const sy = chroma === 1 ? 2 : 1;
			const l = b.ue();
			const r = b.ue();
			const t = b.ue();
			const bo = b.ue();
			width -= sx * (l + r);
			height -= sy * (t + bo);
		}
		const bitDepthLuma = b.ue() + 8;
		const bitDepthChroma = b.ue() + 8;
		return { ptl, chroma, bitDepthLuma, bitDepthChroma, width, height, temporalLayers: sub + 1, nested };
	} catch {
		return null;
	}
}

export function hvcC(vps: Uint8Array[], sps: Uint8Array[], pps: Uint8Array[]): Uint8Array | null {
	const info = sps[0] ? hevcSps(sps[0]) : null;
	if (!info) return null;
	const w = new Writer();
	w.u8(1).bytes(info.ptl.subarray(0, 12));
	w.u16(0xf000).u8(0xfc).u8(0xfc | info.chroma).u8(0xf8 | (info.bitDepthLuma - 8)).u8(0xf8 | (info.bitDepthChroma - 8));
	w.u16(0).u8((info.temporalLayers << 3) | ((info.nested ? 1 : 0) << 2) | 3);
	const arrays: [number, Uint8Array[]][] = [
		[32, vps],
		[33, sps],
		[34, pps],
	];
	w.u8(arrays.filter(([, l]) => l.length).length);
	for (const [type, list] of arrays) {
		if (!list.length) continue;
		w.u8(0x80 | type).u16(list.length);
		for (const x of list) w.u16(x.length).bytes(x);
	}
	return w.done();
}

// ---------- Codec strings ----------

function reverseBits32(v: number) {
	let r = 0;
	for (let i = 0; i < 32; i++) {
		r = (r << 1) | (v & 1);
		v >>>= 1;
	}
	return r >>> 0;
}

/** The codecs parameter for a track, from its configuration bytes. */
export function codecString(t: Track): string | null {
	const c = t.config;
	switch (t.codec) {
		case 'h264':
			return c && c.length >= 4 ? `avc1.${hex(c[1])}${hex(c[2])}${hex(c[3])}` : (t.declared ?? 'avc1.640028');
		case 'hevc': {
			if (!c || c.length < 13) return t.declared ?? 'hvc1.1.6.L120.90';
			const space = ['', 'A', 'B', 'C'][c[1] >> 6];
			const tier = c[1] & 0x20 ? 'H' : 'L';
			const profile = c[1] & 0x1f;
			const compat = reverseBits32(((c[2] << 24) | (c[3] << 16) | (c[4] << 8) | c[5]) >>> 0).toString(16);
			const cons = Array.from(c.subarray(6, 12));
			while (cons.length && cons[cons.length - 1] === 0) cons.pop();
			return `hvc1.${space}${profile}.${compat}.${tier}${c[12]}${cons.map((x) => `.${hex(x).toUpperCase()}`).join('')}`;
		}
		case 'vp9': {
			if (c && c.length >= 8 && c[0] === 1) {
				// vpcC full box: version 1
				return `vp09.${String(c[4]).padStart(2, '0')}.${String(c[5]).padStart(2, '0')}.${String(c[6] >> 4).padStart(2, '0')}`;
			}
			return `vp09.00.${t.height && t.height > 1080 ? '50' : '40'}.${String(t.bitDepth ?? 8).padStart(2, '0')}`;
		}
		case 'av1': {
			if (!c || c.length < 4) return 'av01.0.08M.08';
			const profile = c[1] >> 5;
			const level = c[1] & 31;
			const tier = c[2] & 0x80 ? 'H' : 'M';
			const high = (c[2] >> 6) & 1;
			const twelve = (c[2] >> 5) & 1;
			const depth = profile === 2 && high ? (twelve ? 12 : 10) : high ? 10 : 8;
			return `av01.${profile}.${String(level).padStart(2, '0')}${tier}.${String(depth).padStart(2, '0')}`;
		}
		case 'vp8':
			return 'vp8';
		case 'aac': {
			const ot = c && c.length ? c[0] >> 3 : 2;
			return `mp4a.40.${ot === 31 && c && c.length > 1 ? 32 + (((c[0] & 7) << 3) | (c[1] >> 5)) : ot}`;
		}
		case 'mp3':
			return 'mp4a.6B';
		case 'ac3':
			return 'ac-3';
		case 'eac3':
			return 'ec-3';
		case 'opus':
			return 'opus';
		case 'flac':
			return 'flac';
		case 'vorbis':
			return 'vorbis';
		default:
			return null;
	}
}

// ---------- Audio frames ----------

const AAC_RATES = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];

export interface Frame {
	/** Offset of the frame (its header) in the buffer. */
	at: number;
	size: number;
	samples: number;
	rate: number;
	channels: number;
}

/** One ADTS frame at `at`, or null. The payload follows the 7 or 9 byte header. */
export function adts(b: Uint8Array, at: number): (Frame & { header: number; config: Uint8Array }) | null {
	if (at + 7 > b.length || b[at] !== 0xff || (b[at + 1] & 0xf6) !== 0xf0) return null;
	const profile = (b[at + 2] >> 6) + 1;
	const sri = (b[at + 2] >> 2) & 15;
	const ch = ((b[at + 2] & 1) << 2) | (b[at + 3] >> 6);
	const size = ((b[at + 3] & 3) << 11) | (b[at + 4] << 3) | (b[at + 5] >> 5);
	const header = b[at + 1] & 1 ? 7 : 9;
	if (size < header || !AAC_RATES[sri]) return null;
	return {
		at,
		size,
		header,
		samples: 1024 * ((b[at + 6] & 3) + 1),
		rate: AAC_RATES[sri],
		channels: ch,
		config: new Uint8Array([(profile << 3) | (sri >> 1), ((sri & 1) << 7) | (ch << 3)]),
	};
}

const AC3_RATES = [48000, 44100, 32000];
const AC3_SIZES = [
	[64, 69, 96], [64, 70, 96], [80, 87, 120], [80, 88, 120], [96, 104, 144], [96, 105, 144], [112, 121, 168], [112, 122, 168],
	[128, 139, 192], [128, 140, 192], [160, 174, 240], [160, 175, 240], [192, 208, 288], [192, 209, 288], [224, 243, 336], [224, 244, 336],
	[256, 278, 384], [256, 279, 384], [320, 348, 480], [320, 349, 480], [384, 417, 576], [384, 418, 576], [448, 487, 672], [448, 488, 672],
	[512, 557, 768], [512, 558, 768], [640, 696, 960], [640, 697, 960], [768, 835, 1152], [768, 836, 1152], [896, 975, 1344], [896, 976, 1344],
	[1024, 1114, 1536], [1024, 1115, 1536], [1152, 1253, 1728], [1152, 1254, 1728], [1280, 1393, 1920], [1280, 1394, 1920],
];
const ACMOD_CHANNELS = [2, 1, 2, 3, 3, 4, 4, 5];

/** An AC-3 or E-AC-3 frame at `at`. */
export function ac3(b: Uint8Array, at: number): (Frame & { eac3: boolean }) | null {
	if (at + 8 > b.length || b[at] !== 0x0b || b[at + 1] !== 0x77) return null;
	const bsid = b[at + 5] >> 3;
	if (bsid <= 10) {
		const fscod = b[at + 4] >> 6;
		const frmsizecod = b[at + 4] & 63;
		if (fscod > 2 || frmsizecod > 37) return null;
		const bits = new Bits(b.subarray(at + 6));
		const acmod = bits.read(3);
		if ((acmod & 1) && acmod !== 1) bits.skip(2);
		if (acmod & 4) bits.skip(2);
		if (acmod === 2) bits.skip(2);
		const lfe = bits.read(1);
		return { at, size: AC3_SIZES[frmsizecod][fscod] * 2, samples: 1536, rate: AC3_RATES[fscod], channels: ACMOD_CHANNELS[acmod] + lfe, eac3: false };
	}
	if (bsid > 16) return null;
	const size = ((((b[at + 2] & 7) << 8) | b[at + 3]) + 1) * 2;
	const fscod = b[at + 4] >> 6;
	const blocks = fscod === 3 ? 6 : [1, 2, 3, 6][(b[at + 4] >> 4) & 3];
	const rate = fscod === 3 ? [24000, 22050, 16000][(b[at + 4] >> 4) & 3] : AC3_RATES[fscod];
	const acmod = (b[at + 4] >> 1) & 7;
	const lfe = b[at + 4] & 1;
	return { at, size, samples: blocks * 256, rate, channels: ACMOD_CHANNELS[acmod] + lfe, eac3: true };
}

/** dac3 (AC-3) or dec3 (E-AC-3) box body from a frame. */
export function ac3Box(b: Uint8Array, at: number): Uint8Array {
	const bsid = b[at + 5] >> 3;
	if (bsid <= 10) {
		const fscod = b[at + 4] >> 6;
		const frmsizecod = b[at + 4] & 63;
		const bsmod = b[at + 5] & 7;
		const bits = new Bits(b.subarray(at + 6));
		const acmod = bits.read(3);
		if ((acmod & 1) && acmod !== 1) bits.skip(2);
		if (acmod & 4) bits.skip(2);
		if (acmod === 2) bits.skip(2);
		const lfe = bits.read(1);
		const rate = frmsizecod >> 1;
		// fscod(2) bsid(5) bsmod(3) acmod(3) lfeon(1) bit_rate_code(5) reserved(5)
		const v = (fscod << 22) | (bsid << 17) | (bsmod << 14) | (acmod << 11) | (lfe << 10) | (rate << 5);
		return new Uint8Array([(v >> 16) & 255, (v >> 8) & 255, v & 255]);
	}
	const fscod = b[at + 4] >> 6;
	const acmod = (b[at + 4] >> 1) & 7;
	const lfe = b[at + 4] & 1;
	const size = ((((b[at + 2] & 7) << 8) | b[at + 3]) + 1) * 2;
	const blocks = fscod === 3 ? 6 : [1, 2, 3, 6][(b[at + 4] >> 4) & 3];
	const rate = fscod === 3 ? [24000, 22050, 16000][(b[at + 4] >> 4) & 3] : AC3_RATES[fscod];
	const kbps = Math.round((size * 8 * rate) / (blocks * 256) / 1000);
	// data_rate(13) num_ind_sub(3) | fscod(2) bsid(5) reserved(1) asvc(1) bsmod(3) acmod(3) lfeon(1) reserved(3) num_dep_sub(4) reserved(1)
	const w = new Writer();
	w.u16((kbps << 3) | 0);
	w.u8((fscod << 6) | (bsid << 1));
	w.u8((0 << 7) | (0 << 4) | (acmod << 1) | lfe);
	w.u8(0);
	return w.done();
}

const MPA_RATES = [
	[11025, 12000, 8000],
	[0, 0, 0],
	[22050, 24000, 16000],
	[44100, 48000, 32000],
];
const MPA_BITRATES: Record<string, number[]> = {
	'3-1': [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448],
	'3-2': [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384],
	'3-3': [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
	'2-1': [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256],
	'2-2': [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
};

/** An MPEG audio (layer 1, 2 or 3) frame at `at`. */
export function mpa(b: Uint8Array, at: number): (Frame & { layer: number }) | null {
	if (at + 4 > b.length || b[at] !== 0xff || (b[at + 1] & 0xe0) !== 0xe0) return null;
	const version = (b[at + 1] >> 3) & 3; // 3 MPEG-1, 2 MPEG-2, 0 MPEG-2.5
	const layer = 4 - ((b[at + 1] >> 1) & 3);
	const bri = b[at + 2] >> 4;
	const sri = (b[at + 2] >> 2) & 3;
	const pad = (b[at + 2] >> 1) & 1;
	if (version === 1 || layer === 4 || bri === 0 || bri === 15 || sri === 3) return null;
	const rate = MPA_RATES[version][sri];
	const table = MPA_BITRATES[`${version === 3 ? 3 : 2}-${version === 3 ? layer : layer === 1 ? 1 : 2}`];
	const kbps = table[bri];
	let size: number;
	let samples: number;
	if (layer === 1) {
		size = (Math.floor((12000 * kbps) / rate) + pad) * 4;
		samples = 384;
	} else {
		const k = layer === 3 && version !== 3 ? 72000 : 144000;
		size = Math.floor((k * kbps) / rate) + pad;
		samples = layer === 3 && version !== 3 ? 576 : 1152;
	}
	const channels = b[at + 3] >> 6 === 3 ? 1 : 2;
	return { at, size, samples, rate, channels, layer };
}

/** Splits an elementary audio stream into frames with `parse`; returns the frames and the rest. */
export function frames<F extends Frame>(b: Uint8Array, parse: (b: Uint8Array, at: number) => F | null): { list: F[]; rest: number } {
	const list: F[] = [];
	let at = 0;
	while (at < b.length) {
		const f = parse(b, at);
		if (!f) {
			// Too few bytes left to tell: keep them for the next packet.
			if (b.length - at < 16) break;
			at++;
			continue;
		}
		if (at + f.size > b.length) break;
		list.push(f);
		at += f.size;
	}
	return { list, rest: at };
}
